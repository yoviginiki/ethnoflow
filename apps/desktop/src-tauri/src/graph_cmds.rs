//! The audio graph in the app (F1.6) and the drone on it (F3.2). One output stream
//! owns the graph; slot 0 is the drone. Parameters change through atomics (no
//! restart, no click); the root is intonated to the project's instrument profile when
//! the user asks for that, otherwise to equal temperament at the project's A4.

use std::sync::Arc;

use audio_engine::{DroneParams, DroneSource, GraphCmd, GraphParams, GraphPlayer};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::analysis_cmds::DEFAULT_A4_HZ;
use crate::commands::{err, CmdResult};
use crate::state::{AppState, Inner};

const DRONE_SLOT: usize = 0;
/// Headphone-safe master level (the layer level is the user's control).
const MASTER_DB: f32 = -6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intonation {
    /// Follow the project's instrument profile (measured cents per note).
    Profile,
    /// Equal temperament at the project's A4.
    Tempered,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneSettings {
    pub root_midi: u8,
    /// Manual fine tuning of the root, in cents (added to the intonation).
    pub root_cents: f64,
    pub fifth: f32,
    pub octave: f32,
    pub sub: f32,
    pub warmth: f32,
    pub movement: f32,
    pub breath: f32,
    pub detune_cents: f32,
    pub cutoff_hz: f32,
    /// Layer level in dB (−40..0).
    pub level_db: f32,
    pub intonation: Intonation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroneStatus {
    pub sample_rate: u32,
    pub root_hz: f32,
    /// Cents applied to the root relative to tempered at the project A4 (profile + manual).
    pub root_cents_applied: f64,
    /// Cents applied to the fifth relative to the tempered fifth above the root.
    pub fifth_cents_applied: f32,
    /// What the intonation actually came from (`tempered` when no profile note exists).
    pub intonation_used: Intonation,
    pub layer_peak: f32,
    pub master_peak: f32,
    pub limiter_gain: f32,
    pub stream_errors: u64,
}

struct Resolved {
    root_hz: f32,
    root_cents: f64,
    fifth_cents: f32,
    used: Intonation,
}

/// Root and fifth frequencies for the settings, from the profile when asked and
/// available. Nothing here assumes a key: the root is whatever the user chose.
fn resolve(s: &Inner, set: &DroneSettings) -> Resolved {
    let project_a4 = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(DEFAULT_A4_HZ);
    let tempered =
        |cents: f64| audio_engine::drone_root_hz(set.root_midi as f64, cents, project_a4);
    if set.intonation == Intonation::Profile {
        if let Some(profile) = crate::instrument_cmds::project_profile(s) {
            if let Some(root) = profile.note(set.root_midi) {
                // The profile's offsets are relative to its own A4.
                let hz = audio_engine::drone_root_hz(
                    set.root_midi as f64,
                    root.cents_offset + set.root_cents,
                    profile.a4_hz,
                );
                let root_cents = 1200.0 * (hz as f64 / tempered(0.0) as f64).log2();
                let fifth_cents = profile
                    .note(set.root_midi.saturating_add(7))
                    .map(|n| (n.cents_offset - root.cents_offset) as f32)
                    .unwrap_or(0.0);
                return Resolved {
                    root_hz: hz,
                    root_cents,
                    fifth_cents,
                    used: Intonation::Profile,
                };
            }
        }
    }
    Resolved {
        root_hz: tempered(set.root_cents),
        root_cents: set.root_cents,
        fifth_cents: 0.0,
        used: Intonation::Tempered,
    }
}

fn apply(
    params: &DroneParams,
    layer: &audio_engine::LayerParams,
    set: &DroneSettings,
    r: &Resolved,
) {
    params.root_hz.set(r.root_hz);
    params.fifth.set(set.fifth);
    params.octave.set(set.octave);
    params.sub.set(set.sub);
    params.warmth.set(set.warmth);
    params.movement.set(set.movement);
    params.breath.set(set.breath);
    params.detune_cents.set(set.detune_cents);
    params.cutoff_hz.set(set.cutoff_hz);
    params.fifth_cents.set(r.fifth_cents);
    layer.gain_db.set(set.level_db);
}

fn status(s: &mut Inner, r: &Resolved) -> CmdResult<DroneStatus> {
    let graph = s
        .graph
        .as_mut()
        .ok_or_else(|| err("no_drone", "The drone is not running"))?;
    let st = graph.status();
    let m = graph.params().meters(1);
    Ok(DroneStatus {
        sample_rate: st.sample_rate,
        root_hz: r.root_hz,
        root_cents_applied: r.root_cents,
        fifth_cents_applied: r.fifth_cents,
        intonation_used: r.used,
        layer_peak: m.layers.first().copied().unwrap_or(0.0),
        master_peak: m.master_peak,
        limiter_gain: m.limiter_gain,
        stream_errors: st.stream_errors,
    })
}

fn validate(set: &DroneSettings) -> CmdResult<()> {
    if set.root_midi > 127 || !(-100.0..=100.0).contains(&set.root_cents) {
        return Err(err("invalid_argument", "root outside the allowed range"));
    }
    for v in [
        set.fifth,
        set.octave,
        set.sub,
        set.warmth,
        set.movement,
        set.breath,
    ] {
        if !(0.0..=1.0).contains(&v) {
            return Err(err("invalid_argument", "levels must be 0..1"));
        }
    }
    if !(-60.0..=0.0).contains(&set.level_db) {
        return Err(err("invalid_argument", "level outside -60..0 dB"));
    }
    Ok(())
}

#[tauri::command]
pub async fn start_drone(
    state: State<'_, AppState>,
    device: Option<String>,
    settings: DroneSettings,
) -> CmdResult<DroneStatus> {
    validate(&settings)?;
    let mut s = state.lock();
    let r = resolve(&s, &settings);
    let params = Arc::new(DroneParams::default());
    if s.graph.is_none() {
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(MASTER_DB);
        let player = GraphPlayer::start(device.as_deref(), gp)?;
        s.graph = Some(player);
    }
    let graph = s.graph.as_mut().expect("graph just started");
    // Start silent and ramp to the level: no click on start.
    graph.params().layers[DRONE_SLOT].gain_db.set(-60.0);
    apply(&params, &graph.params().layers[DRONE_SLOT], &settings, &r);
    graph.params().layers[DRONE_SLOT].gain_db.set(-60.0);
    let ok = graph.send(GraphCmd::SetSource {
        slot: DRONE_SLOT,
        source: Box::new(DroneSource::new(Arc::clone(&params))),
    });
    if !ok {
        return Err(err("graph_busy", "Graph command queue is full"));
    }
    graph.params().layers[DRONE_SLOT]
        .gain_db
        .set(settings.level_db);
    s.drone = Some(params);
    s.drone_settings = Some(settings);
    status(&mut s, &r)
}

#[tauri::command]
pub async fn set_drone(
    state: State<'_, AppState>,
    settings: DroneSettings,
) -> CmdResult<DroneStatus> {
    validate(&settings)?;
    let mut s = state.lock();
    let r = resolve(&s, &settings);
    let (Some(params), Some(graph)) = (s.drone.as_ref(), s.graph.as_ref()) else {
        return Err(err("no_drone", "The drone is not running"));
    };
    apply(params, &graph.params().layers[DRONE_SLOT], &settings, &r);
    s.drone_settings = Some(settings);
    status(&mut s, &r)
}

#[tauri::command]
pub async fn stop_drone(state: State<'_, AppState>) -> CmdResult<()> {
    state.lock().stop_graph();
    Ok(())
}

#[tauri::command]
pub async fn drone_status(state: State<'_, AppState>) -> CmdResult<Option<DroneStatus>> {
    let mut s = state.lock();
    let Some(settings) = s.drone_settings.clone() else {
        return Ok(None);
    };
    let r = resolve(&s, &settings);
    status(&mut s, &r).map(Some)
}
