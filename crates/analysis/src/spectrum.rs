//! Spectrum and spectrogram for the editor's analysis views (Audacity programme, stage
//! A11, 2026-11-01): Plot-Spectrum-like averaged magnitude in log-spaced bands, and a
//! columns × rows dB grid (log frequency rows) for the spectrogram view. Mono mix of the
//! channels, Hann windows, dBFS relative to a full-scale sine.

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumBand {
    pub hz: f32,
    pub db: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spectrogram {
    pub cols: usize,
    pub rows: usize,
    pub min_hz: f32,
    pub max_hz: f32,
    /// Row-major, rows × cols, dBFS (−100 floor); row 0 = the lowest frequency.
    pub db: Vec<f32>,
}

fn mono(samples: &[f32], channels: usize) -> Vec<f32> {
    let ch = channels.max(1);
    samples
        .chunks(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect()
}

fn hann(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / n as f32).cos())
        .collect()
}

/// Magnitudes (linear, full-scale sine = 1) of one frame of `x` starting at `start`.
fn frame_mags(
    x: &[f32],
    start: usize,
    n: usize,
    win: &[f32],
    planner: &mut FftPlanner<f32>,
) -> Vec<f32> {
    let fft = planner.plan_fft_forward(n);
    let mut buf: Vec<Complex<f32>> = (0..n)
        .map(|i| Complex::new(x.get(start + i).copied().unwrap_or(0.0) * win[i], 0.0))
        .collect();
    fft.process(&mut buf);
    // Hann window sum = n/2; a full-scale sine gives a peak of n/4 → normalise so it reads 1.
    let scale = 4.0 / n as f32;
    buf[..n / 2 + 1].iter().map(|c| c.norm() * scale).collect()
}

fn to_db(m: f32) -> f32 {
    (20.0 * m.max(1e-5).log10()).max(-100.0)
}

/// Log-spaced band edges between `lo` and `hi` Hz.
fn edges(lo: f32, hi: f32, bands: usize) -> Vec<f32> {
    (0..=bands)
        .map(|i| lo * (hi / lo).powf(i as f32 / bands as f32))
        .collect()
}

/// Averaged spectrum in `bands` log-spaced bands from 20 Hz to the Nyquist (dBFS).
pub fn spectrum(
    samples: &[f32],
    channels: usize,
    sample_rate: u32,
    bands: usize,
) -> Vec<SpectrumBand> {
    let x = mono(samples, channels);
    let n = 4096.min(x.len().next_power_of_two().max(256));
    let win = hann(n);
    let mut planner = FftPlanner::<f32>::new();
    let hop = n / 2;
    let frames = if x.len() <= n {
        1
    } else {
        (x.len() - n) / hop + 1
    };
    let mut acc = vec![0.0f32; n / 2 + 1];
    for f in 0..frames {
        for (k, m) in frame_mags(&x, f * hop, n, &win, &mut planner)
            .iter()
            .enumerate()
        {
            acc[k] += m * m;
        }
    }
    let bands = bands.clamp(4, 512);
    let nyq = sample_rate as f32 / 2.0;
    let e = edges(20.0, nyq, bands);
    let bin_hz = sample_rate as f32 / n as f32;
    (0..bands)
        .map(|b| {
            let (lo, hi) = (e[b], e[b + 1]);
            let k0 = (lo / bin_hz).floor() as usize;
            let k1 = ((hi / bin_hz).ceil() as usize).max(k0 + 1).min(acc.len());
            let power: f32 = acc[k0.min(acc.len() - 1)..k1].iter().sum::<f32>() / frames as f32;
            SpectrumBand {
                hz: (lo * hi).sqrt(),
                db: to_db(power.sqrt()),
            }
        })
        .collect()
}

/// A `cols` × `rows` spectrogram (log frequency 40 Hz … Nyquist), dBFS per cell.
pub fn spectrogram(
    samples: &[f32],
    channels: usize,
    sample_rate: u32,
    cols: usize,
    rows: usize,
) -> Spectrogram {
    let x = mono(samples, channels);
    let cols = cols.clamp(1, 4096);
    let rows = rows.clamp(1, 512);
    let n = 1024usize;
    let win = hann(n);
    let mut planner = FftPlanner::<f32>::new();
    let nyq = sample_rate as f32 / 2.0;
    let e = edges(40.0, nyq, rows);
    let bin_hz = sample_rate as f32 / n as f32;
    let mut db = vec![-100.0f32; rows * cols];
    let span = x.len().saturating_sub(n).max(1) as f32;
    for c in 0..cols {
        let start = ((c as f32 + 0.5) / cols as f32 * span) as usize;
        let mags = frame_mags(&x, start, n, &win, &mut planner);
        for r in 0..rows {
            let k0 = (e[r] / bin_hz).floor() as usize;
            let k1 = ((e[r + 1] / bin_hz).ceil() as usize)
                .max(k0 + 1)
                .min(mags.len());
            let m = mags[k0.min(mags.len() - 1)..k1]
                .iter()
                .fold(0.0f32, |a, v| a.max(*v));
            db[r * cols + c] = to_db(m);
        }
    }
    Spectrogram {
        cols,
        rows,
        min_hz: 40.0,
        max_hz: nyq,
        db,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, seconds: f32, sr: u32) -> Vec<f32> {
        (0..(seconds * sr as f32) as usize)
            .map(|i| (i as f32 * hz / sr as f32 * std::f32::consts::TAU).sin())
            .collect()
    }

    #[test]
    fn the_spectrum_peaks_in_the_band_of_the_tone() {
        let sr = 48_000;
        let s = spectrum(&tone(1000.0, 1.0, sr), 1, sr, 64);
        let best = s
            .iter()
            .max_by(|a, b| a.db.partial_cmp(&b.db).unwrap())
            .unwrap();
        assert!(
            (best.hz / 1000.0).log2().abs() < 0.2,
            "peak band at {} Hz",
            best.hz
        );
        assert!(
            best.db > -3.0 && best.db < 3.0,
            "full-scale sine reads about 0 dBFS: {}",
            best.db
        );
        let quiet = s
            .iter()
            .filter(|b| b.hz < 300.0 || b.hz > 4000.0)
            .map(|b| b.db)
            .fold(-100.0f32, f32::max);
        assert!(quiet < best.db - 40.0, "elsewhere far below: {quiet}");
    }

    #[test]
    fn the_spectrogram_follows_a_change_of_pitch() {
        let sr = 48_000;
        let mut x = tone(440.0, 1.0, sr);
        x.extend(tone(3000.0, 1.0, sr));
        let g = spectrogram(&x, 1, sr, 20, 48);
        let argmax = |c: usize| {
            (0..g.rows)
                .max_by(|&a, &b| {
                    g.db[a * g.cols + c]
                        .partial_cmp(&g.db[b * g.cols + c])
                        .unwrap()
                })
                .unwrap()
        };
        let (first, last) = (argmax(1), argmax(18));
        assert!(last > first + 5, "rows {first} → {last}");
    }
}
