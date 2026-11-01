//! Noise reduction by spectral subtraction (Audacity programme, stage A5, 2026-11-01):
//! a noise profile is the per-bin magnitude of a noise-only selection (STFT, Hann 2048,
//! hop 512); `denoise` attenuates every bin whose magnitude sits below
//! `sensitivity × profile` by up to `reduction_db`, with smoothing over frequency and
//! time so the result does not "bubble". Pure functions on interleaved f32 audio.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

pub const FRAME: usize = 2048;
pub const HOP: usize = 512;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseProfile {
    /// Mean magnitude per bin (FRAME / 2 + 1 bins), per channel.
    pub bins: Vec<Vec<f32>>,
    pub channels: usize,
    pub sample_rate: u32,
}

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos())
        .collect()
}

fn channel(samples: &[f32], channels: usize, c: usize) -> Vec<f32> {
    samples
        .iter()
        .skip(c)
        .step_by(channels.max(1))
        .copied()
        .collect()
}

/// Complex STFT frames of one channel (zero-padded tail so every sample is covered).
fn stft_complex(x: &[f32], win: &[f32], planner: &mut FftPlanner<f32>) -> Vec<Vec<Complex<f32>>> {
    let fft = planner.plan_fft_forward(FRAME);
    let n_frames = if x.len() <= FRAME {
        1
    } else {
        (x.len() - FRAME).div_ceil(HOP) + 1
    };
    let mut frames = Vec::with_capacity(n_frames);
    let mut buf = vec![Complex::new(0.0f32, 0.0); FRAME];
    for f in 0..n_frames {
        let start = f * HOP;
        for i in 0..FRAME {
            let v = x.get(start + i).copied().unwrap_or(0.0);
            buf[i] = Complex::new(v * win[i], 0.0);
        }
        fft.process(&mut buf);
        frames.push(buf[..FRAME / 2 + 1].to_vec());
    }
    frames
}

/// Inverse STFT with overlap-add; Hann analysis + Hann synthesis at 75 % overlap sums to
/// a constant, which we divide out.
fn istft(
    frames: &[Vec<Complex<f32>>],
    len: usize,
    win: &[f32],
    planner: &mut FftPlanner<f32>,
) -> Vec<f32> {
    let ifft = planner.plan_fft_inverse(FRAME);
    let mut out = vec![0.0f32; len + FRAME];
    let mut norm = vec![0.0f32; len + FRAME];
    let mut buf = vec![Complex::new(0.0f32, 0.0); FRAME];
    for (f, spec) in frames.iter().enumerate() {
        buf[..(FRAME / 2 + 1)].copy_from_slice(&spec[..(FRAME / 2 + 1)]);
        for i in FRAME / 2 + 1..FRAME {
            buf[i] = spec[FRAME - i].conj();
        }
        ifft.process(&mut buf);
        let start = f * HOP;
        for i in 0..FRAME {
            let w = win[i];
            out[start + i] += buf[i].re / FRAME as f32 * w;
            norm[start + i] += w * w;
        }
    }
    out.truncate(len);
    for (i, v) in out.iter_mut().enumerate() {
        if norm[i] > 1e-6 {
            *v /= norm[i];
        }
    }
    out
}

/// The noise profile of a noise-only selection (interleaved samples).
pub fn noise_profile(samples: &[f32], channels: usize, sample_rate: u32) -> NoiseProfile {
    let channels = channels.max(1);
    let win = hann(FRAME);
    let mut planner = FftPlanner::<f32>::new();
    let bins = (0..channels)
        .map(|c| {
            let x = channel(samples, channels, c);
            let frames = stft_complex(&x, &win, &mut planner);
            let mut acc = vec![0.0f32; FRAME / 2 + 1];
            for fr in &frames {
                for (k, v) in fr.iter().enumerate() {
                    acc[k] += v.norm();
                }
            }
            let n = frames.len().max(1) as f32;
            acc.iter().map(|v| v / n).collect()
        })
        .collect();
    NoiseProfile {
        bins,
        channels,
        sample_rate,
    }
}

