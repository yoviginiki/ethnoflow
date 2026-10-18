//! Bloom (F1B.6): a generative melody on the pitch set — a seeded sparse process plays
//! soft notes (sine with a little second and third harmonic by `brightness`, slow attack,
//! long release); `phrase` turns random picks into stepwise phrases with breaths (after
//! 3–7 notes the next trigger is a rest), `density` is notes per minute, `note_s` the
//! held length. Silence between notes is kept; the same seed blooms the same way.
//! No allocation in `render`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::{PitchSet, Trigger};
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;

pub const BLOOM_VOICES: usize = 6;

pub struct BloomParams {
    pub pitches: PitchSet,
    /// Notes per minute (0 = silent).
    pub density: SmoothedParam,
    /// Held length of a note, seconds.
    pub note_s: SmoothedParam,
    /// 0..1 harmonic content.
    pub brightness: SmoothedParam,
    pub attack_s: SmoothedParam,
    /// 0 random picks … 1 stepwise phrases with breaths.
    pub phrase: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for BloomParams {
    fn default() -> Self {
        Self {
            pitches: PitchSet::default(),
            density: SmoothedParam::new(16.0, 0.0, 120.0),
            note_s: SmoothedParam::new(1.6, 0.2, 8.0),
            brightness: SmoothedParam::new(0.3, 0.0, 1.0),
            attack_s: SmoothedParam::new(0.25, 0.005, 2.0),
            phrase: SmoothedParam::new(0.7, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Voice {
    hz: f32,
    phase: f32,
    env: f32,
    gate: u32,
    amp: f32,
    active: bool,
}

pub struct BloomSource {
    params: Arc<BloomParams>,
    trigger: Trigger,
    seed_used: u32,
    density: Ramp,
    brightness: Ramp,
    voices: [Voice; BLOOM_VOICES],
    next_voice: usize,
    /// Position in the pitch set for stepwise motion, and notes left in the phrase.
    index: usize,
    phrase_left: u32,
}

impl BloomSource {
    pub fn new(params: Arc<BloomParams>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        Self {
            trigger: Trigger::new(seed),
            seed_used: seed,
            density: Ramp::new(params.density.target()),
            brightness: Ramp::new(params.brightness.target()),
            voices: [Voice::default(); BLOOM_VOICES],
            next_voice: 0,
            index: 0,
            phrase_left: 0,
            params,
        }
    }

    fn note_on(&mut self, sr: f32) {
        let p = Arc::clone(&self.params);
        let n = p.pitches.len();
        if n == 0 {
            return;
        }
        let phrase = p.phrase.target();
        let rng = self.trigger.rng();
        if self.phrase_left == 0 {
            // A breath: the first trigger after a phrase is a rest (scaled by `phrase`).
            self.phrase_left = 3 + rng.below(5) as u32;
            if rng.unit() < phrase * 0.8 {
                return;
            }
        }
        self.phrase_left = self.phrase_left.saturating_sub(1);
        let stepwise = rng.unit() < phrase;
        self.index = if stepwise {
            let step = if rng.unit() < 0.25 { 2 } else { 1 };
            let dir = if rng.unit() < 0.5 { -1 } else { 1 };
            let mid = n as i32 / 2;
            let cur = self.index as i32;
            // Pull back towards the middle at the edges.
            let d = if (cur - mid).abs() > n as i32 / 3 {
                -(cur - mid).signum()
            } else {
                dir
            };
            (cur + d * step).clamp(0, n as i32 - 1) as usize
        } else {
            rng.below(n)
        };
        let hz = p.pitches.get(self.index.min(n - 1));
        let vel = 0.55 + 0.45 * rng.unit();
        let gate = (p.note_s.target() * sr) as u32;
        let slot = self
            .voices
            .iter()
            .position(|v| !v.active)
            .unwrap_or(self.next_voice % BLOOM_VOICES);
        self.next_voice = slot + 1;
        self.voices[slot] = Voice {
            hz,
            phase: 0.0,
            env: 0.0,
            gate,
            amp: 0.28 * vel,
            active: true,
        };
    }
}

impl Source for BloomSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            self.trigger.reset(seed);
            self.seed_used = seed;
            self.phrase_left = 0;
        }
        let ramp = ramp_samples(sample_rate);
        self.density.begin(p.density.target(), ramp);
        self.brightness.begin(p.brightness.target(), ramp);
        let n = out.len();
        let density = if p.pitches.is_empty() {
            0.0
        } else {
            self.density.current
        };
        let at = self.trigger.tick(n, density, sr);
        let att = 1.0 / (p.attack_s.target().max(0.001) * sr);
        let rel = (-6.9078 / (p.note_s.target().max(0.2) * 0.9 * sr)).exp();
        let bright = self.brightness.current;
        for (i, s) in out.iter_mut().enumerate() {
            if at == Some(i) {
                self.note_on(sr);
            }
            let mut acc = 0.0f32;
            for v in self.voices.iter_mut() {
                if !v.active {
                    continue;
                }
                if v.gate > 0 {
                    v.gate -= 1;
                    v.env = (v.env + att).min(1.0);
                } else {
                    v.env *= rel;
                    if v.env < 1e-4 {
                        v.active = false;
                        continue;
                    }
                }
                let t = v.phase * std::f32::consts::TAU;
                let y = t.sin()
                    + bright * 0.5 * (2.0 * t).sin()
                    + bright * bright * 0.25 * (3.0 * t).sin();
                v.phase += v.hz / sr;
                if v.phase >= 1.0 {
                    v.phase -= 1.0;
                }
                acc += y * v.env * v.amp;
            }
            *s = undenormal(super::gen::soft(acc));
        }
    }

    fn reset(&mut self) {
        self.voices = [Voice::default(); BLOOM_VOICES];
        self.phrase_left = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    fn render(set: impl Fn(&BloomParams), seconds: f64) -> crate::AudioBuffer {
        let p = Arc::new(BloomParams::default());
        p.pitches
            .set(&[293.66, 329.63, 349.23, 392.0, 440.0, 493.88, 523.25]); // D4 dorian
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(BloomSource::new(p)));
        g.render_offline(seconds, 1)
    }

    fn onsets(b: &crate::AudioBuffer) -> usize {
        let mut count = 0;
        let mut above = false;
        for c in b.samples.chunks(2400) {
            let peak = c.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            let now = peak > 0.02;
            if now && !above {
                count += 1;
            }
            above = now;
        }
        count
    }

    #[test]
    fn bloom_plays_notes_from_the_set_density_counts_and_seeds_repeat() {
        let a = render(
            |p| {
                p.density.set(60.0);
                p.note_s.set(0.3);
                p.phrase.set(0.0);
                p.attack_s.set(0.01);
            },
            20.0,
        );
        assert!(a.samples.iter().all(|v| v.is_finite()));
        let dense = onsets(&a);
        let sparse = onsets(&render(
            |p| {
                p.density.set(6.0);
                p.note_s.set(0.3);
                p.phrase.set(0.0);
                p.attack_s.set(0.01);
            },
            20.0,
        ));
        assert!(dense > sparse * 2, "dense {dense} vs sparse {sparse}");
        assert!(dense >= 10, "about one note per second: {dense}");
        // Pitches land on the set (within 10 cents) wherever the tracker is confident.
        let set = [293.66f64, 329.63, 349.23, 392.0, 440.0, 493.88, 523.25];
        let points = contour(&a, &PitchConfig::new(48_000), 0.02);
        let mut voiced = 0;
        let mut on = 0;
        for pt in points.iter().filter(|p| p.confidence > 0.9) {
            if let Some(hz) = pt.hz {
                voiced += 1;
                if set.iter().any(|s| (1200.0 * (hz / s).log2()).abs() < 12.0) {
                    on += 1;
                }
            }
        }
        assert!(voiced > 50, "voiced frames {voiced}");
        assert!(
            on as f64 > voiced as f64 * 0.8,
            "{on}/{voiced} frames on the set"
        );
        // Same seed → same samples; another seed → different.
        let b = render(
            |p| {
                p.density.set(60.0);
                p.note_s.set(0.3);
                p.phrase.set(0.0);
                p.attack_s.set(0.01);
            },
            20.0,
        );
        assert_eq!(a.samples, b.samples);
        let c = render(
            |p| {
                p.density.set(60.0);
                p.note_s.set(0.3);
                p.phrase.set(0.0);
                p.attack_s.set(0.01);
                p.seed.store(9, Ordering::Relaxed);
            },
            20.0,
        );
        assert_ne!(a.samples, c.samples);
        // Empty set: silence.
        let p = Arc::new(BloomParams::default());
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(BloomSource::new(p)));
        assert!(g
            .render_offline(2.0, 1)
            .samples
            .iter()
            .all(|v| v.abs() < 1e-6));
    }

    #[test]
    fn phrases_move_stepwise_more_often_than_random_picks() {
        // Mean interval between consecutive notes (in set steps) is smaller with phrase = 1.
        let steps = |phrase: f32| {
            let b = render(
                |p| {
                    p.density.set(60.0);
                    p.note_s.set(0.25);
                    p.phrase.set(phrase);
                    p.attack_s.set(0.005);
                },
                30.0,
            );
            let set = [293.66f64, 329.63, 349.23, 392.0, 440.0, 493.88, 523.25];
            let mut last: Option<usize> = None;
            let mut total = 0usize;
            let mut count = 0usize;
            for pt in contour(&b, &PitchConfig::new(48_000), 0.02)
                .iter()
                .filter(|p| p.confidence > 0.9)
            {
                let Some(hz) = pt.hz else { continue };
                let idx = set
                    .iter()
                    .position(|s| (1200.0 * (hz / s).log2()).abs() < 12.0);
                if let Some(i) = idx {
                    if let Some(l) = last {
                        if l != i {
                            total += l.abs_diff(i);
                            count += 1;
                        }
                    }
                    last = Some(i);
                }
            }
            total as f64 / count.max(1) as f64
        };
        let stepwise = steps(1.0);
        let random = steps(0.0);
        assert!(stepwise < random, "stepwise {stepwise} vs random {random}");
    }
}
