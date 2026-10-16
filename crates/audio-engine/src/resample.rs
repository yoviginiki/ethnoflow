//! Simple linear-interpolation resampler. Good enough for Phase 0 playback when the
//! output device rate differs from the recording rate. A proper band-limited resampler
//! (e.g. `rubato`) is planned for export-quality conversion in Phase 1/5 — see docs/DECISIONS.md.

use crate::buffer::AudioBuffer;

/// Resample `input` to `target_rate` using linear interpolation per channel.
pub fn resample_linear(input: &AudioBuffer, target_rate: u32) -> AudioBuffer {
    if input.sample_rate == target_rate || input.is_empty() || target_rate == 0 {
        let mut out = input.clone();
        if target_rate != 0 {
            out.sample_rate = target_rate;
        }
        return out;
    }
    let ch = input.channels as usize;
    let in_frames = input.frames();
    let ratio = input.sample_rate as f64 / target_rate as f64;
    let out_frames = ((in_frames as f64) / ratio).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(out_frames * ch);
    for i in 0..out_frames {
        let src_pos = i as f64 * ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;
        let idx1 = (idx + 1).min(in_frames - 1);
        let idx = idx.min(in_frames - 1);
        for c in 0..ch {
            let a = input.samples[idx * ch + c];
            let b = input.samples[idx1 * ch + c];
            out.push(a + (b - a) * frac);
        }
    }
    AudioBuffer::new(target_rate, input.channels, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_rate_is_identity() {
        let b = AudioBuffer::sine(48_000, 440.0, 0.1, 0.5);
        assert_eq!(resample_linear(&b, 48_000), b);
    }

    #[test]
    fn length_scales_with_ratio() {
        let b = AudioBuffer::sine(44_100, 440.0, 1.0, 0.5);
        let r = resample_linear(&b, 48_000);
        assert_eq!(r.sample_rate, 48_000);
        assert_eq!(r.frames(), 48_000);
        let d = resample_linear(&b, 22_050);
        assert_eq!(d.frames(), 22_050);
    }

    #[test]
    fn preserves_sine_frequency_roughly() {
        // Count zero crossings before and after; they should match the tone frequency.
        let b = AudioBuffer::sine(44_100, 100.0, 1.0, 0.9);
        let r = resample_linear(&b, 48_000);
        let zc = |s: &[f32]| {
            s.windows(2)
                .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
                .count()
        };
        let a = zc(&b.samples);
        let c = zc(&r.samples);
        assert!((a as i64 - c as i64).abs() <= 2, "{a} vs {c}");
        assert!(r.peak() <= 0.9 + 1e-6);
    }

    #[test]
    fn stereo_channels_are_independent() {
        // Left = constant 0.5, right = constant -0.25
        let samples: Vec<f32> = (0..200).flat_map(|_| [0.5_f32, -0.25]).collect();
        let b = AudioBuffer::new(8_000, 2, samples);
        let r = resample_linear(&b, 11_025);
        assert_eq!(r.channels, 2);
        for f in r.samples.as_chunks::<2>().0 {
            assert!((f[0] - 0.5).abs() < 1e-6);
            assert!((f[1] + 0.25).abs() < 1e-6);
        }
    }
}
