//! WAV read/write. Writing supports 32-bit float (default) and 24-bit / 16-bit PCM.
//! Reading accepts 8/16/24/32-bit PCM and 32-bit float and always returns f32.

use std::path::Path;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;
use crate::error::{AudioError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WavFormat {
    /// IEEE float, 32 bit. Lossless for the internal f32 representation.
    Float32,
    /// Integer PCM, 24 bit. The common DAW interchange format.
    Pcm24,
    /// Integer PCM, 16 bit.
    Pcm16,
}

impl Default for WavFormat {
    fn default() -> Self {
        WavFormat::Float32
    }
}

fn spec_for(buffer: &AudioBuffer, format: WavFormat) -> WavSpec {
    let (bits, sample_format) = match format {
        WavFormat::Float32 => (32, SampleFormat::Float),
        WavFormat::Pcm24 => (24, SampleFormat::Int),
        WavFormat::Pcm16 => (16, SampleFormat::Int),
    };
    WavSpec {
        channels: buffer.channels,
        sample_rate: buffer.sample_rate,
        bits_per_sample: bits,
        sample_format,
    }
}

/// Write `buffer` to `path`. Existing files are overwritten.
pub fn write_wav<P: AsRef<Path>>(path: P, buffer: &AudioBuffer, format: WavFormat) -> Result<()> {
    if buffer.sample_rate == 0 {
        return Err(AudioError::InvalidArgument("sample_rate must be > 0".into()));
    }
    let spec = spec_for(buffer, format);
    let mut writer = WavWriter::create(path, spec)?;
    match format {
        WavFormat::Float32 => {
            for &s in &buffer.samples {
                writer.write_sample(s)?;
            }
        }
        WavFormat::Pcm24 => {
            // Clamp then scale to the 24-bit signed range; hound packs i32 into 24 bits.
            let max = 8_388_607.0_f32; // 2^23 - 1
            for &s in &buffer.samples {
                let v = (s.clamp(-1.0, 1.0) * max).round() as i32;
                writer.write_sample(v)?;
            }
        }
        WavFormat::Pcm16 => {
            let max = 32_767.0_f32;
            for &s in &buffer.samples {
                let v = (s.clamp(-1.0, 1.0) * max).round() as i16;
                writer.write_sample(v)?;
            }
        }
    }
    writer.finalize()?;
    Ok(())
}

/// Read a WAV file into an f32 `AudioBuffer`.
pub fn read_wav<P: AsRef<Path>>(path: P) -> Result<AudioBuffer> {
    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();
    let samples: Vec<f32> = match (spec.sample_format, spec.bits_per_sample) {
        (SampleFormat::Float, 32) => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()?,
        (SampleFormat::Int, bits) if bits <= 32 => {
            let scale = if bits == 8 {
                // hound reads 8-bit as i8 via the i32 path? It yields i8 range [-128,127].
                128.0_f32
            } else {
                ((1u64 << (bits - 1)) as f32).max(1.0)
            };
            reader
                .samples::<i32>()
                .map(|r| r.map(|v| v as f32 / scale))
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
        (fmt, bits) => {
            return Err(AudioError::UnsupportedFormat(format!("{fmt:?} {bits}-bit")));
        }
    };
    Ok(AudioBuffer::new(spec.sample_rate, spec.channels, samples))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavInfo {
    pub sample_rate: u32,
    pub channels: u16,
    pub bits_per_sample: u16,
    pub is_float: bool,
    pub frames: u32,
    pub duration_seconds: f64,
}

/// Read only the header of a WAV file.
pub fn wav_info<P: AsRef<Path>>(path: P) -> Result<WavInfo> {
    let reader = WavReader::open(path)?;
    let spec = reader.spec();
    let frames = reader.duration();
    Ok(WavInfo {
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        bits_per_sample: spec.bits_per_sample,
        is_float: spec.sample_format == SampleFormat::Float,
        frames,
        duration_seconds: frames as f64 / spec.sample_rate.max(1) as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .fold(0.0_f32, |m, (x, y)| m.max((x - y).abs()))
    }

    #[test]
    fn float32_roundtrip_is_exact() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f32.wav");
        let src = AudioBuffer::sine(48_000, 440.0, 0.25, 0.8);
        write_wav(&p, &src, WavFormat::Float32).unwrap();
        let back = read_wav(&p).unwrap();
        assert_eq!(back.sample_rate, 48_000);
        assert_eq!(back.channels, 1);
        assert_eq!(back.samples.len(), src.samples.len());
        assert_eq!(max_abs_diff(&src.samples, &back.samples), 0.0);
    }

    #[test]
    fn pcm24_roundtrip_within_quantisation() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("p24.wav");
        let mut src = AudioBuffer::sine(44_100, 220.0, 0.1, 0.9);
        src.channels = 1;
        write_wav(&p, &src, WavFormat::Pcm24).unwrap();
        let info = wav_info(&p).unwrap();
        assert_eq!(info.bits_per_sample, 24);
        assert!(!info.is_float);
        let back = read_wav(&p).unwrap();
        assert_eq!(back.samples.len(), src.samples.len());
        // one LSB at 24 bit ≈ 1.2e-7
        assert!(max_abs_diff(&src.samples, &back.samples) < 2.0e-7);
    }

    #[test]
    fn pcm16_roundtrip_within_quantisation() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("p16.wav");
        let src = AudioBuffer::sine(22_050, 1000.0, 0.05, 0.5);
        write_wav(&p, &src, WavFormat::Pcm16).unwrap();
        let back = read_wav(&p).unwrap();
        assert!(max_abs_diff(&src.samples, &back.samples) < 4.0e-5);
    }

    #[test]
    fn stereo_interleaving_is_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("st.wav");
        let src = AudioBuffer::new(8_000, 2, vec![0.1, -0.1, 0.2, -0.2, 0.3, -0.3]);
        write_wav(&p, &src, WavFormat::Float32).unwrap();
        let back = read_wav(&p).unwrap();
        assert_eq!(back.channels, 2);
        assert_eq!(back.samples, src.samples);
        let info = wav_info(&p).unwrap();
        assert_eq!(info.frames, 3);
    }

    #[test]
    fn clipping_input_is_clamped_for_pcm() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("clip.wav");
        let src = AudioBuffer::new(8_000, 1, vec![2.0, -3.0, 0.0]);
        write_wav(&p, &src, WavFormat::Pcm16).unwrap();
        let back = read_wav(&p).unwrap();
        assert!(back.samples[0] > 0.999 && back.samples[0] <= 1.0);
        assert!(back.samples[1] <= -0.999);
        assert_eq!(back.samples[2], 0.0);
    }

    #[test]
    fn corrupt_file_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.wav");
        std::fs::write(&p, b"RIFF this is not a wav file at all").unwrap();
        assert!(read_wav(&p).is_err());
        assert!(wav_info(&p).is_err());
        assert!(read_wav(dir.path().join("missing.wav")).is_err());
    }

    #[test]
    fn zero_sample_rate_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("zero.wav");
        let src = AudioBuffer::new(0, 1, vec![0.0]);
        assert!(write_wav(&p, &src, WavFormat::Float32).is_err());
    }
}
