//! The graph on an output device. Sources are swapped through a lock-free command
//! ring; replaced sources go back through a second ring so they are dropped off the
//! audio thread (the callback never frees memory).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::DeviceTrait;
use cpal::{FromSample, SampleFormat, SizedSample};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use serde::{Deserialize, Serialize};

use super::{Graph, GraphParams, Silence, Source};
use crate::devices::{find_device, Direction};
use crate::error::{AudioError, Result};
use crate::stream_thread::{StreamInfo, StreamThread};

pub enum GraphCmd {
    SetSource {
        slot: usize,
        source: Box<dyn Source>,
    },
    Clear {
        slot: usize,
    },
    Reset,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatus {
    pub sample_rate: u32,
    pub channels: u16,
    pub stream_errors: u64,
    /// Commands the callback could not apply because the ring was full.
    pub dropped_commands: u64,
}

struct Shared {
    stream_errors: AtomicU64,
    dropped: AtomicU64,
}

pub struct GraphPlayer {
    stream: StreamThread,
    cmds: HeapProd<GraphCmd>,
    trash: HeapCons<Box<dyn Source>>,
    params: Arc<GraphParams>,
    shared: Arc<Shared>,
    info: StreamInfo,
}

impl GraphPlayer {
    pub fn start(device_name: Option<&str>, params: Arc<GraphParams>) -> Result<Self> {
        let device = find_device(Direction::Output, device_name)?;
        let supported = device.default_output_config()?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let info = StreamInfo {
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        };
        let (cmds, cmd_rx) = HeapRb::<GraphCmd>::new(64).split();
        let (trash_tx, trash) = HeapRb::<Box<dyn Source>>::new(64).split();
        let shared = Arc::new(Shared {
            stream_errors: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        });
        let graph = Graph::new(Arc::clone(&params), info.sample_rate);
        let cb_shared = Arc::clone(&shared);
        let (stream, info) = StreamThread::spawn("ethnoflow-graph", move || {
            let err_shared = Arc::clone(&cb_shared);
            let err_fn = move |e: cpal::StreamError| {
                log::error!("graph stream error: {e}");
                err_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
            };
            macro_rules! build {
                ($t:ty) => {
                    build_output::<$t, _>(
                        &device, &config, graph, cmd_rx, trash_tx, cb_shared, err_fn,
                    )?
                };
            }
            let stream = match sample_format {
                SampleFormat::F32 => build!(f32),
                SampleFormat::I16 => build!(i16),
                SampleFormat::U16 => build!(u16),
                SampleFormat::I32 => build!(i32),
                SampleFormat::I8 => build!(i8),
                SampleFormat::U8 => build!(u8),
                SampleFormat::F64 => build!(f64),
                other => return Err(AudioError::UnsupportedFormat(format!("{other:?}"))),
            };
            Ok((stream, info))
        })?;
        Ok(Self {
            stream,
            cmds,
            trash,
            params,
            shared,
            info,
        })
    }

    pub fn params(&self) -> &Arc<GraphParams> {
        &self.params
    }

    pub fn sample_rate(&self) -> u32 {
        self.info.sample_rate
    }

    /// Queue a command; `false` if the ring is full. Also frees replaced sources.
    pub fn send(&mut self, cmd: GraphCmd) -> bool {
        self.collect();
        self.cmds.try_push(cmd).is_ok()
    }

    /// Drop sources the callback handed back (call from any non-audio thread).
    pub fn collect(&mut self) {
        while let Some(old) = self.trash.try_pop() {
            drop(old);
        }
    }

    pub fn status(&mut self) -> GraphStatus {
        self.collect();
        GraphStatus {
            sample_rate: self.info.sample_rate,
            channels: self.info.channels,
            stream_errors: self.shared.stream_errors.load(Ordering::Relaxed),
            dropped_commands: self.shared.dropped.load(Ordering::Relaxed),
        }
    }

    pub fn stop(mut self) {
        self.stream.stop();
        self.collect();
    }
}

#[allow(clippy::too_many_arguments)]
fn build_output<T, E>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut graph: Graph,
    mut cmds: HeapCons<GraphCmd>,
    mut trash: HeapProd<Box<dyn Source>>,
    shared: Arc<Shared>,
    err_fn: E,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
    E: FnMut(cpal::StreamError) + Send + 'static,
{
    let channels = config.channels as usize;
    let mut scratch = vec![0.0f32; 8192 * channels];
    let stream = device.build_output_stream(
        config,
        move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
            while let Some(cmd) = cmds.try_pop() {
                match cmd {
                    GraphCmd::SetSource { slot, source } => {
                        let old = graph.replace_source(slot, source);
                        if let Some(old) = old {
                            if trash.try_push(old).is_err() {
                                // Trash full: the source leaks until the player stops
                                // rather than being freed here. Counted for diagnostics.
                                shared.dropped.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    GraphCmd::Clear { slot } => {
                        if let Some(old) = graph.replace_source(slot, Box::new(Silence)) {
                            if trash.try_push(old).is_err() {
                                shared.dropped.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                    GraphCmd::Reset => graph.reset(),
                }
            }
            let n = out.len().min(scratch.len());
            graph.render(&mut scratch[..n], channels);
            for (dst, &s) in out.iter_mut().zip(scratch.iter()) {
                *dst = T::from_sample(s);
            }
            for dst in out.iter_mut().skip(n) {
                *dst = T::from_sample(0.0f32);
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::SineSource;

    /// Real device: start, swap a sine in, run briefly, stop (skips without an output).
    #[test]
    fn graph_player_runs_and_swaps_sources() {
        let params = Arc::new(GraphParams::default());
        params.master.gain_db.set(-30.0);
        params.layers[0].gain_db.set(0.0);
        match GraphPlayer::start(None, Arc::clone(&params)) {
            Ok(mut p) => {
                assert!(p.send(GraphCmd::SetSource {
                    slot: 0,
                    source: Box::new(SineSource::new(330.0, 0.2)),
                }));
                std::thread::sleep(std::time::Duration::from_millis(300));
                assert!(p.send(GraphCmd::SetSource {
                    slot: 0,
                    source: Box::new(SineSource::new(440.0, 0.2)),
                }));
                std::thread::sleep(std::time::Duration::from_millis(200));
                let st = p.status();
                assert_eq!(st.dropped_commands, 0);
                assert!(params.layers[0].peak() > 0.0, "layer meter moved");
                p.stop();
                eprintln!("graph player ok @ {} Hz", st.sample_rate);
            }
            Err(e) => eprintln!("no output device in this environment: {e}"),
        }
    }
}
