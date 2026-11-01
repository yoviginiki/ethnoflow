//! Per-track processing for the arrangement's tracks (home-studio item 4, 2026-11-01):
//! a three-band EQ (low shelf, peaking mid, high shelf; RBJ cookbook biquads) and a
//! feed-forward compressor (threshold / ratio / attack / release / make-up, hard knee,
//! peak detector in dB). Coefficients are recomputed once per block from the targets;
//! the per-sample path allocates nothing.

use std::f32::consts::PI;

#[derive(Clone, Copy, Default)]
pub struct BiquadState {
    z1: f32,
    z2: f32,
}

#[derive(Clone, Copy)]
pub struct BiquadCoeffs {
    pub b0: f32,
    pub b1: f32,
    pub b2: f32,
    pub a1: f32,
    pub a2: f32,
}

impl Default for BiquadCoeffs {
    fn default() -> Self {
        Self::bypass()
    }
}

impl BiquadCoeffs {
    pub fn bypass() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }

    /// Low shelf at `f0` with `gain_db` (RBJ, S = 1).
    pub fn low_shelf(sr: f32, f0: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 0.01 {
            return Self::bypass();
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * (f0 / sr).clamp(0.0001, 0.49);
        let (sn, cs) = (w0.sin(), w0.cos());
        let alpha = sn / 2.0 * (2.0f32).sqrt();
        let sqa = 2.0 * a.sqrt() * alpha;
        let b0 = a * ((a + 1.0) - (a - 1.0) * cs + sqa);
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * cs);
        let b2 = a * ((a + 1.0) - (a - 1.0) * cs - sqa);
        let a0 = (a + 1.0) + (a - 1.0) * cs + sqa;
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * cs);
        let a2 = (a + 1.0) + (a - 1.0) * cs - sqa;
        Self::norm(b0, b1, b2, a0, a1, a2)
    }

    /// High shelf at `f0` with `gain_db`.
    pub fn high_shelf(sr: f32, f0: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 0.01 {
            return Self::bypass();
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * (f0 / sr).clamp(0.0001, 0.49);
        let (sn, cs) = (w0.sin(), w0.cos());
        let alpha = sn / 2.0 * (2.0f32).sqrt();
        let sqa = 2.0 * a.sqrt() * alpha;
        let b0 = a * ((a + 1.0) + (a - 1.0) * cs + sqa);
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * cs);
        let b2 = a * ((a + 1.0) + (a - 1.0) * cs - sqa);
        let a0 = (a + 1.0) - (a - 1.0) * cs + sqa;
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * cs);
        let a2 = (a + 1.0) - (a - 1.0) * cs - sqa;
        Self::norm(b0, b1, b2, a0, a1, a2)
    }

    /// Peaking EQ at `f0`, `q`, with `gain_db`.
    pub fn peak(sr: f32, f0: f32, q: f32, gain_db: f32) -> Self {
        if gain_db.abs() < 0.01 {
            return Self::bypass();
        }
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * (f0 / sr).clamp(0.0001, 0.49);
        let (sn, cs) = (w0.sin(), w0.cos());
        let alpha = sn / (2.0 * q.max(0.1));
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * cs;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * cs;
        let a2 = 1.0 - alpha / a;
        Self::norm(b0, b1, b2, a0, a1, a2)
    }

    fn norm(b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    /// Transposed direct form II, one sample.
    #[inline]
    pub fn run(&self, st: &mut BiquadState, x: f32) -> f32 {
        let y = self.b0 * x + st.z1;
        st.z1 = crate::graph::params::undenormal(self.b1 * x - self.a1 * y + st.z2);
        st.z2 = crate::graph::params::undenormal(self.b2 * x - self.a2 * y);
        y
    }
}

/// Three-band EQ coefficients for one track and one block.
#[derive(Clone, Copy, Default)]
pub struct Eq3 {
    pub low: BiquadCoeffs,
    pub mid: BiquadCoeffs,
    pub high: BiquadCoeffs,
    pub active: bool,
}

impl Eq3 {
    pub const LOW_HZ: f32 = 200.0;
    pub const HIGH_HZ: f32 = 4000.0;
    pub fn new(sr: f32, low_db: f32, mid_db: f32, mid_hz: f32, high_db: f32) -> Self {
        let active = low_db.abs() >= 0.01 || mid_db.abs() >= 0.01 || high_db.abs() >= 0.01;
        Self {
            low: BiquadCoeffs::low_shelf(sr, Self::LOW_HZ, low_db),
            mid: BiquadCoeffs::peak(sr, mid_hz.clamp(100.0, 8000.0), 1.0, mid_db),
            high: BiquadCoeffs::high_shelf(sr, Self::HIGH_HZ, high_db),
            active,
        }
    }
    #[inline]
    pub fn run(&self, st: &mut [BiquadState; 3], x: f32) -> f32 {
        if !self.active {
            return x;
        }
        let y = self.low.run(&mut st[0], x);
        let y = self.mid.run(&mut st[1], y);
        self.high.run(&mut st[2], y)
    }
}

/// Feed-forward compressor settings for one block (hard knee, peak detection in dB).
#[derive(Clone, Copy)]
pub struct CompBlock {
    pub threshold_db: f32,
    pub ratio: f32,
    /// Per-sample envelope coefficients.
    pub attack: f32,
    pub release: f32,
    pub makeup: f32,
    pub active: bool,
}

