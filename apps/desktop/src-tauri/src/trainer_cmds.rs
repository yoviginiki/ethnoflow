//! Trainer (F2.8): the evaluation lives in music-core; this just exposes it with the
//! level presets. Modes (Listen / Wait / Rhythm) run in the UI on the tuner's pitch.

use music_core::{evaluate, Note, Report, Tolerance};
use serde::Deserialize;

use crate::commands::{err, CmdResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Beginner,
    Intermediate,
    Advanced,
}

impl Level {
    pub fn tolerance(self) -> Tolerance {
        match self {
            Level::Beginner => Tolerance::beginner(),
            Level::Intermediate => Tolerance::intermediate(),
            Level::Advanced => Tolerance::advanced(),
        }
    }
}

#[tauri::command]
pub async fn evaluate_performance(
    expected: Vec<Note>,
    played: Vec<Note>,
    level: Level,
    latency_ms: f64,
) -> CmdResult<Report> {
    if !latency_ms.is_finite() || !(-500.0..=500.0).contains(&latency_ms) {
        return Err(err("invalid_argument", "latency outside ±500 ms"));
    }
    Ok(evaluate(&expected, &played, &level.tolerance(), latency_ms))
}
