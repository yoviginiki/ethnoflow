//! Microphone recorder.
//!
//! Data path: host audio callback → lock-free ring buffer → drain thread → growable Vec.
//! The audio callback never allocates, never locks and never touches the file system.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::DeviceTrait;
use cpal::{FromSample, SampleFormat, SizedSample};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;
use crate::devices::{find_device, Direction};
use crate::error::{AudioError, Result};
use crate::stream_thread::{StreamInfo, StreamThread};

/// Ring buffer capacity in samples (interleaved). 2 s at 96 kHz stereo — generous so a
/// briefly stalled drain thread does not cause overruns.
const RING_CAPACITY_SAMPLES: usize = 96_000 * 2 * 2;
const CHUNK: usize = 1024;
const CLIP_THRESHOLD: f32 = 0.999;
/// Phase 0 keeps the take in RAM. This cap (30 min) bounds memory until Phase 1 streams
/// to disk: 30 min × 48 kHz × stereo × 4 bytes ≈ 690 MB worst case.
pub const DEFAULT_MAX_SECONDS: f64 = 30.0 * 60.0;

#[derive(Debug, Default)]
struct Shared {
    /// Max |sample| since the last `take_peak`, stored as f32 bits (non-negative floats
    /// order correctly as u32).
    peak_bits: AtomicU32,
    frames_captured: AtomicU64,
    overrun_samples: AtomicU64,
    clipped_samples: AtomicU64,
    stream_errors: AtomicU64,
    drain_stop: AtomicBool,
    /// Set when the in-memory limit was reached; further samples are discarded.
    capped: AtomicBool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderStatus {
    pub sample_rate: u32,
    pub channels: u16,
    pub frames_captured: u64,
    pub seconds_captured: f64,
    /// Peak level since the previous status query (linear, 0..1).
    pub peak: f32,
    pub overrun_samples: u64,
    pub clipped_samples: u64,
    pub stream_errors: u64,
    /// True once the in-memory limit was hit; the caller should stop the recorder.
    pub capped: bool,
    pub max_seconds: f64,
}

pub struct Recorder {
    stream: StreamThread,
    drain: Option<JoinHandle<Vec<f32>>>,
    shared: Arc<Shared>,
    info: StreamInfo,
    device_name: String,
    max_seconds: f64,
}

impl Recorder {
    /// Start capturing from `device_name` (or the default input device) with the
    /// default in-memory limit ([`DEFAULT_MAX_SECONDS`]).
    pub fn start(device_name: Option<&str>) -> Result<Self> {
        Self::start_with_limit(device_name, DEFAULT_MAX_SECONDS)
    }

    /// Start capturing with an explicit in-memory limit in seconds.
    pub fn start_with_limit(device_name: Option<&str>, max_seconds: f64) -> Result<Self> {
        if !(max_seconds > 0.0) {
            return Err(AudioError::InvalidArgument(
                "max_seconds must be > 0".into(),
            ));
        }
        let device = find_device(Direction::Input, device_name)?;
        let resolved_name = device.name().unwrap_or_else(|_| "unknown".into());
        let supported = device.default_input_config()?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let info = StreamInfo {
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        };

        let shared = Arc::new(Shared::default());
        let rb = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES);
        let (prod, mut cons) = rb.split();

