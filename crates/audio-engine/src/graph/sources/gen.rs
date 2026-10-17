//! Shared building blocks for the generative ambient sources (F1A.1/F1A.7): a
//! seeded RNG (the same seed always gives the same sound), a sparse trigger with
//! exponentially distributed gaps, a pitch set the UI fills from the project's
//! scale, and a state-variable filter. Nothing here allocates after construction.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

/// xorshift32: tiny, deterministic, good enough for audio noise and musical dice.
#[derive(Debug, Clone, Copy)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        // Mix the seed so neighbouring seeds do not start with correlated streams.
        let mut x = seed ^ 0x9E37_79B9;
        if x == 0 {
            x = 0x2545_F491;
        }
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        Self(x)
    }

    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    #[inline]
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 * (1.0 / 16_777_216.0)
    }

    /// Uniform in [−1, 1).
    #[inline]
    pub fn bipolar(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }

    /// Index in `0..n` (n ≥ 1).
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        ((self.unit() * n as f32) as usize).min(n.saturating_sub(1))
    }
}

pub const MAX_PITCHES: usize = 8;

/// Up to eight frequencies in Hz, set by the UI (scale degrees in a register, or a
/// "no key" set), read by the generative sources. Lock-free.
pub struct PitchSet {
    hz: [AtomicU32; MAX_PITCHES],
    len: AtomicUsize,
}

impl Default for PitchSet {
    fn default() -> Self {
        Self {
            hz: std::array::from_fn(|_| AtomicU32::new(0)),
            len: AtomicUsize::new(0),
        }
    }
}

impl PitchSet {
    /// Replace the set (at most `MAX_PITCHES`, non-positive values are skipped).
    pub fn set(&self, hz: &[f32]) {
        let mut n = 0;
        for &f in hz.iter() {
            if n >= MAX_PITCHES {
                break;
            }
            if f.is_finite() && f > 0.0 {
                self.hz[n].store(f.to_bits(), Ordering::Relaxed);
                n += 1;
            }
        }
        self.len.store(n, Ordering::Relaxed);
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Relaxed).min(MAX_PITCHES)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn get(&self, i: usize) -> f32 {
        f32::from_bits(self.hz[i.min(MAX_PITCHES - 1)].load(Ordering::Relaxed))
    }

    /// A random member, or `None` when the set is empty.
    #[inline]
    pub fn pick(&self, rng: &mut Rng) -> Option<f32> {
        let n = self.len();
        if n == 0 {
            None
        } else {
            Some(self.get(rng.below(n)))
        }
    }
}

/// Sparse event generator: gaps are exponentially distributed around
/// `60 / density_per_minute` seconds with a floor, so a layer "breathes" instead of
/// ticking, and the same seed gives the same sequence of frames.
pub struct Trigger {
    rng: Rng,
    /// Frames until the next event; negative = not drawn yet.
    remaining: i64,
    /// Density the pending gap was drawn for (a change rescales the gap).
    drawn_density: f32,
}

impl Trigger {
    pub const MIN_GAP_SECONDS: f32 = 0.25;

    pub fn new(seed: u32) -> Self {
        Self {
            rng: Rng::new(seed.wrapping_mul(2_654_435_761).wrapping_add(7)),
            remaining: -1,
            drawn_density: 0.0,
        }
    }

    pub fn reset(&mut self, seed: u32) {
        *self = Self::new(seed);
    }

    fn draw(&mut self, density_per_minute: f32, sample_rate: f32) -> i64 {
        let mean = 60.0 / density_per_minute.max(0.01);
        let u = self.rng.unit().min(0.999_999);
        let gap = (-(1.0 - u).ln() * mean).max(Self::MIN_GAP_SECONDS);
        (gap * sample_rate) as i64
    }

