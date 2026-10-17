//! Pattern synth (F4.5/F4.6 engine side): a small polyphonic voice that plays a
//! looped note pattern from the beat clock — ostinato and arpeggio figures. Notes
//! carry their own Hz (so scale cents are honoured) and a length in beats, so
//! nothing hangs when the pattern or the chord changes.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use super::rhythm::{BeatClock, Cursor};
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;
use crate::synth::Voice as Timbre;

pub const PATTERN_VOICES: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatternNote {
    pub beat: f64,
    pub length_beats: f64,
    pub hz: f32,
    pub velocity: f32,
}

pub struct NotePatternData {
    pub events: Vec<PatternNote>,
    pub beats: Vec<f64>,
    pub length_beats: f64,
}

impl NotePatternData {
    pub fn new(mut events: Vec<PatternNote>, length_beats: f64) -> Self {
        events.retain(|e| e.beat >= 0.0 && e.beat < length_beats.max(1e-9) && e.hz > 0.0);
        events.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap());
        let beats = events.iter().map(|e| e.beat).collect();
        Self {
            events,
            beats,
            length_beats,
        }
    }
}

pub struct PatternParams {
    /// 0 sine, 1 triangle, 2 flute (see `synth::Voice`).
    pub timbre: AtomicU8,
    pub attack_s: SmoothedParam,
    pub release_s: SmoothedParam,
    /// One-pole low-pass on the sum, Hz.
    pub tone_hz: SmoothedParam,
    /// 0..1 how much velocity shapes loudness.
    pub dynamics: SmoothedParam,
}

impl Default for PatternParams {
    fn default() -> Self {
        Self {
            timbre: AtomicU8::new(2),
            attack_s: SmoothedParam::new(0.01, 0.001, 2.0),
            release_s: SmoothedParam::new(0.15, 0.01, 4.0),
            tone_hz: SmoothedParam::new(6000.0, 200.0, 20_000.0),
            dynamics: SmoothedParam::new(0.6, 0.0, 1.0),
        }
    }
}

#[derive(Clone, Copy)]
struct PVoice {
    hz: f32,
    phase: f64,
    t: f64,
    env: f32,
    gain: f32,
    gate_until: f64,
    gate: bool,
    active: bool,
    age: u64,
}

impl PVoice {
    const fn silent() -> Self {
        Self {
            hz: 0.0,
            phase: 0.0,
            t: 0.0,
            env: 0.0,
            gain: 0.0,
            gate_until: 0.0,
            gate: false,
            active: false,
            age: 0,
        }
    }
}

pub struct PatternSynthSource {
    params: Arc<PatternParams>,
    clock: Arc<BeatClock>,
    pattern: Arc<NotePatternData>,
    cursor: Cursor,
    voices: [PVoice; PATTERN_VOICES],
    tone: Ramp,
    lp: f32,
    counter: u64,
    frame: u64,
}

impl PatternSynthSource {
    pub fn new(
        params: Arc<PatternParams>,
        clock: Arc<BeatClock>,
        pattern: Arc<NotePatternData>,
    ) -> Self {
        Self {
            tone: Ramp::new(params.tone_hz.target()),
            params,
            clock,
            pattern,
            cursor: Cursor::new(),
            voices: [PVoice::silent(); PATTERN_VOICES],
            lp: 0.0,
            counter: 0,
            frame: 0,
        }
    }

    fn note_on(&mut self, n: PatternNote, until_beat: f64, dynamics: f32) {
        let slot = self
            .voices
            .iter()
            .position(|v| !v.active)
            .unwrap_or_else(|| {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.age)
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            });
        self.counter += 1;
        let v = &mut self.voices[slot];
        *v = PVoice {
            hz: n.hz,
            phase: 0.0,
            t: 0.0,
            env: 0.0,
            gain: 0.3 * (1.0 - dynamics + dynamics * n.velocity.clamp(0.05, 1.0)),
            gate_until: until_beat,
            gate: true,
            active: true,
            age: self.counter,
        };
    }
}