        // Drain thread: moves samples from the ring buffer into a growable Vec.
        let drain_shared = Arc::clone(&shared);
        let max_samples = (max_seconds * info.sample_rate as f64) as usize * info.channels as usize;
        let drain = thread::Builder::new()
            .name("ethnoflow-record-drain".into())
            .spawn(move || {
                let mut out: Vec<f32> = Vec::with_capacity(info.sample_rate as usize * 60);
                let mut tmp = [0.0_f32; CHUNK * 4];
                loop {
                    let n = cons.pop_slice(&mut tmp);
                    if n > 0 {
                        let room = max_samples.saturating_sub(out.len());
                        let take = n.min(room);
                        out.extend_from_slice(&tmp[..take]);
                        if take < n {
                            drain_shared.capped.store(true, Ordering::Release);
                        }
                        continue;
                    }
                    if drain_shared.drain_stop.load(Ordering::Acquire) {
                        // Stream is already dropped; one final sweep.
                        let n = cons.pop_slice(&mut tmp);
                        if n > 0 {
                            out.extend_from_slice(&tmp[..n]);
                            continue;
                        }
                        break;
                    }
                    thread::sleep(Duration::from_millis(2));
                }
                out
            })
            .map_err(|e| AudioError::Stream(format!("cannot spawn drain thread: {e}")))?;

