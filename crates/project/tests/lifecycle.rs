//! Project lifecycle: create → record → close → reopen (identical bytes), crash recovery,
//! missing/changed originals, relink, delete-to-trash, import, trim.

use std::path::Path;

use audio_engine::{read_wav, AudioBuffer, TakeWriter};
use project::{checksum_file, sanitize_name, Project, ProjectError, TakeHealth, TakeState};

fn write_take(path: &Path, buf: &AudioBuffer) -> u64 {
    let mut w = TakeWriter::create(path, buf.sample_rate, buf.channels).unwrap();
    for c in buf.samples.chunks(1024) {
        w.write(c).unwrap();
    }
    w.finish().unwrap()
}

fn tone() -> AudioBuffer {
    AudioBuffer::sine(48_000, 293.66, 3.0, 0.5)
}

#[test]
fn recorded_take_survives_close_and_reopen_bit_exact() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::create(dir.path(), "Кавал проба").unwrap();
    assert!(p.root().ends_with("Кавал проба.ethnoflow"));
    assert!(p.root().join("assets/originals").is_dir());
    assert_eq!(p.manifest().name, "Кавал проба");

    let t = p.begin_recording(Some("Microphone Array")).unwrap();
    assert_eq!(t.state, TakeState::Recording);
    let src = tone();
    write_take(&p.take_path(&t), &src);
    let t = p.finish_recording(&t.id, 0, 0).unwrap();
    assert_eq!(t.state, TakeState::Complete);
    assert_eq!(t.frames, Some(src.frames() as u64));
    let sum = t.blake3.clone().unwrap();
    let root = p.root().to_path_buf();
    drop(p);

    let (p, report) = Project::open(&root).unwrap();
    assert!(report.recovered.is_empty());
    assert_eq!(report.checks.len(), 1);
    assert_eq!(report.checks[0].health, TakeHealth::Ok);
    let t2 = p.take(&t.id).unwrap();
    assert_eq!(t2, t);
    assert_eq!(checksum_file(&p.take_path(&t2)).unwrap(), sum);
    assert_eq!(read_wav(p.take_path(&t2)).unwrap(), src);
}

#[test]
fn take_interrupted_by_a_crash_is_recovered_on_open() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::create(dir.path(), "crash").unwrap();
    let t = p.begin_recording(None).unwrap();
    let path = p.take_path(&t);
    let src = AudioBuffer::sine(8_000, 200.0, 2.6, 0.4);
    let mut w = TakeWriter::create(&path, 8_000, 1).unwrap();
    for c in src.samples.chunks(800) {
        w.write(c).unwrap();
    }
    // Process dies: no finalize, no destructor, buffered tail never reaches the disk.
    std::mem::forget(w);
    let root = p.root().to_path_buf();
    drop(p);

    let (p, report) = Project::open(&root).unwrap();
    assert_eq!(report.recovered, vec![t.id.clone()]);
    let t = p.take(&t.id).unwrap();
    assert_eq!(t.state, TakeState::Recovered);
    let back = read_wav(p.take_path(&t)).unwrap();
    // Everything up to at least the last checkpoint (2 s) is there, bit-exact.
    assert!(back.frames() >= 16_000, "{} frames", back.frames());
    assert_eq!(t.frames, Some(back.frames() as u64));
    assert_eq!(back.samples[..], src.samples[..back.samples.len()]);
    assert_eq!(report.checks[0].health, TakeHealth::Ok);
}

#[test]
fn recording_that_never_created_a_file_is_dropped() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::create(dir.path(), "empty").unwrap();
    p.begin_recording(None).unwrap();
    let root = p.root().to_path_buf();
    drop(p);
    let (p, report) = Project::open(&root).unwrap();
    assert_eq!(report.dropped_empty, 1);
    assert!(p.takes().unwrap().is_empty());
}

#[test]
fn orphan_wav_in_originals_is_adopted() {
    let dir = tempfile::tempdir().unwrap();
    let p = Project::create(dir.path(), "orphan").unwrap();
    let root = p.root().to_path_buf();
    drop(p);
    write_take(&root.join("assets/originals/lost.wav"), &tone());
    let (p, report) = Project::open(&root).unwrap();
    assert_eq!(report.recovered, vec!["lost".to_string()]);
    assert_eq!(p.take("lost").unwrap().frames, Some(tone().frames() as u64));
}

