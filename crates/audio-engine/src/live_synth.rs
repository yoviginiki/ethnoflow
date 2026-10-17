//! Live synthesiser: note on/off from the UI (computer keyboard, later MIDI) to an
//! output stream. The callback owns the voices; commands arrive through an SPSC ring
//! buffer, so it never allocates or locks. 8 voices, oldest stolen when full.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use cpal::traits::DeviceTrait;
use cpal::{FromSample, SampleFormat, SizedSample};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use serde::{Deserialize, Serialize};

use crate::devices::{find_device, Direction};
use crate::error::{AudioError, Result};
use crate::stream_thread::{StreamInfo, StreamThread};
use crate::synth::Voice as Timbre;

const VOICES: usize = 8;
const ATTACK_S: f64 = 0.008;
const RELEASE_S: f64 = 0.12;
const GAIN: f64 = 0.28;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Cmd {
    On { midi: u8, velocity: f32 },
    Off { midi: u8 },
    AllOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Off,
    Attack,
    Sustain,
    Release,
}

#[derive(Debug, Clone, Copy)]
struct VoiceState {
    midi: u8,
    freq: f64,
    phase: f64,
    gain: f64,
    env: f64,
    stage: Stage,
    age: u64,
    t: f64,
}

impl VoiceState {
    const fn silent() -> Self {
        Self {
            midi: 0,
            freq: 0.0,
            phase: 0.0,
            gain: 0.0,
            env: 0.0,
            stage: Stage::Off,
            age: 0,
            t: 0.0,
        }
    }
}

/// The DSP core, usable offline in tests. Lives inside the audio callback.
pub struct LiveEngine {
    sample_rate: f64,
    a4_hz: f64,
    timbre: Timbre,
    voices: [VoiceState; VOICES],
    clock: u64,
}

impl LiveEngine {
    pub fn new(sample_rate: u32, a4_hz: f64, timbre: Timbre) -> Self {
        Self {
            sample_rate: sample_rate.max(1) as f64,
            a4_hz,
            timbre,
            voices: [VoiceState::silent(); VOICES],
            clock: 0,
        }
    }

