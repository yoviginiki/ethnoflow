//! Incremental 32-bit float WAV writer for recordings.
//!
//! Runs on the recorder's drain thread (never on the audio callback). The WAV header is
//! rewritten every [`CHECKPOINT_SECONDS`] so that a crash or power loss leaves a file
//! that is valid up to the last checkpoint; [`crate::wav::repair_wav_header`] recovers
//! the samples written after it.

use std::fs::{File, OpenOptions};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use hound::{SampleFormat, WavSpec, WavWriter};

use crate::error::{AudioError, Result};

/// Header checkpoint interval.
pub const CHECKPOINT_SECONDS: f64 = 1.0;
/// RIFF sizes are u32. Stay well below 4 GiB including the header.
const MAX_DATA_BYTES: u64 = 0xFFF0_0000;

pub struct TakeWriter {
    writer: WavWriter<BufWriter<File>>,
    path: PathBuf,
    sample_rate: u32,
    channels: u16,
    samples_written: u64,
    samples_since_checkpoint: u64,
    checkpoint_every: u64,
    max_samples: u64,
}

impl TakeWriter {
    /// Create a new float32 WAV at `path`. Fails if the file already exists: originals
    /// are immutable and are never overwritten.
    pub fn create(path: &Path, sample_rate: u32, channels: u16) -> Result<Self> {
        if sample_rate == 0 || channels == 0 {
            return Err(AudioError::InvalidArgument(
                "sample_rate and channels must be > 0".into(),
            ));
        }
        let file = OpenOptions::new().write(true).create_new(true).open(path)?;
        let spec = WavSpec {
            channels,
            sample_rate,
            bits_per_sample: 32,
            sample_format: SampleFormat::Float,
        };
        let mut writer = WavWriter::new(BufWriter::new(file), spec)?;
        // Make the empty file a valid WAV immediately.
        writer.flush()?;
        let ch = channels as u64;
        Ok(Self {
            writer,
            path: path.to_path_buf(),
            sample_rate,
            channels,
            samples_written: 0,
            samples_since_checkpoint: 0,
            checkpoint_every: (CHECKPOINT_SECONDS * sample_rate as f64) as u64 * ch,
            max_samples: MAX_DATA_BYTES / 4 / ch * ch,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn frames_written(&self) -> u64 {
        self.samples_written / self.channels as u64
    }

    /// Longest take this writer can hold, in seconds (WAV 4 GiB limit).
    pub fn max_seconds(&self) -> f64 {
        (self.max_samples / self.channels as u64) as f64 / self.sample_rate as f64
    }

    /// Append interleaved samples. Returns how many were accepted; fewer than
    /// `samples.len()` means the size limit was reached.
    pub fn write(&mut self, samples: &[f32]) -> Result<usize> {
        let room = self.max_samples.saturating_sub(self.samples_written);
        let n = samples.len().min(room as usize);
        for &s in &samples[..n] {
            self.writer.write_sample(s)?;
        }
        self.samples_written += n as u64;
        self.samples_since_checkpoint += n as u64;
        // Checkpoint only on a frame boundary; otherwise the header would describe a
        // partial frame and hound reports UnfinishedSample.
        if self.samples_since_checkpoint >= self.checkpoint_every
            && self.samples_written.is_multiple_of(self.channels as u64)
        {
            self.writer.flush()?;
            self.samples_since_checkpoint = 0;
        }
        Ok(n)
    }

    /// Write the final header and close the file. A trailing partial frame (possible
    /// only if the host delivered an odd sample count) is padded with silence.
    pub fn finish(mut self) -> Result<u64> {
        let ch = self.channels as u64;
        while !self.samples_written.is_multiple_of(ch) {
            self.writer.write_sample(0.0_f32)?;
            self.samples_written += 1;
        }
        let frames = self.samples_written / ch;
        self.writer.finalize()?;
        Ok(frames)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav::{read_wav, repair_wav_header, wav_info};
    use crate::AudioBuffer;

    #[test]
    fn written_take_reads_back_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.wav");
        let src = AudioBuffer::sine(48_000, 330.0, 2.5, 0.7);
        let mut w = TakeWriter::create(&p, 48_000, 1).unwrap();
        // Uneven chunk sizes, as a drain thread would deliver.
        for chunk in src.samples.chunks(997) {
            assert_eq!(w.write(chunk).unwrap(), chunk.len());
        }
        assert_eq!(w.finish().unwrap(), src.frames() as u64);
        let back = read_wav(&p).unwrap();
        assert_eq!(back, src);
    }

    #[test]
    fn existing_file_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("orig.wav");
        std::fs::write(&p, b"original bytes").unwrap();
        assert!(TakeWriter::create(&p, 48_000, 2).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), b"original bytes");
    }

    #[test]
    fn file_is_valid_at_every_checkpoint() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("cp.wav");
        let mut w = TakeWriter::create(&p, 8_000, 2).unwrap();
        assert_eq!(
            wav_info(&p).unwrap().frames,
            0,
            "empty take must be a valid WAV"
        );
        let second = vec![0.25_f32; 8_000 * 2];
        w.write(&second).unwrap();
        w.write(&second).unwrap();
        // Two checkpoints passed; a reader sees at least the first full second.
        assert!(wav_info(&p).unwrap().frames >= 8_000);
        w.finish().unwrap();
        assert_eq!(wav_info(&p).unwrap().frames, 16_000);
    }

