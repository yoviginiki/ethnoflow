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
use crate::state::{AppState, Inner, KeyboardTarget};

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
            // K4: the sustain pedal (CC 64) holds note-offs until it is released.
            let mut sustain = false;
            let mut pending = [false; 128];
            while !t_stop.load(Ordering::Acquire) {
                buf.clear();
                port.drain(&mut buf);
                if !buf.is_empty() {
                    // Sound first (lowest latency), then the UI queue.
                    let st = app.state::<AppState>();
                    let mut s = st.lock();
                    for ev in &buf {
                        match *ev {
                            MidiEvent::NoteOn { note, velocity, .. } => {
                                pending[note as usize] = false;
                                send_note(&mut s, note, true, velocity as f32 / 127.0);
                            }
                            MidiEvent::NoteOff { note, .. } => {
                                if sustain {
                                    pending[note as usize] = true;
                                } else {
                                    send_note(&mut s, note, false, 0.0);
                                }
                            }
                            MidiEvent::Control {
                                controller: 64,
                                value,
                                ..
                            } => {
                                let down = value >= 64;
                                if sustain && !down {
                                    for (n, p) in pending.iter_mut().enumerate() {
                                        if *p {
                                            *p = false;
                                            send_note(&mut s, n as u8, false, 0.0);
                                        }
                                    }
                                }
                                sustain = down;
                            }
                            MidiEvent::Control {
                                controller: 1,
                                value,
                                ..
                            } => send_control(&mut s, None, Some(value as f32 / 127.0)),
                            MidiEvent::PitchBend { value, .. } => {
                                // The wheel spans ±2 semitones (the common default).
                                send_control(&mut s, Some(value as f32 / 8192.0 * 2.0), None)
                            }
                            MidiEvent::Control { .. } | MidiEvent::Other { .. } => {}
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

/// One note to wherever the keyboard points (K1/K2): the auditioned instrument when
/// chosen and running, else the live synth when it runs.
fn send_note(s: &mut Inner, midi: u8, on: bool, velocity: f32) {
    if s.keyboard_target == KeyboardTarget::Instrument {
        if let Some(a) = s.audition.as_mut() {
            a.note(midi, on, velocity);
            return;
        }
    }
    if let Some(synth) = s.live_synth.as_mut() {
        synth.send(if on {
            SynthCmd::On { midi, velocity }
        } else {
            SynthCmd::Off { midi }
        });
    }
}

/// Pitch bend (semitones) and / or mod wheel (0..1) to wherever the keyboard points
/// (K4): the auditioned instrument's live block, or the live synth.
pub(crate) fn send_control(s: &mut Inner, bend: Option<f32>, modulation: Option<f32>) {
    if s.keyboard_target == KeyboardTarget::Instrument {
        if let Some(a) = s.audition.as_ref() {
            if let Some(b) = bend {
                a.params.live().set_bend(b);
            }
            if let Some(m) = modulation {
                a.params.live().set_modulation(m);
            }
            return;
        }
    }
    if let Some(synth) = s.live_synth.as_mut() {
        if let Some(b) = bend {
            synth.send(SynthCmd::Bend { semitones: b });
        }
        if let Some(m) = modulation {
            synth.send(SynthCmd::Modulation { amount: m });
        }
    }
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
