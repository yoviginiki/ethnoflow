//! A project on disk: `<name>.ethnoflow/` with the layout from master plan §10.
//!
//! Originals in `assets/originals/` are immutable: they are created once (recording or
//! import copy), checksummed with BLAKE3, and afterwards only read. Deleting a take moves
//! its file to `trash/` inside the project instead of destroying it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use audio_engine::{repair_wav_header, wav_info};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::db::{self, AudioFacts, NewTake, Take, TakeSource, TakeState};
use crate::error::{ProjectError, Result};
use crate::manifest::{Manifest, MANIFEST_FILE};

pub const PROJECT_EXT: &str = "ethnoflow";
pub const ORIGINALS_DIR: &str = "assets/originals";
pub const TRASH_DIR: &str = "trash";
const LAYOUT: &[&str] = &[
    ORIGINALS_DIR,
    "assets/derived",
    "tracks",
    "samples",
    "analysis",
    "variations",
    "lessons",
    TRASH_DIR,
];

/// Outcome of checking one take's original against its stored checksum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TakeHealth {
    Ok,
    Missing,
    ChecksumMismatch,
    /// No checksum stored (should not happen for finished takes).
    Unverified,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeCheck {
    pub id: String,
    pub health: TakeHealth,
}

/// What happened while opening a project.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OpenReport {
    /// Takes that were unfinished (crash) and have been repaired.
    pub recovered: Vec<String>,
    /// Takes left in "recording" state that could not be repaired (file unreadable).
    pub unrecoverable: Vec<String>,
    /// Recording entries whose file was never created; removed.
    pub dropped_empty: usize,
    pub checks: Vec<TakeCheck>,
}

pub struct Project {
    root: PathBuf,
    conn: Connection,
    manifest: Manifest,
}

/// Turn a user-typed name into a safe folder name (Windows rules are the strictest).
pub fn sanitize_name(name: &str) -> Result<String> {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim_end_matches(['.', ' ']).to_string();
    if cleaned.is_empty() || cleaned.chars().all(|c| c == '_') {
        return Err(ProjectError::InvalidName);
    }
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
        || ((stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.len() == 4
            && stem.as_bytes()[3].is_ascii_digit());
    Ok(if reserved {
        format!("{cleaned}_")
    } else {
        cleaned
    })
}

/// BLAKE3 of a file's bytes, hex.
pub fn checksum_file(path: &Path) -> Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut f = std::fs::File::open(path)?;
    std::io::copy(&mut f, &mut hasher)?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn new_take_id() -> String {
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed) % 1000;
    format!("take-{}-{n:03}", crate::now_ms())
}

impl Project {
    /// True if `path` looks like a project folder.
    pub fn is_project_dir(path: &Path) -> bool {
        path.join(MANIFEST_FILE).is_file()
    }

    /// Create `<parent>/<name>.ethnoflow/`. Fails if it already exists.
    pub fn create(parent: &Path, name: &str) -> Result<Self> {
        let folder = format!("{}.{PROJECT_EXT}", sanitize_name(name)?);
        let root = parent.join(folder);
        if root.exists() {
            return Err(ProjectError::AlreadyExists(root));
        }
        std::fs::create_dir_all(parent)?;
        std::fs::create_dir(&root)?;
        for d in LAYOUT {
            std::fs::create_dir_all(root.join(d))?;
        }
        let manifest = Manifest::new(name.trim(), crate::now_ms());
        manifest.save(&root)?;
        let conn = db::open(&root.join(db::DB_FILE))?;
        Ok(Self {
            root,
            conn,
            manifest,
        })
    }

