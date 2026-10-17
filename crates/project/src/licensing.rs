//! Audio asset licensing (F1A.8): the machine-readable manifest of every audio file
//! that ships with the application, and the checks the test suite runs against the
//! repository: no unlisted audio in the shipped folders, no disallowed licence, no
//! removed asset still on disk, CC0 only with documented provenance.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Licences an asset in the installer may carry (see docs/AUDIO-LICENSING.md).
pub const ALLOWED_LICENSES: [&str; 4] = ["original", "CC0-1.0", "CC-BY-4.0", "CC-BY-3.0"];
/// Licences that must never appear, even by mistake.
pub const FORBIDDEN_LICENSES: [&str; 6] = [
    "CC-BY-NC",
    "CC-BY-NC-4.0",
    "CC-BY-ND",
    "CC-BY-SA",
    "personal-use",
    "unknown",
];
pub const AUDIO_EXTENSIONS: [&str; 7] = ["wav", "mp3", "flac", "ogg", "aif", "aiff", "mid"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetStatus {
    Active,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetRecord {
    /// Path relative to the repository root.
    pub path: String,
    pub sha256: String,
    pub license: String,
    pub creator: String,
    /// Where it came from: URL + uploader + date for downloads, "recorded by …" for own.
    pub origin: String,
    #[serde(default)]
    pub attribution: String,
    /// ISO date.
    pub added: String,
    pub status: AssetStatus,
    #[serde(default)]
    pub removed_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssetManifest {
    pub schema_version: u32,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub assets: Vec<AssetRecord>,
}

impl AssetManifest {
    pub fn load(path: &Path) -> crate::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    /// Problems with the manifest itself (independent of the disk).
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            problems.push(format!("schema_version {}", self.schema_version));
        }
        let mut seen = HashSet::new();
        for a in &self.assets {
            if !seen.insert(a.path.as_str()) {
                problems.push(format!("{}: listed twice", a.path));
            }
            if FORBIDDEN_LICENSES
                .iter()
                .any(|f| a.license.eq_ignore_ascii_case(f))
                || !ALLOWED_LICENSES.contains(&a.license.as_str())
            {
                problems.push(format!(
                    "{}: licence '{}' is not allowed",
                    a.path, a.license
                ));
            }
            if a.sha256.len() != 64 || !a.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                problems.push(format!("{}: sha256 is not 64 hex chars", a.path));
            }
            if a.creator.trim().is_empty() || a.origin.trim().is_empty() {
                problems.push(format!("{}: creator and origin are required", a.path));
            }
            if a.license == "CC0-1.0" && !a.origin.contains("http") {
                problems.push(format!(
                    "{}: CC0 needs documented provenance (source URL in origin)",
                    a.path
                ));
            }
            if a.license.starts_with("CC-BY") && a.attribution.trim().is_empty() {
                problems.push(format!("{}: CC BY needs an attribution text", a.path));
            }
            if a.status == AssetStatus::Removed && a.removed_reason.is_none() {
                problems.push(format!("{}: removed without a reason", a.path));
            }
        }
        problems
    }

    /// Problems between the manifest and the repository: audio files in `shipped_dirs`
    /// without an entry, active entries whose file is missing, removed entries whose
    /// file is still there. `root` is the repository root.
    pub fn check_against_disk(&self, root: &Path, shipped_dirs: &[&str]) -> Vec<String> {
        let mut problems = self.validate();
        let listed: HashSet<String> = self.assets.iter().map(|a| normalise(&a.path)).collect();
        for dir in shipped_dirs {
            for file in audio_files_under(&root.join(dir)) {
                let rel = normalise(&file.strip_prefix(root).unwrap_or(&file).to_string_lossy());
                if !listed.contains(&rel) {
                    problems.push(format!("{rel}: audio file without a manifest entry"));
                }
            }
        }
        for a in &self.assets {
            let exists = root.join(&a.path).exists();
            match a.status {
                AssetStatus::Active if !exists => {
                    problems.push(format!("{}: active but missing on disk", a.path))
                }
                AssetStatus::Removed if exists => problems.push(format!(
                    "{}: marked removed but still present — delete it",
                    a.path
                )),
                _ => {}
            }
        }
        problems
    }
}

