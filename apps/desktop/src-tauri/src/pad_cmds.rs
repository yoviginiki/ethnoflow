//! Liquid pad commands (F3.3): a chord of up to six notes on graph slot 1, an emotion
//! preset with the X/Y macro on top, intonation like the drone. Chord and macro
//! changes go through atomics: voices crossfade, nothing restarts.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::{Emotion, GraphCmd, PadMacro, PadParams, PadSource, PAD_VOICES};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::graph_cmds::{ensure_graph, note_hz, release_slot, Intonation, PAD_SLOT};
use crate::state::{AppState, Inner};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PadNote {
    pub midi: u8,
    pub cents: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadSettings {
    /// At most `PAD_VOICES` notes; empty = silence (voices release).
    pub notes: Vec<PadNote>,
    pub emotion: Emotion,
    /// X/Y macro in 0..1 (0.5/0.5 = the preset as it is).
    pub x: f32,
    pub y: f32,
    pub level_db: f32,
    /// Filter LFO rate; the UI derives it from the project tempo when it wants sync.
    pub lfo_hz: f32,
    pub seed: u32,
    pub intonation: Intonation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadStatus {
    pub sample_rate: u32,
    pub notes_hz: Vec<f32>,
    /// `profile` when every note had a profile measurement.
    pub intonation_used: Intonation,
    /// The macro after the X/Y offset, so the UI can show what the sound really is.
    pub macro_applied: PadMacro,
    pub layer_peak: f32,
    pub master_peak: f32,
    pub limiter_gain: f32,
    pub stream_errors: u64,
}

fn validate(set: &PadSettings) -> CmdResult<()> {
    if set.notes.len() > PAD_VOICES {
        return Err(err("invalid_argument", "at most six pad notes"));
    }
    for n in &set.notes {
        if n.midi > 127 || !(-100.0..=100.0).contains(&n.cents) {
            return Err(err(
                "invalid_argument",
                "pad note outside the allowed range",
            ));
        }
    }
    if !(0.0..=1.0).contains(&set.x) || !(0.0..=1.0).contains(&set.y) {
        return Err(err("invalid_argument", "x/y must be 0..1"));
    }
    if !(-60.0..=6.0).contains(&set.level_db) {
        return Err(err("invalid_argument", "level outside -60..6 dB"));
    }
    if !(0.005..=8.0).contains(&set.lfo_hz) {
        return Err(err("invalid_argument", "lfo rate outside 0.005..8 Hz"));
    }
    Ok(())
}

pub(crate) fn apply(s: &Inner, params: &PadParams, set: &PadSettings) -> (Vec<f32>, Intonation) {
    let mut used = Intonation::Profile;
    let mut hz = Vec::with_capacity(set.notes.len());
    for n in &set.notes {
        let (f, u) = note_hz(s, n.midi, n.cents, set.intonation);
        if u != Intonation::Profile {
            used = Intonation::Tempered;
        }
        hz.push(f);
    }
    for (i, v) in params.voices.iter().enumerate() {
        v.set(hz.get(i).copied().unwrap_or(0.0));
    }
    set.emotion.preset().with_xy(set.x, set.y).apply(params);
    params.lfo_hz.set(set.lfo_hz);
    params.seed.store(set.seed, Ordering::Relaxed);
    if let Some(g) = s.graph.as_ref() {
        g.params().layers[PAD_SLOT].gain_db.set(set.level_db);
    }
    (hz, used)
}

fn status(
    s: &mut Inner,
    set: &PadSettings,
    hz: Vec<f32>,
    used: Intonation,
) -> CmdResult<PadStatus> {
    let graph = s
        .graph
        .as_mut()
        .ok_or_else(|| err("no_pad", "The pad is not running"))?;
    let st = graph.status();
    let m = graph.params().meters(2);
    Ok(PadStatus {
        sample_rate: st.sample_rate,
        notes_hz: hz,
        intonation_used: used,
        macro_applied: set.emotion.preset().with_xy(set.x, set.y),
        layer_peak: m.layers.get(PAD_SLOT).copied().unwrap_or(0.0),
        master_peak: m.master_peak,
        limiter_gain: m.limiter_gain,
        stream_errors: st.stream_errors,
    })
}

#[tauri::command]
pub async fn start_pad(
    state: State<'_, AppState>,
    device: Option<String>,
    settings: PadSettings,
) -> CmdResult<PadStatus> {
    validate(&settings)?;
    let mut s = state.lock();
    ensure_graph(&mut s, device.as_deref())?;
    let params = Arc::new(PadParams::default());
    let (hz, used) = apply(&s, &params, &settings);
    let graph = s.graph.as_mut().expect("graph just started");
    let sr = graph.sample_rate();
    graph.params().layers[PAD_SLOT].gain_db.set(-60.0);
    let ok = graph.send(GraphCmd::SetSource {
        slot: PAD_SLOT,
        source: Box::new(PadSource::new(Arc::clone(&params)).with_sample_rate(sr)),
    });
    if !ok {
        return Err(err("graph_busy", "Graph command queue is full"));
    }
    graph.params().layers[PAD_SLOT]
        .gain_db
        .set(settings.level_db);
    s.pad = Some(params);
    s.pad_settings = Some(settings.clone());
    crate::bus_cmds::refresh(&mut s, crate::bus_cmds::BUS_PAD);
    status(&mut s, &settings, hz, used)
}

#[tauri::command]
pub async fn set_pad(state: State<'_, AppState>, settings: PadSettings) -> CmdResult<PadStatus> {
    validate(&settings)?;
    let mut s = state.lock();
    let Some(params) = s.pad.clone() else {
        return Err(err("no_pad", "The pad is not running"));
    };
    let (hz, used) = apply(&s, &params, &settings);
    s.pad_settings = Some(settings.clone());
    crate::bus_cmds::refresh(&mut s, crate::bus_cmds::BUS_PAD);
    status(&mut s, &settings, hz, used)
}

#[tauri::command]
pub async fn stop_pad(state: State<'_, AppState>) -> CmdResult<()> {
    release_slot(&mut state.lock(), PAD_SLOT);
    Ok(())
}

#[tauri::command]
pub async fn pad_status(state: State<'_, AppState>) -> CmdResult<Option<PadStatus>> {
    let mut s = state.lock();
    let Some(settings) = s.pad_settings.clone() else {
        return Ok(None);
    };
    let mut hz = Vec::new();
    let mut used = Intonation::Profile;
    for n in &settings.notes {
        let (f, u) = note_hz(&s, n.midi, n.cents, settings.intonation);
        if u != Intonation::Profile {
            used = Intonation::Tempered;
        }
        hz.push(f);
    }
    status(&mut s, &settings, hz, used).map(Some)
}

/// The emotion presets, for the UI to show what each one does.
#[tauri::command]
pub async fn pad_emotions() -> CmdResult<Vec<(Emotion, PadMacro)>> {
    Ok(Emotion::ALL.iter().map(|e| (*e, e.preset())).collect())
}
