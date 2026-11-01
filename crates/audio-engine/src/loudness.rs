//! Integrated loudness (ITU-R BS.1770-4 / EBU R128): K-weighting (a +4 dB high shelf
//! near 1.7 kHz, then a 38 Hz high-pass), mean square over 400 ms blocks with 75 %
//! overlap, absolute gate at −70 LUFS and a relative gate 10 LU under the ungated mean.
//! Used by the song export to land a mix on a target (−14 LUFS for phones and streaming).
//! Home-studio item 4, 2026-11-01.

use crate::buffer::AudioBuffer;

#[derive(Clone, Copy)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
    z1: f64,
    z2: f64,
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b0: b[0] / a[0],
            b1: b[1] / a[0],
            b2: b[2] / a[0],
            a1: a[1] / a[0],
            a2: a[2] / a[0],
            z1: 0.0,
            z2: 0.0,
        }
    }
    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

/// The two K-weighting stages for a sample rate (BS.1770-4 Annex, derived for any rate
/// from the 48 kHz specification via the analogue prototypes).
fn k_weighting(sample_rate: u32) -> (Biquad, Biquad) {
    let fs = sample_rate.max(1) as f64;
    // Stage 1: high shelf, f0 = 1681.974 Hz, gain +3.9998 dB, Q = 0.7071752.
    let (f0, g_db, q) = (
        1_681.974_450_955_533,
        3.999_843_853_973_347,
        0.707_175_221_410_369,
    );
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let vh = 10f64.powf(g_db / 20.0);
    let vb = vh.powf(0.499_666_774_155);
    let a0 = 1.0 + k / q + k * k;
    let stage1 = Biquad::new(
        [
            (vh + vb * k / q + k * k) / a0,
            2.0 * (k * k - vh) / a0,
            (vh - vb * k / q + k * k) / a0,
        ],
        [1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    );
    // Stage 2: high-pass, f0 = 38.135 Hz, Q = 0.5003.
    let (f0, q) = (38.135_470_876_002_17, 0.500_327_041_801_136_7);
    let k = (std::f64::consts::PI * f0 / fs).tan();
    let a0 = 1.0 + k / q + k * k;
    let stage2 = Biquad::new(
        [1.0 / a0, -2.0 / a0, 1.0 / a0],
        [1.0, 2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    );
    (stage1, stage2)
}

/// Integrated loudness in LUFS, or `None` for silence / audio shorter than one block.
pub fn integrated_lufs(buf: &AudioBuffer) -> Option<f64> {
    let ch = buf.channels.max(1) as usize;
    let frames = buf.frames();
    let fs = buf.sample_rate.max(1) as usize;
    let block = (fs as f64 * 0.4) as usize;
    let hop = block / 4;
    if frames < block || block == 0 {
        return None;
    }
    // K-weight every channel (L/R/C weight 1; surround weights not needed here).
    let mut weighted: Vec<Vec<f64>> = Vec::with_capacity(ch);
    for c in 0..ch {
        let (mut s1, mut s2) = k_weighting(buf.sample_rate);
        let mut out = Vec::with_capacity(frames);
        for f in 0..frames {
            let x = buf.samples[f * ch + c] as f64;
            out.push(s2.run(s1.run(x)));
        }
        weighted.push(out);
    }
    // Block loudness: −0.691 + 10 log10(Σ_channels mean square).
    let mut blocks: Vec<f64> = Vec::new();
    let mut start = 0usize;
    while start + block <= frames {
        let mut sum = 0.0;
        for w in &weighted {
            let ms: f64 = w[start..start + block].iter().map(|v| v * v).sum::<f64>() / block as f64;
            sum += ms;
        }
        blocks.push(-0.691 + 10.0 * sum.max(1e-30).log10());
        start += hop;
    }
    if blocks.is_empty() {
        return None;
    }
    let energy = |l: f64| 10f64.powf((l + 0.691) / 10.0);
    let gated: Vec<f64> = blocks.iter().copied().filter(|&l| l > -70.0).collect();
    if gated.is_empty() {
        return None;
    }
    let mean = gated.iter().map(|&l| energy(l)).sum::<f64>() / gated.len() as f64;
    let relative = -0.691 + 10.0 * mean.log10() - 10.0;
    let kept: Vec<f64> = gated.into_iter().filter(|&l| l > relative).collect();
    if kept.is_empty() {
        return None;
    }
    let mean = kept.iter().map(|&l| energy(l)).sum::<f64>() / kept.len() as f64;
    Some(-0.691 + 10.0 * mean.log10())
}

/// Gain (dB) that brings `measured` to `target`, clamped to a sane range.
pub fn gain_to_target_db(measured: f64, target: f64) -> f64 {
    (target - measured).clamp(-40.0, 40.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stereo_sine(db: f32, seconds: f32) -> AudioBuffer {
        let amp = 10f32.powf(db / 20.0);
        let mono = AudioBuffer::sine(48_000, 1000.0, seconds, amp);
        let mut s = Vec::with_capacity(mono.samples.len() * 2);
        for v in &mono.samples {
            s.push(*v);
            s.push(*v);
        }
        AudioBuffer::new(48_000, 2, s)
    }

    #[test]
    fn full_scale_sines_read_as_the_standard_says() {
        // BS.1770: a 0 dBFS 1 kHz sine in ONE channel reads -3.01 LUFS; in both channels of
        // a stereo file the two mean squares add and it reads 0 LUFS (the K-weighting's
        // +0.69 dB at 1 kHz cancels the -0.691 offset).
        let mono = integrated_lufs(&AudioBuffer::sine(48_000, 1000.0, 3.0, 1.0)).unwrap();
        assert!((mono + 3.01).abs() < 0.15, "mono {mono}");
        let l = integrated_lufs(&stereo_sine(0.0, 3.0)).unwrap();
        assert!(l.abs() < 0.15, "stereo {l}");
        let q = integrated_lufs(&stereo_sine(-20.0, 3.0)).unwrap();
        assert!((q + 20.0).abs() < 0.15, "{q}");
        assert!((gain_to_target_db(q, -14.0) - 6.0).abs() < 0.2);
    }

    #[test]
    fn silence_and_very_short_audio_have_no_loudness() {
        assert!(integrated_lufs(&AudioBuffer::new(48_000, 2, vec![0.0; 48_000 * 2])).is_none());
        assert!(integrated_lufs(&AudioBuffer::new(48_000, 1, vec![0.5; 100])).is_none());
    }
}