fn normalise(p: &str) -> String {
    p.replace('\\', "/").trim_start_matches("./").to_string()
}

/// Every audio file under `dir`, recursively (empty when the folder does not exist).
pub fn audio_files_under(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p
                .extension()
                .map(|x| {
                    let x = x.to_string_lossy().to_ascii_lowercase();
                    AUDIO_EXTENSIONS.contains(&x.as_str())
                })
                .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    /// The CI gate (F1A.8): every shipped audio file has a licence record.
    #[test]
    fn repository_assets_are_all_licensed() {
        let root = repo_root();
        let m = AssetManifest::load(&root.join("assets/asset-manifest.json")).unwrap();
        let problems = m.check_against_disk(
            &root,
            &[
                "assets",
                "apps/desktop/src-tauri/resources",
                "apps/desktop/public",
            ],
        );
        assert!(problems.is_empty(), "{problems:#?}");
        assert!(root.join("docs/AUDIO-LICENSING.md").exists());
        assert!(root.join("assets/ATTRIBUTION.md").exists());
    }

    fn record(license: &str) -> AssetRecord {
        AssetRecord {
            path: "assets/audio/x.wav".into(),
            sha256: "ab".repeat(32),
            license: license.into(),
            creator: "Someone".into(),
            origin: "https://example.org/sound/1 by someone, 2026-10-17".into(),
            attribution: "Someone — Sound (CC BY 4.0)".into(),
            added: "2026-10-17".into(),
            status: AssetStatus::Active,
            removed_reason: None,
        }
    }

    #[test]
    fn manifest_rules_reject_bad_licences_and_missing_provenance() {
        let ok = AssetManifest {
            schema_version: 1,
            note: String::new(),
            assets: vec![record("CC-BY-4.0")],
        };
        assert!(ok.validate().is_empty(), "{:?}", ok.validate());
        for bad in ["CC-BY-NC", "cc-by-nc-4.0", "personal-use", "unknown", "GPL"] {
            let m = AssetManifest {
                assets: vec![record(bad)],
                ..ok.clone()
            };
            assert!(!m.validate().is_empty(), "{bad} must fail");
        }
        let mut cc0 = record("CC0-1.0");
        cc0.origin = "found on the internet".into();
        let m = AssetManifest {
            assets: vec![cc0],
            ..ok.clone()
        };
        assert!(m.validate().iter().any(|p| p.contains("provenance")));
        let mut removed = record("original");
        removed.status = AssetStatus::Removed;
        let m = AssetManifest {
            assets: vec![removed],
            ..ok.clone()
        };
        assert!(m.validate().iter().any(|p| p.contains("without a reason")));
    }

    #[test]
    fn disk_check_finds_unlisted_audio_and_lingering_removed_files() {
        let dir = tempfile::tempdir().unwrap();
        let shipped = dir.path().join("assets/audio");
        std::fs::create_dir_all(&shipped).unwrap();
        std::fs::write(shipped.join("stray.wav"), b"RIFF").unwrap();
        std::fs::write(shipped.join("gone.wav"), b"RIFF").unwrap();
        let mut gone = record("original");
        gone.path = "assets/audio/gone.wav".into();
        gone.status = AssetStatus::Removed;
        gone.removed_reason = Some("uncertain rights".into());
        let mut missing = record("original");
        missing.path = "assets/audio/missing.wav".into();
        let m = AssetManifest {
            schema_version: 1,
            note: String::new(),
            assets: vec![gone, missing],
        };
        let problems = m.check_against_disk(dir.path(), &["assets"]);
        assert!(
            problems.iter().any(|p| p.contains("stray.wav")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("still present")),
            "{problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.contains("missing on disk")),
            "{problems:?}"
        );
        std::fs::remove_file(shipped.join("gone.wav")).unwrap();
        let after = m.check_against_disk(dir.path(), &["assets"]);
        assert!(!after.iter().any(|p| p.contains("still present")));
    }
}