#[test]
fn changed_and_missing_originals_are_detected_and_relinkable() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::create(dir.path(), "verify").unwrap();
    let t = p.begin_recording(None).unwrap();
    write_take(&p.take_path(&t), &tone());
    let t = p.finish_recording(&t.id, 0, 0).unwrap();
    let path = p.take_path(&t);

    // Changed byte → mismatch.
    let good = std::fs::read(&path).unwrap();
    let mut bad = good.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0x01;
    std::fs::write(&path, &bad).unwrap();
    assert_eq!(
        p.verify_takes().unwrap()[0].health,
        TakeHealth::ChecksumMismatch
    );

    // Missing → detected; relink only accepts the identical file.
    let backup = dir.path().join("backup.wav");
    std::fs::write(&backup, &good).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert_eq!(p.verify_takes().unwrap()[0].health, TakeHealth::Missing);
    let wrong = dir.path().join("wrong.wav");
    std::fs::write(&wrong, &bad).unwrap();
    assert!(matches!(
        p.relink_take(&t.id, &wrong),
        Err(ProjectError::ChecksumMismatch(_))
    ));
    p.relink_take(&t.id, &backup).unwrap();
    assert_eq!(p.verify_takes().unwrap()[0].health, TakeHealth::Ok);
}

#[test]
fn delete_moves_the_original_to_trash() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::create(dir.path(), "del").unwrap();
    let t = p.begin_recording(None).unwrap();
    write_take(&p.take_path(&t), &tone());
    let t = p.finish_recording(&t.id, 0, 0).unwrap();
    let moved = p.delete_take(&t.id).unwrap().unwrap();
    assert!(moved.starts_with(p.root().join("trash")));
    assert_eq!(checksum_file(&moved).unwrap(), t.blake3.unwrap());
    assert!(p.takes().unwrap().is_empty());
}

#[test]
fn import_keeps_original_bytes_and_records_facts() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("Re Song.MP3");
    std::fs::write(&src, b"pretend these are mp3 bytes").unwrap();
    let mut p = Project::create(dir.path(), "imp").unwrap();
    let t = p.import_original(&src, 44_100, 2, 1234).unwrap();
    assert!(t.file.ends_with(".mp3"));
    assert_eq!(t.original_name.as_deref(), Some("Re Song.MP3"));
    assert_eq!(
        (t.sample_rate, t.channels, t.frames),
        (Some(44_100), Some(2), Some(1234))
    );
    assert_eq!(
        std::fs::read(p.take_path(&t)).unwrap(),
        std::fs::read(&src).unwrap()
    );
}

#[test]
fn rename_and_trim_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::create(dir.path(), "edit").unwrap();
    let t = p.begin_recording(None).unwrap();
    write_take(&p.take_path(&t), &tone());
    p.finish_recording(&t.id, 0, 0).unwrap();
    assert_eq!(
        p.rename_take(&t.id, Some("  Хоро  "))
            .unwrap()
            .label
            .as_deref(),
        Some("Хоро")
    );
    assert_eq!(p.rename_take(&t.id, Some("   ")).unwrap().label, None);
    let frames = tone().frames() as u64;
    let t2 = p.set_trim(&t.id, 100, Some(frames - 100)).unwrap();
    assert_eq!(
        (t2.trim_in_frames, t2.trim_out_frames),
        (100, Some(frames - 100))
    );
    assert!(p.set_trim(&t.id, 500, Some(400)).is_err());
    assert!(p.set_trim(&t.id, 0, Some(frames + 1)).is_err());
    // Trim never touches the original.
    assert_eq!(read_wav(p.take_path(&t2)).unwrap(), tone());
}