    /// Advance `frames`; returns the offset of an event inside this block, if one
    /// falls in it (at most one per call, which is fine for blocks of 64 frames).
    #[inline]
    pub fn tick(
        &mut self,
        frames: usize,
        density_per_minute: f32,
        sample_rate: f32,
    ) -> Option<usize> {
        if density_per_minute <= 0.0 {
            // Silent: forget the pending gap so density > 0 restarts promptly.
            self.remaining = -1;
            return None;
        }
        if self.remaining < 0 {
            // First event after (re)start: within the first mean gap, not at 0.
            self.remaining = self.draw(density_per_minute, sample_rate) / 2;
            self.drawn_density = density_per_minute;
        } else if density_per_minute != self.drawn_density {
            // The slider moved: stretch or shrink the pending gap accordingly, so a
            // jump from sparse to dense is heard now, not after the old long gap.
            let scale = self.drawn_density / density_per_minute;
            self.remaining = (self.remaining as f32 * scale) as i64;
            self.drawn_density = density_per_minute;
        }
        let n = frames as i64;
        if self.remaining >= n {
            self.remaining -= n;
            return None;
        }
        let at = self.remaining.max(0) as usize;
        let next = self.draw(density_per_minute, sample_rate);
        self.drawn_density = density_per_minute;
        self.remaining = next - (n - at as i64);
        Some(at.min(frames.saturating_sub(1)))
    }

    pub fn rng(&mut self) -> &mut Rng {
        &mut self.rng
    }
}

/// Slow bounded random walk: a new target every `period` seconds, slewed towards.
/// Output in [−1, 1]. Deterministic for a seed.
pub struct Wander {
    rng: Rng,
    value: f32,
    target: f32,
    countdown: u32,
}

impl Wander {
    pub fn new(seed: u32) -> Self {
        let mut rng = Rng::new(seed ^ 0x5bd1_e995);
        let v = rng.bipolar() * 0.5;
        Self {
            rng,
            value: v,
            target: v,
            countdown: 0,
        }
    }

    /// One step per block of `frames`; `period_s` between new targets.
    #[inline]
    pub fn step(&mut self, frames: usize, period_s: f32, sample_rate: f32) -> f32 {
        if self.countdown == 0 {
            // Next target: a bounded step from the current one, not a jump anywhere.
            let step = self.rng.bipolar() * 0.6;
            self.target = (self.target + step).clamp(-1.0, 1.0);
            let jitter = 0.5 + self.rng.unit();
            self.countdown = ((period_s.max(0.05) * jitter * sample_rate) as u32).max(1);
        }
        let n = frames as u32;
        self.countdown = self.countdown.saturating_sub(n);
        // Slew: reach ~63% of the way in a third of the period.
        let tau = (period_s.max(0.05) / 3.0) * sample_rate;
        let a = 1.0 - (-(frames as f32) / tau.max(1.0)).exp();
        self.value += a * (self.target - self.value);
        self.value
    }
}

/// Trapezoidal state-variable filter (Cytomic/Zavalishin): stable at any cutoff
/// sweep, gives low-, band- and high-pass at once.
#[derive(Debug, Clone, Copy, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
    g: f32,
    k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct SvfOut {
    pub lp: f32,
    pub bp: f32,
    pub hp: f32,
}

impl Svf {
    /// Set coefficients (per block). `q` ≥ 0.5.
    #[inline]
    pub fn set(&mut self, cutoff_hz: f32, q: f32, sample_rate: f32) {
        let fc = cutoff_hz.clamp(10.0, sample_rate * 0.45);
        self.g = (std::f32::consts::PI * fc / sample_rate).tan();
        self.k = 1.0 / q.max(0.5);
        self.a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        self.a2 = self.g * self.a1;
        self.a3 = self.g * self.a2;
    }

    /// `bp` is the unnormalised band-pass: gain Q at the centre (multiply by `1/q`
    /// for unity), which is what a noise-excited resonator wants.
    #[inline]
    pub fn process(&mut self, x: f32) -> SvfOut {
        let v3 = x - self.ic2;
        let v1 = self.a1 * self.ic1 + self.a2 * v3;
        let v2 = self.ic2 + self.a2 * self.ic1 + self.a3 * v3;
        self.ic1 = super::super::params::undenormal(2.0 * v1 - self.ic1);
        self.ic2 = super::super::params::undenormal(2.0 * v2 - self.ic2);
        SvfOut {
            lp: v2,
            bp: v1,
            hp: x - self.k * v1 - v2,
        }
    }

