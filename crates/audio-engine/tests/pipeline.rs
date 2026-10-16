//! Integration tests for the Phase 0 vertical slice:
//! buffer → peaks → WAV export → WAV re-import → identical data.
//! The device-backed variant runs only where an input device exists.

use audio_engine::{compute_peaks, read_wav, write_wav, AudioBuffer, Recorder, WavFormat};
use std::time::Duration;

#[test]
fn synthetic_take_roundtrips_through_export_and_import() {
    let dir = tempfile::tempdir().unwrap();
    let take = AudioBuffer::sine(48_000, 220.0, 2.0, 0.6);

    let peaks = compute_peaks(&take, 1600);
    assert_eq!(peaks.min.len(), 1600);
    assert!(peaks.max.iter().all(|&v| v <= 0.6 + 1e-6));

    for fmt in [WavFormat::Float32, WavFormat::Pcm24, WavFormat::Pcm16] {
        let path = dir.path().join(format!("take-{fmt:?}.wav"));
        write_wav(&path, &take, fmt).unwrap();
        let back = read_wav(&path).unwrap();
        assert_eq!(back.sample_rate, take.sample_rate);
        assert_eq!(back.channels, take.channels);
        assert_eq!(
            back.frames(),
            take.frames(),
            "{fmt:?}: frame count must be exact"
        );
        let tol = match fmt {
            WavFormat::Float32 => 0.0,
            WavFormat::Pcm24 => 2e-7,
            WavFormat::Pcm16 => 4e-5,
        };
        let worst = take
            .samples
            .iter()
            .zip(&back.samples)
            .fold(0.0_f32, |m, (a, b)| m.max((a - b).abs()));
        assert!(worst <= tol, "{fmt:?}: max diff {worst} > {tol}");
        // Peaks of the re-imported file match the original's shape.
        let p2 = compute_peaks(&back, 1600);
        for (a, b) in peaks.max.iter().zip(&p2.max) {
            assert!((a - b).abs() <= tol.max(1e-6));
        }
    }
}

#[test]
fn stereo_take_keeps_channel_identity_through_export() {
    let dir = tempfile::tempdir().unwrap();
    let left = AudioBuffer::sine(44_100, 300.0, 0.5, 0.5);
    let right = AudioBuffer::sine(44_100, 700.0, 0.5, 0.25);
    let samples: Vec<f32> = left
        .samples
        .iter()
        .zip(&right.samples)
        .flat_map(|(&l, &r)| [l, r])
        .collect();
    let take = AudioBuffer::new(44_100, 2, samples);
    let path = dir.path().join("stereo.wav");
    write_wav(&path, &take, WavFormat::Pcm24).unwrap();
    let back = read_wav(&path).unwrap();
    assert_eq!(back.channels, 2);
    let l_peak = back
        .samples
        .iter()
        .step_by(2)
        .fold(0.0_f32, |m, &s| m.max(s.abs()));
    let r_peak = back
        .samples
        .iter()
        .skip(1)
        .step_by(2)
        .fold(0.0_f32, |m, &s| m.max(s.abs()));
    assert!((l_peak - 0.5).abs() < 1e-3);
    assert!((r_peak - 0.25).abs() < 1e-3);
}

/// Real device path (skipped with a message when no input device exists).
#[test]
fn device_record_export_import() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original.wav");
    let rec = match Recorder::start(None, &original) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skipping device test: {e}");
            return;
        }
    };
    std::thread::sleep(Duration::from_millis(200));
    let recorded = rec.stop().unwrap();
    let take = read_wav(&recorded.path).unwrap();
    assert!(!take.is_empty(), "device delivered no samples");
    assert_eq!(take.frames() as u64, recorded.frames);
    let path = dir.path().join("device.wav");
    write_wav(&path, &take, WavFormat::Float32).unwrap();
    let back = read_wav(&path).unwrap();
    assert_eq!(back, take, "float32 export must be lossless");
    let peaks = compute_peaks(&back, 800);
    assert!(!peaks.min.is_empty());
    eprintln!(
        "device test: {} frames @ {} Hz x{} → {} bytes",
        take.frames(),
        take.sample_rate,
        take.channels,
        std::fs::metadata(&path).unwrap().len()
    );
}
