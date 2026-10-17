//! Arrangement (F5.1): tracks of clips that reference takes, saved as
//! `arrangement.json` next to `project.json`. The takes themselves are never changed;
//! a clip is a window (`offset`, `length`) into a take placed at `start` on the timeline.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{ProjectError, Result};

pub const ARRANGEMENT_SCHEMA_VERSION: u32 = 1;
pub const ARRANGEMENT_FILE: &str = "arrangement.json";
pub const MAX_TRACKS: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: String,
    pub take_id: String,
    /// Timeline position of the clip's first sample, seconds.
    pub start: f64,
    /// Seconds into the take's (trimmed) audio where the clip begins.
    pub offset: f64,
    pub length: f64,
    #[serde(default)]
    pub gain_db: f64,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default)]
    pub mute: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub gain_db: f64,
    /// −1 … +1.
    #[serde(default)]
    pub pan: f64,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub solo: bool,
    #[serde(default)]
    pub clips: Vec<Clip>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arrangement {
    pub schema_version: u32,
    #[serde(default)]
    pub tracks: Vec<Track>,
    /// Loop region on the timeline, seconds, if set.
    #[serde(default)]
    pub loop_region: Option<(f64, f64)>,
}

impl Default for Arrangement {
    fn default() -> Self {
        Self {
            schema_version: ARRANGEMENT_SCHEMA_VERSION,
            tracks: Vec::new(),
            loop_region: None,
        }
    }
}

impl Arrangement {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(ARRANGEMENT_FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)?;
        let a: Arrangement = serde_json::from_str(&text)?;
        if a.schema_version > ARRANGEMENT_SCHEMA_VERSION {
            return Err(ProjectError::NewerSchema {
                what: "arrangement",
                found: a.schema_version,
                supported: ARRANGEMENT_SCHEMA_VERSION,
            });
        }
        Ok(a)
    }

    /// Atomic save: write a temp file, then rename over the old one.
    pub fn save(&self, root: &Path) -> Result<()> {
        let path = root.join(ARRANGEMENT_FILE);
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Validate: lengths positive, offsets non-negative, track count bounded.
    pub fn validate(&self) -> Result<()> {
        if self.tracks.len() > MAX_TRACKS {
            return Err(ProjectError::InvalidState(format!(
                "at most {MAX_TRACKS} tracks"
            )));
        }
        for t in &self.tracks {
            if !(-1.0..=1.0).contains(&t.pan) || !t.gain_db.is_finite() {
                return Err(ProjectError::InvalidState("track pan/gain".into()));
            }
            for c in &t.clips {
                if !(c.start >= 0.0 && c.offset >= 0.0 && c.length > 0.0)
                    || !c.start.is_finite()
                    || !c.length.is_finite()
                    || c.fade_in < 0.0
                    || c.fade_out < 0.0
                {
                    return Err(ProjectError::InvalidState(format!(
                        "clip {} has an invalid position",
                        c.id
                    )));
                }
            }
        }
        if let Some((a, b)) = self.loop_region {
            if !(a >= 0.0 && b > a) {
                return Err(ProjectError::InvalidState("loop region".into()));
            }
        }
        Ok(())
    }

    /// End of the last clip, seconds.
    pub fn duration(&self) -> f64 {
        self.tracks
            .iter()
            .flat_map(|t| t.clips.iter())
            .map(|c| c.start + c.length)
            .fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_validate_and_duration() {
        let dir = std::env::temp_dir().join(format!("ethnoflow-arr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut a = Arrangement::default();
        a.tracks.push(Track {
            id: "t1".into(),
            name: "Kaval".into(),
            gain_db: -3.0,
            pan: 0.2,
            mute: false,
            solo: false,
            clips: vec![Clip {
                id: "c1".into(),
                take_id: "take-1".into(),
                start: 1.5,
                offset: 0.25,
                length: 4.0,
                gain_db: 0.0,
                fade_in: 0.01,
                fade_out: 0.1,
                mute: false,
            }],
        });
        a.validate().unwrap();
        assert!((a.duration() - 5.5).abs() < 1e-9);
        a.save(&dir).unwrap();
        let b = Arrangement::load(&dir).unwrap();
        assert_eq!(a, b);
        assert_eq!(
            Arrangement::load(&dir.join("nope")).unwrap(),
            Arrangement::default()
        );
        a.tracks[0].clips[0].length = 0.0;
        assert!(a.validate().is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
