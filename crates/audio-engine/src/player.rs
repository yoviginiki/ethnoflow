//! Buffer playback through an output device.
//!
//! The buffer is adapted (channel-mapped and resampled) once, before the stream starts,
//! so the real-time callback is a plain indexed copy with an atomic read position.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use cpal::traits::DeviceTrait;
use cpal::{FromSample, SampleFormat, SizedSample};
use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;
use crate::devices::{find_device, Direction};
use crate::error::{AudioError, Result};
use crate::resample::resample_linear;
use crate::stream_thread::{StreamInfo, StreamThread};

#[derive(Debug, Default)]
struct Shared {
    /// Read position in frames.
    position: AtomicUsize,
    finished: AtomicBool,
    stream_errors: AtomicU64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerStatus {
    pub position_frames: usize,
    pub position_seconds: f64,
    pub total_frames: usize,
    pub duration_seconds: f64,
    pub finished: bool,
    pub sample_rate: u32,
    pub channels: u16,
    pub stream_errors: u64,
}

pub struct Player {
    stream: StreamThread,
    shared: Arc<Shared>,
    info: StreamInfo,
    total_frames: usize,
}

/// Match `buffer` to the device's channel count and sample rate.
pub fn adapt_for_device(buffer: &AudioBuffer, channels: u16, sample_rate: u32) -> AudioBuffer {
    let resampled = resample_linear(buffer, sample_rate);
    if resampled.channels == channels {
        return resampled;
    }
    let mono = resampled.to_mono();
    let ch = channels as usize;
    let mut out = Vec::with_capacity(mono.frames() * ch);
    for &s in &mono.samples {
        for _ in 0..ch {
            out.push(s);
        }
    }
    AudioBuffer::new(sample_rate, channels, out)
}

impl Player {
    /// Start playing `buffer` on `device_name` (or the default output) from `start_frame`.
    pub fn start(
        buffer: &AudioBuffer,
        device_name: Option<&str>,
        start_frame: usize,
    ) -> Result<Self> {
        if buffer.is_empty() {
            return Err(AudioError::InvalidArgument("buffer is empty".into()));
        }
        let device = find_device(Direction::Output, device_name)?;
        let supported = device.default_output_config()?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let info = StreamInfo {
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        };

        // Convert the requested start frame from the buffer's rate to the device rate.
        let start_frame_device = (start_frame as f64 * info.sample_rate as f64
            / buffer.sample_rate.max(1) as f64) as usize;
        let adapted = Arc::new(adapt_for_device(buffer, info.channels, info.sample_rate));
        let total_frames = adapted.frames();
        let shared = Arc::new(Shared::default());
        shared
            .position
            .store(start_frame_device.min(total_frames), Ordering::Relaxed);

        let cb_shared = Arc::clone(&shared);
        let (stream, info) = StreamThread::spawn("ethnoflow-play-stream", move || {
            let err_shared = Arc::clone(&cb_shared);
            let err_fn = move |e: cpal::StreamError| {
                log::error!("output stream error: {e}");
                err_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
            };
            let stream = match sample_format {
                SampleFormat::F32 => {
                    build_output::<f32, _>(&device, &config, adapted, cb_shared, err_fn)?
                }
                SampleFormat::I16 => {
                    build_output::<i16, _>(&device, &config, adapted, cb_shared, err_fn)?
                }
                SampleFormat::U16 => {
                    build_output::<u16, _>(&device, &config, adapted, cb_shared, err_fn)?
                }
                SampleFormat::I32 => {
                    build_output::<i32, _>(&device, &config, adapted, cb_shared, err_fn)?
                }
                SampleFormat::I8 => {
                    build_output::<i8, _>(&device, &config, adapted, cb_shared, err_fn)?
                }
                SampleFormat::U8 => {
                    build_output::<u8, _>(&device, &config, adapted, cb_shared, err_fn)?
                }
                SampleFormat::F64 => {
                    build_output::<f64, _>(&device, &config, adapted, cb_shared, err_fn)?
                }
                other => return Err(AudioError::UnsupportedFormat(format!("{other:?}"))),
            };
            Ok((stream, info))
        })?;

        Ok(Self {
            stream,
            shared,
            info,
            total_frames,
        })
    }