    /// Simulates the on-disk state after a crash: samples reached the disk but the
    /// header still describes the last checkpoint, and a torn partial frame trails the
    /// data. Repair must recover every whole frame, bit-exact.
    #[test]
    fn crash_mid_recording_is_recoverable() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("crash.wav");
        let left = AudioBuffer::sine(16_000, 440.0, 3.3, 0.5);
        let src = AudioBuffer::new(
            16_000,
            2,
            left.samples.iter().flat_map(|&s| [s, -s]).collect(),
        );
        let mut w = TakeWriter::create(&p, 16_000, 2).unwrap();
        for chunk in src.samples.chunks(1600) {
            w.write(chunk).unwrap();
        }
        w.finish().unwrap();

        // Stale header: claim only the first second (as at the first checkpoint).
        let mut bytes = std::fs::read(&p).unwrap();
        let data_pos = bytes.windows(4).position(|c| c == b"data").unwrap();
        let stale = 16_000u32 * 2 * 4;
        bytes[data_pos + 4..data_pos + 8].copy_from_slice(&stale.to_le_bytes());
        let riff = (data_pos as u32 + 8 + stale) - 8;
        bytes[4..8].copy_from_slice(&riff.to_le_bytes());
        bytes.extend_from_slice(&[0xAB, 0xCD, 0xEF]); // torn frame
        std::fs::write(&p, &bytes).unwrap();
        assert_eq!(wav_info(&p).unwrap().frames, 16_000);

        let repaired = repair_wav_header(&p).unwrap();
        assert_eq!(repaired, Some(src.frames() as u64));
        let after = read_wav(&p).unwrap();
        assert_eq!(after, src);
        // Repairing an intact file is a no-op.
        assert_eq!(repair_wav_header(&p).unwrap(), None);
    }

    #[test]
    fn size_limit_is_enforced() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("lim.wav");
        let mut w = TakeWriter::create(&p, 48_000, 2).unwrap();
        w.max_samples = 10;
        assert_eq!(w.write(&[0.1; 8]).unwrap(), 8);
        assert_eq!(w.write(&[0.1; 8]).unwrap(), 2);
        assert_eq!(w.write(&[0.1; 8]).unwrap(), 0);
        assert_eq!(w.finish().unwrap(), 5);
    }

    #[test]
    fn max_seconds_reflects_the_wav_limit() {
        let dir = tempfile::tempdir().unwrap();
        let w = TakeWriter::create(&dir.path().join("m.wav"), 48_000, 2).unwrap();
        // ~4 GiB / (48 kHz × 2 ch × 4 B) ≈ 3.1 h
        let h = w.max_seconds() / 3600.0;
        assert!(h > 3.0 && h < 3.2, "{h}");
    }
}