/// Spectral subtraction against `profile`. `reduction_db` 0..40 is how far a noise bin
/// goes down; `sensitivity` 0.5..4 scales the profile (higher = more taken as noise).
pub fn denoise(
    samples: &[f32],
    channels: usize,
    profile: &NoiseProfile,
    reduction_db: f32,
    sensitivity: f32,
) -> Vec<f32> {
    let channels = channels.max(1);
    let floor = 10f32.powf(-reduction_db.clamp(0.0, 40.0) / 20.0);
    let sens = sensitivity.clamp(0.5, 4.0);
    let win = hann(FRAME);
    let mut planner = FftPlanner::<f32>::new();
    let len = samples.len() / channels;
    let mut out = vec![0.0f32; samples.len()];
    for c in 0..channels {
        let prof = &profile.bins[c.min(profile.bins.len() - 1)];
        let x = channel(samples, channels, c);
        let mut frames = stft_complex(&x, &win, &mut planner);
        // Per-frame gains: 1 where the bin is clearly above the noise, `floor` where it is
        // at or below it, smoothed over neighbouring bins and over time (attack/release).
        let n_bins = FRAME / 2 + 1;
        let mut prev = vec![1.0f32; n_bins];
        for fr in frames.iter_mut() {
            let mut g: Vec<f32> = (0..n_bins)
                .map(|k| {
                    let noise = prof[k] * sens;
                    let mag = fr[k].norm();
                    if noise <= 1e-9 {
                        1.0
                    } else if mag <= noise {
                        floor
                    } else {
                        // Soft knee between 1× and 2× the noise level.
                        let r = ((mag / noise) - 1.0).clamp(0.0, 1.0);
                        floor + (1.0 - floor) * r
                    }
                })
                .collect();
            // Frequency smoothing (3-bin mean) against musical noise.
            let raw = g.clone();
            for k in 1..n_bins - 1 {
                g[k] = (raw[k - 1] + raw[k] + raw[k + 1]) / 3.0;
            }
            // Time smoothing: gains open fast, close slowly.
            for k in 0..n_bins {
                g[k] = if g[k] > prev[k] {
                    g[k]
                } else {
                    prev[k] * 0.6 + g[k] * 0.4
                };
                prev[k] = g[k];
                fr[k] *= g[k];
            }
        }
        let y = istft(&frames, len, &win, &mut planner);
        for (i, v) in y.iter().enumerate() {
            out[i * channels + c] = *v;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lcg(seed: &mut u32) -> f32 {
        *seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (*seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
    }

    #[test]
    fn a_tone_in_noise_gets_cleaner_by_more_than_ten_db() {
        let sr = 48_000u32;
        let mut seed = 7u32;
        let noise_only: Vec<f32> = (0..sr as usize).map(|_| lcg(&mut seed) * 0.05).collect();
        let mixed: Vec<f32> = (0..2 * sr as usize)
            .map(|i| {
                (i as f32 * 440.0 / sr as f32 * std::f32::consts::TAU).sin() * 0.3
                    + lcg(&mut seed) * 0.05
            })
            .collect();
        let profile = noise_profile(&noise_only, 1, sr);
        let clean = denoise(&mixed, 1, &profile, 24.0, 1.5);
        assert_eq!(clean.len(), mixed.len());
        // Noise power: compare the residual after removing the tone (least squares on the
        // known sinusoid) before and after.
        // Least-squares fit of the 440 Hz sinusoid (both quadratures, so the slice offset
        // does not matter): the fitted amplitude and the residual (= the noise).
        let fit = |x: &[f32]| {
            let n = x.len() as f32;
            let (mut c, mut s) = (0.0f32, 0.0f32);
            for (i, v) in x.iter().enumerate() {
                let ph = i as f32 * 440.0 / sr as f32 * std::f32::consts::TAU;
                c += v * ph.cos();
                s += v * ph.sin();
            }
            (c * 2.0 / n, s * 2.0 / n)
        };
        let tone_power = |x: &[f32]| {
            let (c, s) = fit(x);
            (c * c + s * s).sqrt()
        };
        let residual = |x: &[f32]| {
            let (c, s) = fit(x);
            let n = x.len() as f32;
            (x.iter()
                .enumerate()
                .map(|(i, v)| {
                    let ph = i as f32 * 440.0 / sr as f32 * std::f32::consts::TAU;
                    (v - c * ph.cos() - s * ph.sin()).powi(2)
                })
                .sum::<f32>()
                / n)
                .sqrt()
        };
        let before = residual(&mixed[FRAME..mixed.len() - FRAME]);
        let after = residual(&clean[FRAME..clean.len() - FRAME]);
        let gain_db = 20.0 * (before / after.max(1e-9)).log10();
        assert!(gain_db > 10.0, "noise down by {gain_db:.1} dB only");
        let tone_before = tone_power(&mixed[FRAME..mixed.len() - FRAME]);
        let tone_after = tone_power(&clean[FRAME..clean.len() - FRAME]);
        assert!(
            (tone_after / tone_before) > 0.8,
            "the tone survives: {tone_before} → {tone_after}"
        );
    }
}
