//! Trainer (F2.8): the evaluation lives in music-core; this just exposes it with the
//! level presets. Modes (Listen / Wait / Rhythm) run in the UI on the tuner's pitch.

use music_core::{evaluate, Note, Report, Tolerance};
use serde::{Deserialize, Serialize};

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

// ---------------------------------------------------------------- latency (F1.4 / F2.8)

#[derive(Debug, Clone, Serialize)]
pub struct LatencyResult {
    pub ms: f64,
    pub spread_ms: f64,
    pub per_click_ms: Vec<Option<f64>>,
    pub heard: usize,
    pub clicks: usize,
}

const LATENCY_CLICKS: [f64; 4] = [0.5, 1.0, 1.5, 2.0];
const LATENCY_SETTLE_S: f64 = 0.4;
const LATENCY_WINDOW_S: f64 = 0.4;

/// Round-trip latency: records the input while a click burst plays on the output and
/// measures when each click arrives. Blocks for ≈ 4 s (runs off the main thread); the
/// state lock is released while the streams run.
#[tauri::command]
pub async fn measure_latency(
    state: tauri::State<'_, crate::state::AppState>,
    input: Option<String>,
    output: Option<String>,
    sample_rate: Option<u32>,
) -> CmdResult<LatencyResult> {
    use audio_engine::{
        click_burst, estimate_roundtrip, read_wav, Player, Recorder, StreamRequest,
    };
    let tmp = {
        let mut s = state.lock();
        if s.mode_busy() {
            return Err(err("busy", "Stop recording or playback first"));
        }
        s.stop_monitor();
        if let Some(t) = s.tone.take() {
            t.stop();
        }
        std::env::temp_dir().join(format!(
            "ethnoflow-latency-{}.wav",
            crate::reference_cmds::now_ms()
        ))
    };
    let rec = Recorder::start_with(
        input.as_deref(),
        &tmp,
        StreamRequest {
            sample_rate,
            channels: None,
        },
    )?;
    let t0 = std::time::Instant::now();
    std::thread::sleep(std::time::Duration::from_secs_f64(LATENCY_SETTLE_S));
    let burst = click_burst(48_000, &LATENCY_CLICKS, 0.005, 0.7);
    let player = Player::start(&burst, output.as_deref(), 0)?;
    let started_at = t0.elapsed().as_secs_f64();
    std::thread::sleep(std::time::Duration::from_secs_f64(
        LATENCY_CLICKS[LATENCY_CLICKS.len() - 1] + LATENCY_WINDOW_S + 0.3,
    ));
    player.stop();
    let recorded = rec.stop()?;
    let buf = read_wav(&recorded.path)?;
    let _ = std::fs::remove_file(&tmp);
    let est = estimate_roundtrip(&buf, &LATENCY_CLICKS, started_at, LATENCY_WINDOW_S)
        .ok_or_else(|| {
            err(
                "no_clicks_heard",
                "The microphone did not hear the clicks: turn the speakers up a little, or hold the headphones to the microphone",
            )
        })?;
    Ok(LatencyResult {
        ms: est.ms,
        spread_ms: est.spread_ms,
        heard: est.per_click_ms.iter().filter(|p| p.is_some()).count(),
        clicks: LATENCY_CLICKS.len(),
        per_click_ms: est.per_click_ms,
    })
}

// ---------------------------------------------------------------- loop drill (F2.8)

/// Play the current notes inside `[start_s, end_s)` once, slowed by `tempo_factor`
/// (0.25..=1.0), through the notes player, so a drill pass can be evaluated like the
/// Rhythm mode. Positions reported by `notes_playback_status` are on the slowed timeline.
#[tauri::command]
pub async fn play_notes_drill(
    state: tauri::State<'_, crate::state::AppState>,
    start_s: f64,
    end_s: f64,
    tempo_factor: f64,
    device: Option<String>,
    voice: Option<audio_engine::Voice>,
) -> CmdResult<audio_engine::PlayerStatus> {
    use audio_engine::{render_notes, Player, SynthConfig, Voice};
    if !(start_s.is_finite() && end_s.is_finite() && end_s > start_s)
        || !(0.25..=1.0).contains(&tempo_factor)
    {
        return Err(err("invalid_argument", "bad drill range or tempo factor"));
    }
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording first"));
    }
    s.stop_playback();
    let all = crate::analysis_cmds::current_notes(&s)?;
    let notes: Vec<Note> = all
        .into_iter()
        .filter(|n| n.start >= start_s - 1e-6 && n.start < end_s)
        .map(|n| Note {
            start: (n.start - start_s) / tempo_factor,
            end: (n.end.min(end_s) - start_s) / tempo_factor,
            bend: n.bend.iter().map(|(t, c)| (t / tempo_factor, *c)).collect(),
            ..n
        })
        .collect();
    if notes.is_empty() {
        return Err(err("no_notes", "There are no notes in the drill range"));
    }
    let a4 = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(crate::analysis_cmds::DEFAULT_A4_HZ);
    let cfg = SynthConfig {
        a4_hz: a4,
        voice: voice.unwrap_or(Voice::Flute),
        ..SynthConfig::new(crate::midi_cmds::RENDER_SAMPLE_RATE)
    };
    let buf = render_notes(&crate::midi_cmds::synth_notes(&notes), &cfg);
    let player = Player::start(&buf, device.as_deref(), 0)?;
    let st = player.status();
    s.notes_player = Some(player);
    Ok(st)
}