#[test]
fn derived_take_is_a_new_file_linked_to_its_parent() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::create(dir.path(), "cut").unwrap();
    let t = p.begin_recording(None).unwrap();
    let src = tone();
    write_take(&p.take_path(&t), &src);
    let parent = p.finish_recording(&t.id, 0, 0).unwrap();
    let piece = src.slice_frames(48_000, 96_000);
    let d = p
        .create_derived_take(&parent.id, Some(" Фраза 1 "), |path| {
            audio_engine::write_wav(path, &piece, audio_engine::WavFormat::Float32)
        })
        .unwrap();
    assert_eq!(d.source, project::TakeSource::Derived);
    assert_eq!(d.derived_from.as_deref(), Some(parent.id.as_str()));
    assert_eq!(d.label.as_deref(), Some("Фраза 1"));
    assert_eq!(d.frames, Some(48_000));
    assert!(d.file.starts_with("assets/derived/"));
    assert_eq!(read_wav(p.take_path(&d)).unwrap(), piece);
    // Parent untouched, both verify, and the derived take survives reopen.
    assert_eq!(read_wav(p.take_path(&parent)).unwrap(), src);
    let root = p.root().to_path_buf();
    drop(p);
    let (p, report) = Project::open(&root).unwrap();
    assert!(report.checks.iter().all(|c| c.health == TakeHealth::Ok));
    assert_eq!(p.take(&d.id).unwrap(), d);
    // A failing writer leaves nothing behind.
    let before = p.takes().unwrap().len();
    let mut p = p;
    let r = p.create_derived_take(&parent.id, None, |path| {
        std::fs::write(path, b"partial").unwrap();
        Err(audio_engine::AudioError::Wav("boom".into()))
    });
    assert!(r.is_err());
    assert_eq!(p.takes().unwrap().len(), before);
    assert_eq!(
        std::fs::read_dir(root.join("assets/derived"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn analysis_documents_roundtrip_and_leave_audio_alone() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::create(dir.path(), "ana").unwrap();
    let t = p.begin_recording(None).unwrap();
    write_take(&p.take_path(&t), &tone());
    let t = p.finish_recording(&t.id, 0, 0).unwrap();
    assert_eq!(
        p.load_analysis::<Vec<(f64, f64)>>(&t.id, "contour")
            .unwrap(),
        None
    );
    let contour = vec![(0.0, 440.0), (0.01, 441.5)];
    p.save_analysis(&t.id, "contour", &contour).unwrap();
    p.save_analysis(&t.id, "contour", &contour).unwrap(); // overwrite is fine
    assert_eq!(
        p.load_analysis::<Vec<(f64, f64)>>(&t.id, "contour")
            .unwrap(),
        Some(contour)
    );
    assert!(p
        .analysis_path(&t.id, "contour")
        .ends_with(format!("analysis/{}.contour.json", t.id)));
    assert!(!p
        .root()
        .join("analysis")
        .join(format!("{}.contour.json.tmp", t.id))
        .exists());
    assert_eq!(
        p.verify_takes().unwrap()[0].health,
        TakeHealth::Ok,
        "audio untouched"
    );
}

#[test]
fn derived_midi_take_links_to_its_parent_and_verifies() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = Project::create(dir.path(), "tomidi").unwrap();
    let t = p.begin_recording(None).unwrap();
    write_take(&p.take_path(&t), &tone());
    let parent = p.finish_recording(&t.id, 0, 0).unwrap();
    let bytes = b"MThd\0\0\0\x06\0\0\0\x01\x01\xE0MTrk\0\0\0\x04\0\xFF\x2F\0".to_vec();
    let m = p
        .create_derived_midi(
            &parent.id,
            Some("За плейъра"),
            (48_000, 1, 96_000),
            |path| std::fs::write(path, &bytes),
        )
        .unwrap();
    assert_eq!(m.source, project::TakeSource::MidiFile);
    assert_eq!(m.derived_from.as_deref(), Some(parent.id.as_str()));
    assert!(m.file.ends_with(".mid"));
    assert_eq!(
        (m.sample_rate, m.channels, m.frames),
        (Some(48_000), Some(1), Some(96_000))
    );
    assert_eq!(std::fs::read(p.take_path(&m)).unwrap(), bytes);
    assert!(p
        .verify_takes()
        .unwrap()
        .iter()
        .all(|c| c.health == TakeHealth::Ok));
    let root = p.root().to_path_buf();
    drop(p);
    let (p, _) = Project::open(&root).unwrap();
    assert_eq!(p.take(&m.id).unwrap(), m);
}

#[test]
fn names_are_made_safe_and_duplicates_refused() {
    assert_eq!(sanitize_name(" a/b:c? ").unwrap(), "a_b_c_");
    assert_eq!(sanitize_name("con").unwrap(), "con_");
    assert_eq!(sanitize_name("Песен.").unwrap(), "Песен");
    assert!(sanitize_name("  ").is_err());
    assert!(sanitize_name("///").is_err());
    let dir = tempfile::tempdir().unwrap();
    Project::create(dir.path(), "dup").unwrap();
    assert!(matches!(
        Project::create(dir.path(), "dup"),
        Err(ProjectError::AlreadyExists(_))
    ));
    assert!(matches!(
        Project::open(dir.path()),
        Err(ProjectError::NotAProject(_))
    ));
}
