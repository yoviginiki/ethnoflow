//! `project.json`: the portable, human-readable part of a project.
//!
//! Musical timing is user data, never assumed: a new project has an empty tempo map and
//! no time signature until the user sets them.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{ProjectError, Result};

pub const MANIFEST_FILE: &str = "project.json";
pub const FORMAT_ID: &str = "ethnoflow-project";
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempoPoint {
    pub at_seconds: f64,
    pub bpm: f64,
}

/// A time signature from a given bar. `grouping` lists beat groups in `unit` notes,
/// e.g. 7/8 as [2, 2, 3] or [3, 2, 2]; empty means "no grouping given".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeSignaturePoint {
    pub from_bar: u32,
    pub numerator: u32,
    pub unit: u32,
    #[serde(default)]
    pub grouping: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub at_seconds: f64,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub schema_version: u32,
    pub name: String,
    pub created_at_ms: i64,
    pub app_version: String,
    #[serde(default)]
    pub tempo_map: Vec<TempoPoint>,
    #[serde(default)]
    pub time_signatures: Vec<TimeSignaturePoint>,
    #[serde(default)]
    pub markers: Vec<Marker>,
}

impl Manifest {
    pub fn new(name: &str, created_at_ms: i64) -> Self {
        Self {
            format: FORMAT_ID.into(),
            schema_version: MANIFEST_SCHEMA_VERSION,
            name: name.into(),
            created_at_ms,
            app_version: env!("CARGO_PKG_VERSION").into(),
            tempo_map: Vec::new(),
            time_signatures: Vec::new(),
            markers: Vec::new(),
        }
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(MANIFEST_FILE);
        let text = std::fs::read_to_string(&path)
            .map_err(|_| ProjectError::NotAProject(root.to_path_buf()))?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        if value.get("format").and_then(|v| v.as_str()) != Some(FORMAT_ID) {
            return Err(ProjectError::NotAProject(root.to_path_buf()));
        }
        let found = value
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        if found > MANIFEST_SCHEMA_VERSION {
            return Err(ProjectError::NewerSchema {
                what: "project.json",
                found,
                supported: MANIFEST_SCHEMA_VERSION,
            });
        }
        // Version 1 is the first released format; future versions migrate here.
        Ok(serde_json::from_value(value)?)
    }

    /// Write atomically: temp file, fsync, rename over the old manifest.
    pub fn save(&self, root: &Path) -> Result<()> {
        let tmp = root.join(format!("{MANIFEST_FILE}.tmp"));
        {
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
            f.write_all(b"\n")?;
            f.sync_all()?;
        }
        std::fs::rename(&tmp, root.join(MANIFEST_FILE))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_keeps_every_field() {
        let dir = tempfile::tempdir().unwrap();
        let mut m = Manifest::new("Кавал", 1_700_000_000_000);
        m.tempo_map.push(TempoPoint {
            at_seconds: 0.0,
            bpm: 96.5,
        });
        m.time_signatures.push(TimeSignaturePoint {
            from_bar: 1,
            numerator: 7,
            unit: 8,
            grouping: vec![2, 2, 3],
        });
        m.markers.push(Marker {
            at_seconds: 12.5,
            name: "Припев".into(),
        });
        m.save(dir.path()).unwrap();
        assert_eq!(Manifest::load(dir.path()).unwrap(), m);
        assert!(!dir.path().join("project.json.tmp").exists());
    }

    #[test]
    fn new_manifest_assumes_no_tempo_or_meter() {
        let m = Manifest::new("x", 0);
        assert!(m.tempo_map.is_empty() && m.time_signatures.is_empty());
    }

    #[test]
    fn newer_schema_and_foreign_files_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join(MANIFEST_FILE);
        std::fs::write(&p, r#"{"format":"ethnoflow-project","schema_version":99}"#).unwrap();
        assert!(matches!(
            Manifest::load(dir.path()),
            Err(ProjectError::NewerSchema { found: 99, .. })
        ));
        std::fs::write(&p, r#"{"format":"something-else"}"#).unwrap();
        assert!(matches!(
            Manifest::load(dir.path()),
            Err(ProjectError::NotAProject(_))
        ));
        std::fs::remove_file(&p).unwrap();
        assert!(matches!(
            Manifest::load(dir.path()),
            Err(ProjectError::NotAProject(_))
        ));
    }
}
