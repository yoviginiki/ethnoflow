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

/// Incremental waveform peaks for a take that is still being recorded.
///
/// Fed by the drain thread with interleaved samples in arbitrary chunk sizes (frames may
/// be split across calls). Stores fixed-width mono min/max bins (10 ms by default), so
/// memory grows by ~800 B per second of audio, and reduces them on demand.
#[derive(Debug, Clone)]
pub struct LivePeaks {
    sample_rate: u32,
    channels: u16,
    frames_per_bin: usize,
    min: Vec<f32>,
    max: Vec<f32>,
    frames: usize,
    // current partial frame
    ch_idx: u16,
    frame_sum: f32,
    // current partial bin
    bin_frames: usize,
    bin_lo: f32,
    bin_hi: f32,
}

impl LivePeaks {
    pub fn new(sample_rate: u32, channels: u16) -> Self {
        Self::with_bin(sample_rate, channels, (sample_rate as usize / 100).max(1))
    }

    pub fn with_bin(sample_rate: u32, channels: u16, frames_per_bin: usize) -> Self {
        assert!(channels > 0 && frames_per_bin > 0);
        Self {
            sample_rate,
            channels,
            frames_per_bin,
            min: Vec::new(),
            max: Vec::new(),
            frames: 0,
            ch_idx: 0,
            frame_sum: 0.0,
            bin_frames: 0,
            bin_lo: f32::INFINITY,
            bin_hi: f32::NEG_INFINITY,
        }
    }

    pub fn frames(&self) -> usize {
        self.frames
    }

    pub fn push(&mut self, interleaved: &[f32]) {
        let ch = self.channels;
        for &s in interleaved {
            self.frame_sum += s;
            self.ch_idx += 1;
            if self.ch_idx < ch {
                continue;
            }
            let v = self.frame_sum / ch as f32;
            self.ch_idx = 0;
            self.frame_sum = 0.0;
            self.frames += 1;
            self.bin_lo = self.bin_lo.min(v);
            self.bin_hi = self.bin_hi.max(v);
            self.bin_frames += 1;
            if self.bin_frames == self.frames_per_bin {
                self.min.push(self.bin_lo.clamp(-1.0, 1.0));
                self.max.push(self.bin_hi.clamp(-1.0, 1.0));
                self.bin_frames = 0;
                self.bin_lo = f32::INFINITY;
                self.bin_hi = f32::NEG_INFINITY;
            }
        }
    }

    /// Reduce everything seen so far to at most `bins` min/max pairs.
    pub fn snapshot(&self, bins: usize) -> WaveformPeaks {
        let bins = bins.max(1);
        let mut src_min = self.min.clone();
        let mut src_max = self.max.clone();
        if self.bin_frames > 0 {
            src_min.push(self.bin_lo.clamp(-1.0, 1.0));
            src_max.push(self.bin_hi.clamp(-1.0, 1.0));
        }
        let group = src_min.len().div_ceil(bins).max(1);
        let reduce = |v: &[f32], f: fn(f32, f32) -> f32, init: f32| -> Vec<f32> {
            v.chunks(group)
                .map(|c| c.iter().copied().fold(init, f))
                .collect()
        };
        WaveformPeaks {
            sample_rate: self.sample_rate,
            frames: self.frames,
            duration_seconds: self.frames as f64 / self.sample_rate.max(1) as f64,
            frames_per_bin: self.frames_per_bin * group,
            min: reduce(&src_min, f32::min, f32::INFINITY),
            max: reduce(&src_max, f32::max, f32::NEG_INFINITY),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_peaks_match_offline_peaks() {
        let left = AudioBuffer::sine(8_000, 50.0, 1.0, 0.8);
        let st = AudioBuffer::new(
            8_000,
            2,
            left.samples.iter().flat_map(|&s| [s, s * 0.5]).collect(),
        );
        let mut live = LivePeaks::with_bin(8_000, 2, 80);
        // Odd chunk size splits frames across pushes.
        for c in st.samples.chunks(333) {
            live.push(c);
        }
        assert_eq!(live.frames(), 8_000);
        let a = live.snapshot(100);
        let b = compute_peaks(&st, 100);
        assert_eq!(a.min.len(), 100);
        assert_eq!(a.frames_per_bin, b.frames_per_bin);
        for i in 0..100 {
            assert!((a.min[i] - b.min[i]).abs() < 1e-6);
            assert!((a.max[i] - b.max[i]).abs() < 1e-6);
        }
    }

    #[test]
    fn live_peaks_include_partial_bin_and_bound_output() {
        let mut live = LivePeaks::with_bin(1_000, 1, 10);
        live.push(&[0.5; 25]);
        let p = live.snapshot(1000);
        assert_eq!(p.min.len(), 3, "2 full bins + 1 partial");
        let p = live.snapshot(2);
        assert_eq!(p.min.len(), 2);
        assert!(LivePeaks::new(48_000, 2).snapshot(10).min.is_empty());
    }

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