    pub fn clear(&mut self) {
        self.ic1 = 0.0;
        self.ic2 = 0.0;
    }
}

/// Soft clip that stays linear for small signals and never exceeds ±1.
#[inline]
pub fn soft(x: f32) -> f32 {
    x / (1.0 + x.abs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic_and_uniform_enough() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        let va: Vec<u32> = (0..8).map(|_| a.next_u32()).collect();
        let vb: Vec<u32> = (0..8).map(|_| b.next_u32()).collect();
        assert_eq!(va, vb);
        let mut c = Rng::new(43);
        assert_ne!(c.next_u32(), va[0]);
        let mut r = Rng::new(7);
        let mean: f32 = (0..20_000).map(|_| r.unit()).sum::<f32>() / 20_000.0;
        assert!((mean - 0.5).abs() < 0.02, "{mean}");
        for _ in 0..1000 {
            assert!(r.below(3) < 3);
        }
    }

    #[test]
    fn trigger_gaps_follow_the_density_and_the_seed() {
        let count = |seed: u32, density: f32| {
            let mut t = Trigger::new(seed);
            let mut events = Vec::new();
            let mut frame = 0usize;
            for _ in 0..(600 * 48_000 / 64) {
                if let Some(at) = t.tick(64, density, 48_000.0) {
                    events.push(frame + at);
                }
                frame += 64;
            }
            events
        };
        let a = count(1, 30.0);
        let b = count(1, 30.0);
        assert_eq!(a, b, "same seed, same frames");
        // 10 minutes at 30/min ≈ 300 events; allow a wide band (exponential gaps).
        assert!(
            (220..=380).contains(&a.len()),
            "{} events in 10 min",
            a.len()
        );
        let c = count(2, 30.0);
        assert_ne!(a, c);
        assert!(count(1, 0.0).is_empty());
        for w in a.windows(2) {
            assert!(w[1] - w[0] >= (Trigger::MIN_GAP_SECONDS * 48_000.0) as usize);
        }
    }

    #[test]
    fn pitch_set_skips_invalid_values_and_picks_members() {
        let p = PitchSet::default();
        assert!(p.is_empty());
        p.set(&[220.0, 0.0, f32::NAN, 330.0, -5.0]);
        assert_eq!(p.len(), 2);
        let mut r = Rng::new(3);
        for _ in 0..50 {
            let f = p.pick(&mut r).unwrap();
            assert!(f == 220.0 || f == 330.0);
        }
        p.set(&[1.0; 12]);
        assert_eq!(p.len(), MAX_PITCHES);
    }

    #[test]
    fn svf_bandpass_passes_its_centre_and_rejects_far_away() {
        let gain_at = |hz: f32| {
            let mut f = Svf::default();
            f.set(1000.0, 8.0, 48_000.0);
            let mut peak = 0.0f32;
            let mut ph = 0.0f32;
            for i in 0..48_000 {
                let x = (ph * std::f32::consts::TAU).sin();
                ph += hz / 48_000.0;
                let y = f.process(x).bp / 8.0; // unity at the centre
                if i > 24_000 {
                    peak = peak.max(y.abs());
                }
            }
            peak
        };
        let at = gain_at(1000.0);
        let far = gain_at(100.0);
        assert!(at > 0.8 && at < 1.2, "{at}");
        assert!(far < at * 0.15, "{far} vs {at}");
    }

    #[test]
    fn wander_stays_bounded_and_moves() {
        let mut w = Wander::new(5);
        let mut min = 1.0f32;
        let mut max = -1.0f32;
        for _ in 0..(120 * 48_000 / 64) {
            let v = w.step(64, 4.0, 48_000.0);
            assert!((-1.0..=1.0).contains(&v));
            min = min.min(v);
            max = max.max(v);
        }
        assert!(max - min > 0.3, "{min}..{max}");
    }
}
