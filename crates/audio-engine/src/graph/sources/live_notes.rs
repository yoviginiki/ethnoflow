//! Live notes for the instrument sources (keyboard everywhere, K2, 2026-11-01): a
//! single-producer / single-consumer ring of note on/off messages inside a source's
//! parameter block. The UI (or the MIDI forwarding thread, under the same app lock)
//! pushes; the audio callback pops at the start of each block. No allocation, no lock.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

const SLOTS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveNote {
    pub midi: u8,
    pub on: bool,
    /// 0..1.
    pub velocity: f32,
}

pub struct LiveNotes {
    slots: [AtomicU32; SLOTS],
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl Default for LiveNotes {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveNotes {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| AtomicU32::new(0)),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn pack(n: LiveNote) -> u32 {
        let vel = (n.velocity.clamp(0.0, 1.0) * 127.0).round() as u32;
        (n.midi.min(127) as u32) | (vel << 8) | ((n.on as u32) << 16)
    }

    fn unpack(w: u32) -> LiveNote {
        LiveNote {
            midi: (w & 0x7f) as u8,
            velocity: ((w >> 8) & 0x7f) as f32 / 127.0,
            on: (w >> 16) & 1 == 1,
        }
    }

    /// Queue a note message. Returns false when the ring is full (the message is dropped).
    pub fn note(&self, midi: u8, on: bool, velocity: f32) -> bool {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= SLOTS {
            return false;
        }
        self.slots[head % SLOTS].store(
            Self::pack(LiveNote { midi, on, velocity }),
            Ordering::Relaxed,
        );
        self.head.store(head.wrapping_add(1), Ordering::Release);
        true
    }

    /// Next message for the audio thread, if any.
    pub fn pop(&self) -> Option<LiveNote> {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let w = self.slots[tail % SLOTS].load(Ordering::Relaxed);
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(Self::unpack(w))
    }
}

/// Which drum a key plays: General MIDI numbers where they exist (36 kick, 38 snare,
/// 37 ghost / side stick, 42 closed hat, 46 open hat, 70 shaker), any other key cycles
/// through the six so every key does something.
pub fn drum_kind_for_midi(midi: u8) -> super::rhythm::DrumKind {
    use super::rhythm::DrumKind as K;
    match midi {
        35 | 36 => K::Kick,
        38 | 40 => K::Snare,
        37 | 39 => K::Ghost,
        42 | 44 => K::HatClosed,
        46 => K::HatOpen,
        54 | 69 | 70 | 82 => K::Shaker,
        m => match m % 6 {
            0 => K::Kick,
            1 => K::Snare,
            2 => K::Ghost,
            3 => K::HatClosed,
            4 => K::HatOpen,
            _ => K::Shaker,
        },
    }
}

/// Equal temperament at A4 = 440 Hz (the instruments' own reference).
#[inline]
pub fn midi_hz(midi: u8) -> f32 {
    440.0 * 2f32.powf((midi as f32 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_come_out_in_order_and_the_ring_refuses_overflow() {
        let q = LiveNotes::new();
        assert!(q.note(60, true, 0.5));
        assert!(q.note(60, false, 0.0));
        let a = q.pop().unwrap();
        assert_eq!((a.midi, a.on), (60, true));
        assert!((a.velocity - 0.5).abs() < 0.01);
        let b = q.pop().unwrap();
        assert_eq!((b.midi, b.on), (60, false));
        assert!(q.pop().is_none());
        for i in 0..SLOTS {
            assert!(q.note(i as u8, true, 1.0));
        }
        assert!(!q.note(1, true, 1.0), "full");
        assert_eq!(q.pop().unwrap().midi, 0);
        assert!((midi_hz(69) - 440.0).abs() < 1e-3);
    }
}
