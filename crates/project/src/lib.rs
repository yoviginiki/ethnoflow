//! EthnoFlow project format and persistence.
//!
//! A project is a folder `<name>.ethnoflow/` (master plan §10):
//! `project.json` (portable manifest), `project.db` (SQLite metadata, checksums,
//! settings, migrations) and `assets/originals/` (immutable recordings and imports).
//! See docs/DECISIONS.md ADR-0015.

pub mod ambient;
pub mod ambient_presets;
pub mod arrangement;
pub mod db;
pub mod error;
pub mod instrument;
pub mod manifest;
pub mod project;

pub use ambient::{Ambient, AmbientLayer, LayerSource, Macros, AMBIENT_FILE, MAX_AMBIENT_LAYERS};
pub use ambient_presets::{factory_presets, Preset, TAGS as AMBIENT_TAGS};
pub use arrangement::{
    Arrangement, Clip, Track, ARRANGEMENT_FILE, MAX_TRACKS as MAX_ARRANGEMENT_TRACKS,
};
pub use db::{Take, TakeSource, TakeState};
pub use error::{ProjectError, Result};
pub use instrument::{
    InstrumentProfile, InstrumentType, ProfileDiagnosis, ProfileNote, ProfileStore, ProfileSummary,
};
pub use manifest::{Manifest, Marker, TempoPoint, TimeSignaturePoint};
pub use project::{
    checksum_file, sanitize_name, OpenReport, Project, TakeCheck, TakeHealth, DERIVED_DIR,
    ORIGINALS_DIR, PROJECT_EXT,
};

/// Milliseconds since the epoch (for ids the shell generates).
pub fn now_ms_pub() -> i64 {
    now_ms()
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
