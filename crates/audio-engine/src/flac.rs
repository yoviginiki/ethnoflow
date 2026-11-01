//! FLAC export (home-studio item 8, 2026-11-01): lossless, 16 or 24 bit, through the
//! pure-Rust `flacenc` encoder (ADR-0023). The internal f32 audio is clipped to ±1 and
//! quantised with rounding; a FLAC file reads back bit-exact to the PCM WAV of the same
//! depth.

use std::path::Path;

use flacenc::component::BitRepr;
use flacenc::error::Verify;

use crate::buffer::AudioBuffer;
use crate::error::{AudioError, Result};

/// Encode `buffer` to FLAC bytes at `bits` (16 or 24) per sample.
pub fn encode_flac(buffer: &AudioBuffer, bits: u32) -> Result<Vec<u8>> {
    if buffer.sample_rate == 0 || buffer.channels == 0 {
        return Err(AudioError::InvalidArgument("empty buffer".into()));
    }
    if bits != 16 && bits != 24 {
        return Err(AudioError::InvalidArgument("FLAC depth 16 or 24".into()));
    }
    if buffer.frames() == 0 {
        return Err(AudioError::InvalidArgument("nothing to write".into()));
    }
    let scale = ((1u32 << (bits - 1)) - 1) as f32;
    let samples: Vec<i32> = buffer
        .samples
        .iter()
        .map(|v| (v.clamp(-1.0, 1.0) * scale).round() as i32)
        .collect();
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| AudioError::InvalidArgument(format!("flac config: {e:?}")))?;
    let source = flacenc::source::MemSource::from_samples(
        &samples,
        buffer.channels as usize,
        bits as usize,
        buffer.sample_rate as usize,
    );
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| AudioError::InvalidArgument(format!("flac encode: {e:?}")))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|e| AudioError::InvalidArgument(format!("flac write: {e:?}")))?;
    Ok(sink.as_slice().to_vec())
}

/// Write `buffer` as a FLAC file. Existing files are overwritten.
pub fn write_flac<P: AsRef<Path>>(path: P, buffer: &AudioBuffer, bits: u32) -> Result<()> {
    let bytes = encode_flac(buffer, bits)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flac_file_starts_with_the_magic_and_is_smaller_than_the_pcm() {
        let buf = AudioBuffer::sine(48_000, 440.0, 2.0, 0.5);
        let bytes = encode_flac(&buf, 24).unwrap();
        assert_eq!(&bytes[..4], b"fLaC");
        let pcm_bytes = buf.samples.len() * 3;
        assert!(
            bytes.len() < pcm_bytes / 2,
            "{} bytes vs {} raw",
            bytes.len(),
            pcm_bytes
        );
        assert!(encode_flac(&buf, 20).is_err());
        assert!(encode_flac(&AudioBuffer::empty(48_000, 1), 16).is_err());
    }
}
