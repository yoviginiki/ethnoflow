//! Pulse (Soundscapes S3): a non-percussive pulse — a soft swell every `period_s`
//! seconds, a filtered tone on the pitch set mixed with a noise band at the same pitch
//! (`color` 0 tone … 1 noise). It is not always there: `presence` is the share of a
//! 2–3 minute cycle the pulse is present, with long fades in and out, so it comes and
//! goes like a heartbeat under the scene. Everything random is seeded.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::{PitchSet, Rng, Svf};
use crate::graph::params::{ramp_samples, Ramp, SmoothedParam};
use crate::graph::Source;

pub struct PulseParams {
    pub pitches: PitchSet,
    /// Seconds between pulses (0.25..8).
    pub period_s: SmoothedParam,
    /// 0..1 share of the time the pulse is present (1 = always, 0 = never).
    pub presence: SmoothedParam,
    /// 0 a soft tone … 1 a breathy noise band.
    pub color: SmoothedParam,
    /// 0..1 how much of the period the swell takes (0 short, 1 almost continuous).
    pub softness: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for PulseParams {
    fn default() -> Self {
        Self {
            pitches: PitchSet::default(),
            period_s: SmoothedParam::new(1.5, 0.25, 8.0),
            presence: SmoothedParam::new(0.6, 0.0, 1.0),
            color: SmoothedParam::new(0.3, 0.0, 1.0),
            softness: SmoothedParam::new(0.6, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

/// Seconds of one presence cycle (on + off), before jitter.
const CYCLE_S: f32 = 150.0;
/// Fade in / out of the presence gate, seconds.
const GATE_FADE_S: f32 = 20.0;

pub struct PulseSource {
    params: Arc<PulseParams>,
    rng: Rng,
    seed_used: u32,
    period: Ramp,
    presence: Ramp,
    color: Ramp,
    softness: Ramp,
    /// Position inside the current pulse, seconds.
    beat_t: f32,
    /// Position inside the current presence cycle, seconds, and that cycle's length.
    cycle_t: f32,
    cycle_len: f32,
    /// Smoothed presence gate 0..1.
    gate: f32,
    hz: f32,
    phase: f32,
    band: Svf,
    tone_lp: Svf,
}

impl PulseSource {
    pub fn new(params: Arc<PulseParams>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        let mut rng = Rng::new(seed);
        let cycle_len = CYCLE_S * (0.8 + 0.4 * rng.unit());
        // Start somewhere in the cycle so two pulses with different seeds do not breathe
        // together; the gate starts at its target so the first seconds are not a fade.
        let cycle_t = rng.unit() * cycle_len;
        Self {
            rng,
            seed_used: seed,
            period: Ramp::new(params.period_s.target()),
            presence: Ramp::new(params.presence.target()),
            color: Ramp::new(params.color.target()),
            softness: Ramp::new(params.softness.target()),
            beat_t: 0.0,
            cycle_t,
            cycle_len,
            gate: -1.0,
            hz: 0.0,
            phase: 0.0,
            band: Svf::default(),
            tone_lp: Svf::default(),
            params,
        }
    }

    fn reseed(&mut self, seed: u32) {
        self.rng = Rng::new(seed);
        self.seed_used = seed;
        self.cycle_len = CYCLE_S * (0.8 + 0.4 * self.rng.unit());
        self.cycle_t = self.rng.unit() * self.cycle_len;
        self.beat_t = 0.0;
        self.gate = -1.0;
        self.band.clear();
        self.tone_lp.clear();
    }

    /// Target of the presence gate at the current point of the cycle (0 or 1).
    fn gate_target(&self, presence: f32) -> f32 {
        if presence >= 0.999 {
            return 1.0;
        }
        if presence <= 0.001 {
            return 0.0;
        }
        if self.cycle_t < presence * self.cycle_len {
            1.0
        } else {
            0.0
        }
    }

    fn pick_pitch(&mut self, p: &PulseParams) -> f32 {
        // The lower pitches of the set: a pulse sits under the scene.
        let n = p.pitches.len();
        if n == 0 {
            return 0.0;
        }
        let i = self.rng.below(n.min(3));
        p.pitches.get(i)
    }
}

impl Source for PulseSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            self.reseed(seed);
        }
        let ramp = ramp_samples(sample_rate);
        self.period.begin(p.period_s.target(), ramp);
        self.presence.begin(p.presence.target(), ramp);
        self.color.begin(p.color.target(), ramp);
        self.softness.begin(p.softness.target(), ramp);
        if self.hz <= 0.0 {
            self.hz = self.pick_pitch(&p);
        }
        let dt = 1.0 / sr;
        let gate_k = 1.0 - (-dt / (GATE_FADE_S / 3.0)).exp();
        let target = self.gate_target(self.presence.current);
        if self.gate < 0.0 {
            self.gate = target;
        }
        let hz = self.hz.max(20.0);
        self.band.set(hz, 6.0, sr);
        self.tone_lp.set(hz * 3.0, 0.7, sr);
        for s in out.iter_mut() {
            let period = self.period.next(p.period_s.target()).max(0.05);
            let _ = self.presence.next(p.presence.target());
            let color = self.color.next(p.color.target());
            let soft = self.softness.next(p.softness.target());
            self.gate += gate_k * (target - self.gate);
            // Swell: a raised-cosine bump over `width` of the period, silence after.
            let width = (0.2 + 0.75 * soft).min(0.95) * period;
            let env = if self.beat_t < width {
                let x = self.beat_t / width;
                0.5 - 0.5 * (x * std::f32::consts::TAU).cos()
            } else {
                0.0
            };
            let tone = if hz > 20.0 {
                self.phase = (self.phase + hz * dt).fract();
                let x = (self.phase * std::f32::consts::TAU).sin();
                self.tone_lp
                    .process(x + 0.25 * (2.0 * self.phase * std::f32::consts::TAU).sin())
                    .lp
            } else {
                0.0
            };
            let noise = self.band.process(self.rng.bipolar()).bp * 1.6;
            let v = tone * (1.0 - color) + noise * color;
            *s = (v * env * self.gate * 0.6).clamp(-1.0, 1.0);
            self.beat_t += dt;
            if self.beat_t >= period {
                self.beat_t -= period;
                // A new pulse may land on another low pitch of the set (rarely).
                if self.rng.unit() < 0.15 {
                    self.hz = self.pick_pitch(&p);
                }
            }
            self.cycle_t += dt;
            if self.cycle_t >= self.cycle_len {
                self.cycle_t -= self.cycle_len;
                self.cycle_len = CYCLE_S * (0.8 + 0.4 * self.rng.unit());
            }
        }
    }

    fn reset(&mut self) {
        self.reseed(self.seed_used);
        self.hz = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};

    fn render(set: impl Fn(&PulseParams), seconds: f64) -> AudioBuffer {
        let p = Arc::new(PulseParams::default());
        p.pitches.set(&[110.0, 146.83, 164.81]);
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 8_000);
        g.set_source(0, Box::new(PulseSource::new(p)));
        g.render_offline(seconds, 1)
    }

    fn window_rms(b: &AudioBuffer, secs: usize) -> Vec<f32> {
        b.samples
            .chunks(8_000 * secs)
            .map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt())
            .collect()
    }

