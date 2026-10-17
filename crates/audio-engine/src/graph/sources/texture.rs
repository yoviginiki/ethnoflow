//! Filtered texture (F1A.1): noise excites a bank of resonators tuned to the pitch
//! set (scale degrees when the project has a tonal centre, a harmonic series when it
//! does not). Each resonator breathes on its own slow walk; "brightness" adds the
//! octave partials, "motion" the depth of the breathing.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::{soft, PitchSet, Rng, Svf, Wander, MAX_PITCHES};
use crate::graph::params::{ramp_samples, Ramp, SmoothedParam};
use crate::graph::Source;

pub struct TextureParams {
    pub pitches: PitchSet,
    /// 0..1 level of the octave partials.
    pub brightness: SmoothedParam,
    /// 0..1 depth of the slow level movement per resonator.
    pub motion: SmoothedParam,
    /// 0..1 how ringing the resonators are (narrowness).
    pub resonance: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for TextureParams {
    fn default() -> Self {
        Self {
            pitches: PitchSet::default(),
            brightness: SmoothedParam::new(0.3, 0.0, 1.0),
            motion: SmoothedParam::new(0.5, 0.0, 1.0),
            resonance: SmoothedParam::new(0.6, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

struct Res {
    f: Svf,
    f2: Svf,
    walk: Wander,
    level: f32,
}

pub struct TextureSource {
    params: Arc<TextureParams>,
    rng: Rng,
    seed_used: u32,
    brightness: Ramp,
    motion: Ramp,
    resonance: Ramp,
    res: [Res; MAX_PITCHES],
}

impl TextureSource {
    pub fn new(params: Arc<TextureParams>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        Self {
            rng: Rng::new(seed),
            seed_used: seed,
            brightness: Ramp::new(params.brightness.target()),
            motion: Ramp::new(params.motion.target()),
            resonance: Ramp::new(params.resonance.target()),
            res: std::array::from_fn(|i| Res {
                f: Svf::default(),
                f2: Svf::default(),
                walk: Wander::new(seed.wrapping_add(101 + i as u32 * 7)),
                level: 0.5,
            }),
            params,
        }
    }

    fn reseed(&mut self, seed: u32) {
        self.rng = Rng::new(seed);
        self.seed_used = seed;
        for (i, r) in self.res.iter_mut().enumerate() {
            r.walk = Wander::new(seed.wrapping_add(101 + i as u32 * 7));
            r.f.clear();
            r.f2.clear();
        }
    }
}

impl Source for TextureSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        // One refcount bump per block: lets the strike/reseed paths borrow `self` mutably.
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            self.reseed(seed);
        }
        let ramp = ramp_samples(sample_rate);
        self.brightness.begin(p.brightness.target(), ramp);
        self.motion.begin(p.motion.target(), ramp);
        self.resonance.begin(p.resonance.target(), ramp);
        let n = out.len();
        let count = p.pitches.len();
        if count == 0 {
            out.fill(0.0);
            for _ in 0..n {
                self.brightness.next(p.brightness.target());
                self.motion.next(p.motion.target());
                self.resonance.next(p.resonance.target());
            }
            return;
        }
        let motion = self.motion.current;
        let bright = self.brightness.current;
        let q = 15.0 + self.resonance.current * 150.0;
        for (i, r) in self.res.iter_mut().enumerate().take(count) {
            let hz = p.pitches.get(i);
            r.f.set(hz, q, sr);
            r.f2.set(hz * 2.0, q, sr);
            let w = r.walk.step(n, 5.0 + i as f32 * 0.7, sr);
            r.level = 1.0 - motion * (0.5 + 0.5 * w) * 0.9;
        }
        // Resonator gain grows with Q; normalise so the texture sits around −20 dBFS.
        let norm = 0.9 / (q.sqrt() * (count as f32).sqrt());
        out.fill(0.0);
        for s in out.iter_mut() {
            let _ = self.brightness.next(p.brightness.target());
            let _ = self.motion.next(p.motion.target());
            let _ = self.resonance.next(p.resonance.target());
            let w = self.rng.bipolar();
            let mut acc = 0.0f32;
            for r in self.res.iter_mut().take(count) {
                let a = r.f.process(w).bp;
                let b = r.f2.process(w).bp;
                acc += (a + b * bright * 0.7) * r.level;
            }
            *s = soft(acc * norm);
        }
    }

    fn reset(&mut self) {
        self.reseed(self.seed_used);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};

    fn render(set: impl Fn(&TextureParams), seconds: f64) -> AudioBuffer {
        let p = Arc::new(TextureParams::default());
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(TextureSource::new(p)));
        g.render_offline(seconds, 1)
    }

    /// Energy near `hz` via a narrow band-pass, relative to the total.
    fn band_share(b: &AudioBuffer, hz: f32) -> f32 {
        let mut f = Svf::default();
        f.set(hz, 30.0, 48_000.0);
        let (mut band, mut all) = (0.0f32, 0.0f32);
        for &x in &b.samples[9600..] {
            let y = f.process(x).bp;
            band += y * y;
            all += x * x;
        }
        band / all.max(1e-9)
    }

    #[test]
    fn resonators_sit_on_the_pitch_set_and_empty_set_is_silent() {
        let b = render(|p| p.pitches.set(&[220.0, 330.0]), 3.0);
        assert!(b.samples.iter().all(|s| s.is_finite()));
        assert!(b.peak() <= 1.0);
        assert!(b.rms() > 0.01, "audible: {}", b.rms());
        let on = band_share(&b, 220.0);
        let off = band_share(&b, 270.0);
        assert!(on > off * 4.0, "220 Hz share {on} vs 270 Hz {off}");
        let silent = render(|_| {}, 0.5);
        assert_eq!(silent.peak(), 0.0);
    }

    #[test]
    fn motion_moves_the_level_and_brightness_adds_the_octave() {
        let moving = render(
            |p| {
                p.pitches.set(&[196.0]);
                p.motion.set(1.0);
            },
            24.0,
        );
        let w: Vec<f32> = moving
            .samples
            .chunks(48_000)
            .map(|c| (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt())
            .collect();
        let (min, max) = w[2..]
            .iter()
            .fold((1.0f32, 0.0f32), |(lo, hi), &x| (lo.min(x), hi.max(x)));
        assert!(max > min * 1.3, "motion: {min}..{max}");
        let dark = render(
            |p| {
                p.pitches.set(&[196.0]);
                p.brightness.set(0.0);
                p.motion.set(0.0);
            },
            3.0,
        );
        let bright = render(
            |p| {
                p.pitches.set(&[196.0]);
                p.brightness.set(1.0);
                p.motion.set(0.0);
            },
            3.0,
        );
        assert!(band_share(&bright, 392.0) > band_share(&dark, 392.0) * 1.5);
    }
}
