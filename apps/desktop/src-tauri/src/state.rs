//! Application state shared across Tauri commands.
//!
//! One "take" (the current recording or imported file) lives in memory as an immutable
//! `Arc<AudioBuffer>`. Phase 1 replaces this with the project system (SQLite + on-disk
//! originals); the command surface is designed so the UI will not need to change.

use std::sync::{Arc, Mutex};

use audio_engine::{AudioBuffer, Player, Recorder};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TakeSource {
    None,
    Microphone,
    ImportedFile,
}

#[derive(Default)]
pub struct Inner {
    pub recorder: Option<Recorder>,
    pub player: Option<Player>,
    pub take: Option<Arc<AudioBuffer>>,
    pub take_source: Option<TakeSource>,
    pub take_label: Option<String>,
}

#[derive(Default)]
pub struct AppState {
    pub inner: Mutex<Inner>,
}

impl AppState {
    pub fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A poisoned mutex means a command panicked; recovering the data is still correct here.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}
