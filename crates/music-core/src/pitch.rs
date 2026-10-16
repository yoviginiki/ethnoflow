//! Frequency ↔ MIDI ↔ note name, with a configurable tuning reference.
//!
//! Nothing here assumes a key. Pitch classes are numbered 0 = C … 11 = B; names are
//! returned as data (pitch class, octave, cents) and the UI decides how to spell them.

use serde::{Deserialize, Serialize};

/// Tuning reference: the frequency of A4 (MIDI 69). Default 440 Hz, user-settable.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    pub a4_hz: f64,
}

impl Default for Reference {
    fn default() -> Self {
        Self { a4_hz: 440.0 }
    }
}

impl Reference {
    pub fn new(a4_hz: f64) -> Self {
        Self { a4_hz }
    }

    /// Fractional MIDI number (69.0 = A4 at the reference).
    pub fn hz_to_midi(&self, hz: f64) -> f64 {
        69.0 + 12.0 * (hz / self.a4_hz).log2()
    }

    pub fn midi_to_hz(&self, midi: f64) -> f64 {
        self.a4_hz * 2f64.powf((midi - 69.0) / 12.0)
    }

    /// Nearest tempered note and the deviation from it.
    pub fn pitch_of(&self, hz: f64) -> Option<Pitch> {
        if !(hz.is_finite() && hz > 0.0) {
            return None;
        }
        Some(Pitch::from_midi_fraction(self.hz_to_midi(hz)))
    }
}

/// A tempered note plus the deviation from it in cents (−50 ≤ cents < 50).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Pitch {
    pub midi: i32,
    pub cents: f64,
}

impl Pitch {
    pub fn from_midi_fraction(m: f64) -> Self {
        let midi = m.round() as i32;
        let cents = (m - midi as f64) * 100.0;
        // round() sends x.5 away from zero; keep cents in [-50, 50).
        if cents >= 50.0 {
            Self {
                midi: midi + 1,
                cents: cents - 100.0,
            }
        } else {
            Self { midi, cents }
        }
    }

    pub fn midi_fraction(&self) -> f64 {
        self.midi as f64 + self.cents / 100.0
    }

    /// 0 = C … 11 = B.
    pub fn pitch_class(&self) -> u8 {
        self.midi.rem_euclid(12) as u8
    }

    /// Scientific octave number (MIDI 60 = C4).
    pub fn octave(&self) -> i32 {
        self.midi.div_euclid(12) - 1
    }
}

/// Note names with sharps, for logs and tests; the UI localises spelling.
pub const PITCH_CLASS_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub fn pitch_class_name(pc: u8) -> &'static str {
    PITCH_CLASS_NAMES[(pc % 12) as usize]
}

/// Interval in cents between two frequencies (positive if `b` is higher).
pub fn cents_between(a_hz: f64, b_hz: f64) -> f64 {
    1200.0 * (b_hz / a_hz).log2()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_69_at_any_reference() {
        for a4 in [415.0, 432.0, 440.0, 442.0, 444.0] {
            let r = Reference::new(a4);
            assert!((r.hz_to_midi(a4) - 69.0).abs() < 1e-12);
            assert!((r.midi_to_hz(69.0) - a4).abs() < 1e-9);
            let p = r.pitch_of(a4).unwrap();
            assert_eq!((p.midi, p.pitch_class(), p.octave()), (69, 9, 4));
            assert!(p.cents.abs() < 1e-9);
        }
    }

    #[test]
    fn cents_are_signed_and_bounded() {
        let r = Reference::default();
        let sharp = r.pitch_of(r.midi_to_hz(60.0 + 0.3)).unwrap();
        assert_eq!(sharp.midi, 60);
        assert!((sharp.cents - 30.0).abs() < 1e-9);
        let flat = r.pitch_of(r.midi_to_hz(60.0 - 0.3)).unwrap();
        assert_eq!(flat.midi, 60);
        assert!((flat.cents + 30.0).abs() < 1e-9);
        // Exactly a quarter tone up rounds to the next note, -50 cents.
        let q = Pitch::from_midi_fraction(60.5);
        assert_eq!((q.midi, q.cents), (61, -50.0));
        let q2 = Pitch::from_midi_fraction(59.5);
        assert_eq!((q2.midi, q2.cents), (60, -50.0));
    }

    #[test]
    fn invalid_frequencies_give_none() {
        let r = Reference::default();
        assert!(r.pitch_of(0.0).is_none());
        assert!(r.pitch_of(-5.0).is_none());
        assert!(r.pitch_of(f64::NAN).is_none());
    }

    #[test]
    fn octave_and_class_follow_midi() {
        let p = Pitch {
            midi: 60,
            cents: 0.0,
        };
        assert_eq!((p.pitch_class(), p.octave()), (0, 4));
        let p = Pitch {
            midi: 59,
            cents: 0.0,
        };
        assert_eq!((p.pitch_class(), p.octave()), (11, 3));
        assert_eq!(pitch_class_name(p.pitch_class()), "B");
        assert!((cents_between(440.0, 880.0) - 1200.0).abs() < 1e-9);
    }
}
