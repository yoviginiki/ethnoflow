//! Wind (F1A.1): noise through a resonant band-pass whose centre wanders slowly,
//! with gusts (a slow amplitude walk). Everything slow is a seeded random walk, so
//! the same seed always blows the same way.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::{soft, Rng, Svf, Wander};
use crate::graph::params::{ramp_samples, Ramp, SmoothedParam};
use crate::graph::Source;

pub struct WindParams {
    /// 0..1 depth of the gusts (amplitude movement).
    pub gust: SmoothedParam,
    /// 0..1 how fast the wind changes (slow … lively).
    pub speed: SmoothedParam,
    /// Centre of the whistle, Hz.
    pub center_hz: SmoothedParam,
    /// 0..1 how narrow/whistling the band is.
    pub resonance: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for WindParams {
    fn default() -> Self {
        Self {
            gust: SmoothedParam::new(0.6, 0.0, 1.0),
            speed: SmoothedParam::new(0.3, 0.0, 1.0),
            center_hz: SmoothedParam::new(600.0, 80.0, 6000.0),
            resonance: SmoothedParam::new(0.5, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

pub struct WindSource {
    params: Arc<WindParams>,
    rng: Rng,
    seed_used: u32,
    gust: Ramp,
    speed: Ramp,
    center: Ramp,
    resonance: Ramp,
    pitch_walk: Wander,
    gust_walk: Wander,
    filter: Svf,
    body: Svf,
    amp: f32,
}

impl WindSource {
    pub fn new(params: Arc<WindParams>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        Self {
            rng: Rng::new(seed),
            seed_used: seed,
            gust: Ramp::new(params.gust.target()),
            speed: Ramp::new(params.speed.target()),
            center: Ramp::new(params.center_hz.target()),
            resonance: Ramp::new(params.resonance.target()),
            pitch_walk: Wander::new(seed.wrapping_add(11)),
            gust_walk: Wander::new(seed.wrapping_add(23)),
            filter: Svf::default(),
            body: Svf::default(),
            amp: 0.5,
            params,
        }
    }

    fn reseed(&mut self, seed: u32) {
        self.rng = Rng::new(seed);
        self.seed_used = seed;
        self.pitch_walk = Wander::new(seed.wrapping_add(11));
        self.gust_walk = Wander::new(seed.wrapping_add(23));
        self.filter.clear();
        self.body.clear();
    }
}

impl Source for WindSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        // One refcount bump per block: lets the strike/reseed paths borrow `self` mutably.
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            self.reseed(seed);
        }
        let ramp = ramp_samples(sample_rate);
        self.gust.begin(p.gust.target(), ramp);
        self.speed.begin(p.speed.target(), ramp);
        self.center.begin(p.center_hz.target(), ramp);
        self.resonance.begin(p.resonance.target(), ramp);
        // Block-rate modulation (parameters are smoothed anyway).
        let n = out.len();
        let speed = self.speed.current;
        let period = 6.0 - 5.0 * speed; // 6 s (calm) … 1 s (lively) between targets
        let wander = self.pitch_walk.step(n, period, sr); // ±1 octave
        let gust_v = self.gust_walk.step(n, period * 1.7, sr); // −1..1
        let gust = self.gust.current;
        let target_amp = 0.25 + 0.75 * (0.5 + 0.5 * gust_v).powf(1.0 + gust * 2.0);
        let target_amp = target_amp * (1.0 - gust * 0.3) + gust * 0.3 * target_amp;
        let center = self.center.current * 2f32.powf(wander * (0.5 + 0.5 * gust));
        let q = 0.8 + self.resonance.current * 12.0;
        self.filter.set(center, q, sr);
        // The band-pass has gain Q at the centre; √Q keeps the noise level even.
        let whistle_gain = 1.4 / q.sqrt();
        // A broad low body under the whistle so it is not only a thin tone.
        self.body.set((center * 0.25).max(60.0), 0.7, sr);
        let a_amp = 1.0 - (-(n as f32) / (0.2 * sr)).exp();
        for s in out.iter_mut() {
            let _ = self.gust.next(p.gust.target());
            let _ = self.speed.next(p.speed.target());
            let _ = self.center.next(p.center_hz.target());
            let _ = self.resonance.next(p.resonance.target());
            let w = self.rng.bipolar();
            let whistle = self.filter.process(w).bp;
            let body = self.body.process(w).lp;
            *s = soft((whistle * whistle_gain + body * 0.6) * self.amp) * 0.9;
        }
        self.amp += a_amp * (target_amp - self.amp);
    }

    fn reset(&mut self) {
        self.reseed(self.seed_used);
        self.amp = 0.5;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};

    fn render(set: impl Fn(&WindParams), seconds: f64) -> AudioBuffer {
        let p = Arc::new(WindParams::default());
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(WindSource::new(p)));
        g.render_offline(seconds, 1)
    }

    fn window_rms(b: &AudioBuffer) -> Vec<f32> {
        b.samples
            .chunks(48_000)
            .map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt())
            .collect()
    }

    #[test]
    fn gusts_move_the_level_and_nothing_clicks() {
        let b = render(
            |p| {
                p.gust.set(1.0);
                p.speed.set(0.8);
            },
            20.0,
        );
        assert!(b.samples.iter().all(|s| s.is_finite()));
        assert!(b.peak() <= 1.0);
        let w = window_rms(&b);
        let (min, max) = w[1..]
            .iter()
            .fold((1.0f32, 0.0f32), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        assert!(max > min * 1.3, "gusts: {min}..{max}");
        let max_step = b
            .samples
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        // Noise moves fast by nature; a click would be a full-scale step.
        assert!(max_step < 0.6, "{max_step}");
    }

    #[test]
    fn calm_wind_is_steadier_and_the_seed_fixes_it() {
        let calm = render(
            |p| {
                p.gust.set(0.0);
                p.speed.set(0.0);
            },
            12.0,
        );
        let w = window_rms(&calm);
        let (min, max) = w[2..]
            .iter()
            .fold((1.0f32, 0.0f32), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        assert!(max < min * 1.6, "calm: {min}..{max}");
        let a = render(|p| p.seed.store(4, Ordering::Relaxed), 1.0);
        let b = render(|p| p.seed.store(4, Ordering::Relaxed), 1.0);
        let c = render(|p| p.seed.store(5, Ordering::Relaxed), 1.0);
        assert_eq!(a.samples, b.samples);
        assert_ne!(a.samples, c.samples);
    }
}
