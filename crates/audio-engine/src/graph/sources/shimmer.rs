//! Delay-line pitch shifter for the master "shimmer" (F1B.3): two read taps crawling
//! through a short buffer at `1 − ratio` samples per sample, each under a Hann window
//! and half a window apart, so the seam of one tap is always covered by the other.
//! Allocated once; `process` never allocates.

pub struct PitchShifter {
    buf: Vec<f32>,
    write: usize,
    /// Position of tap 1 inside the window, 0..1 (tap 2 is at +0.5).
    phase: f32,
    ratio: f32,
    win: f32,
}

impl PitchShifter {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate.max(1) as f32;
        let win = (0.06 * sr).max(64.0);
        let len = (win * 2.5) as usize + 4;
        Self {
            buf: vec![0.0; len],
            write: 0,
            phase: 0.0,
            ratio: 2.0,
            win,
        }
    }

    /// Frequency ratio (2 = one octave up).
    pub fn set_ratio(&mut self, ratio: f32) {
        self.ratio = ratio.clamp(0.25, 4.0);
    }

    pub fn clear(&mut self) {
        self.buf.fill(0.0);
        self.phase = 0.0;
    }

    #[inline]
    fn tap(&self, delay: f32) -> f32 {
        let n = self.buf.len() as f32;
        let mut pos = self.write as f32 - delay;
        while pos < 0.0 {
            pos += n;
        }
        let i = pos.floor();
        let frac = pos - i;
        let i0 = (i as usize) % self.buf.len();
        let i1 = (i0 + 1) % self.buf.len();
        self.buf[i0] * (1.0 - frac) + self.buf[i1] * frac
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        self.buf[self.write] = x;
        self.write = (self.write + 1) % self.buf.len();
        // Taps move against the write head by (ratio − 1) samples per sample.
        self.phase += (self.ratio - 1.0) / self.win;
        self.phase = self.phase.rem_euclid(1.0);
        let p2 = (self.phase + 0.5).rem_euclid(1.0);
        let hann = |p: f32| 0.5 - 0.5 * (p * std::f32::consts::TAU).cos();
        // A tap's delay runs across the window; the window is zero where the delay jumps.
        // Pitch up: the delay shrinks while the window is open (read head outruns write).
        let d1 = 1.0 + (1.0 - self.phase) * self.win;
        let d2 = 1.0 + (1.0 - p2) * self.win;
        self.tap(d1) * hann(self.phase) + self.tap(d2) * hann(p2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_octave_up_doubles_the_zero_crossings() {
        let sr = 48_000u32;
        let mut s = PitchShifter::new(sr);
        s.set_ratio(2.0);
        let mut out = Vec::with_capacity(sr as usize);
        for i in 0..sr as usize {
            let x = (i as f32 * 220.0 / sr as f32 * std::f32::consts::TAU).sin();
            out.push(s.process(x));
        }
        let z = out[sr as usize / 2..]
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count() as f32
            * 2.0; // per second (half a second measured)
        assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.5));
        assert!((z - 880.0).abs() < 120.0, "crossings/s {z} (expect ~880)");
    }
}
