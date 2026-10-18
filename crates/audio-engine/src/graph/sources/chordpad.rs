//! Chord pad (P1 R3): a polyphonic pad that plays the section's chord on the shared
//! beat clock — two detuned band-limited saws (or triangles) per note, an attack /
//! release envelope, a state-variable low-pass with slow movement. Unlike the ambient
//! `PadSource` (free-running, XY-driven) this one follows a chord *pattern*: each event
//! says which pitches sound from which beat for how long, so it can breathe with the
//! bars and change with the harmony.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use super::gen::{soft, Svf};
use super::rhythm::{BeatClock, Cursor, PatternInbox};
use crate::graph::params::undenormal;
use crate::graph::params::{ramp_samples, Ramp, SmoothedParam};
use crate::graph::Source;

pub const CHORD_NOTES: usize = 6;
pub const CHORDPAD_VOICES: usize = 12;

#[derive(Debug, Clone, PartialEq)]
pub struct ChordEvent {
    pub beat: f64,
    pub length_beats: f64,
    /// Up to `CHORD_NOTES` frequencies; extra ones are ignored.
    pub hz: Vec<f32>,
    pub velocity: f32,
}

pub struct ChordPatternData {
    pub events: Vec<ChordEvent>,
    pub beats: Vec<f64>,
    pub length_beats: f64,
}

impl ChordPatternData {
    pub fn new(mut events: Vec<ChordEvent>, length_beats: f64) -> Self {
        events.retain(|e| e.beat >= 0.0 && e.beat < length_beats.max(1e-9) && !e.hz.is_empty());
        events.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap());
        let beats = events.iter().map(|e| e.beat).collect();
        Self {
            events,
            beats,
            length_beats,
        }
    }
}

pub struct ChordPadParams {
    /// 0 saw, 1 triangle.
    pub wave: AtomicU8,
    pub attack_s: SmoothedParam,
    pub release_s: SmoothedParam,
    /// Detune between the two oscillators of a note, cents (0..30).
    pub detune_cents: SmoothedParam,
    /// Low-pass cutoff, Hz (200..12 000).
    pub cutoff_hz: SmoothedParam,
    /// 0..1 slow cutoff movement (LFO ≈ 0.08 Hz, ±1 octave at 1).
    pub movement: SmoothedParam,
    /// 0..1 how much velocity shapes loudness.
    pub dynamics: SmoothedParam,
}

impl Default for ChordPadParams {
    fn default() -> Self {
        Self {
            wave: AtomicU8::new(0),
            attack_s: SmoothedParam::new(0.6, 0.005, 4.0),
            release_s: SmoothedParam::new(1.2, 0.05, 8.0),
            detune_cents: SmoothedParam::new(8.0, 0.0, 30.0),
            cutoff_hz: SmoothedParam::new(1800.0, 200.0, 12_000.0),
            movement: SmoothedParam::new(0.3, 0.0, 1.0),
            dynamics: SmoothedParam::new(0.4, 0.0, 1.0),
        }
    }
}

#[derive(Clone, Copy)]
struct Voice {
    hz: f32,
    phase_a: f32,
    phase_b: f32,
    env: f32,
    gain: f32,
    gate_until: f64,
    gate: bool,
    active: bool,
    age: u64,
}

impl Voice {
    const fn silent() -> Self {
        Self {
            hz: 0.0,
            phase_a: 0.0,
            phase_b: 0.0,
            env: 0.0,
            gain: 0.0,
            gate_until: 0.0,
            gate: false,
            active: false,
            age: 0,
        }
    }
}

/// PolyBLEP saw in −1..1 for phase `t` in 0..1 and phase increment `dt`.
#[inline]
fn saw_blep(t: f32, dt: f32) -> f32 {
    let mut y = 2.0 * t - 1.0;
    if t < dt {
        let x = t / dt;
        y -= x + x - x * x - 1.0;
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        y -= x * x + x + x + 1.0;
    }
    y
}

#[inline]
fn tri(t: f32) -> f32 {
    4.0 * (t - (t + 0.5).floor()).abs() - 1.0
}

pub struct ChordPadSource {
    params: Arc<ChordPadParams>,
    clock: Arc<BeatClock>,
    pattern: Arc<ChordPatternData>,
    cursor: Cursor,
    voices: [Voice; CHORDPAD_VOICES],
    lp: Svf,
    cutoff: Ramp,
    lfo_phase: f32,
    counter: u64,
    frame: u64,
    inbox: Option<PatternInbox<ChordPatternData>>,
}

impl ChordPadSource {
    pub fn with_inbox(mut self, inbox: PatternInbox<ChordPatternData>) -> Self {
        self.inbox = Some(inbox);
        self
    }

    pub fn new(
        params: Arc<ChordPadParams>,
        clock: Arc<BeatClock>,
        pattern: Arc<ChordPatternData>,
    ) -> Self {
        Self {
            cutoff: Ramp::new(params.cutoff_hz.target()),
            params,
            clock,
            pattern,
            cursor: Cursor::new(),
            voices: [Voice::silent(); CHORDPAD_VOICES],
            lp: Svf::default(),
            lfo_phase: 0.0,
            counter: 0,
            frame: 0,
            inbox: None,
        }
    }

