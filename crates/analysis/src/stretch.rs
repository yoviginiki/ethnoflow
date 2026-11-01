//! Time stretch and pitch shift by a phase vocoder (Audacity programme, stage A6,
//! 2026-11-01): STFT with a Hann window of 2048 and an analysis hop of 512; the true
//! frequency of every bin is tracked from the phase advance, and frames are laid down at
//! a synthesis hop of 512 × factor with their phases propagated, so the tempo changes
//! without the pitch. Pitch shift = stretch by the ratio, then resample back to the
//! original length. Pure functions on interleaved f32 audio.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

const N: usize = 2048;
const HA: usize = 512;

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos())
        .collect()
}

fn princarg(p: f32) -> f32 {
    let two_pi = 2.0 * std::f32::consts::PI;
    p - (p / two_pi).round() * two_pi
}

fn stretch_channel(x: &[f32], factor: f32, planner: &mut FftPlanner<f32>) -> Vec<f32> {
    let win = hann(N);
    let fft = planner.plan_fft_forward(N);
    let ifft = planner.plan_fft_inverse(N);
    let hs = HA as f32 * factor;
    let n_frames = if x.len() <= N {
        1
    } else {
        (x.len() - N).div_ceil(HA) + 1
    };
    let out_len = (x.len() as f32 * factor).ceil() as usize;
    let mut out = vec![0.0f32; out_len + N];
    let mut norm = vec![0.0f32; out_len + N];
    let bins = N / 2 + 1;
    let mut last_phase = vec![0.0f32; bins];
    let mut syn_phase = vec![0.0f32; bins];
    let mut buf = vec![Complex::new(0.0f32, 0.0); N];
    let two_pi = 2.0 * std::f32::consts::PI;
    for k in 0..n_frames {
        let start = k * HA;
        for i in 0..N {
            let v = x.get(start + i).copied().unwrap_or(0.0);
            buf[i] = Complex::new(v * win[i], 0.0);
        }
        fft.process(&mut buf);
        // Phase propagation per bin.
        let mut spec = vec![Complex::new(0.0f32, 0.0); N];
        for b in 0..bins {
            let mag = buf[b].norm();
            let phase = buf[b].arg();
            let expected = two_pi * b as f32 * HA as f32 / N as f32;
            let delta = princarg(phase - last_phase[b] - expected);
            last_phase[b] = phase;
            let true_adv = expected + delta; // radians per analysis hop
            if k == 0 {
                syn_phase[b] = phase;
            } else {
                syn_phase[b] += true_adv * factor;
            }
            spec[b] = Complex::from_polar(mag, syn_phase[b]);
        }
        for b in bins..N {
            spec[b] = spec[N - b].conj();
        }
        ifft.process(&mut spec);
        let pos = (k as f32 * hs).round() as usize;
        if pos >= out_len {
            break;
        }
        for i in 0..N {
            if pos + i < out.len() {
                out[pos + i] += spec[i].re / N as f32 * win[i];
                norm[pos + i] += win[i] * win[i];
            }
        }
    }
    out.truncate(out_len);
    for (i, v) in out.iter_mut().enumerate() {
        if norm[i] > 1e-3 {
            *v /= norm[i];
        }
    }
    out
}

/// Change the length by `factor` (0.25..4) without changing the pitch.
pub fn time_stretch(samples: &[f32], channels: usize, factor: f32) -> Vec<f32> {
    let channels = channels.max(1);
    let factor = factor.clamp(0.25, 4.0);
    let mut planner = FftPlanner::<f32>::new();
    let len = samples.len() / channels;
    let out_len = (len as f32 * factor).ceil() as usize;
    let mut out = vec![0.0f32; out_len * channels];
    for c in 0..channels {
        let x: Vec<f32> = samples.iter().skip(c).step_by(channels).copied().collect();
        let y = stretch_channel(&x, factor, &mut planner);
        for (i, v) in y.iter().enumerate().take(out_len) {
            out[i * channels + c] = *v;
        }
    }
    out
}

/// Linear resampling of interleaved audio to `out_len` frames.
fn resample(samples: &[f32], channels: usize, out_len: usize) -> Vec<f32> {
    let len = samples.len() / channels;
    if len < 2 || out_len == 0 {
        return vec![0.0; out_len * channels];
    }
    let mut out = vec![0.0f32; out_len * channels];
    let ratio = (len - 1) as f64 / (out_len.max(2) - 1) as f64;
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(len - 1);
        let t = (pos - i0 as f64) as f32;
        for c in 0..channels {
            let a = samples[i0 * channels + c];
            let b = samples[i1 * channels + c];
            out[i * channels + c] = a + (b - a) * t;
        }
    }
    out
}

/// Shift the pitch by `semitones` (±24) without changing the length.
pub fn pitch_shift(samples: &[f32], channels: usize, semitones: f32) -> Vec<f32> {
    let channels = channels.max(1);
    let ratio = 2f32.powf(semitones.clamp(-24.0, 24.0) / 12.0);
    let len = samples.len() / channels;
    // Stretch by the ratio, then play it back faster / slower to the original length.
    let stretched = time_stretch(samples, channels, ratio);
    resample(&stretched, channels, len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, seconds: f32, sr: u32) -> Vec<f32> {
        (0..(seconds * sr as f32) as usize)
            .map(|i| (i as f32 * hz / sr as f32 * std::f32::consts::TAU).sin() * 0.5)
            .collect()
    }
    fn crossings_per_second(x: &[f32], sr: u32) -> f32 {
        let a = x.len() / 4;
        let b = x.len() * 3 / 4;
        let n = x[a..b]
            .windows(2)
            .filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0))
            .count();
        n as f32 / ((b - a) as f32 / sr as f32)
    }

    #[test]
    fn stretching_changes_the_length_but_not_the_pitch() {
        let sr = 48_000;
        let x = tone(440.0, 1.0, sr);
        let y = time_stretch(&x, 1, 1.5);
        assert!((y.len() as f32 / x.len() as f32 - 1.5).abs() < 0.01);
        let cps = crossings_per_second(&y, sr);
        assert!(
            (cps - 880.0).abs() < 30.0,
            "440 Hz stays 440 Hz: {cps} crossings/s"
        );
        let rms = (y.iter().map(|v| v * v).sum::<f32>() / y.len() as f32).sqrt();
        assert!(rms > 0.25, "level kept ({rms})");
    }

    #[test]
    fn pitch_shift_keeps_the_length_and_moves_the_pitch() {
        let sr = 48_000;
        let x = tone(440.0, 1.0, sr);
        let up = pitch_shift(&x, 1, 12.0);
        assert_eq!(up.len(), x.len());
        let cps = crossings_per_second(&up, sr);
        assert!(
            (cps - 1760.0).abs() < 60.0,
            "an octave up: {cps} crossings/s"
        );
        let down = pitch_shift(&x, 1, -12.0);
        let cps = crossings_per_second(&down, sr);
        assert!(
            (cps - 440.0).abs() < 20.0,
            "an octave down: {cps} crossings/s"
        );
    }
}
