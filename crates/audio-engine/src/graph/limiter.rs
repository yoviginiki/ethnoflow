//! Master limiter (F1.6): a simple look-ahead-free peak limiter with fast attack and
//! exponential release, followed by a hard ceiling. Never outputs above the ceiling.

#[derive(Debug, Clone, Copy)]
pub struct Limiter {
    ceiling: f32,
    gain: f32,
    release_coeff: f32,
    peak_hold: f32,
    peak_decay: f32,
}

impl Limiter {
    /// `ceiling` linear (e.g. 0.966 = −0.3 dBFS), `release_seconds` ≈ 0.08.
    pub fn new(sample_rate: u32, ceiling: f32, release_seconds: f32) -> Self {
        let sr = sample_rate.max(1) as f32;
        Self {
            ceiling,
            gain: 1.0,
            release_coeff: (-1.0 / (release_seconds.max(0.001) * sr)).exp(),
            peak_hold: 0.0,
            peak_decay: (-1.0 / (0.5 * sr)).exp(),
        }
    }

    /// Process one frame in place (all channels share the gain).
    #[inline]
    pub fn process_frame(&mut self, frame: &mut [f32]) {
        let mut peak = 0.0f32;
        for s in frame.iter() {
            peak = peak.max(s.abs());
        }
        // Instant attack: if this frame would exceed the ceiling, pull the gain down now.
        let needed = if peak * self.gain > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        if needed < self.gain {
            self.gain = needed;
        } else {
            // Release towards unity.
            self.gain = 1.0 - (1.0 - self.gain) * self.release_coeff;
        }
        for s in frame.iter_mut() {
            *s = (*s * self.gain).clamp(-self.ceiling, self.ceiling);
            self.peak_hold = self.peak_hold.max(s.abs());
        }
        self.peak_hold *= self.peak_decay;
    }

    /// Recent output peak (decaying hold), for the master meter.
    pub fn peak(&self) -> f32 {
        self.peak_hold
    }

    pub fn gain_reduction(&self) -> f32 {
        self.gain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn never_exceeds_the_ceiling_and_recovers() {
        let mut l = Limiter::new(48_000, 0.966, 0.08);
        let mut worst = 0.0f32;
        // 0.2 s of a +12 dB sine (peak 4.0), then 0.5 s of silence-ish quiet signal.
        for i in 0..(48_000 * 7 / 10) {
            let t = i as f32 / 48_000.0;
            let x = if i < 9_600 {
                4.0 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
            } else {
                0.1 * (2.0 * std::f32::consts::PI * 220.0 * t).sin()
            };
            let mut f = [x, x];
            l.process_frame(&mut f);
            worst = worst.max(f[0].abs()).max(f[1].abs());
        }
        assert!(worst <= 0.966 + 1e-6, "peak {worst}");
        assert!(
            l.gain_reduction() > 0.95,
            "gain recovered to {}",
            l.gain_reduction()
        );
        assert!(l.peak() > 0.0);
    }
}
