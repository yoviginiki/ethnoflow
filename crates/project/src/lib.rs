//! EthnoFlow project format and persistence.
//!
//! A project is a folder `<name>.ethnoflow/` (master plan §10):
//! `project.json` (portable manifest), `project.db` (SQLite metadata, checksums,
//! settings, migrations) and `assets/originals/` (immutable recordings and imports).
//! See docs/DECISIONS.md ADR-0015.

pub mod arrangement;
pub mod db;
pub mod error;
pub mod instrument;
pub mod manifest;
pub mod project;

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

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}