    #[test]
    fn the_pulse_comes_and_goes_with_presence() {
        // 10 minutes at 40 % presence: some 10 s windows are silent, some are loud.
        let b = render(|p| p.presence.set(0.4), 600.0);
        assert!(b.samples.iter().all(|s| s.is_finite()));
        assert!(b.peak() <= 1.0);
        let w = window_rms(&b, 10);
        let loud = w.iter().cloned().fold(0.0f32, f32::max);
        let silent = w.iter().filter(|x| **x < loud * 0.02).count();
        let present = w.iter().filter(|x| **x > loud * 0.3).count();
        assert!(silent >= 5, "silent windows {silent} of {}", w.len());
        assert!(present >= 5, "present windows {present} of {}", w.len());
    }

    #[test]
    fn full_presence_pulses_every_period_and_nothing_clicks() {
        let b = render(
            |p| {
                p.presence.set(1.0);
                p.period_s.set(1.0);
                p.softness.set(0.3);
            },
            12.0,
        );
        // Per-second RMS never drops to silence; inside a second the swell rises and falls.
        let w = window_rms(&b, 1);
        assert!(w[1..].iter().all(|x| *x > 0.01), "{w:?}");
        let step = b
            .samples
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(step < 0.3, "click {step}");
        // Seeded: same seed, same sound.
        let a = render(|p| p.seed.store(3, Ordering::Relaxed), 2.0);
        let c = render(|p| p.seed.store(3, Ordering::Relaxed), 2.0);
        assert_eq!(a.samples, c.samples);
    }
}