    fn chord_on(&mut self, e: &ChordEvent, until: f64, dynamics: f32) {
        // A new chord releases the sounding one (legato overlap through the release).
        for v in self.voices.iter_mut() {
            v.gate = false;
        }
        let gain = 0.22 * (1.0 - dynamics + dynamics * e.velocity.clamp(0.05, 1.0));
        for &hz in e.hz.iter().take(CHORD_NOTES) {
            if hz <= 0.0 {
                continue;
            }
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
            self.voices[slot] = Voice {
                hz,
                phase_a: 0.0,
                phase_b: 0.37,
                env: 0.0,
                gain,
                gate_until: until,
                gate: true,
                active: true,
                age: self.counter,
            };
        }
    }
}

impl Source for ChordPadSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let f = self.frame;
        self.render_at(out, sample_rate, f);
        self.frame += out.len() as u64;
    }

    fn render_at(&mut self, out: &mut [f32], sample_rate: u32, frame: u64) {
        let sr = sample_rate.max(1) as f32;
        let p = Arc::clone(&self.params);
        self.cutoff
            .begin(p.cutoff_hz.target(), ramp_samples(sample_rate));
        let wave = p.wave.load(Ordering::Relaxed);
        let att = 1.0 / (p.attack_s.target() * sr);
        let rel = (-6.9078 / (p.release_s.target() * sr)).exp();
        let detune = (p.detune_cents.target() / 1200.0).exp2();
        let movement = p.movement.target();
        let dynamics = p.dynamics.target();
        let beat0 = self.clock.beat_at(frame, sample_rate);
        let bpf = self.clock.beats_per_frame(sample_rate);
        let generation = self.clock.generation.load(Ordering::Relaxed);
        let swapped = self
            .inbox
            .as_mut()
            .map(|i| i.take(&mut self.pattern))
            .unwrap_or(false);
        if let Some(b) = beat0 {
            if swapped || self.cursor.needs_seek(generation) {
                self.cursor
                    .seek(&self.pattern.beats, self.pattern.length_beats, b);
                self.cursor.mark(generation);
            }
        } else {
            for v in self.voices.iter_mut() {
                v.gate = false;
            }
        }
        let lfo_inc = 0.08 / sr;
        for (i, s) in out.iter_mut().enumerate() {
            let beat = beat0.map(|b0| b0 + i as f64 * bpf);
            if let Some(beat) = beat {
                while let Some(idx) =
                    self.cursor
                        .due(&self.pattern.beats, self.pattern.length_beats, beat)
                {
                    let e = Arc::clone(&self.pattern);
                    let ev = &e.events[idx];
                    let loops = (beat / e.length_beats).floor();
                    let until = ev.beat + loops * e.length_beats + ev.length_beats;
                    self.chord_on(ev, until, dynamics);
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
                let dt_a = v.hz / sr;
                let dt_b = v.hz * detune / sr;
                let y = if wave == 0 {
                    saw_blep(v.phase_a, dt_a) + saw_blep(v.phase_b, dt_b)
                } else {
                    tri(v.phase_a) + tri(v.phase_b)
                };
                v.phase_a += dt_a;
                if v.phase_a >= 1.0 {
                    v.phase_a -= 1.0;
                }
                v.phase_b += dt_b;
                if v.phase_b >= 1.0 {
                    v.phase_b -= 1.0;
                }
                acc += y * 0.5 * v.gain * v.env;
            }
            self.lfo_phase += lfo_inc;
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= 1.0;
            }
            let lfo = (self.lfo_phase * std::f32::consts::TAU).sin();
            let cutoff = self.cutoff.next(p.cutoff_hz.target()) * (movement * lfo).exp2();
            self.lp.set(cutoff.clamp(80.0, 16_000.0), 0.8, sr);
            *s = undenormal(soft(self.lp.process(acc).lp));
        }
    }

    fn reset(&mut self) {
        self.voices = [Voice::silent(); CHORDPAD_VOICES];
        self.cursor = Cursor::new();
        self.lp.clear();
        self.frame = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, GraphParams};

    fn goertzel(x: &[f32], hz: f32, sr: f32) -> f32 {
        let w = std::f32::consts::TAU * hz / sr;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &v in x {
            let s0 = v + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2).sqrt() / x.len() as f32
    }

    #[test]
    fn chord_tones_sound_and_the_pad_releases_after_the_event() {
        let clock = Arc::new(BeatClock::new(120.0)); // beat = 0.5 s
        clock.start(0, 0.0);
        let pat = Arc::new(ChordPatternData::new(
            vec![ChordEvent {
                beat: 0.0,
                length_beats: 2.0,
                hz: vec![220.0, 277.18, 329.63],
                velocity: 0.9,
            }],
            8.0,
        ));
        let p = Arc::new(ChordPadParams::default());
        p.attack_s.set(0.05);
        p.release_s.set(0.2);
        p.cutoff_hz.set(6000.0);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(ChordPadSource::new(p, clock, pat)));
        let b = g.render_offline(3.0, 1);
        let x = &b.samples;
        assert!(x.iter().all(|v| v.is_finite()));
        let sustain = &x[24_000..48_000]; // 0.5–1.0 s: all three notes held
        let peak = sustain.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.05 && peak <= 1.0, "audible {peak}");
        for hz in [220.0, 277.18, 329.63] {
            let on = goertzel(sustain, hz, 48_000.0);
            let off = goertzel(sustain, hz * 1.0595, 48_000.0); // a semitone up: not in the chord
            assert!(on > off * 3.0, "{hz} Hz present: {on} vs {off}");
        }
        // Released: 1 s after the event ends (ends at 1.0 s) the tail is quiet.
        let tail = x[120_000..].iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(tail < 0.01, "tail {tail}");
    }
}
