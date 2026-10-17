//! MIDI input (ADR-0020): list ports, open one, receive note events through a
//! lock-free ring. The port callback (system thread) only parses and pushes.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use midir::{MidiInput, MidiInputConnection};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::{HeapCons, HeapProd, HeapRb};
use serde::{Deserialize, Serialize};

use crate::error::{AudioError, Result};

const CLIENT_NAME: &str = "EthnoFlow Studio";
const RING_EVENTS: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MidiEvent {
    NoteOn {
        channel: u8,
        note: u8,
        velocity: u8,
        t_us: u64,
    },
    NoteOff {
        channel: u8,
        note: u8,
        t_us: u64,
    },
    /// Any other channel message (CC, pitch bend, …) — counted, not interpreted yet.
    Other {
        status: u8,
        t_us: u64,
    },
}

/// Parse one complete MIDI message (midir delivers whole messages).
pub fn parse_message(bytes: &[u8], t_us: u64) -> Option<MidiEvent> {
    let &status = bytes.first()?;
    if status < 0x80 {
        return None; // not a status byte (no running status from midir)
    }
    let channel = status & 0x0f;
    match status & 0xf0 {
        0x90 if bytes.len() >= 3 => {
            if bytes[2] == 0 {
                Some(MidiEvent::NoteOff {
                    channel,
                    note: bytes[1] & 0x7f,
                    t_us,
                })
            } else {
                Some(MidiEvent::NoteOn {
                    channel,
                    note: bytes[1] & 0x7f,
                    velocity: bytes[2] & 0x7f,
                    t_us,
                })
            }
        }
        0x80 if bytes.len() >= 3 => Some(MidiEvent::NoteOff {
            channel,
            note: bytes[1] & 0x7f,
            t_us,
        }),
        0x80 | 0x90 => None, // truncated note message
        0xf0 => None,        // system messages (clock, active sensing) are ignored
        _ => Some(MidiEvent::Other { status, t_us }),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiPortInfo {
    pub name: String,
}

pub fn list_inputs() -> Result<Vec<MidiPortInfo>> {
    let input = MidiInput::new(CLIENT_NAME).map_err(|e| AudioError::DeviceConfig(e.to_string()))?;
    Ok(input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .map(|name| MidiPortInfo { name })
        .collect())
}

pub struct MidiIn {
    _conn: MidiInputConnection<()>,
    cons: HeapCons<MidiEvent>,
    dropped: Arc<AtomicU64>,
    name: String,
}

impl MidiIn {
    /// Open the port called `name` (exact match).
    pub fn open(name: &str) -> Result<Self> {
        let input =
            MidiInput::new(CLIENT_NAME).map_err(|e| AudioError::DeviceConfig(e.to_string()))?;
        let port = input
            .ports()
            .into_iter()
            .find(|p| input.port_name(p).map(|n| n == name).unwrap_or(false))
            .ok_or_else(|| AudioError::DeviceNotFound(name.to_string()))?;
        let (mut prod, cons): (HeapProd<MidiEvent>, HeapCons<MidiEvent>) =
            HeapRb::new(RING_EVENTS).split();
        let dropped = Arc::new(AtomicU64::new(0));
        let cb_dropped = Arc::clone(&dropped);
        let conn = input
            .connect(
                &port,
                "ethnoflow-midi-in",
                move |t_us, bytes, _| {
                    if let Some(ev) = parse_message(bytes, t_us) {
                        if prod.try_push(ev).is_err() {
                            cb_dropped.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                },
                (),
            )
            .map_err(|e| AudioError::Stream(e.to_string()))?;
        Ok(Self {
            _conn: conn,
            cons,
            dropped,
            name: name.to_string(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Events dropped because nobody drained the ring in time.
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Drain everything received so far.
    pub fn drain(&mut self, out: &mut Vec<MidiEvent>) {
        while let Some(ev) = self.cons.try_pop() {
            out.push(ev);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_note_messages_and_ignores_system_bytes() {
        assert_eq!(
            parse_message(&[0x90, 60, 100], 5),
            Some(MidiEvent::NoteOn {
                channel: 0,
                note: 60,
                velocity: 100,
                t_us: 5
            })
        );
        assert_eq!(
            parse_message(&[0x91, 62, 0], 1),
            Some(MidiEvent::NoteOff {
                channel: 1,
                note: 62,
                t_us: 1
            }),
            "velocity 0 = off"
        );
        assert_eq!(
            parse_message(&[0x83, 64, 40], 2),
            Some(MidiEvent::NoteOff {
                channel: 3,
                note: 64,
                t_us: 2
            })
        );
        assert_eq!(
            parse_message(&[0xB0, 7, 100], 0),
            Some(MidiEvent::Other {
                status: 0xB0,
                t_us: 0
            })
        );
        assert_eq!(parse_message(&[0xF8], 0), None, "clock");
        assert_eq!(parse_message(&[0xFE], 0), None, "active sensing");
        assert_eq!(parse_message(&[60, 100], 0), None, "data without status");
        assert_eq!(parse_message(&[0x90, 60], 0), None, "truncated");
    }

    #[test]
    fn listing_never_panics_and_unknown_port_is_an_error() {
        let ports = list_inputs();
        assert!(ports.is_ok() || ports.is_err());
        assert!(MidiIn::open("definitely-not-a-midi-port-ethnoflow").is_err());
        if let Ok(p) = &ports {
            eprintln!(
                "midi inputs: {:?}",
                p.iter().map(|x| &x.name).collect::<Vec<_>>()
            );
        }
    }
}
