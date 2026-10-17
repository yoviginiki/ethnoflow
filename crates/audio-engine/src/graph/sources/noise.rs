//! Noise colours (F1A.1): one slider from white through pink to brown. Pink is
//! Kellet's economy filter, brown a leaky integrator; levels are matched so the
//! colour slider changes the timbre, not the loudness.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::Rng;
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;

pub struct NoiseParams {
    /// 0 = white, 0.5 = pink, 1 = brown (blended in between).
    pub color: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for NoiseParams {
    fn default() -> Self {
        Self {
            color: SmoothedParam::new(0.5, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

pub struct NoiseSource {
    params: Arc<NoiseParams>,
    rng: Rng,
    seed_used: u32,
    color: Ramp,
    b: [f32; 3],
    brown: f32,
}

impl NoiseSource {
    pub fn new(params: Arc<NoiseParams>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        Self {
            rng: Rng::new(seed),
            seed_used: seed,
            color: Ramp::new(params.color.target()),
            b: [0.0; 3],
            brown: 0.0,
            params,
        }
    }
}

impl Source for NoiseSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let p = &self.params;
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            self.rng = Rng::new(seed);
            self.seed_used = seed;
        }
        self.color
            .begin(p.color.target(), ramp_samples(sample_rate));
        for s in out.iter_mut() {
            let w = self.rng.bipolar();
            // Pink (Kellet, 3 poles) — roughly −3 dB/oct from 20 Hz up.
            self.b[0] = undenormal(0.99765 * self.b[0] + w * 0.099_046);
            self.b[1] = undenormal(0.96300 * self.b[1] + w * 0.296_516_4);
            self.b[2] = undenormal(0.57000 * self.b[2] + w * 1.052_691_3);
            let pink = (self.b[0] + self.b[1] + self.b[2] + w * 0.1848) * 0.2;
            // Brown: leaky integrator, −6 dB/oct.
            self.brown = undenormal((self.brown + 0.02 * w) / 1.02);
            let brown = self.brown * 3.0;
            let white = w * 0.45;
            let c = self.color.next(p.color.target());
            *s = if c < 0.5 {
                let m = c * 2.0;
                white * (1.0 - m) + pink * m
            } else {
                let m = (c - 0.5) * 2.0;
                pink * (1.0 - m) + brown * m
            };
        }
    }

    fn reset(&mut self) {
        self.rng = Rng::new(self.seed_used);
        self.b = [0.0; 3];
        self.brown = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};

    fn render(color: f32, seed: u32) -> AudioBuffer {
        let p = Arc::new(NoiseParams::default());
        p.color.set(color);
        p.seed.store(seed, Ordering::Relaxed);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(NoiseSource::new(p)));
        g.render_offline(2.0, 1)
    }

    /// Share of energy below ~200 Hz (one-pole low-pass), a cheap spectral tilt.
    fn low_share(b: &AudioBuffer) -> f32 {
        let a = 1.0 - (-2.0 * std::f32::consts::PI * 200.0 / 48_000.0).exp();
        let mut lp = 0.0f32;
        let (mut low, mut all) = (0.0f32, 0.0f32);
        for &x in &b.samples[4800..] {
            lp += a * (x - lp);
            low += lp * lp;
            all += x * x;
        }
        low / all.max(1e-9)
    }

    #[test]
    fn colours_tilt_the_spectrum_and_keep_the_level() {
        let white = render(0.0, 1);
        let pink = render(0.5, 1);
        let brown = render(1.0, 1);
        for b in [&white, &pink, &brown] {
            assert!(b.samples.iter().all(|s| s.is_finite()));
            let r = b.rms();
            assert!((0.08..0.5).contains(&r), "rms {r}");
            assert!(b.peak() <= 1.0);
        }
        let (w, p, br) = (low_share(&white), low_share(&pink), low_share(&brown));
        assert!(
            w < p && p < br,
            "low-frequency share white {w} pink {p} brown {br}"
        );
        // Levels within 6 dB of each other across the slider.
        let ratio = white.rms().max(brown.rms()) / white.rms().min(brown.rms());
        assert!(ratio < 2.0, "level ratio {ratio}");
    }

    #[test]
    fn same_seed_same_noise_different_seed_different() {
        let a = render(0.3, 9);
        let b = render(0.3, 9);
        let c = render(0.3, 10);
        assert_eq!(a.samples, b.samples);
        assert_ne!(a.samples, c.samples);
    }
}
