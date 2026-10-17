//! Bells (F1A.1/F1A.7): sparse FM/modal strikes on notes from the pitch set. The
//! trigger is a seeded sparse process (density in strikes per minute, silence is
//! kept), so a seed plays the same bells again; a different seed never repeats them.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::{PitchSet, Trigger};
use crate::graph::params::{ramp_samples, Ramp, SmoothedParam};
use crate::graph::Source;

pub const BELL_VOICES: usize = 8;

pub struct BellParams {
    pub pitches: PitchSet,
    /// Strikes per minute (0 = silent). Default keeps the spec's ≤ 1 note / 4 s.
    pub density: SmoothedParam,
    /// Decay to −60 dB, seconds.
    pub decay_s: SmoothedParam,
    /// 0..1: FM index and inharmonic partial (0 = almost a sine).
    pub brightness: SmoothedParam,
    /// 0..1 random variation of strike loudness.
    pub velocity_spread: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for BellParams {
    fn default() -> Self {
        Self {
            pitches: PitchSet::default(),
            density: SmoothedParam::new(12.0, 0.0, 120.0),
            decay_s: SmoothedParam::new(4.0, 0.2, 30.0),
            brightness: SmoothedParam::new(0.4, 0.0, 1.0),
            velocity_spread: SmoothedParam::new(0.5, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Voice {
    hz: f32,
    car: f32,
    modp: f32,
    part: f32,
    env: f32,
    env2: f32,
    amp: f32,
    index: f32,
    active: bool,
}

pub struct BellSource {
    params: Arc<BellParams>,
    trigger: Trigger,
    seed_used: u32,
    density: Ramp,
    decay: Ramp,
    brightness: Ramp,
    spread: Ramp,
    voices: [Voice; BELL_VOICES],
    next_voice: usize,
}

impl BellSource {
    pub fn new(params: Arc<BellParams>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        Self {
            trigger: Trigger::new(seed),
            seed_used: seed,
            density: Ramp::new(params.density.target()),
            decay: Ramp::new(params.decay_s.target()),
            brightness: Ramp::new(params.brightness.target()),
            spread: Ramp::new(params.velocity_spread.target()),
            voices: [Voice::default(); BELL_VOICES],
            next_voice: 0,
            params,
        }
    }

    fn strike(&mut self, hz: f32, brightness: f32, spread: f32) {
        // Prefer a free voice, else the oldest (round robin).
        let i = (0..BELL_VOICES)
            .map(|k| (self.next_voice + k) % BELL_VOICES)
            .find(|&k| !self.voices[k].active)
            .unwrap_or(self.next_voice);
        self.next_voice = (i + 1) % BELL_VOICES;
        let rng = self.trigger.rng();
        let vel = 1.0 - spread * rng.unit() * 0.7;
        self.voices[i] = Voice {
            hz,
            car: 0.0,
            modp: 0.0,
            part: 0.0,
            env: 1.0,
            env2: 1.0,
            amp: 0.35 * vel,
            index: 0.3 + brightness * 3.5,
            active: true,
        };
    }
}

impl BellSource {
    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn render_span(
        &mut self,
        out: &mut [f32],
        p: &BellParams,
        sr: f32,
        k1: f32,
        k2: f32,
        bright: f32,
        mod_ratio: f32,
    ) {
        for s in out.iter_mut() {
            let _ = self.density.next(p.density.target());
            let _ = self.decay.next(p.decay_s.target());
            let _ = self.brightness.next(p.brightness.target());
            let _ = self.spread.next(p.velocity_spread.target());
            let mut acc = 0.0f32;
            for v in self.voices.iter_mut() {
                if !v.active {
                    continue;
                }
                let m = (v.modp * std::f32::consts::TAU).sin() * v.index * v.env * v.env;
                let tone = (v.car * std::f32::consts::TAU + m).sin();
                let partial = (v.part * std::f32::consts::TAU).sin() * 0.3 * bright * v.env2;
                acc += (tone * v.env + partial) * v.amp;
                v.car += v.hz / sr;
                v.modp += v.hz * mod_ratio / sr;
                v.part += v.hz * 2.756 / sr;
                if v.car >= 1.0 {
                    v.car -= 1.0;
                }
                if v.modp >= 1.0 {
                    v.modp -= 1.0;
                }
                if v.part >= 1.0 {
                    v.part -= 1.0;
                }
                v.env *= k1;
                v.env2 *= k2;
                if v.env < 1e-4 {
                    v.active = false;
                }
            }
            *s = acc;
        }
    }
}

impl Source for BellSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        // One refcount bump per block: lets the strike/reseed paths borrow `self` mutably.
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            self.trigger.reset(seed);
            self.seed_used = seed;
        }
        let ramp = ramp_samples(sample_rate);
        self.density.begin(p.density.target(), ramp);
        self.decay.begin(p.decay_s.target(), ramp);
        self.brightness.begin(p.brightness.target(), ramp);
        self.spread.begin(p.velocity_spread.target(), ramp);
        let n = out.len();
        let density = if p.pitches.is_empty() {
            0.0
        } else {
            self.density.current
        };
        let strike_at = self.trigger.tick(n, density, sr);
        // Decay coefficients per block: env reaches 1e-3 after decay_s.
        let decay = self.decay.current.max(0.05);
        let k1 = (-6.9078 / (decay * sr)).exp();
        let k2 = (-6.9078 / (decay * 0.35 * sr)).exp(); // the bright partial dies faster
        let bright = self.brightness.current;
        let mod_ratio = 1.4 + bright * 2.1; // 1.4 (soft) … 3.5 (metallic)
                                            // The strike lands on its exact sample, so the output does not depend on how
                                            // the host chunks the audio (offline bounce == live).
        let at = strike_at.unwrap_or(n).min(n);
        let (head, tail) = out.split_at_mut(at);
        self.render_span(head, &p, sr, k1, k2, bright, mod_ratio);
        if strike_at.is_some() {
            let hz = p.pitches.pick(self.trigger.rng()).unwrap_or(0.0);
            if hz > 0.0 {
                let (b, s) = (self.brightness.current, self.spread.current);
                self.strike(hz, b, s);
            }
        }
        self.render_span(tail, &p, sr, k1, k2, bright, mod_ratio);
    }

    fn reset(&mut self) {
        self.trigger.reset(self.seed_used);
        self.voices = [Voice::default(); BELL_VOICES];
        self.next_voice = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    fn render(set: impl Fn(&BellParams), seconds: f64) -> AudioBuffer {
        let p = Arc::new(BellParams::default());
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(BellSource::new(p)));
        g.render_offline(seconds, 1)
    }

    fn strikes(b: &AudioBuffer) -> usize {
        // Count rising edges of the 20 ms envelope above −30 dBFS.
        let mut count = 0;
        let mut above = false;
        for c in b.samples.chunks(960) {
            let peak = c.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            let now = peak > 0.03;
            if now && !above {
                count += 1;
            }
            above = now;
        }
        count
    }

    #[test]
    fn density_sets_how_often_bells_ring_and_silence_is_kept() {
        let b = render(
            |p| {
                p.pitches.set(&[440.0, 554.4, 659.3]);
                p.density.set(30.0);
                p.decay_s.set(0.5);
            },
            60.0,
        );
        assert!(b.samples.iter().all(|s| s.is_finite()));
        assert!(b.peak() <= 1.0);
        let n = strikes(&b);
        assert!((15..=50).contains(&n), "{n} strikes in a minute at 30/min");
        let quiet = render(
            |p| {
                p.pitches.set(&[440.0]);
                p.density.set(0.0);
            },
            5.0,
        );
        assert_eq!(quiet.peak(), 0.0);
        let no_pitches = render(|p| p.density.set(60.0), 5.0);
        assert_eq!(no_pitches.peak(), 0.0);
    }

    #[test]
    fn a_soft_bell_is_in_tune() {
        let b = render(
            |p| {
                p.pitches.set(&[440.0]);
                p.density.set(1.0); // one strike early, then nothing for a while
                p.brightness.set(0.0);
                p.velocity_spread.set(0.0);
                p.decay_s.set(6.0);
            },
            8.0,
        );
        let first = b
            .samples
            .iter()
            .position(|s| s.abs() > 0.01)
            .expect("a strike") as f64
            / 48_000.0;
        let mut hz: Vec<f64> = contour(&b, &PitchConfig::new(48_000), 0.02)
            .into_iter()
            .filter(|p| p.t > first + 0.1 && p.t < first + 1.5)
            .filter_map(|p| p.hz)
            .collect();
        hz.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = hz[hz.len() / 2];
        let cents = 1200.0 * (med / 440.0).log2();
        assert!(cents.abs() < 5.0, "{med:.2} Hz ({cents:.1} c)");
    }

    #[test]
    fn seed_reproduces_and_changes_the_sequence() {
        let a = render(
            |p| {
                p.pitches.set(&[220.0, 330.0]);
                p.density.set(40.0);
            },
            10.0,
        );
        let b = render(
            |p| {
                p.pitches.set(&[220.0, 330.0]);
                p.density.set(40.0);
            },
            10.0,
        );
        let c = render(
            |p| {
                p.pitches.set(&[220.0, 330.0]);
                p.density.set(40.0);
                p.seed.store(99, Ordering::Relaxed);
            },
            10.0,
        );
        assert_eq!(a.samples, b.samples);
        assert_ne!(a.samples, c.samples);
    }
}