    pub fn status(&self) -> PlayerStatus {
        let pos = self
            .shared
            .position
            .load(Ordering::Relaxed)
            .min(self.total_frames);
        let sr = self.info.sample_rate.max(1) as f64;
        PlayerStatus {
            position_frames: pos,
            position_seconds: pos as f64 / sr,
            total_frames: self.total_frames,
            duration_seconds: self.total_frames as f64 / sr,
            finished: self.shared.finished.load(Ordering::Relaxed),
            sample_rate: self.info.sample_rate,
            channels: self.info.channels,
            stream_errors: self.shared.stream_errors.load(Ordering::Relaxed),
        }
    }

    pub fn is_finished(&self) -> bool {
        self.shared.finished.load(Ordering::Relaxed)
    }

    pub fn stop(mut self) {
        self.stream.stop();
    }
}

fn build_output<T, E>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<AudioBuffer>,
    shared: Arc<Shared>,
    err_fn: E,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
    E: FnMut(cpal::StreamError) + Send + 'static,
{
    let channels = config.channels as usize;
    let stream = device.build_output_stream(
        config,
        move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
            let total = buffer.frames();
            let mut pos = shared.position.load(Ordering::Relaxed);
            for frame in out.chunks_mut(channels) {
                if pos >= total {
                    for s in frame.iter_mut() {
                        *s = T::from_sample(0.0_f32);
                    }
                    continue;
                }
                let src = &buffer.samples[pos * channels..(pos + 1) * channels];
                for (dst, &s) in frame.iter_mut().zip(src.iter()) {
                    *dst = T::from_sample(s);
                }
                pos += 1;
            }
            shared.position.store(pos, Ordering::Relaxed);
            if pos >= total {
                shared.finished.store(true, Ordering::Relaxed);
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

    #[test]
    fn adapt_mono_to_stereo_duplicates() {
        let b = AudioBuffer::new(48_000, 1, vec![0.1, 0.2, 0.3]);
        let a = adapt_for_device(&b, 2, 48_000);
        assert_eq!(a.channels, 2);
        assert_eq!(a.samples, vec![0.1, 0.1, 0.2, 0.2, 0.3, 0.3]);
    }

    #[test]
    fn adapt_stereo_to_mono_averages() {
        let b = AudioBuffer::new(48_000, 2, vec![1.0, 0.0, 0.5, 0.5]);
        let a = adapt_for_device(&b, 1, 48_000);
        assert_eq!(a.samples, vec![0.5, 0.5]);
    }

    #[test]
    fn adapt_changes_rate_and_channels_together() {
        let b = AudioBuffer::sine(44_100, 440.0, 1.0, 0.5);
        let a = adapt_for_device(&b, 2, 48_000);
        assert_eq!(a.sample_rate, 48_000);
        assert_eq!(a.channels, 2);
        assert_eq!(a.frames(), 48_000);
    }

    #[test]
    fn empty_buffer_is_rejected() {
        let r = Player::start(&AudioBuffer::empty(48_000, 1), None, 0);
        assert!(r.is_err());
    }

    /// Plays 200 ms of a quiet tone if an output device exists; otherwise fails cleanly.
    #[test]
    fn play_short_tone_without_panicking() {
        let tone = AudioBuffer::sine(48_000, 440.0, 0.2, 0.05);
        match Player::start(&tone, None, 0) {
            Ok(p) => {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
                while !p.is_finished() && std::time::Instant::now() < deadline {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                let st = p.status();
                eprintln!("played {:?}", st);
                p.stop();
                assert!(st.finished, "playback did not finish within 3 s");
            }
            Err(e) => eprintln!("no output device in this environment: {e}"),
        }
    }
}
