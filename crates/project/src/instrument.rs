//! Instrument profiles (FEATURE_SPEC F2.5): global per user, not per project.
//!
//! A profile says which instrument the user plays, its nominal tuning, and — after the
//! "meet my instrument" wizard — what each note *actually* measured as, with a sample
//! of it. Projects reference a profile by id + version. Nothing is inferred: a profile
//! exists only because the user created it.

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{ProjectError, Result};

pub const PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstrumentType {
    Kaval,
    Shakuhachi,
    Flute,
    Recorder,
    Gaida,
    Voice,
    Monochord,
    Piano,
    MidiKeyboard,
    Other,
}

impl InstrumentType {
    pub const ALL: [InstrumentType; 10] = [
        InstrumentType::Kaval,
        InstrumentType::Shakuhachi,
        InstrumentType::Flute,
        InstrumentType::Recorder,
        InstrumentType::Gaida,
        InstrumentType::Voice,
        InstrumentType::Monochord,
        InstrumentType::Piano,
        InstrumentType::MidiKeyboard,
        InstrumentType::Other,
    ];

    /// Typical sounding range (MIDI) used to propose the calibration list; the user
    /// can skip notes or extend it.
    pub fn default_range(self) -> (u8, u8) {
        match self {
            InstrumentType::Kaval => (62, 93),      // D4 .. A6 (kaval in D)
            InstrumentType::Shakuhachi => (62, 86), // D4 .. D6 (1.8 shaku)
            InstrumentType::Flute => (60, 96),
            InstrumentType::Recorder => (72, 98),
            InstrumentType::Gaida => (62, 74),
            InstrumentType::Voice => (48, 84),
            InstrumentType::Monochord => (36, 72),
            InstrumentType::Piano | InstrumentType::MidiKeyboard => (21, 108),
            InstrumentType::Other => (48, 84),
        }
    }

    /// True for instruments whose player's pitch can be measured by the tuner.
    pub fn is_acoustic(self) -> bool {
        !matches!(self, InstrumentType::MidiKeyboard)
    }
}

