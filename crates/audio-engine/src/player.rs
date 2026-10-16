//! Buffer playback through an output device.
//!
//! The buffer is adapted (channel-mapped and resampled) once, before the stream starts,
//! so the real-time callback is a plain indexed copy with an atomic read position.
//! Loop range and click are controlled through atomics and can change while playing.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use cpal::traits::DeviceTrait;
use cpal::{FromSample, SampleFormat, SizedSample};
use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;
use crate::devices::{find_device, Direction};
use crate::error::{AudioError, Result};
use crate::metronome::{ClickConfig, ClickSynth};
use crate::resample::resample_linear;
use crate::stream_thread::{StreamInfo, StreamThread};

#[derive(Debug, Default)]
struct Shared {
    /// Read position in frames.
    position: AtomicUsize,
    finished: AtomicBool,
    stream_errors: AtomicU64,
    /// Loop range in device frames; `loop_end == 0` means no loop.
    loop_start: AtomicUsize,
    loop_end: AtomicUsize,
    click_on: AtomicBool,
}

/// Optional playback features. Frames are in the *buffer's* sample rate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayOptions {
    /// Loop `[start, end)` until stopped.
    pub loop_frames: Option<(usize, usize)>,
    /// Click aligned so that buffer frame 0 is beat 1 of bar 1.
    pub click: Option<ClickConfig>,
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
    pub looping: bool,
}

pub struct Player {
    stream: StreamThread,
    shared: Arc<Shared>,
    info: StreamInfo,
    total_frames: usize,
    source_rate: u32,
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
        Self::start_with(buffer, device_name, start_frame, &PlayOptions::default())
    }

    /// Start with loop and/or click options.
    pub fn start_with(
        buffer: &AudioBuffer,
        device_name: Option<&str>,
        start_frame: usize,
        opts: &PlayOptions,
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
        let click = match &opts.click {
            Some(cfg) => {
                shared.click_on.store(true, Ordering::Relaxed);
                Some(ClickSynth::new(cfg, info.sample_rate, 0)?)
            }
            None => None,
        };

        let cb_shared = Arc::clone(&shared);
        let (stream, info) = StreamThread::spawn("ethnoflow-play-stream", move || {
            let err_shared = Arc::clone(&cb_shared);
            let err_fn = move |e: cpal::StreamError| {
                log::error!("output stream error: {e}");
                err_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
            };
            macro_rules! build {
                ($t:ty) => {
                    build_output::<$t, _>(&device, &config, adapted, click, cb_shared, err_fn)?
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

        let player = Self {
            stream,
            shared,
            info,
            total_frames,
            source_rate: buffer.sample_rate.max(1),
        };
        player.set_loop(opts.loop_frames);
        Ok(player)
    }

    fn to_device_frames(&self, f: usize) -> usize {
        (f as f64 * self.info.sample_rate as f64 / self.source_rate as f64).round() as usize
    }

    /// Set or clear the loop range (buffer frames) while playing.
    pub fn set_loop(&self, range: Option<(usize, usize)>) {
        match range {
            Some((a, b)) if b > a => {
                let a = self
                    .to_device_frames(a)
                    .min(self.total_frames.saturating_sub(1));
                let b = self.to_device_frames(b).min(self.total_frames).max(a + 1);
                self.shared.loop_start.store(a, Ordering::Relaxed);
                self.shared.loop_end.store(b, Ordering::Relaxed);
            }
            _ => self.shared.loop_end.store(0, Ordering::Relaxed),
        }
    }

    /// Mute or unmute the click (only has an effect if started with one).
    pub fn set_click(&self, on: bool) {
        self.shared.click_on.store(on, Ordering::Relaxed);
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
            looping: self.shared.loop_end.load(Ordering::Relaxed) > 0,
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
    click: Option<ClickSynth>,
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
            render_block(out, channels, &buffer, click.as_ref(), &shared);
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

/// Body of the output callback. No allocation, no locks, no I/O.
fn render_block<T: SizedSample + FromSample<f32>>(
    out: &mut [T],
    channels: usize,
    buffer: &AudioBuffer,
    click: Option<&ClickSynth>,
    shared: &Shared,
) {
    let total = buffer.frames();
    let loop_end = shared.loop_end.load(Ordering::Relaxed).min(total);
    let loop_start = shared.loop_start.load(Ordering::Relaxed);
    let looping = loop_end > loop_start;
    let click = click.filter(|_| shared.click_on.load(Ordering::Relaxed));
    let mut pos = shared.position.load(Ordering::Relaxed);
    for frame in out.chunks_mut(channels) {
        if looping && pos >= loop_end {
            pos = loop_start;
        }
        if pos >= total {
            for s in frame.iter_mut() {
                *s = T::from_sample(0.0_f32);
            }
            continue;
        }
        let c = click.map_or(0.0, |k| k.sample_at(pos as u64));
        let src = &buffer.samples[pos * channels..(pos + 1) * channels];
        for (dst, &s) in frame.iter_mut().zip(src.iter()) {
            *dst = T::from_sample(s + c);
        }
        pos += 1;
    }
    shared.position.store(pos, Ordering::Relaxed);
    if pos >= total && !looping {
        shared.finished.store(true, Ordering::Relaxed);
    }
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

    #[test]
    fn loop_wraps_sample_exactly_and_never_finishes() {
        let buf = AudioBuffer::new(1_000, 1, (0..10).map(|i| i as f32).collect());
        let shared = Shared::default();
        shared.position.store(6, Ordering::Relaxed);
        shared.loop_start.store(5, Ordering::Relaxed);
        shared.loop_end.store(8, Ordering::Relaxed);
        let mut out = [0.0_f32; 7];
        render_block(&mut out, 1, &buf, None, &shared);
        assert_eq!(out, [6.0, 7.0, 5.0, 6.0, 7.0, 5.0, 6.0]);
        assert!(!shared.finished.load(Ordering::Relaxed));
        // Clearing the loop lets playback run to the end.
        shared.loop_end.store(0, Ordering::Relaxed);
        let mut out = [9.9_f32; 5];
        render_block(&mut out, 1, &buf, None, &shared);
        assert_eq!(out, [7.0, 8.0, 9.0, 0.0, 0.0]);
        assert!(shared.finished.load(Ordering::Relaxed));
    }

    #[test]
    fn click_is_mixed_on_the_grid_and_mutable() {
        let buf = AudioBuffer::silence(48_000, 2, 1.0);
        let cfg = ClickConfig {
            bpm: 120.0,
            numerator: 4,
            unit: 4,
            grouping: vec![],
            level: 0.5,
        };
        let synth = ClickSynth::new(&cfg, 48_000, 0).unwrap();
        let shared = Shared::default();
        shared.click_on.store(true, Ordering::Relaxed);
        let mut out = vec![0.0_f32; 48_000 * 2];
        render_block(&mut out, 2, &buf, Some(&synth), &shared);
        assert_eq!(out, synth.render(0, 48_000, 2).samples);
        shared.position.store(0, Ordering::Relaxed);
        shared.click_on.store(false, Ordering::Relaxed);
        render_block(&mut out, 2, &buf, Some(&synth), &shared);
        assert!(out.iter().all(|&s| s == 0.0));
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
