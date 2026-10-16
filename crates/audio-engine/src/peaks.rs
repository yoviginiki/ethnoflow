//! Waveform overview data: per-bin min/max over a mono mix, for drawing.
//! Raw audio frames never cross into the UI layer; only this reduced data does.

use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WaveformPeaks {
    pub sample_rate: u32,
    pub frames: usize,
    pub duration_seconds: f64,
    /// Number of source frames represented by each bin (last bin may hold fewer).
    pub frames_per_bin: usize,
    /// min values per bin, in [-1, 1]
    pub min: Vec<f32>,
    /// max values per bin, in [-1, 1]
    pub max: Vec<f32>,
}

/// Reduce `buffer` to at most `bins` (min, max) pairs over the mono mix.
/// If the buffer has fewer frames than `bins`, one bin per frame is produced.
pub fn compute_peaks(buffer: &AudioBuffer, bins: usize) -> WaveformPeaks {
    let bins = bins.max(1);
    let mono = buffer.to_mono();
    let frames = mono.frames();
    if frames == 0 {
        return WaveformPeaks {
            sample_rate: buffer.sample_rate,
            frames: 0,
            duration_seconds: 0.0,
            frames_per_bin: 1,
            min: Vec::new(),
            max: Vec::new(),
        };
    }
    let frames_per_bin = frames.div_ceil(bins).max(1);
    let mut min = Vec::with_capacity(bins);
    let mut max = Vec::with_capacity(bins);
    for chunk in mono.samples.chunks(frames_per_bin) {
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for &s in chunk {
            if s < lo {
                lo = s;
            }
            if s > hi {
                hi = s;
            }
        }
        min.push(lo.clamp(-1.0, 1.0));
        max.push(hi.clamp(-1.0, 1.0));
    }
    WaveformPeaks {
        sample_rate: buffer.sample_rate,
        frames,
        duration_seconds: buffer.duration_seconds(),
        frames_per_bin,
        min,
        max,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_gives_no_bins() {
        let p = compute_peaks(&AudioBuffer::empty(48_000, 1), 100);
        assert!(p.min.is_empty() && p.max.is_empty());
        assert_eq!(p.frames, 0);
    }

    #[test]
    fn bin_count_never_exceeds_request() {
        let b = AudioBuffer::sine(48_000, 440.0, 1.0, 1.0);
        for bins in [1usize, 7, 100, 1000, 48_000, 100_000] {
            let p = compute_peaks(&b, bins);
            assert!(p.min.len() <= bins, "bins={bins} got {}", p.min.len());
            assert_eq!(p.min.len(), p.max.len());
            assert!(!p.min.is_empty());
        }
    }

    #[test]
    fn sine_bins_reach_amplitude() {
        let b = AudioBuffer::sine(48_000, 440.0, 1.0, 0.7);
        // 100 bins → 480 frames per bin, more than one full period (109 frames) each.
        let p = compute_peaks(&b, 100);
        assert_eq!(p.frames_per_bin, 480);
        for (lo, hi) in p.min.iter().zip(p.max.iter()) {
            assert!(*hi > 0.69 && *hi <= 0.7, "hi {hi}");
            assert!(*lo < -0.69 && *lo >= -0.7, "lo {lo}");
        }
    }

    #[test]
    fn silence_bins_are_zero() {
        let p = compute_peaks(&AudioBuffer::silence(44_100, 2, 0.2), 50);
        assert!(p.min.iter().all(|&v| v == 0.0));
        assert!(p.max.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn exact_small_input() {
        let b = AudioBuffer::new(8_000, 1, vec![0.1, -0.5, 0.3, 0.9, -0.2, 0.0]);
        let p = compute_peaks(&b, 3);
        assert_eq!(p.frames_per_bin, 2);
        assert_eq!(p.min, vec![-0.5, 0.3, -0.2]);
        assert_eq!(p.max, vec![0.1, 0.9, 0.0]);
    }
}
