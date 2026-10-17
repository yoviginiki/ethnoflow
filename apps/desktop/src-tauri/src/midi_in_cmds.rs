//! MIDI keyboard input (F1.4.4): a forwarding thread drains the port's ring every
//! ~2 ms into the live synth (when running) and into a ring the UI polls.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use audio_engine::{list_midi_inputs, MidiEvent, MidiIn, MidiPortInfo, SynthCmd};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::commands::{err, CmdResult};
use crate::state::AppState;

const UI_QUEUE_MAX: usize = 4096;

pub struct MidiInput {
    pub name: String,
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    /// Events for the UI (drained by `poll_midi_events`).
    pub queue: Arc<Mutex<Vec<MidiEvent>>>,
}

impl Drop for MidiInput {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MidiInputStatus {
    pub name: String,
    pub events: Vec<MidiEvent>,
}

#[tauri::command]
pub async fn list_midi_ports() -> CmdResult<Vec<MidiPortInfo>> {
    Ok(list_midi_inputs()?)
}

#[tauri::command]
pub async fn open_midi_input(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> CmdResult<()> {
    let mut port = MidiIn::open(&name)?;
    let stop = Arc::new(AtomicBool::new(false));
    let queue = Arc::new(Mutex::new(Vec::new()));
    let (t_stop, t_queue) = (Arc::clone(&stop), Arc::clone(&queue));
    let join = thread::Builder::new()
        .name("ethnoflow-midi-forward".into())
        .spawn(move || {
            let mut buf = Vec::with_capacity(64);
            while !t_stop.load(Ordering::Acquire) {
                buf.clear();
                port.drain(&mut buf);
                if !buf.is_empty() {
                    // Live synth first (lowest latency), then the UI queue.
                    let st = app.state::<AppState>();
                    let mut s = st.lock();
                    if let Some(synth) = s.live_synth.as_mut() {
                        for ev in &buf {
                            match *ev {
                                MidiEvent::NoteOn { note, velocity, .. } => {
                                    synth.send(SynthCmd::On {
                                        midi: note,
                                        velocity: velocity as f32 / 127.0,
                                    });
                                }
                                MidiEvent::NoteOff { note, .. } => {
                                    synth.send(SynthCmd::Off { midi: note });
                                }
                                MidiEvent::Other { .. } => {}
                            }
                        }
                    }
                    drop(s);
                    let mut q = t_queue.lock().unwrap_or_else(|e| e.into_inner());
                    q.extend_from_slice(&buf);
                    if q.len() > UI_QUEUE_MAX {
                        let cut = q.len() - UI_QUEUE_MAX;
                        q.drain(..cut);
                    }
                }
                thread::sleep(Duration::from_millis(2));
            }
        })
        .map_err(|e| err("thread", e.to_string()))?;
    let mut s = state.lock();
    s.midi_input = Some(MidiInput {
        name,
        stop,
        join: Some(join),
        queue,
    });
    Ok(())
}

#[tauri::command]
pub async fn close_midi_input(state: State<'_, AppState>) -> CmdResult<()> {
    state.lock().midi_input = None; // Drop stops the thread and closes the port
    Ok(())
}

/// Events received since the last poll (for keyboard lights and the trainer).
#[tauri::command]
pub async fn poll_midi_events(state: State<'_, AppState>) -> CmdResult<Option<MidiInputStatus>> {
    let s = state.lock();
    let Some(m) = s.midi_input.as_ref() else {
        return Ok(None);
    };
    let events = std::mem::take(&mut *m.queue.lock().unwrap_or_else(|e| e.into_inner()));
    Ok(Some(MidiInputStatus {
        name: m.name.clone(),
        events,
    }))
}