        let cb_shared = Arc::clone(&shared);
        let channels = info.channels as u64;
        let (stream, info) = StreamThread::spawn("ethnoflow-record-stream", move || {
            let err_shared = Arc::clone(&cb_shared);
            let err_fn = move |e: cpal::StreamError| {
                log::error!("input stream error: {e}");
                err_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
            };
            let stream = match sample_format {
                SampleFormat::F32 => {
                    build_input::<f32, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::I16 => {
                    build_input::<i16, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::U16 => {
                    build_input::<u16, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::I32 => {
                    build_input::<i32, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::I8 => {
                    build_input::<i8, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::U8 => {
                    build_input::<u8, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::F64 => {
                    build_input::<f64, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                other => return Err(AudioError::UnsupportedFormat(format!("{other:?}"))),
            };
            Ok((stream, info))
        })
        .inspect_err(|_e| {
            // Make sure the drain thread does not linger if stream start failed.
            shared.drain_stop.store(true, Ordering::Release);
        })?;

        Ok(Self {
            stream,
            drain: Some(drain),
            shared,
            info,
            device_name: resolved_name,
            max_seconds,
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn sample_rate(&self) -> u32 {
        self.info.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.info.channels
    }

    /// Current status. Resets the peak meter.
    pub fn status(&self) -> RecorderStatus {
        let frames = self.shared.frames_captured.load(Ordering::Relaxed);
        RecorderStatus {
            sample_rate: self.info.sample_rate,
            channels: self.info.channels,
            frames_captured: frames,
            seconds_captured: frames as f64 / self.info.sample_rate as f64,
            peak: f32::from_bits(self.shared.peak_bits.swap(0, Ordering::AcqRel)),
            overrun_samples: self.shared.overrun_samples.load(Ordering::Relaxed),
            clipped_samples: self.shared.clipped_samples.load(Ordering::Relaxed),
            stream_errors: self.shared.stream_errors.load(Ordering::Relaxed),
            capped: self.shared.capped.load(Ordering::Acquire),
            max_seconds: self.max_seconds,
        }
    }

    /// Stop capturing and return everything recorded.
    pub fn stop(mut self) -> Result<AudioBuffer> {
        self.stream.stop();
        self.shared.drain_stop.store(true, Ordering::Release);
        let samples = match self.drain.take() {
            Some(j) => j
                .join()
                .map_err(|_| AudioError::Stream("drain thread panicked".into()))?,
            None => Vec::new(),
        };
        // Drop partial trailing frame if the host delivered an odd sample count.
        let ch = self.info.channels as usize;
        let whole = samples.len() / ch * ch;
        let mut samples = samples;
        samples.truncate(whole);
        Ok(AudioBuffer::new(
            self.info.sample_rate,
            self.info.channels,
            samples,
        ))
    }
}

fn build_input<T, E>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut prod: ringbuf::HeapProd<f32>,
    shared: Arc<Shared>,
    channels: u64,
    err_fn: E,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
    E: FnMut(cpal::StreamError) + Send + 'static,
{
    let mut scratch = [0.0_f32; CHUNK];
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            let mut block_peak = 0.0_f32;
            let mut clipped = 0u64;
            let mut lost = 0u64;
            for chunk in data.chunks(CHUNK) {
                let n = chunk.len();
                for (i, &s) in chunk.iter().enumerate() {
                    let v: f32 = s.to_sample();
                    let a = v.abs();
                    if a > block_peak {
                        block_peak = a;
                    }
                    if a >= CLIP_THRESHOLD {
                        clipped += 1;
                    }
                    scratch[i] = v;
                }
                let pushed = prod.push_slice(&scratch[..n]);
                lost += (n - pushed) as u64;
            }
            shared
                .frames_captured
                .fetch_add(data.len() as u64 / channels.max(1), Ordering::Relaxed);
            shared
                .peak_bits
                .fetch_max(block_peak.to_bits(), Ordering::AcqRel);
            if clipped > 0 {
                shared.clipped_samples.fetch_add(clipped, Ordering::Relaxed);
            }
            if lost > 0 {
                shared.overrun_samples.fetch_add(lost, Ordering::Relaxed);
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

    /// Exercises the exact ring-buffer → drain pattern used by the recorder, without hardware.
    #[test]
    fn ring_buffer_drain_preserves_order_and_counts_overruns() {
        let rb = HeapRb::<f32>::new(8);
        let (mut prod, mut cons) = rb.split();
        let data: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let pushed = prod.push_slice(&data);
        assert_eq!(pushed, 8, "capacity 8 must accept exactly 8 samples");
        let lost = data.len() - pushed;
        assert_eq!(lost, 2);
        let mut out = [0.0_f32; 16];
        let n = cons.pop_slice(&mut out);
        assert_eq!(n, 8);
        assert_eq!(&out[..8], &data[..8]);
    }

    #[test]
    fn peak_bits_fetch_max_orders_floats() {
        let a = AtomicU32::new(0);
        a.fetch_max(0.25_f32.to_bits(), Ordering::AcqRel);
        a.fetch_max(0.75_f32.to_bits(), Ordering::AcqRel);
        a.fetch_max(0.5_f32.to_bits(), Ordering::AcqRel);
        assert_eq!(f32::from_bits(a.swap(0, Ordering::AcqRel)), 0.75);
        assert_eq!(f32::from_bits(a.load(Ordering::Relaxed)), 0.0);
    }

    /// With a tiny limit the drain thread must stop growing the buffer and flag `capped`.
    #[test]
    fn in_memory_limit_is_enforced() {
        match Recorder::start_with_limit(None, 0.05) {
            Ok(rec) => {
                std::thread::sleep(Duration::from_millis(300));
                let st = rec.status();
                let sr = st.sample_rate as usize;
                let ch = st.channels as usize;
                let buf = rec.stop().expect("stop");
                let max = (0.05 * sr as f64) as usize * ch;
                assert!(buf.samples.len() <= max, "{} > {}", buf.samples.len(), max);
                if st.frames_captured as usize * ch > max {
                    assert!(st.capped, "cap reached but not flagged");
                }
            }
            Err(e) => eprintln!("no input device in this environment: {e}"),
        }
        assert!(Recorder::start_with_limit(None, 0.0).is_err());
    }

    /// Starting on a machine with no input device must fail cleanly, not hang or panic.
    /// On a machine with a device this records ~100 ms and checks the buffer shape.
    #[test]
    fn start_stop_without_panicking() {
        match Recorder::start(None) {
            Ok(rec) => {
                std::thread::sleep(Duration::from_millis(150));
                let st = rec.status();
                let buf = rec.stop().expect("stop");
                assert_eq!(buf.channels, st.channels);
                assert_eq!(buf.sample_rate, st.sample_rate);
                assert_eq!(buf.samples.len() % buf.channels as usize, 0);
                eprintln!(
                    "recorded {} frames @ {} Hz x{} (overruns {})",
                    buf.frames(),
                    buf.sample_rate,
                    buf.channels,
                    st.overrun_samples
                );
            }
            Err(e) => eprintln!("no input device in this environment: {e}"),
        }
    }
}
