//! The audio graph in the app (F1.6) and the drone on it (F3.2). One output stream
//! owns the graph; slot 0 is the drone, slot 1 the pad (`pad_cmds.rs`), slot 2 the
//! arrangement's clips (`arrangement_cmds.rs`), slots 4..12 the ambient rack
//! (`ambient_cmds.rs`). Parameters
//! change through atomics (no restart, no click); notes are intonated to the
//! project's instrument profile when the user asks for that, otherwise to equal
//! temperament at the project's A4.

use std::sync::Arc;

use audio_engine::{DroneParams, DroneSource, GraphCmd, GraphParams, GraphPlayer};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::analysis_cmds::DEFAULT_A4_HZ;
use crate::commands::{err, CmdResult};
use crate::state::{AppState, Inner};

pub const DRONE_SLOT: usize = 0;
pub const PAD_SLOT: usize = 1;
/// Headphone-safe master level (the layer level is the user's control).
const DEFAULT_MASTER_DB: f32 = 0.0;

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
    /// Layer level in dB (−60..0).
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

/// A note's frequency under the chosen intonation. Returns the Hz and whether the
/// profile was actually used (it has a measurement for that note).
pub fn note_hz(s: &Inner, midi: u8, cents: f64, intonation: Intonation) -> (f32, Intonation) {
    let project_a4 = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(DEFAULT_A4_HZ);
    if intonation == Intonation::Profile {
        if let Some(profile) = crate::instrument_cmds::project_profile(s) {
            if let Some(n) = profile.note(midi) {
                // The profile's offsets are relative to its own A4.
                let hz =
                    audio_engine::drone_root_hz(midi as f64, n.cents_offset + cents, profile.a4_hz);
                return (hz, Intonation::Profile);
            }
        }
    }
    (
        audio_engine::drone_root_hz(midi as f64, cents, project_a4),
        Intonation::Tempered,
    )
}

/// Start the output stream if nothing on the graph is playing yet.
pub fn ensure_graph(s: &mut Inner, device: Option<&str>) -> CmdResult<()> {
    if s.graph.is_none() {
        let gp = Arc::new(GraphParams::default());
        gp.master
            .gain_db
            .set(s.master_db.unwrap_or(DEFAULT_MASTER_DB));
        for l in gp.layers.iter() {
            l.gain_db.set(-60.0);
        }
        s.graph = Some(GraphPlayer::start(device, gp)?);
    }
    Ok(())
}

/// Fade a slot out, clear it, and stop the stream when no slot is in use any more.
pub fn release_slot(s: &mut Inner, slot: usize) {
    if let Some(g) = s.graph.as_mut() {
        g.params().layers[slot].gain_db.set(-60.0);
        std::thread::sleep(std::time::Duration::from_millis(30));
        g.send(GraphCmd::Clear { slot });
    }
    match slot {
        DRONE_SLOT => {
            s.drone = None;
            s.drone_settings = None;
        }
        PAD_SLOT => {
            s.pad = None;
            s.pad_settings = None;
        }
        crate::arrangement_cmds::ARRANGE_SLOT => s.clips = None,
        _ => {}
    }
    if s.graph_idle() {
        s.stop_graph();
    }
}

pub(crate) struct Resolved {
    root_hz: f32,
    root_cents: f64,
    fifth_cents: f32,
    used: Intonation,
}

/// Root and fifth frequencies for the settings. Nothing here assumes a key: the root
/// is whatever the user chose.
pub(crate) fn resolve(s: &Inner, set: &DroneSettings) -> Resolved {
    let (root_hz, used) = note_hz(s, set.root_midi, set.root_cents, set.intonation);
    let (tempered, _) = note_hz(s, set.root_midi, 0.0, Intonation::Tempered);
    let root_cents = 1200.0 * (root_hz as f64 / tempered as f64).log2();
    let fifth_cents = if used == Intonation::Profile {
        let (fifth_hz, fifth_used) =
            note_hz(s, set.root_midi.saturating_add(7), 0.0, Intonation::Profile);
        if fifth_used == Intonation::Profile {
            let ratio = fifth_hz as f64 / root_hz as f64;
            (1200.0 * ratio.log2() - 700.0) as f32 + (set.root_cents as f32)
        } else {
            0.0
        }
    } else {
        0.0
    };
    Resolved {
        root_hz,
        root_cents,
        fifth_cents,
        used,
    }
}

pub(crate) fn apply(
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
    let m = graph.params().meters(2);
    Ok(DroneStatus {
        sample_rate: st.sample_rate,
        root_hz: r.root_hz,
        root_cents_applied: r.root_cents,
        fifth_cents_applied: r.fifth_cents,
        intonation_used: r.used,
        layer_peak: m.layers.get(DRONE_SLOT).copied().unwrap_or(0.0),
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
    if !(-60.0..=6.0).contains(&set.level_db) {
        return Err(err("invalid_argument", "level outside -60..6 dB"));
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
    ensure_graph(&mut s, device.as_deref())?;
    let graph = s.graph.as_mut().expect("graph just started");
    // Start silent and ramp to the level: no click on start.
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
    crate::bus_cmds::refresh(&mut s, crate::bus_cmds::BUS_DRONE);
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
    crate::bus_cmds::refresh(&mut s, crate::bus_cmds::BUS_DRONE);
    status(&mut s, &r)
}

#[tauri::command]
pub async fn stop_drone(state: State<'_, AppState>) -> CmdResult<()> {
    release_slot(&mut state.lock(), DRONE_SLOT);
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

/// Master level of the background (drone + pad), in dB; remembered for the next start.
#[tauri::command]
pub async fn set_graph_master(state: State<'_, AppState>, db: f32) -> CmdResult<()> {
    if !(-120.0..=6.0).contains(&db) {
        return Err(err("invalid_argument", "master outside -120..6 dB"));
    }
    let mut s = state.lock();
    s.master_db = Some(db);
    if let Some(g) = s.graph.as_ref() {
        g.params().master.gain_db.set(db);
    }
    Ok(())
}
