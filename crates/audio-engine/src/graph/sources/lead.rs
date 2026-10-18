//! Lead (P1 R3): a monophonic melodic voice on the shared beat clock — saw / pulse /
//! triangle through a resonant state-variable low-pass with a filter envelope, glide
//! between notes, delayed vibrato, drive. It plays a `NotePatternData` (the same type
//! the ostinato and arpeggio use), one note at a time: overlapping notes become legato
//! (no retrigger, glide), separated notes retrigger the envelopes.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use super::gen::{soft, Svf};
use super::patsynth::NotePatternData;
use super::rhythm::{BeatClock, Cursor, PatternInbox};
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;

pub struct LeadParams {
    /// 0 saw, 1 pulse, 2 triangle.
    pub wave: AtomicU8,
    /// Portamento time, s (0 = none).
    pub glide_s: SmoothedParam,
    pub vibrato_hz: SmoothedParam,
    /// Vibrato depth, cents (0..100).
    pub vibrato_cents: SmoothedParam,
    /// Filter cutoff at rest, Hz (200..12 000).
    pub cutoff_hz: SmoothedParam,
    /// 0..0.95 resonance.
    pub resonance: SmoothedParam,
    /// 0..1 how far the filter envelope opens the cutoff (up to +3 octaves).
    pub env_amount: SmoothedParam,
    pub attack_s: SmoothedParam,
    pub release_s: SmoothedParam,
    /// 0..1 drive before the filter.
    pub drive: SmoothedParam,
}

impl Default for LeadParams {
    fn default() -> Self {
        Self {
            wave: AtomicU8::new(0),
            glide_s: SmoothedParam::new(0.04, 0.0, 1.0),
            vibrato_hz: SmoothedParam::new(5.5, 0.5, 9.0),
            vibrato_cents: SmoothedParam::new(12.0, 0.0, 100.0),
            cutoff_hz: SmoothedParam::new(1200.0, 200.0, 12_000.0),
            resonance: SmoothedParam::new(0.3, 0.0, 0.95),
            env_amount: SmoothedParam::new(0.5, 0.0, 1.0),
            attack_s: SmoothedParam::new(0.01, 0.001, 2.0),
            release_s: SmoothedParam::new(0.2, 0.01, 4.0),
            drive: SmoothedParam::new(0.2, 0.0, 1.0),
        }
    }
}

pub struct LeadSource {
    params: Arc<LeadParams>,
    clock: Arc<BeatClock>,
    pattern: Arc<NotePatternData>,
    cursor: Cursor,
    inbox: Option<PatternInbox<NotePatternData>>,
    // Mono voice state.
    hz: f32,
    target_hz: f32,
    phase: f32,
    env: f32,
    fenv: f32,
    gate: bool,
    gate_until: f64,
    note_t: f32,
    vel: f32,
    vib_phase: f32,
    lp: Svf,
    cutoff: Ramp,
    frame: u64,
}

impl LeadSource {
    pub fn with_inbox(mut self, inbox: PatternInbox<NotePatternData>) -> Self {
        self.inbox = Some(inbox);
        self
    }

    pub fn new(
        params: Arc<LeadParams>,
        clock: Arc<BeatClock>,
        pattern: Arc<NotePatternData>,
    ) -> Self {
        Self {
            cutoff: Ramp::new(params.cutoff_hz.target()),
            params,
            clock,
            pattern,
            cursor: Cursor::new(),
            inbox: None,
            hz: 0.0,
            target_hz: 0.0,
            phase: 0.0,
            env: 0.0,
            fenv: 0.0,
            gate: false,
            gate_until: 0.0,
            note_t: 0.0,
            vel: 0.8,
            vib_phase: 0.0,
            lp: Svf::default(),
            frame: 0,
        }
    }

    fn note_on(&mut self, hz: f32, vel: f32, until: f64) {
        let legato = self.gate && self.env > 0.05;
        self.target_hz = hz;
        if !legato || self.hz <= 0.0 {
            self.hz = if self.hz <= 0.0 { hz } else { self.hz };
            self.fenv = 1.0;
            self.note_t = 0.0;
        }
        self.gate = true;
        self.gate_until = until;
        self.vel = vel.clamp(0.05, 1.0);
    }
}

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

