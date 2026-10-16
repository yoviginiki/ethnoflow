//! Application state shared across Tauri commands.
//!
//! Audio lives on disk in the open project; in memory there is at most one loaded take
//! (the selected one, trimmed) as an immutable `Arc<AudioBuffer>` for playback and
//! peaks. Raw frames never leave the Rust side.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use audio_engine::{AudioBuffer, Metronome, Player, Recorder};
use project::{Project, TakeHealth};

pub struct ActiveRecording {
    pub recorder: Recorder,
    pub take_id: String,
    /// Length of the count-in, in seconds; becomes the take's trim-in on stop.
    pub count_in_seconds: f64,
}

pub struct LoadedTake {
    pub id: String,
    /// The playable region (trim applied) — a copy; the original file is untouched.
    pub buffer: Arc<AudioBuffer>,
}

#[derive(Default)]
pub struct Inner {
    pub project: Option<Project>,
    /// Result of the last checksum verification, by take id.
    pub health: HashMap<String, TakeHealth>,
    pub recording: Option<ActiveRecording>,
    pub metronome: Option<Metronome>,
    pub player: Option<Player>,
    pub tone: Option<Player>,
    pub loaded: Option<LoadedTake>,
}

impl Inner {
    pub fn stop_playback(&mut self) {
        if let Some(p) = self.player.take() {
            p.stop();
        }
    }

    pub fn stop_metronome(&mut self) {
        if let Some(m) = self.metronome.take() {
            m.stop();
        }
    }
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