impl Source for PatternSynthSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let f = self.frame;
        self.render_at(out, sample_rate, f);
        self.frame += out.len() as u64;
    }

    fn render_at(&mut self, out: &mut [f32], sample_rate: u32, frame: u64) {
        let sr = sample_rate.max(1) as f64;
        let p = Arc::clone(&self.params);
        self.tone
            .begin(p.tone_hz.target(), ramp_samples(sample_rate));
        let timbre = match p.timbre.load(Ordering::Relaxed) {
            0 => Timbre::Sine,
            1 => Timbre::Triangle,
            _ => Timbre::Flute,
        };
        let att = (1.0 / (p.attack_s.target() as f64 * sr)) as f32;
        let rel = (-6.9078 / (p.release_s.target() * sample_rate as f32)).exp();
        let dynamics = p.dynamics.target();
        let beat0 = self.clock.beat_at(frame, sample_rate);
        let bpf = self.clock.beats_per_frame(sample_rate);
        let generation = self.clock.generation.load(Ordering::Relaxed);
        if let Some(b) = beat0 {
            if self.cursor.needs_seek(generation) {
                self.cursor
                    .seek(&self.pattern.beats, self.pattern.length_beats, b);
                self.cursor.mark(generation);
            }
        } else {
            for v in self.voices.iter_mut() {
                v.gate = false;
            }
        }
        for (i, s) in out.iter_mut().enumerate() {
            let beat = beat0.map(|b0| b0 + i as f64 * bpf);
            if let Some(beat) = beat {
                while let Some(idx) =
                    self.cursor
                        .due(&self.pattern.beats, self.pattern.length_beats, beat)
                {
                    let n = self.pattern.events[idx];
                    let loops = (beat / self.pattern.length_beats).floor();
                    let until = n.beat + loops * self.pattern.length_beats + n.length_beats;
                    self.note_on(n, until, dynamics);
                }
            }
            let mut acc = 0.0f32;
            for v in self.voices.iter_mut() {
                if !v.active {
                    continue;
                }
                if let Some(beat) = beat {
                    if v.gate && beat >= v.gate_until {
                        v.gate = false;
                    }
                }
                if v.gate {
                    v.env = (v.env + att).min(1.0);
                } else {
                    v.env *= rel;
                    if v.env < 1e-4 {
                        v.active = false;
                        continue;
                    }
                }
                acc += v.gain * v.env * crate::synth::wave(timbre, v.phase, v.t) as f32;
                v.phase += v.hz as f64 / sr;
                if v.phase >= 1.0 {
                    v.phase -= 1.0;
                }
                v.t += 1.0 / sr;
            }
            let tone = self.tone.next(p.tone_hz.target());
            let a = 1.0 - (-2.0 * std::f32::consts::PI * tone / sample_rate as f32).exp();
            self.lp = undenormal(self.lp + a * (acc - self.lp));
            *s = super::gen::soft(self.lp);
        }
    }

    fn reset(&mut self) {
        self.voices = [PVoice::silent(); PATTERN_VOICES];
        self.cursor = Cursor::new();
        self.lp = 0.0;
        self.frame = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    #[test]
    fn pattern_notes_sound_on_their_beats_in_tune_and_do_not_hang() {
        let clock = Arc::new(BeatClock::new(120.0)); // beat = 0.5 s
        clock.start(0, 0.0);
        let pat = Arc::new(NotePatternData::new(
            vec![
                PatternNote {
                    beat: 0.0,
                    length_beats: 0.4,
                    hz: 440.0,
                    velocity: 0.9,
                },
                PatternNote {
                    beat: 1.0,
                    length_beats: 0.4,
                    hz: 523.25,
                    velocity: 0.9,
                },
            ],
            2.0,
        ));
        let p = Arc::new(PatternParams::default());
        p.timbre.store(0, Ordering::Relaxed);
        p.release_s.set(0.05);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(
            0,
            Box::new(PatternSynthSource::new(p, Arc::clone(&clock), pat)),
        );
        let b = g.render_offline(2.0, 1);
        assert!(b.samples.iter().all(|s| s.is_finite()));
        let pitch = |from: f64, to: f64| {
            let mut v: Vec<f64> = contour(&b, &PitchConfig::new(48_000), 0.02)
                .into_iter()
                .filter(|p| p.t >= from && p.t < to)
                .filter_map(|p| p.hz)
                .collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        assert!((pitch(0.03, 0.18) - 440.0).abs() < 3.0);
        assert!((pitch(0.53, 0.68) - 523.25).abs() < 3.0);
        // Note 1 is gated 0.2 s + 50 ms release: silent by 0.3 s until the next at 0.5 s.
        assert!(b.slice_frames(15_000, 23_500).rms() < 0.01);
        // The loop repeats: a note at 1.0 s again.
        assert!(b.slice_frames(48_500, 52_000).rms() > 0.05);
        // Stop the clock: everything releases, nothing hangs.
        clock.stop();
        let tail = g.render_offline(0.5, 1);
        assert!(
            tail.slice_frames(12_000, 24_000).rms() < 1e-3,
            "{}",
            tail.slice_frames(12_000, 24_000).rms()
        );
    }
}
