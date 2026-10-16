//! In-memory audio buffer: interleaved f32 samples in [-1.0, 1.0].

use serde::{Deserialize, Serialize};

/// Interleaved PCM audio held in memory as 32-bit float.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AudioBuffer {
    pub sample_rate: u32,
    pub channels: u16,
    /// Interleaved samples: frame 0 = samples[0..channels], frame 1 = samples[channels..2*channels], ...
    pub samples: Vec<f32>,
}

impl AudioBuffer {
    pub fn new(sample_rate: u32, channels: u16, samples: Vec<f32>) -> Self {
        assert!(channels > 0, "channels must be > 0");
        Self {
            sample_rate,
            channels,
            samples,
        }
    }

    pub fn empty(sample_rate: u32, channels: u16) -> Self {
        Self::new(sample_rate, channels, Vec::new())
    }

    /// Number of frames (samples per channel).
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels as usize
    }

    pub fn duration_seconds(&self) -> f64 {
        if self.sample_rate == 0 {
            return 0.0;
        }
        self.frames() as f64 / self.sample_rate as f64
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Down-mix to mono by averaging channels. Returns a new buffer with channels = 1.
    pub fn to_mono(&self) -> AudioBuffer {
        if self.channels == 1 {
            return self.clone();
        }
        let ch = self.channels as usize;
        let mut out = Vec::with_capacity(self.frames());
        for frame in self.samples.chunks_exact(ch) {
            let sum: f32 = frame.iter().sum();
            out.push(sum / ch as f32);
        }
        AudioBuffer::new(self.sample_rate, 1, out)
    }

    /// Absolute peak sample value (0.0 for empty).
    pub fn peak(&self) -> f32 {
        self.samples
            .iter()
            .fold(0.0_f32, |acc, &s| acc.max(s.abs()))
    }

    /// Root mean square over all samples (0.0 for empty).
    pub fn rms(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum_sq: f64 = self.samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
        (sum_sq / self.samples.len() as f64).sqrt() as f32
    }

    /// Count of samples at or beyond full scale (|s| >= 1.0). Useful for clipping detection.
    pub fn clipped_sample_count(&self) -> usize {
        self.samples.iter().filter(|&&s| s.abs() >= 1.0).count()
    }

    /// Returns a copy of the frame range [start_frame, end_frame).
    pub fn slice_frames(&self, start_frame: usize, end_frame: usize) -> AudioBuffer {
        let ch = self.channels as usize;
        let start = start_frame.min(self.frames()) * ch;
        let end = end_frame.min(self.frames()) * ch;
        let end = end.max(start);
        AudioBuffer::new(
            self.sample_rate,
            self.channels,
            self.samples[start..end].to_vec(),
        )
    }

    /// Generate a mono sine tone. `amplitude` in [0, 1].
    pub fn sine(sample_rate: u32, freq_hz: f32, seconds: f32, amplitude: f32) -> AudioBuffer {
        let n = (sample_rate as f32 * seconds).round() as usize;
        let samples = (0..n)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                amplitude * (2.0 * std::f32::consts::PI * freq_hz * t).sin()
            })
            .collect();
        AudioBuffer::new(sample_rate, 1, samples)
    }

    /// Generate digital silence.
    pub fn silence(sample_rate: u32, channels: u16, seconds: f32) -> AudioBuffer {
        let n = (sample_rate as f32 * seconds).round() as usize * channels as usize;
        AudioBuffer::new(sample_rate, channels, vec![0.0; n])
    }
}

/// Convert a linear amplitude to dBFS. Returns -inf style floor (-120 dB) for zero.
pub fn linear_to_dbfs(x: f32) -> f32 {
    if x <= 0.0 {
        -120.0
    } else {
        (20.0 * x.log10()).max(-120.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_duration() {
        let b = AudioBuffer::new(48_000, 2, vec![0.0; 96_000]);
        assert_eq!(b.frames(), 48_000);
        assert!((b.duration_seconds() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn sine_has_expected_peak_and_rms() {
        let b = AudioBuffer::sine(44_100, 440.0, 1.0, 0.5);
        assert_eq!(b.frames(), 44_100);
        assert!((b.peak() - 0.5).abs() < 1e-3, "peak {}", b.peak());
        // RMS of a sine = A / sqrt(2)
        let expected_rms = 0.5 / 2.0_f32.sqrt();
        assert!((b.rms() - expected_rms).abs() < 1e-3, "rms {}", b.rms());
        assert_eq!(b.clipped_sample_count(), 0);
    }

    #[test]
    fn silence_is_silent() {
        let b = AudioBuffer::silence(48_000, 1, 0.5);
        assert_eq!(b.frames(), 24_000);
        assert_eq!(b.peak(), 0.0);
        assert_eq!(b.rms(), 0.0);
        assert_eq!(linear_to_dbfs(b.peak()), -120.0);
    }

    #[test]
    fn clipping_is_detected() {
        let b = AudioBuffer::new(48_000, 1, vec![0.2, 1.0, -1.0, 1.5, -0.99]);
        assert_eq!(b.clipped_sample_count(), 3);
    }

    #[test]
    fn stereo_to_mono_averages() {
        let b = AudioBuffer::new(48_000, 2, vec![1.0, 0.0, 0.5, 0.5, -1.0, 1.0]);
        let m = b.to_mono();
        assert_eq!(m.channels, 1);
        assert_eq!(m.samples, vec![0.5, 0.5, 0.0]);
    }

    #[test]
    fn slice_frames_respects_channels_and_bounds() {
        let b = AudioBuffer::new(8_000, 2, (0..12).map(|i| i as f32).collect());
        let s = b.slice_frames(1, 3);
        assert_eq!(s.samples, vec![2.0, 3.0, 4.0, 5.0]);
        let s2 = b.slice_frames(5, 100);
        assert_eq!(s2.samples, vec![10.0, 11.0]);
        let s3 = b.slice_frames(10, 2);
        assert!(s3.is_empty());
    }

    #[test]
    fn dbfs_conversion() {
        assert!((linear_to_dbfs(1.0) - 0.0).abs() < 1e-6);
        assert!((linear_to_dbfs(0.5) + 6.0206).abs() < 1e-3);
    }
}