impl Source for LeadSource {
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
        let glide = p.glide_s.target();
        // Per-sample glide coefficient: reach ~95 % of the target in glide_s.
        let k_glide = if glide <= 0.0005 {
            1.0
        } else {
            1.0 - (-3.0 / (glide * sr)).exp()
        };
        let vib_hz = p.vibrato_hz.target();
        let vib_depth = (p.vibrato_cents.target() / 1200.0).exp2() - 1.0;
        let res = p.resonance.target();
        let env_amount = p.env_amount.target();
        let att = 1.0 / (p.attack_s.target() * sr);
        let rel = (-6.9078 / (p.release_s.target() * sr)).exp();
        let k_fenv = (-6.9078 / (0.35 * sr)).exp();
        let drive = 1.0 + 6.0 * p.drive.target();
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
            self.gate = false;
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
                    self.note_on(n.hz, n.velocity, until);
                }
                if self.gate && beat >= self.gate_until {
                    self.gate = false;
                }
            }
            if self.gate {
                self.env = (self.env + att).min(1.0);
            } else {
                self.env *= rel;
            }
            if self.env < 1e-4 && !self.gate {
                *s = 0.0;
                self.lp.process(0.0);
                continue;
            }
            // Glide towards the target, then vibrato after 120 ms of the note.
            self.hz += (self.target_hz - self.hz) * k_glide;
            self.vib_phase += vib_hz / sr;
            if self.vib_phase >= 1.0 {
                self.vib_phase -= 1.0;
            }
            let vib_on = (self.note_t - 0.12).clamp(0.0, 0.25) * 4.0;
            let hz = self.hz
                * (1.0 + vib_depth * vib_on * (self.vib_phase * std::f32::consts::TAU).sin());
            let dt = (hz / sr).clamp(0.0, 0.5);
            let y = match wave {
                0 => saw_blep(self.phase, dt),
                1 => saw_blep(self.phase, dt) - saw_blep((self.phase + 0.5).fract(), dt),
                _ => 4.0 * (self.phase - (self.phase + 0.5).floor()).abs() - 1.0,
            };
            self.phase += dt;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            self.fenv *= k_fenv;
            let cutoff =
                self.cutoff.next(p.cutoff_hz.target()) * (3.0 * env_amount * self.fenv).exp2();
            self.lp
                .set(cutoff.clamp(80.0, 16_000.0), 0.7 + res * 8.0, sr);
            let driven = soft(y * drive * (0.35 + 0.65 * self.vel));
            let filtered = self.lp.process(driven).lp;
            self.note_t += 1.0 / sr;
            *s = undenormal(soft(filtered * self.env * 0.6));
        }
    }

    fn reset(&mut self) {
        self.cursor = Cursor::new();
        self.hz = 0.0;
        self.target_hz = 0.0;
        self.env = 0.0;
        self.fenv = 0.0;
        self.gate = false;
        self.lp.clear();
        self.frame = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::super::patsynth::PatternNote;
    use super::*;
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    #[test]
    fn lead_is_mono_in_tune_and_glides_between_notes() {
        let clock = Arc::new(BeatClock::new(120.0)); // beat = 0.5 s
        clock.start(0, 0.0);
        let pat = Arc::new(NotePatternData::new(
            vec![
                PatternNote {
                    beat: 0.0,
                    length_beats: 1.0,
                    hz: 440.0,
                    velocity: 0.9,
                },
                PatternNote {
                    beat: 2.0,
                    length_beats: 1.0,
                    hz: 587.33,
                    velocity: 0.9,
                },
            ],
            8.0,
        ));
        let p = Arc::new(LeadParams::default());
        p.vibrato_cents.set(0.0);
        p.glide_s.set(0.0);
        p.cutoff_hz.set(4000.0);
        p.release_s.set(0.05);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(LeadSource::new(p, clock, pat)));
        let b = g.render_offline(2.0, 1);
        assert!(b.samples.iter().all(|v| v.is_finite()));
        let peak = b.samples.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.05 && peak <= 1.0, "audible {peak}");
        let pitch = |from: f64, to: f64| {
            let mut v: Vec<f64> = contour(&b, &PitchConfig::new(48_000), 0.02)
                .into_iter()
                .filter(|p| p.t >= from && p.t < to)
                .filter_map(|p| p.hz)
                .collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v.get(v.len() / 2).copied().unwrap_or(0.0)
        };
        let a = pitch(0.1, 0.45);
        let d = pitch(1.1, 1.45);
        assert!((a - 440.0).abs() < 8.0, "first note {a}");
        assert!((d - 587.33).abs() < 10.0, "second note {d}");
        // Between the notes (0.55–0.95 s) the voice is released: quiet.
        let gap = b.samples[28_800..45_600]
            .iter()
            .fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(gap < 0.02, "gap {gap}");
    }
}