/// One calibrated note.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileNote {
    pub midi: u8,
    pub measured_hz: f64,
    /// Deviation from the tempered note at the profile's A4, in cents.
    pub cents_offset: f64,
    /// Standard deviation of the pitch over the held tone, in cents (lower = steadier).
    pub stability_cents: f64,
    /// Relative path of the recorded sample inside the profile's folder, if kept.
    pub sample: Option<String>,
    pub measured_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentProfile {
    pub schema_version: u32,
    pub id: String,
    /// Bumped on every save that changes notes, so projects can detect drift.
    pub version: u32,
    pub instrument: InstrumentType,
    pub name: String,
    /// Nominal tuning as the user states it, e.g. "D" or "1.8 shaku"; free text.
    pub tuning: String,
    /// Written-vs-sounding offset in semitones for transposing instruments (0 = none).
    pub transpose_semitones: i32,
    pub a4_hz: f64,
    /// Sorted by midi, at most one entry per note.
    pub notes: Vec<ProfileNote>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl InstrumentProfile {
    pub fn new(instrument: InstrumentType, name: &str, tuning: &str, a4_hz: f64) -> Self {
        let now = crate::now_ms();
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            id: format!("instr-{now}"),
            version: 1,
            instrument,
            name: name.trim().to_string(),
            tuning: tuning.trim().to_string(),
            transpose_semitones: 0,
            a4_hz,
            notes: Vec::new(),
            created_at_ms: now,
            updated_at_ms: now,
        }
    }

    /// Insert or replace the measurement of one note.
    pub fn set_note(&mut self, note: ProfileNote) {
        match self.notes.binary_search_by_key(&note.midi, |n| n.midi) {
            Ok(i) => self.notes[i] = note,
            Err(i) => self.notes.insert(i, note),
        }
        self.version += 1;
        self.updated_at_ms = crate::now_ms();
    }

    pub fn remove_note(&mut self, midi: u8) -> bool {
        let before = self.notes.len();
        self.notes.retain(|n| n.midi != midi);
        if self.notes.len() != before {
            self.version += 1;
            self.updated_at_ms = crate::now_ms();
            true
        } else {
            false
        }
    }

    pub fn note(&self, midi: u8) -> Option<&ProfileNote> {
        self.notes.iter().find(|n| n.midi == midi)
    }

    /// Nearest calibrated note to a fractional MIDI value, with its distance in cents.
    pub fn nearest(&self, midi_fraction: f64) -> Option<(&ProfileNote, f64)> {
        self.notes
            .iter()
            .map(|n| {
                (
                    n,
                    (midi_fraction - (n.midi as f64 + n.cents_offset / 100.0)) * 100.0,
                )
            })
            .min_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
    }

    /// Mean and spread of the offsets: a consistent mean points at the instrument,
    /// a large spread at the playing (FEATURE_SPEC F2.5 step 3).
    pub fn diagnosis(&self) -> Option<ProfileDiagnosis> {
        if self.notes.is_empty() {
            return None;
        }
        let n = self.notes.len() as f64;
        let mean = self.notes.iter().map(|x| x.cents_offset).sum::<f64>() / n;
        let var = self
            .notes
            .iter()
            .map(|x| (x.cents_offset - mean).powi(2))
            .sum::<f64>()
            / n;
        Some(ProfileDiagnosis {
            notes: self.notes.len(),
            mean_offset_cents: mean,
            spread_cents: var.sqrt(),
            mean_stability_cents: self.notes.iter().map(|x| x.stability_cents).sum::<f64>() / n,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileDiagnosis {
    pub notes: usize,
    pub mean_offset_cents: f64,
    pub spread_cents: f64,
    pub mean_stability_cents: f64,
}

/// Folder of profiles: `<dir>/<id>/profile.json` plus samples next to it.
pub struct ProfileStore {
    dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: String,
    pub version: u32,
    pub instrument: InstrumentType,
    pub name: String,
    pub tuning: String,
    pub notes: usize,
}

impl ProfileStore {
    pub fn new(dir: &Path) -> Self {
        Self {
            dir: dir.to_path_buf(),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn profile_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    fn path(&self, id: &str) -> PathBuf {
        self.profile_dir(id).join("profile.json")
    }

    pub fn list(&self) -> Result<Vec<ProfileSummary>> {
        let mut out = Vec::new();
        if !self.dir.exists() {
            return Ok(out);
        }
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            if !entry.path().is_dir() {
                continue;
            }
            let id = entry.file_name().to_string_lossy().into_owned();
            if let Ok(p) = self.load(&id) {
                out.push(ProfileSummary {
                    id: p.id,
                    version: p.version,
                    instrument: p.instrument,
                    name: p.name,
                    tuning: p.tuning,
                    notes: p.notes.len(),
                });
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn load(&self, id: &str) -> Result<InstrumentProfile> {
        let path = self.path(id);
        let text =
            std::fs::read_to_string(&path).map_err(|_| ProjectError::NotAProject(path.clone()))?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        let found = value
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        if found > PROFILE_SCHEMA_VERSION {
            return Err(ProjectError::NewerSchema {
                what: "instrument profile",
                found,
                supported: PROFILE_SCHEMA_VERSION,
            });
        }
        Ok(serde_json::from_value(value)?)
    }

    /// Atomic save (temp + rename). Creates the profile folder.
    pub fn save(&self, p: &InstrumentProfile) -> Result<()> {
        let dir = self.profile_dir(&p.id);
        std::fs::create_dir_all(&dir)?;
        let path = self.path(&p.id);
        let tmp = dir.join("profile.json.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(serde_json::to_string_pretty(p)?.as_bytes())?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Where a sample for `midi` goes; the caller writes the WAV there.
    pub fn sample_path(&self, id: &str, midi: u8) -> PathBuf {
        self.profile_dir(id).join(format!("note-{midi:03}.wav"))
    }

    /// Remove a whole profile (its folder goes to `<dir>/trash/<id>`).
    pub fn delete(&self, id: &str) -> Result<()> {
        let src = self.profile_dir(id);
        if !src.exists() {
            return Err(ProjectError::NotAProject(src));
        }
        let trash = self.dir.join("trash");
        std::fs::create_dir_all(&trash)?;
        let mut dest = trash.join(id);
        if dest.exists() {
            dest = trash.join(format!("{id}-{}", crate::now_ms()));
        }
        std::fs::rename(&src, &dest)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(midi: u8, cents: f64) -> ProfileNote {
        ProfileNote {
            midi,
            measured_hz: 440.0 * 2f64.powf((midi as f64 + cents / 100.0 - 69.0) / 12.0),
            cents_offset: cents,
            stability_cents: 4.0,
            sample: None,
            measured_at_ms: 0,
        }
    }

    #[test]
    fn notes_are_kept_sorted_and_versioned() {
        let mut p = InstrumentProfile::new(InstrumentType::Kaval, " Кавал ре ", "D", 440.0);
        assert_eq!(p.name, "Кавал ре");
        assert_eq!(p.version, 1);
        p.set_note(note(69, 10.0));
        p.set_note(note(62, -5.0));
        p.set_note(note(69, 12.0)); // replaces
        assert_eq!(
            p.notes.iter().map(|n| n.midi).collect::<Vec<_>>(),
            vec![62, 69]
        );
        assert_eq!(p.note(69).unwrap().cents_offset, 12.0);
        assert_eq!(p.version, 4);
        assert!(p.remove_note(62));
        assert!(!p.remove_note(62));
        assert_eq!(p.version, 5);
    }

    #[test]
    fn nearest_uses_the_measured_pitch_not_the_nominal_one() {
        let mut p = InstrumentProfile::new(InstrumentType::Flute, "f", "C", 440.0);
        p.set_note(note(69, 20.0)); // A plays 20 cents sharp on this instrument
        p.set_note(note(71, 0.0));
        let (n, off) = p.nearest(69.25).unwrap();
        assert_eq!(n.midi, 69);
        assert!(
            (off - 5.0).abs() < 1e-9,
            "25 cents above A, A itself is +20 → 5 cents"
        );
        assert!(
            InstrumentProfile::new(InstrumentType::Voice, "v", "", 440.0)
                .nearest(60.0)
                .is_none()
        );
    }

    #[test]
    fn diagnosis_separates_systematic_from_random() {
        let mut consistent = InstrumentProfile::new(InstrumentType::Kaval, "a", "D", 440.0);
        for m in 62..70 {
            consistent.set_note(note(m, 15.0));
        }
        let d = consistent.diagnosis().unwrap();
        assert!((d.mean_offset_cents - 15.0).abs() < 1e-9 && d.spread_cents < 1e-9);
        let mut random = InstrumentProfile::new(InstrumentType::Kaval, "b", "D", 440.0);
        for (i, m) in (62..70).enumerate() {
            random.set_note(note(m, if i % 2 == 0 { 20.0 } else { -20.0 }));
        }
        let d = random.diagnosis().unwrap();
        assert!(d.mean_offset_cents.abs() < 1e-9 && d.spread_cents > 15.0);
        assert!(
            InstrumentProfile::new(InstrumentType::Other, "c", "", 440.0)
                .diagnosis()
                .is_none()
        );
    }

    #[test]
    fn store_roundtrips_lists_and_trashes() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(&dir.path().join("instruments"));
        assert!(store.list().unwrap().is_empty());
        let mut p = InstrumentProfile::new(InstrumentType::Shakuhachi, "Шакухачи 1.8", "D", 440.0);
        p.set_note(note(62, -8.0));
        store.save(&p).unwrap();
        store.save(&p).unwrap(); // overwrite is fine
        assert_eq!(store.load(&p.id).unwrap(), p);
        let l = store.list().unwrap();
        assert_eq!(l.len(), 1);
        assert_eq!(
            (l[0].notes, l[0].instrument),
            (1, InstrumentType::Shakuhachi)
        );
        assert!(store
            .sample_path(&p.id, 62)
            .ends_with(format!("{}/note-062.wav", p.id)));
        assert!(!store.profile_dir(&p.id).join("profile.json.tmp").exists());
        store.delete(&p.id).unwrap();
        assert!(store.list().unwrap().is_empty());
        assert!(dir
            .path()
            .join("instruments/trash")
            .join(&p.id)
            .join("profile.json")
            .exists());
        assert!(store.load(&p.id).is_err());
    }

    #[test]
    fn newer_profiles_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = ProfileStore::new(dir.path());
        std::fs::create_dir_all(dir.path().join("x")).unwrap();
        std::fs::write(
            dir.path().join("x/profile.json"),
            r#"{"schema_version":99}"#,
        )
        .unwrap();
        assert!(matches!(
            store.load("x"),
            Err(ProjectError::NewerSchema { .. })
        ));
        assert!(
            store.list().unwrap().is_empty(),
            "unreadable profiles are skipped in listings"
        );
    }
}
