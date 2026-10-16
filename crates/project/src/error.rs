use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("project.json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("audio error: {0}")]
    Audio(#[from] audio_engine::AudioError),
    #[error("not an EthnoFlow project: {0}")]
    NotAProject(PathBuf),
    #[error("already exists: {0}")]
    AlreadyExists(PathBuf),
    #[error("invalid project name")]
    InvalidName,
    #[error("take not found: {0}")]
    TakeNotFound(String),
    #[error("{what} schema version {found} is newer than this app supports ({supported})")]
    NewerSchema {
        what: &'static str,
        found: u32,
        supported: u32,
    },
    #[error("checksum mismatch for {0}")]
    ChecksumMismatch(String),
    #[error("invalid state: {0}")]
    InvalidState(String),
}

pub type Result<T> = std::result::Result<T, ProjectError>;
