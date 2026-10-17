//! Live synth commands (F2.7.3): the computer keyboard (and later MIDI) plays notes.

use audio_engine::{LiveSynth, LiveSynthStatus, SynthCmd, Voice};
use tauri::State;

use crate::analysis_cmds::DEFAULT_A4_HZ;
use crate::commands::{err, CmdResult};
use crate::state::AppState;

#[tauri::command]
pub async fn start_live_synth(
    state: State<'_, AppState>,
    device: Option<String>,
    voice: Option<Voice>,
) -> CmdResult<LiveSynthStatus> {
    let mut s = state.lock();
    if let Some(old) = s.live_synth.take() {
        old.stop();
    }
    let a4 = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(DEFAULT_A4_HZ);
    let synth = LiveSynth::start(device.as_deref(), a4, voice.unwrap_or(Voice::Flute))?;
    let st = synth.status();
    s.live_synth = Some(synth);
    Ok(st)
}

#[tauri::command]
pub async fn stop_live_synth(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(s) = state.lock().live_synth.take() {
        s.stop();
    }
    Ok(())
}

#[tauri::command]
pub async fn synth_note(
    state: State<'_, AppState>,
    midi: u8,
    on: bool,
    velocity: Option<f32>,
) -> CmdResult<()> {
    if midi > 127 {
        return Err(err("invalid_argument", "midi outside 0..127"));
    }
    let mut s = state.lock();
    let synth = s
        .live_synth
        .as_mut()
        .ok_or_else(|| err("no_synth", "The live synth is not running"))?;
    let cmd = if on {
        SynthCmd::On {
            midi,
            velocity: velocity.unwrap_or(0.7),
        }
    } else {
        SynthCmd::Off { midi }
    };
    if !synth.send(cmd) {
        return Err(err("synth_busy", "Synth command queue is full"));
    }
    Ok(())
}

#[tauri::command]
pub async fn synth_all_off(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(synth) = state.lock().live_synth.as_mut() {
        synth.send(SynthCmd::AllOff);
    }
    Ok(())
}

#[tauri::command]
pub async fn live_synth_status(state: State<'_, AppState>) -> CmdResult<Option<LiveSynthStatus>> {
    Ok(state.lock().live_synth.as_ref().map(|s| s.status()))
}
