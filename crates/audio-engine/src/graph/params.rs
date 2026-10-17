//! Smoothed parameters (F1.6.2): every audible value moves along a linear ramp of a
//! few milliseconds, so slider moves never click. Targets are set from any thread via
//! atomics; the ramp runs inside the audio callback with no allocation.

use std::sync::atomic::{AtomicU32, Ordering};

/// One smoothed parameter. `target` is written by the UI thread (f32 bits in an
/// atomic); `current` lives in the callback and approaches it at a fixed step.
pub struct SmoothedParam {
    target_bits: AtomicU32,
    pub min: f32,
    pub max: f32,
}

impl SmoothedParam {
    pub fn new(initial: f32, min: f32, max: f32) -> Self {
        Self {
            target_bits: AtomicU32::new(initial.clamp(min, max).to_bits()),
            min,
            max,
        }
    }

    /// Set the target (clamped). Any thread.
    pub fn set(&self, value: f32) {
        self.target_bits
            .store(value.clamp(self.min, self.max).to_bits(), Ordering::Relaxed);
    }

    pub fn target(&self) -> f32 {
        f32::from_bits(self.target_bits.load(Ordering::Relaxed))
    }
}

/// Per-callback ramp state for one parameter (owned by the audio thread).
#[derive(Debug, Clone, Copy)]
pub struct Ramp {
    pub current: f32,
    target: f32,
    step: f32,
}

impl Ramp {
    pub fn new(initial: f32) -> Self {
        Self {
            current: initial,
            target: initial,
            step: 0.0,
        }
    }

    /// Prepare the ramp for a block: `ramp_samples` samples to reach `target`. The step
    /// is recomputed only when the target changes, so a ramp longer than one block stays
    /// linear instead of decaying exponentially block by block.
    #[inline]
    pub fn begin(&mut self, target: f32, ramp_samples: f32) {
        if target == self.target && (self.step != 0.0 || self.current == target) {
            return;
        }
        self.target = target;
        self.step = if ramp_samples > 0.0 {
            (target - self.current) / ramp_samples
        } else {
            0.0
        };
        if ramp_samples <= 0.0 {
            self.current = target;
        }
    }

    /// Advance one sample towards the target; call once per frame.
    #[inline]
    pub fn next(&mut self, target: f32) -> f32 {
        if self.step != 0.0 {
            let before = self.current;
            self.current += self.step;
            // Stop exactly on the target (no overshoot) whichever direction we move.
            if (self.step > 0.0 && self.current >= target)
                || (self.step < 0.0 && self.current <= target)
            {
                self.current = target;
                self.step = 0.0;
            }
            if before == target {
                self.current = target;
                self.step = 0.0;
            }
        }
        self.current
    }
}

/// Ramp length for a sample rate: 10 ms, the middle of the spec's 5–20 ms window.
pub fn ramp_samples(sample_rate: u32) -> f32 {
    sample_rate as f32 * 0.010
}

/// Linear gain from dB; −inf below −120 dB.
#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    if db <= -120.0 {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

/// Flush denormals: values this small become exactly zero (cheap, portable).
#[inline]
pub fn undenormal(x: f32) -> f32 {
    if x.abs() < 1e-20 {
        0.0
    } else {
        x
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_reaches_target_without_overshoot_or_jumps() {
        let mut r = Ramp::new(0.0);
        let n = ramp_samples(48_000); // 480 samples
        r.begin(1.0, n);
        let mut prev = 0.0f32;
        let mut max_jump = 0.0f32;
        for i in 0..600 {
            let v = r.next(1.0);
            assert!(v >= prev - 1e-6, "monotonic");
            max_jump = max_jump.max(v - prev);
            prev = v;
            if i == 479 {
                assert!(
                    (v - 1.0).abs() < 1e-5,
                    "at the target after 480 samples: {v}"
                );
            }
        }
        assert_eq!(prev, 1.0);
        // The biggest step is 1/480 in linear gain: far below 1 dB anywhere above −40 dB.
        assert!(max_jump <= 1.0 / 480.0 + 1e-6, "{max_jump}");
        // Downwards too.
        r.begin(0.25, n);
        for _ in 0..480 {
            r.next(0.25);
        }
        assert!((r.current - 0.25).abs() < 1e-5);
    }

    #[test]
    fn param_clamps_and_conversions() {
        let p = SmoothedParam::new(0.5, 0.0, 1.0);
        p.set(7.0);
        assert_eq!(p.target(), 1.0);
        p.set(-3.0);
        assert_eq!(p.target(), 0.0);
        assert!((db_to_gain(0.0) - 1.0).abs() < 1e-7);
        assert!((db_to_gain(-6.0206) - 0.5).abs() < 1e-4);
        assert_eq!(db_to_gain(-120.0), 0.0);
        assert_eq!(undenormal(1e-30), 0.0);
        assert_eq!(undenormal(0.01), 0.01);
    }
}
