//! Decode compressed and PCM audio files (MP3, FLAC, Ogg Vorbis, WAV) to f32.
//!
//! Decode only: there is no encoder here and none is planned (ADR-0004, ADR-0017).
//! Imports keep the original file bytes in the project; this module produces the
//! in-memory buffer used for playback, peaks and analysis.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::buffer::AudioBuffer;
use crate::error::{AudioError, Result};

/// Audio file extensions the importer accepts (lower case). MIDI (`mid`) is handled
/// separately by the project layer (parsed, not decoded).
pub const IMPORT_EXTENSIONS: &[&str] = &["wav", "mp3", "flac", "ogg"];

fn sym(e: SymError) -> AudioError {
    match e {
        SymError::IoError(io) => AudioError::Io(io),
        SymError::Unsupported(what) => AudioError::UnsupportedFormat(what.to_string()),
        other => AudioError::Wav(format!("decode: {other}")),
    }
}

/// Decode the first audio track of `path` to an interleaved f32 buffer.
/// Corrupt packets are skipped (and counted in the log); a file with no decodable
/// audio is an error.
pub fn decode_file(path: &Path) -> Result<AudioBuffer> {
    let file = File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(sym)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| AudioError::UnsupportedFormat("no audio track".into()))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| AudioError::UnsupportedFormat("no audio codec parameters".into()))?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())
        .map_err(sym)?;

    let mut out: Vec<f32> = Vec::new();
    let mut tmp: Vec<f32> = Vec::new();
    let mut spec: Option<(u32, u16)> = None;
    let mut bad_packets = 0usize;
    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            // Some containers end with an unexpected EOF instead of a clean end.
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(sym(e)),
        };
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let s = buf.spec();
                let this = (s.rate(), s.channels().count() as u16);
                match spec {
                    None => spec = Some(this),
                    Some(prev) if prev != this => {
                        return Err(AudioError::UnsupportedFormat(format!(
                            "stream changes format mid-file ({prev:?} → {this:?})"
                        )))
                    }
                    _ => {}
                }
                tmp.resize(buf.samples_interleaved(), 0.0);
                buf.copy_to_slice_interleaved(&mut tmp);
                out.extend_from_slice(&tmp);
            }
            Err(SymError::DecodeError(_)) => bad_packets += 1,
            Err(e) => return Err(sym(e)),
        }
    }
    if bad_packets > 0 {
        log::warn!(
            "{}: skipped {bad_packets} corrupt packet(s)",
            path.display()
        );
    }
    let (rate, channels) =
        spec.ok_or_else(|| AudioError::UnsupportedFormat("no decodable audio".into()))?;
    if rate == 0 || channels == 0 {
        return Err(AudioError::UnsupportedFormat(
            "invalid rate/channels".into(),
        ));
    }
    let whole = out.len() / channels as usize * channels as usize;
    out.truncate(whole);
    Ok(AudioBuffer::new(rate, channels, out))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav::{write_wav, WavFormat};

    #[test]
    fn decodes_wav_through_symphonia_identically_to_hound() {
        let dir = tempfile::tempdir().unwrap();
        let src = AudioBuffer::new(
            44_100,
            2,
            AudioBuffer::sine(44_100, 330.0, 0.5, 0.5)
                .samples
                .iter()
                .flat_map(|&s| [s, -s * 0.5])
                .collect(),
        );
        for fmt in [WavFormat::Float32, WavFormat::Pcm24, WavFormat::Pcm16] {
            let p = dir.path().join(format!("{fmt:?}.wav"));
            write_wav(&p, &src, fmt).unwrap();
            let a = decode_file(&p).unwrap();
            let b = crate::wav::read_wav(&p).unwrap();
            assert_eq!(
                (a.sample_rate, a.channels, a.frames()),
                (44_100, 2, src.frames())
            );
            let worst = a
                .samples
                .iter()
                .zip(&b.samples)
                .fold(0.0_f32, |m, (x, y)| m.max((x - y).abs()));
            assert!(worst < 1e-6, "{fmt:?}: {worst}");
        }
    }

    #[test]
    fn garbage_and_missing_files_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.mp3");
        std::fs::write(&p, vec![0x5Au8; 4096]).unwrap();
        assert!(decode_file(&p).is_err());
        assert!(decode_file(&dir.path().join("missing.flac")).is_err());
    }

    /// The owner's reference recording, used locally only (never committed or uploaded).
    /// Path from ETHNOFLOW_RE_MP3, else %USERPROFILE%\Downloads\re.mp3; skips if absent.
    #[test]
    fn decodes_owner_reference_mp3_if_present() {
        let path = std::env::var_os("ETHNOFLOW_RE_MP3")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .map(|h| std::path::PathBuf::from(h).join("Downloads").join("re.mp3"))
            });
        let Some(path) = path.filter(|p| p.is_file()) else {
            eprintln!("re.mp3 not present; skipping");
            return;
        };
        let buf = decode_file(&path).unwrap();
        assert!(buf.frames() > 0);
        assert!(buf.peak() > 0.0 && buf.peak() <= 1.5);
        eprintln!(
            "re.mp3: {} Hz x{}, {:.2} s, peak {:.1} dBFS",
            buf.sample_rate,
            buf.channels,
            buf.duration_seconds(),
            crate::linear_to_dbfs(buf.peak())
        );
    }
}
