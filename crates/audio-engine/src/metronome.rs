//! Metronome click synthesis and musical grid.
//!
//! The click is a pure function of the absolute frame index ([`ClickSynth::sample_at`]),
//! so it is sample-accurate, never drifts (beat k sits at round(k · 60 · sr / bpm)),
//! and follows seeks and loops for free. Tempo and meter always come from the user;
//! nothing here defaults to a particular tempo or 4/4.
//!
//! Convention: `bpm` counts `unit` notes per minute (7/8 at 240 = 240 eighth notes).
//! `grouping` (e.g. [2, 2, 3]) marks group starts with a secondary accent.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use cpal::traits::DeviceTrait;
use cpal::{FromSample, SampleFormat, SizedSample};
use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;
use crate::devices::{find_device, Direction};
use crate::error::{AudioError, Result};
use crate::stream_thread::{StreamInfo, StreamThread};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClickConfig {
    pub bpm: f64,
    pub numerator: u32,
    pub unit: u32,
    /// Beat groups in `unit` notes; must sum to `numerator`. Empty = no inner accents.
    #[serde(default)]
    pub grouping: Vec<u32>,
    /// Linear gain 0..1.
    pub level: f32,
}

impl ClickConfig {
    pub fn validate(&self) -> Result<()> {
        let bad = |m: String| Err(AudioError::InvalidArgument(m));
        if !(self.bpm.is_finite() && self.bpm >= 20.0 && self.bpm <= 600.0) {
            return bad(format!("bpm {} outside 20..600", self.bpm));
        }
        if self.numerator == 0 || self.numerator > 32 {
            return bad(format!("numerator {} outside 1..32", self.numerator));
        }
        if !matches!(self.unit, 1 | 2 | 4 | 8 | 16 | 32) {
            return bad(format!("unit {} is not a power of two up to 32", self.unit));
        }
        if !self.grouping.is_empty()
            && (self.grouping.contains(&0) || self.grouping.iter().sum::<u32>() != self.numerator)
        {
            return bad(format!(
                "grouping {:?} does not add up to {}",
                self.grouping, self.numerator
            ));
        }
        if !(0.0..=1.0).contains(&self.level) {
            return bad(format!("level {} outside 0..1", self.level));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClickKind {
    Downbeat,
    GroupStart,
    Beat,
}

/// Position on the musical grid (1-based bar and beat).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridPosition {
    pub bar: u64,
    pub beat: u32,
    /// Fraction of the way to the next beat, 0..1.
    pub beat_fraction: f64,
}

const CLICK_MS: f64 = 25.0;

#[derive(Debug, Clone)]
pub struct ClickSynth {
    sample_rate: u32,
    frames_per_beat: f64,
    kinds: Vec<ClickKind>,
    waves: [Vec<f32>; 3],
    /// Frame of beat 1 of bar 1.
    origin: u64,
}

fn burst(sample_rate: u32, freq: f64, gain: f32) -> Vec<f32> {
    let n = (CLICK_MS / 1000.0 * sample_rate as f64).round() as usize;
    (0..n)
        .map(|i| {
            let t = i as f64 / sample_rate as f64;
            // fast attack (1 ms) then exponential decay
            let env = (t / 0.001).min(1.0) * (-t / 0.006).exp();
            (gain as f64 * env * (2.0 * std::f64::consts::PI * freq * t).sin()) as f32
        })
        .collect()
}

impl ClickSynth {
    pub fn new(cfg: &ClickConfig, sample_rate: u32, origin: u64) -> Result<Self> {
        cfg.validate()?;
        if sample_rate == 0 {
            return Err(AudioError::InvalidArgument(
                "sample_rate must be > 0".into(),
            ));
        }
        let mut kinds = vec![ClickKind::Beat; cfg.numerator as usize];
        let mut at = 0usize;
        for &g in &cfg.grouping {
            kinds[at] = ClickKind::GroupStart;
            at += g as usize;
        }
        kinds[0] = ClickKind::Downbeat;
        Ok(Self {
            sample_rate,
            frames_per_beat: 60.0 * sample_rate as f64 / cfg.bpm,
            kinds,
            waves: [
                burst(sample_rate, 1760.0, cfg.level),
                burst(sample_rate, 1320.0, cfg.level * 0.8),
                burst(sample_rate, 880.0, cfg.level * 0.6),
            ],
            origin,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Frames in one bar (may be fractional).
    pub fn frames_per_bar(&self) -> f64 {
        self.frames_per_beat * self.kinds.len() as f64
    }

    /// Absolute frame of beat `k` (0-based, counted from the origin).
    pub fn beat_frame(&self, k: u64) -> u64 {
        self.origin + (k as f64 * self.frames_per_beat).round() as u64
    }

    pub fn kind_of_beat(&self, k: u64) -> ClickKind {
        self.kinds[(k % self.kinds.len() as u64) as usize]
    }

    /// The last beat at or before `frame`, if the grid has started.
    fn beat_at(&self, frame: u64) -> Option<u64> {
        if frame < self.origin {
            return None;
        }
        let rel = (frame - self.origin) as f64;
        let mut k = (rel / self.frames_per_beat).floor() as u64;
        // Correct for rounding of beat positions.
        if self.beat_frame(k + 1) <= frame {
            k += 1;
        } else if k > 0 && self.beat_frame(k) > frame {
            k -= 1;
        }
        Some(k)
    }

    /// Click output (mono) at an absolute frame. Allocation-free; safe in callbacks.
    #[inline]
    pub fn sample_at(&self, frame: u64) -> f32 {
        let Some(k) = self.beat_at(frame) else {
            return 0.0;
        };
        let off = (frame - self.beat_frame(k)) as usize;
        let wave = match self.kind_of_beat(k) {
            ClickKind::Downbeat => &self.waves[0],
            ClickKind::GroupStart => &self.waves[1],
            ClickKind::Beat => &self.waves[2],
        };
        wave.get(off).copied().unwrap_or(0.0)
    }

    /// Clicks whose onset lies in `[start, end)`.
    pub fn events(&self, start: u64, end: u64) -> Vec<(u64, ClickKind)> {
        let mut k = self.beat_at(start).unwrap_or(0);
        if self.beat_frame(k) < start {
            k += 1;
        }
        let mut out = Vec::new();
        while self.beat_frame(k) < end {
            out.push((self.beat_frame(k), self.kind_of_beat(k)));
            k += 1;
        }
        out
    }

    pub fn position(&self, frame: u64) -> Option<GridPosition> {
        let k = self.beat_at(frame)?;
        let n = self.kinds.len() as u64;
        let span = (self.beat_frame(k + 1) - self.beat_frame(k)).max(1) as f64;
        Some(GridPosition {
            bar: k / n + 1,
            beat: (k % n) as u32 + 1,
            beat_fraction: (frame - self.beat_frame(k)) as f64 / span,
        })
    }

    /// Render `frames` of click (offline), duplicated to `channels`.
    pub fn render(&self, start: u64, frames: usize, channels: u16) -> AudioBuffer {
        let ch = channels.max(1) as usize;
        let mut out = Vec::with_capacity(frames * ch);
        for i in 0..frames as u64 {
            let s = self.sample_at(start + i);
            out.extend(std::iter::repeat_n(s, ch));
        }
        AudioBuffer::new(self.sample_rate, channels.max(1), out)
    }
}

/// Mix a click track into a copy of `buffer` (the input is never modified).
pub fn mix_click(buffer: &AudioBuffer, click: &ClickSynth) -> Result<AudioBuffer> {
    if click.sample_rate() != buffer.sample_rate {
        return Err(AudioError::InvalidArgument(
            "click and buffer sample rates differ".into(),
        ));
    }
    let ch = buffer.channels as usize;
    let mut out = buffer.clone();
    for (i, frame) in out.samples.chunks_mut(ch).enumerate() {
        let c = click.sample_at(i as u64);
        if c != 0.0 {
            for s in frame {
                *s += c;
            }
        }
    }
    Ok(out)
}

// ---------- live metronome stream (count-in / click while recording) ----------

#[derive(Debug, Default)]
struct Shared {
    frame: AtomicU64,
    stream_errors: AtomicU64,
    muted: AtomicBool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetronomeStatus {
    pub frame: u64,
    pub sample_rate: u32,
    pub position: Option<GridPosition>,
    pub stream_errors: u64,
}

/// A running click on an output device, independent of playback.
pub struct Metronome {
    stream: StreamThread,
    shared: Arc<Shared>,
    synth: ClickSynth,
    info: StreamInfo,
}

impl Metronome {
    pub fn start(device_name: Option<&str>, cfg: &ClickConfig) -> Result<Self> {
        cfg.validate()?;
        let device = find_device(Direction::Output, device_name)?;
        let supported = device.default_output_config()?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let info = StreamInfo {
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        };
        let synth = ClickSynth::new(cfg, info.sample_rate, 0)?;
        let shared = Arc::new(Shared::default());
        let (cb_synth, cb_shared) = (synth.clone(), Arc::clone(&shared));
        let (stream, info) = StreamThread::spawn("ethnoflow-metronome", move || {
            let err_shared = Arc::clone(&cb_shared);
            let err_fn = move |e: cpal::StreamError| {
                log::error!("metronome stream error: {e}");
                err_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
            };
            macro_rules! build {
                ($t:ty) => {
                    build_click::<$t, _>(&device, &config, cb_synth, cb_shared, err_fn)?
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
            shared,
            synth,
            info,
        })
    }

    pub fn set_muted(&self, muted: bool) {
        self.shared.muted.store(muted, Ordering::Relaxed);
    }

    pub fn status(&self) -> MetronomeStatus {
        let frame = self.shared.frame.load(Ordering::Relaxed);
        MetronomeStatus {
            frame,
            sample_rate: self.info.sample_rate,
            position: self.synth.position(frame),
            stream_errors: self.shared.stream_errors.load(Ordering::Relaxed),
        }
    }

    pub fn stop(mut self) {
        self.stream.stop();
    }
}

fn build_click<T, E>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    synth: ClickSynth,
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
            let mut frame = shared.frame.load(Ordering::Relaxed);
            let muted = shared.muted.load(Ordering::Relaxed);
            for f in out.chunks_mut(channels) {
                let s = if muted { 0.0 } else { synth.sample_at(frame) };
                for d in f {
                    *d = T::from_sample(s);
                }
                frame += 1;
            }
            shared.frame.store(frame, Ordering::Relaxed);
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(bpm: f64, numerator: u32, unit: u32, grouping: Vec<u32>) -> ClickConfig {
        ClickConfig {
            bpm,
            numerator,
            unit,
            grouping,
            level: 0.5,
        }
    }

    /// Onsets land exactly on round(k · 60 · sr / bpm) for many tempos and meters,
    /// including after an hour (no drift).
    #[test]
    fn clicks_are_sample_accurate_without_drift() {
        for &(bpm, sr) in &[
            (60.0, 48_000u32),
            (97.3, 44_100),
            (174.0, 48_000),
            (233.0, 96_000),
        ] {
            let s = ClickSynth::new(&cfg(bpm, 3, 4, vec![]), sr, 0).unwrap();
            for k in [0u64, 1, 2, 7, 1_000, 3_600 * bpm as u64 / 60] {
                let expect = (k as f64 * 60.0 * sr as f64 / bpm).round() as u64;
                assert_eq!(s.beat_frame(k), expect);
                // The rendered onset: first nonzero sample after silence is at expect+1
                // (sin(0) = 0), and the sample just before the onset is silent.
                assert!(s.sample_at(expect + 1) != 0.0, "bpm {bpm} k {k}");
                if expect > 0 {
                    let gap = s.beat_frame(k) - s.beat_frame(k.saturating_sub(1));
                    if gap > (CLICK_MS / 1000.0 * sr as f64) as u64 + 1 {
                        assert_eq!(s.sample_at(expect - 1), 0.0);
                    }
                }
            }
        }
    }

    #[test]
    fn odd_meter_groupings_place_accents() {
        let s = ClickSynth::new(&cfg(280.0, 7, 8, vec![2, 2, 3]), 48_000, 0).unwrap();
        let kinds: Vec<_> = (0..14).map(|k| s.kind_of_beat(k)).collect();
        use ClickKind::*;
        assert_eq!(
            kinds,
            vec![
                Downbeat, Beat, GroupStart, Beat, GroupStart, Beat, Beat, //
                Downbeat, Beat, GroupStart, Beat, GroupStart, Beat, Beat
            ]
        );
        let s9 = ClickSynth::new(&cfg(300.0, 9, 8, vec![2, 2, 2, 3]), 48_000, 0).unwrap();
        assert_eq!(s9.kind_of_beat(6), GroupStart);
        assert_eq!(s9.kind_of_beat(7), Beat);
        let s5 = ClickSynth::new(&cfg(200.0, 5, 8, vec![3, 2]), 48_000, 0).unwrap();
        assert_eq!(s5.kind_of_beat(3), GroupStart);
    }

    #[test]
    fn events_and_positions_agree() {
        let s = ClickSynth::new(&cfg(120.0, 4, 4, vec![]), 48_000, 1000).unwrap();
        let ev = s.events(0, 48_000 * 2 + 1000);
        assert_eq!(ev.len(), 4);
        assert_eq!(ev[0], (1000, ClickKind::Downbeat));
        assert_eq!(ev[1].0, 1000 + 24_000);
        assert_eq!(s.position(999), None, "before the origin");
        let p = s.position(1000 + 24_000 * 5 + 12_000).unwrap();
        assert_eq!((p.bar, p.beat), (2, 2));
        assert!((p.beat_fraction - 0.5).abs() < 1e-9);
        // Events in a window that starts mid-beat begin at the next onset.
        assert_eq!(s.events(1001, 1000 + 24_001)[0].0, 1000 + 24_000);
    }

    #[test]
    fn seek_gives_same_output_as_continuous_render() {
        let s = ClickSynth::new(&cfg(137.0, 7, 8, vec![3, 2, 2]), 44_100, 0).unwrap();
        let whole = s.render(0, 44_100 * 3, 1);
        let tail = s.render(50_000, 44_100 * 3 - 50_000, 1);
        assert_eq!(&whole.samples[50_000..], &tail.samples[..]);
    }

    #[test]
    fn mixing_creates_a_new_buffer() {
        let take = AudioBuffer::silence(48_000, 2, 1.0);
        let s = ClickSynth::new(&cfg(120.0, 4, 4, vec![]), 48_000, 0).unwrap();
        let mixed = mix_click(&take, &s).unwrap();
        assert!(take.samples.iter().all(|&x| x == 0.0), "original untouched");
        assert!(mixed.peak() > 0.1);
        assert_eq!(mixed.frames(), take.frames());
        let other_rate = AudioBuffer::silence(44_100, 1, 0.1);
        assert!(mix_click(&other_rate, &s).is_err());
    }

    #[test]
    fn invalid_configs_are_rejected() {
        assert!(cfg(0.0, 4, 4, vec![]).validate().is_err());
        assert!(cfg(120.0, 0, 4, vec![]).validate().is_err());
        assert!(cfg(120.0, 7, 6, vec![]).validate().is_err());
        assert!(cfg(120.0, 7, 8, vec![2, 2, 2]).validate().is_err());
        assert!(cfg(120.0, 7, 8, vec![0, 7]).validate().is_err());
        assert!(cfg(f64::NAN, 4, 4, vec![]).validate().is_err());
        assert!(cfg(120.0, 7, 8, vec![2, 2, 3]).validate().is_ok());
    }

    /// Real device: 300 ms of click (skips when there is no output).
    #[test]
    fn live_metronome_runs_and_advances() {
        match Metronome::start(None, &cfg(120.0, 4, 4, vec![])) {
            Ok(m) => {
                // Device start-up can take a while when other streams open at once.
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                let mut st = m.status();
                while st.frame == 0 && std::time::Instant::now() < deadline {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    st = m.status();
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
                let st = m.status();
                m.stop();
                assert!(st.frame > 0, "metronome did not advance within 2 s");
                eprintln!(
                    "metronome advanced {} frames @ {} Hz",
                    st.frame, st.sample_rate
                );
            }
            Err(e) => eprintln!("no output device in this environment: {e}"),
        }
    }
}
