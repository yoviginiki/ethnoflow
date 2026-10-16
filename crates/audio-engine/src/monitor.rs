//! Input monitor: meter and tuner on an input device without recording anything.
//!
//! Same capture path as the recorder (callback → ring buffer → drain thread), but the
//! drain thread only runs the pitch tracker. Nothing is written to disk.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::DeviceTrait;
use cpal::SampleFormat;
use ringbuf::traits::{Consumer, Split};
use ringbuf::HeapRb;
use serde::{Deserialize, Serialize};

use crate::devices::{choose_config, find_device, Direction, StreamRequest};
use crate::error::{AudioError, Result};
use crate::pitch::{ContourPoint, PitchConfig, PitchTracker};
use crate::recorder::{build_input, Shared, CHUNK, RING_CAPACITY_SAMPLES};
use crate::stream_thread::{StreamInfo, StreamThread};

const PITCH_HOP_SECONDS: f64 = 0.02;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitorStatus {
    pub sample_rate: u32,
    pub channels: u16,
    /// Peak since the previous query (linear).
    pub peak: f32,
    pub clipped_samples: u64,
    pub stream_errors: u64,
    pub device_lost: bool,
    pub pitch_hz: Option<f64>,
    pub pitch_confidence: f64,
}

pub struct InputMonitor {
    stream: StreamThread,
    drain: Option<JoinHandle<()>>,
    shared: Arc<Shared>,
    info: StreamInfo,
    device_name: String,
}

impl InputMonitor {
    pub fn start(device_name: Option<&str>, request: StreamRequest) -> Result<Self> {
        let device = find_device(Direction::Input, device_name)?;
        let resolved_name = device.name().unwrap_or_else(|_| "unknown".into());
        let supported = choose_config(&device, Direction::Input, request)?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let info = StreamInfo {
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        };
        let shared = Arc::new(Shared::default());
        let (prod, mut cons) = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES).split();
        let mut tracker = PitchTracker::new(
            PitchConfig {
                min_hz: 70.0,
                ..PitchConfig::new(info.sample_rate)
            },
            (PITCH_HOP_SECONDS * info.sample_rate as f64) as usize,
        );
        let channels = info.channels.max(1) as usize;
        let drain_shared = Arc::clone(&shared);
        let drain = thread::Builder::new()
            .name("ethnoflow-monitor-drain".into())
            .spawn(move || {
                let mut tmp = vec![0.0_f32; CHUNK * 8];
                let mut mono = Vec::with_capacity(CHUNK * 8 / channels + 1);
                let mut est: Vec<ContourPoint> = Vec::new();
                loop {
                    let n = cons.pop_slice(&mut tmp);
                    if n == 0 {
                        if drain_shared.drain_stop.load(Ordering::Acquire) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    mono.clear();
                    mono.extend(
                        tmp[..n]
                            .chunks_exact(channels)
                            .map(|f| f.iter().sum::<f32>() / channels as f32),
                    );
                    est.clear();
                    tracker.push(&mono, &mut est);
                    if let Some(e) = est.last() {
                        drain_shared
                            .pitch_hz_bits
                            .store(e.hz.map_or(0, f64::to_bits), Ordering::Relaxed);
                        drain_shared
                            .pitch_conf_bits
                            .store((e.confidence as f32).to_bits(), Ordering::Relaxed);
                    }
                }
            })
            .map_err(|e| AudioError::Stream(format!("cannot spawn monitor thread: {e}")))?;

        let cb_shared = Arc::clone(&shared);
        let ch = info.channels as u64;
        let started = StreamThread::spawn("ethnoflow-monitor-stream", move || {
            let err_shared = Arc::clone(&cb_shared);
            let err_fn = move |e: cpal::StreamError| {
                log::error!("monitor stream error: {e}");
                if matches!(e, cpal::StreamError::DeviceNotAvailable) {
                    err_shared.device_lost.store(true, Ordering::Release);
                }
                err_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
            };
            macro_rules! build {
                ($t:ty) => {
                    build_input::<$t, _>(&device, &config, prod, cb_shared, ch, err_fn)?
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
        });
        let (stream, info) = match started {
            Ok(v) => v,
            Err(e) => {
                shared.drain_stop.store(true, Ordering::Release);
                let _ = drain.join();
                return Err(e);
            }
        };
        Ok(Self {
            stream,
            drain: Some(drain),
            shared,
            info,
            device_name: resolved_name,
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Current status. Resets the peak.
    pub fn status(&self) -> MonitorStatus {
        MonitorStatus {
            sample_rate: self.info.sample_rate,
            channels: self.info.channels,
            peak: f32::from_bits(self.shared.peak_bits.swap(0, Ordering::AcqRel)),
            clipped_samples: self.shared.clipped_samples.load(Ordering::Relaxed),
            stream_errors: self.shared.stream_errors.load(Ordering::Relaxed),
            device_lost: self.shared.device_lost.load(Ordering::Acquire),
            pitch_hz: match self.shared.pitch_hz_bits.load(Ordering::Relaxed) {
                0 => None,
                bits => Some(f64::from_bits(bits)),
            },
            pitch_confidence: f32::from_bits(self.shared.pitch_conf_bits.load(Ordering::Relaxed))
                as f64,
        }
    }

    pub fn stop(mut self) {
        self.stream.stop();
        self.shared.drain_stop.store(true, Ordering::Release);
        if let Some(j) = self.drain.take() {
            let _ = j.join();
        }
    }
}

impl Drop for InputMonitor {
    fn drop(&mut self) {
        self.stream.stop();
        self.shared.drain_stop.store(true, Ordering::Release);
        if let Some(j) = self.drain.take() {
            let _ = j.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real device: opens, reports frames and stops cleanly (skips without an input).
    #[test]
    fn monitor_runs_without_writing_anything() {
        match InputMonitor::start(None, StreamRequest::default()) {
            Ok(m) => {
                // Device start-up can take a while when other streams open at once.
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                let mut frames = 0;
                while frames == 0 && std::time::Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(50));
                    frames = m.shared.frames_captured.load(Ordering::Relaxed);
                }
                thread::sleep(Duration::from_millis(100));
                let st = m.status();
                m.stop();
                assert!(frames > 0, "no frames captured within 2 s");
                eprintln!("monitor: {} frames, pitch {:?}", frames, st.pitch_hz);
            }
            Err(e) => eprintln!("no input device in this environment: {e}"),
        }
    }
}