impl Default for CompBlock {
    fn default() -> Self {
        Self::new(48_000.0, 0.0, 1.0, 10.0, 100.0, 0.0)
    }
}

impl CompBlock {
    pub fn new(
        sr: f32,
        threshold_db: f32,
        ratio: f32,
        attack_ms: f32,
        release_ms: f32,
        makeup_db: f32,
    ) -> Self {
        let ratio = ratio.max(1.0);
        Self {
            threshold_db,
            ratio,
            attack: (-1.0 / (attack_ms.max(0.1) * 0.001 * sr)).exp(),
            release: (-1.0 / (release_ms.max(1.0) * 0.001 * sr)).exp(),
            makeup: 10f32.powf(makeup_db / 20.0),
            active: ratio > 1.001 || makeup_db.abs() >= 0.01,
        }
    }

    /// `env_db` is the track's detector state (dB, starts at −120). Returns the processed sample.
    #[inline]
    pub fn run(&self, env_db: &mut f32, x: f32) -> f32 {
        if !self.active {
            return x;
        }
        let level_db = 20.0 * x.abs().max(1e-6).log10();
        let coeff = if level_db > *env_db {
            self.attack
        } else {
            self.release
        };
        *env_db = coeff * *env_db + (1.0 - coeff) * level_db;
        let over = *env_db - self.threshold_db;
        let gain_db = if over > 0.0 {
            over * (1.0 / self.ratio - 1.0)
        } else {
            0.0
        };
        x * 10f32.powf(gain_db / 20.0) * self.makeup
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band_rms(sr: f32, hz: f32, coeffs: &Eq3) -> f32 {
        let mut st = [BiquadState::default(); 3];
        let n = (sr * 0.5) as usize;
        let mut acc = 0.0f32;
        let mut cnt = 0usize;
        for i in 0..n {
            let x = (i as f32 * hz / sr * 2.0 * PI).sin() * 0.5;
            let y = coeffs.run(&mut st, x);
            if i > n / 2 {
                acc += y * y;
                cnt += 1;
            }
        }
        (acc / cnt as f32).sqrt()
    }

    #[test]
    fn the_shelves_and_the_peak_move_their_own_bands() {
        let sr = 48_000.0;
        let flat = Eq3::new(sr, 0.0, 0.0, 1000.0, 0.0);
        assert!(!flat.active);
        let low = Eq3::new(sr, 12.0, 0.0, 1000.0, 0.0);
        assert!(
            band_rms(sr, 60.0, &low) / band_rms(sr, 60.0, &flat) > 3.0,
            "+12 dB low shelf lifts 60 Hz"
        );
        assert!(
            (band_rms(sr, 8000.0, &low) / band_rms(sr, 8000.0, &flat) - 1.0).abs() < 0.1,
            "and leaves 8 kHz alone"
        );
        let high = Eq3::new(sr, 0.0, 0.0, 1000.0, -12.0);
        assert!(
            band_rms(sr, 10_000.0, &high) / band_rms(sr, 10_000.0, &flat) < 0.4,
            "−12 dB high shelf cuts 10 kHz"
        );
        let mid = Eq3::new(sr, 0.0, 9.0, 1000.0, 0.0);
        let at = band_rms(sr, 1000.0, &mid) / band_rms(sr, 1000.0, &flat);
        let away = band_rms(sr, 100.0, &mid) / band_rms(sr, 100.0, &flat);
        assert!(
            at > 2.2 && away < 1.2,
            "peak +9 dB at 1 kHz: {at} vs {away} at 100 Hz"
        );
    }

    #[test]
    fn the_compressor_reduces_what_is_over_the_threshold_and_can_make_up() {
        let sr = 48_000.0;
        let c = CompBlock::new(sr, -20.0, 10.0, 1.0, 50.0, 0.0);
        let mut env = -120.0f32;
        let mut peak = 0.0f32;
        for i in 0..48_000 {
            let x = (i as f32 * 1000.0 / sr * 2.0 * PI).sin();
            let y = c.run(&mut env, x);
            if i > 24_000 {
                peak = peak.max(y.abs());
            }
        }
        let peak_db = 20.0 * peak.log10();
        // 0 dBFS in, threshold −20, ratio 10 → about −18 dBFS out.
        assert!((peak_db + 18.0).abs() < 1.5, "{peak_db}");
        let quiet = CompBlock::new(sr, -20.0, 10.0, 1.0, 50.0, 6.0);
        let mut env = -120.0f32;
        let mut peak = 0.0f32;
        for i in 0..48_000 {
            let x = (i as f32 * 1000.0 / sr * 2.0 * PI).sin() * 0.01; // −40 dBFS: under the threshold
            let y = quiet.run(&mut env, x);
            if i > 24_000 {
                peak = peak.max(y.abs());
            }
        }
        assert!(
            (20.0 * peak.log10() + 34.0).abs() < 0.5,
            "untouched below the threshold, +6 dB make-up"
        );
        assert!(!CompBlock::new(sr, 0.0, 1.0, 10.0, 100.0, 0.0).active);
    }
}