    pub fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::On { midi, velocity } => {
                // Retrigger a sounding note, else take a free voice, else the oldest.
                let slot = self
                    .voices
                    .iter()
                    .position(|v| v.stage != Stage::Off && v.midi == midi)
                    .or_else(|| self.voices.iter().position(|v| v.stage == Stage::Off))
                    .unwrap_or_else(|| {
                        self.voices
                            .iter()
                            .enumerate()
                            .min_by_key(|(_, v)| v.age)
                            .map(|(i, _)| i)
                            .unwrap_or(0)
                    });
                let v = &mut self.voices[slot];
                let retrigger = v.stage != Stage::Off && v.midi == midi;
                v.midi = midi;
                v.freq = self.a4_hz * 2f64.powf((midi as f64 - 69.0) / 12.0);
                v.gain = GAIN * (0.2 + 0.8 * velocity.clamp(0.0, 1.0) as f64);
                v.stage = Stage::Attack;
                v.age = self.clock;
                if !retrigger {
                    v.phase = 0.0;
                    v.env = 0.0;
                    v.t = 0.0;
                }
            }
            Cmd::Off { midi } => {
                for v in self.voices.iter_mut() {
                    if v.midi == midi && matches!(v.stage, Stage::Attack | Stage::Sustain) {
                        v.stage = Stage::Release;
                    }
                }
            }
            Cmd::AllOff => {
                for v in self.voices.iter_mut() {
                    if v.stage != Stage::Off {
                        v.stage = Stage::Release;
                    }
                }
            }
        }
        self.clock += 1;
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.stage != Stage::Off).count()
    }

    /// Render `frames` interleaved frames of `channels` (same signal on each).
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        let sr = self.sample_rate;
        let att = 1.0 / (ATTACK_S * sr);
        let rel = 1.0 / (RELEASE_S * sr);
        for frame in out.chunks_mut(channels.max(1)) {
            let mut s = 0.0f64;
            for v in self.voices.iter_mut() {
                if v.stage == Stage::Off {
                    continue;
                }
                match v.stage {
                    Stage::Attack => {
                        v.env += att;
                        if v.env >= 1.0 {
                            v.env = 1.0;
                            v.stage = Stage::Sustain;
                        }
                    }
                    Stage::Release => {
                        v.env -= rel;
                        if v.env <= 0.0 {
                            v.env = 0.0;
                            v.stage = Stage::Off;
                            continue;
                        }
                    }
                    _ => {}
                }
                s += v.gain * v.env * crate::synth::wave(self.timbre, v.phase, v.t);
                v.phase += v.freq / sr;
                if v.phase >= 1.0 {
                    v.phase -= 1.0;
                }
                v.t += 1.0 / sr;
            }
            let y = if s.abs() > 0.8 {
                (0.8 + (s.abs() - 0.8).tanh() * 0.19) * s.signum()
            } else {
                s
            } as f32;
            for ch in frame.iter_mut() {
                *ch = y;
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSynthStatus {
    pub sample_rate: u32,
    pub active_voices: usize,
    pub stream_errors: u64,
}

struct Shared {
    active: AtomicUsize,
    stream_errors: AtomicU64,
}

pub struct LiveSynth {
    stream: StreamThread,
    prod: HeapProd<Cmd>,
    shared: Arc<Shared>,
    info: StreamInfo,
}

impl LiveSynth {
    pub fn start(device_name: Option<&str>, a4_hz: f64, timbre: Timbre) -> Result<Self> {
        let device = find_device(Direction::Output, device_name)?;
        let supported = device.default_output_config()?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let info = StreamInfo {
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        };
        let (prod, cons) = HeapRb::<Cmd>::new(256).split();
        let shared = Arc::new(Shared {
            active: AtomicUsize::new(0),
            stream_errors: AtomicU64::new(0),
        });
        let engine = LiveEngine::new(info.sample_rate, a4_hz, timbre);
        let cb_shared = Arc::clone(&shared);
        let (stream, info) = StreamThread::spawn("ethnoflow-live-synth", move || {
            let err_shared = Arc::clone(&cb_shared);
            let err_fn = move |e: cpal::StreamError| {
                log::error!("live synth stream error: {e}");
                err_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
            };
            macro_rules! build {
                ($t:ty) => {
                    build_output::<$t, _>(&device, &config, engine, cons, cb_shared, err_fn)?
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
            prod,
            shared,
            info,
        })
    }

    /// Queue a command; `false` if the queue is full (the callback is not keeping up).
    pub fn send(&mut self, cmd: Cmd) -> bool {
        self.prod.try_push(cmd).is_ok()
    }

    pub fn status(&self) -> LiveSynthStatus {
        LiveSynthStatus {
            sample_rate: self.info.sample_rate,
            active_voices: self.shared.active.load(Ordering::Relaxed),
            stream_errors: self.shared.stream_errors.load(Ordering::Relaxed),
        }
    }

    pub fn stop(mut self) {
        self.stream.stop();
    }
}

fn build_output<T, E>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut engine: LiveEngine,
    mut cons: HeapCons<Cmd>,
    shared: Arc<Shared>,
    err_fn: E,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
    E: FnMut(cpal::StreamError) + Send + 'static,
{
    let channels = config.channels as usize;
    let mut scratch = vec![0.0f32; 4096 * channels];
    let stream = device.build_output_stream(
        config,
        move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
            while let Some(cmd) = cons.try_pop() {
                engine.handle(cmd);
            }
            let n = out.len().min(scratch.len());
            engine.process(&mut scratch[..n], channels);
            for (dst, &s) in out.iter_mut().zip(scratch.iter()) {
                *dst = T::from_sample(s);
            }
            for dst in out.iter_mut().skip(n) {
                *dst = T::from_sample(0.0f32);
            }
            shared
                .active
                .store(engine.active_voices(), Ordering::Relaxed);
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::pitch::{contour, PitchConfig};

    fn render(engine: &mut LiveEngine, seconds: f64) -> AudioBuffer {
        let n = (seconds * 48_000.0) as usize;
        let mut out = vec![0.0f32; n];
        engine.process(&mut out, 1);
        AudioBuffer::new(48_000, 1, out)
    }

    #[test]
    fn note_on_sounds_at_the_right_pitch_and_off_releases() {
        let mut e = LiveEngine::new(48_000, 440.0, Timbre::Sine);
        e.handle(Cmd::On {
            midi: 69,
            velocity: 0.8,
        });
        let on = render(&mut e, 0.5);
        let mut hz: Vec<f64> = contour(&on, &PitchConfig::new(48_000), 0.01)
            .into_iter()
            .filter_map(|p| p.hz)
            .collect();
        hz.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = hz[hz.len() / 2];
        assert!((1200.0 * (med / 440.0).log2()).abs() < 2.0, "{med} Hz");
        assert_eq!(e.active_voices(), 1);
        e.handle(Cmd::Off { midi: 69 });
        let tail = render(&mut e, 0.3);
        assert_eq!(e.active_voices(), 0, "released after 120 ms");
        assert!(
            tail.slice_frames(48_000 / 5, 48_000 * 3 / 10).peak() == 0.0,
            "silent after release"
        );
        assert!(
            tail.slice_frames(0, 1000).peak() > 0.0,
            "release tail audible"
        );
    }

    #[test]
    fn polyphony_steals_the_oldest_voice_and_all_off_clears() {
        let mut e = LiveEngine::new(48_000, 440.0, Timbre::Triangle);
        for m in 60..(60 + VOICES as u8 + 2) {
            e.handle(Cmd::On {
                midi: m,
                velocity: 0.5,
            });
        }
        assert_eq!(e.active_voices(), VOICES);
        assert!(
            !e.voices.iter().any(|v| v.midi == 60),
            "oldest (60) was stolen"
        );
        assert!(e.voices.iter().any(|v| v.midi == 60 + VOICES as u8 + 1));
        let chord = render(&mut e, 0.2);
        assert!(chord.peak() <= 1.0 && chord.peak() > 0.3);
        e.handle(Cmd::AllOff);
        render(&mut e, 0.3);
        assert_eq!(e.active_voices(), 0);
    }

    /// Real device: opens, plays a short note, stops (skips without an output).
    #[test]
    fn live_synth_runs_on_a_device() {
        match LiveSynth::start(None, 440.0, Timbre::Flute) {
            Ok(mut s) => {
                assert!(s.send(Cmd::On {
                    midi: 62,
                    velocity: 0.3
                }));
                std::thread::sleep(std::time::Duration::from_millis(250));
                let st = s.status();
                assert!(s.send(Cmd::Off { midi: 62 }));
                std::thread::sleep(std::time::Duration::from_millis(100));
                s.stop();
                eprintln!(
                    "live synth: {} active voices @ {} Hz",
                    st.active_voices, st.sample_rate
                );
            }
            Err(e) => eprintln!("no output device in this environment: {e}"),
        }
    }
}