    /// Open a project, run migrations, repair takes interrupted by a crash and verify
    /// every original against its checksum.
    pub fn open(root: &Path) -> Result<(Self, OpenReport)> {
        let manifest = Manifest::load(root)?;
        for d in LAYOUT {
            std::fs::create_dir_all(root.join(d))?;
        }
        let conn = db::open(&root.join(db::DB_FILE))?;
        let mut project = Self {
            root: root.to_path_buf(),
            conn,
            manifest,
        };
        let mut report = project.recover()?;
        report.checks = project.verify_takes()?;
        Ok((project, report))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Change the manifest and save it atomically.
    pub fn update_manifest(&mut self, f: impl FnOnce(&mut Manifest)) -> Result<&Manifest> {
        let mut m = self.manifest.clone();
        f(&mut m);
        m.save(&self.root)?;
        self.manifest = m;
        Ok(&self.manifest)
    }

    pub fn take_path(&self, take: &Take) -> PathBuf {
        self.root.join(&take.file)
    }

    pub fn takes(&self) -> Result<Vec<Take>> {
        db::list_takes(&self.conn)
    }

    pub fn take(&self, id: &str) -> Result<Take> {
        db::get_take(&self.conn, id)
    }

    /// Register a new recording before any audio is written. The returned take's path
    /// is where the recorder must create its file. If the app dies from here on, the
    /// next `open` finds the row in `recording` state and repairs the file.
    pub fn begin_recording(&mut self, device: Option<&str>) -> Result<Take> {
        let id = new_take_id();
        let file = format!("{ORIGINALS_DIR}/{id}.wav");
        db::insert_take(
            &self.conn,
            &NewTake {
                id: &id,
                file: &file,
                source: TakeSource::Microphone,
                state: TakeState::Recording,
                original_name: None,
                device,
            },
        )
    }

    /// The recorder has finalized the file: record its facts and checksum.
    pub fn finish_recording(
        &mut self,
        id: &str,
        overrun_samples: u64,
        clipped_samples: u64,
    ) -> Result<Take> {
        let take = self.take(id)?;
        if take.state != TakeState::Recording {
            return Err(ProjectError::InvalidState(format!("{id} is not recording")));
        }
        let facts = self.audio_facts(&self.take_path(&take))?;
        db::set_take_audio(
            &self.conn,
            id,
            TakeState::Complete,
            &facts,
            overrun_samples,
            clipped_samples,
        )
    }

    /// The recorder failed to start: forget the entry (and an empty file, if any).
    pub fn abort_recording(&mut self, id: &str) -> Result<()> {
        let take = self.take(id)?;
        if take.state != TakeState::Recording {
            return Err(ProjectError::InvalidState(format!("{id} is not recording")));
        }
        let path = self.take_path(&take);
        if path.exists() && wav_info(&path).map(|i| i.frames == 0).unwrap_or(false) {
            std::fs::remove_file(&path)?;
        }
        db::delete_take(&self.conn, id)
    }

    /// Copy an external audio file into the project unchanged (the bytes become the
    /// immutable original). The caller decoded it and passes what it found.
    pub fn import_original(
        &mut self,
        src: &Path,
        sample_rate: u32,
        channels: u16,
        frames: u64,
    ) -> Result<Take> {
        let id = new_take_id();
        let ext = src
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase())
            .filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()))
            .unwrap_or_else(|| "bin".into());
        let file = format!("{ORIGINALS_DIR}/{id}.{ext}");
        let dest = self.root.join(&file);
        if dest.exists() {
            return Err(ProjectError::AlreadyExists(dest));
        }
        std::fs::copy(src, &dest)?;
        let blake3 = checksum_file(&dest)?;
        if blake3 != checksum_file(src)? {
            let _ = std::fs::remove_file(&dest);
            return Err(ProjectError::ChecksumMismatch(src.display().to_string()));
        }
        let original_name = src.file_name().map(|n| n.to_string_lossy().into_owned());
        db::insert_take(
            &self.conn,
            &NewTake {
                id: &id,
                file: &file,
                source: TakeSource::ImportedFile,
                state: TakeState::Recording, // until facts are stored below
                original_name: original_name.as_deref(),
                device: None,
            },
        )?;
        let facts = AudioFacts {
            sample_rate,
            channels,
            frames,
            blake3,
        };
        db::set_take_audio(&self.conn, &id, TakeState::Complete, &facts, 0, 0)
    }

    pub fn rename_take(&mut self, id: &str, label: Option<&str>) -> Result<Take> {
        let label = label.map(str::trim).filter(|l| !l.is_empty());
        db::set_label(&self.conn, id, label)
    }

    /// Non-destructive trim: only the in/out points are stored.
    pub fn set_trim(&mut self, id: &str, trim_in: u64, trim_out: Option<u64>) -> Result<Take> {
        let take = self.take(id)?;
        let frames = take.frames.unwrap_or(0);
        let end = trim_out.unwrap_or(frames);
        if trim_in >= end || end > frames {
            return Err(ProjectError::InvalidState(format!(
                "trim {trim_in}..{end} outside 0..{frames}"
            )));
        }
        db::set_trim(&self.conn, id, trim_in, trim_out.filter(|&o| o != frames))
    }

    /// Remove a take from the project. Its file is moved to `trash/`, never destroyed.
    pub fn delete_take(&mut self, id: &str) -> Result<Option<PathBuf>> {
        let take = self.take(id)?;
        if take.state == TakeState::Recording {
            return Err(ProjectError::InvalidState(format!(
                "{id} is still recording"
            )));
        }
        let src = self.take_path(&take);
        let moved = if src.exists() {
            let name = src
                .file_name()
                .map(|n| n.to_os_string())
                .unwrap_or_default();
            let dest = self.root.join(TRASH_DIR).join(name);
            std::fs::rename(&src, &dest)?;
            Some(dest)
        } else {
            None
        };
        db::delete_take(&self.conn, id)?;
        Ok(moved)
    }

    pub fn verify_takes(&self) -> Result<Vec<TakeCheck>> {
        let mut out = Vec::new();
        for t in self.takes()? {
            if t.state == TakeState::Recording {
                continue;
            }
            let path = self.take_path(&t);
            let health = match &t.blake3 {
                _ if !path.exists() => TakeHealth::Missing,
                None => TakeHealth::Unverified,
                Some(sum) if *sum == checksum_file(&path)? => TakeHealth::Ok,
                Some(_) => TakeHealth::ChecksumMismatch,
            };
            out.push(TakeCheck { id: t.id, health });
        }
        Ok(out)
    }

    /// Restore a missing original from a copy elsewhere. The copy must be byte-identical
    /// to what was recorded (same checksum).
    pub fn relink_take(&mut self, id: &str, from: &Path) -> Result<Take> {
        let take = self.take(id)?;
        let dest = self.take_path(&take);
        if dest.exists() {
            return Err(ProjectError::AlreadyExists(dest));
        }
        let expected = take
            .blake3
            .as_deref()
            .ok_or_else(|| ProjectError::InvalidState(format!("{id} has no checksum")))?;
        if checksum_file(from)? != expected {
            return Err(ProjectError::ChecksumMismatch(from.display().to_string()));
        }
        std::fs::copy(from, &dest)?;
        Ok(take)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>> {
        db::get_setting(&self.conn, key)
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<()> {
        db::set_setting(&self.conn, key, value)
    }

    fn audio_facts(&self, path: &Path) -> Result<AudioFacts> {
        let info = wav_info(path)?;
        Ok(AudioFacts {
            sample_rate: info.sample_rate,
            channels: info.channels,
            frames: info.frames as u64,
            blake3: checksum_file(path)?,
        })
    }

    /// Repair takes interrupted by a crash; adopt WAVs in originals/ with no entry.
    fn recover(&mut self) -> Result<OpenReport> {
        let mut report = OpenReport::default();
        for t in self.takes()? {
            if t.state != TakeState::Recording {
                continue;
            }
            let path = self.take_path(&t);
            if !path.exists() {
                db::delete_take(&self.conn, &t.id)?;
                report.dropped_empty += 1;
                continue;
            }
            match repair_wav_header(&path).and_then(|_| wav_info(&path)) {
                Ok(_) => {
                    let facts = self.audio_facts(&path)?;
                    db::set_take_audio(&self.conn, &t.id, TakeState::Recovered, &facts, 0, 0)?;
                    log::warn!("recovered take {} ({} frames)", t.id, facts.frames);
                    report.recovered.push(t.id);
                }
                Err(e) => {
                    log::error!("cannot recover take {}: {e}", t.id);
                    report.unrecoverable.push(t.id);
                }
            }
        }
        // Orphans: a WAV in originals/ that no row references.
        let known: std::collections::HashSet<String> =
            self.takes()?.into_iter().map(|t| t.file).collect();
        for entry in std::fs::read_dir(self.root.join(ORIGINALS_DIR))? {
            let path = entry?.path();
            let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                continue;
            };
            let rel = format!("{ORIGINALS_DIR}/{name}");
            let is_wav = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
            if !is_wav || known.contains(&rel) {
                continue;
            }
            if repair_wav_header(&path)
                .and_then(|_| wav_info(&path))
                .is_err()
            {
                continue;
            }
            let id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(new_take_id);
            db::insert_take(
                &self.conn,
                &NewTake {
                    id: &id,
                    file: &rel,
                    source: TakeSource::Microphone,
                    state: TakeState::Recording,
                    original_name: None,
                    device: None,
                },
            )?;
            let facts = self.audio_facts(&path)?;
            db::set_take_audio(&self.conn, &id, TakeState::Recovered, &facts, 0, 0)?;
            report.recovered.push(id);
        }
        Ok(report)
    }
}
