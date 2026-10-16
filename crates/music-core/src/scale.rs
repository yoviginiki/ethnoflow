//! Scales and modes as cent patterns from a tonic, including user-defined ones.

use serde::{Deserialize, Serialize};

use crate::pitch::Pitch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleKind {
    Ionian,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
    HarmonicMinor,
    MelodicMinor,
    /// Hijaz / Phrygian dominant: 1 ♭2 3 4 5 ♭6 ♭7.
    PhrygianDominant,
    /// 1 ♭2 3 4 5 ♭6 7 (Byzantine / "Gypsy major").
    DoubleHarmonic,
    MajorPentatonic,
    MinorPentatonic,
}

impl ScaleKind {
    pub const ALL: [ScaleKind; 13] = [
        ScaleKind::Ionian,
        ScaleKind::Dorian,
        ScaleKind::Phrygian,
        ScaleKind::Lydian,
        ScaleKind::Mixolydian,
        ScaleKind::Aeolian,
        ScaleKind::Locrian,
        ScaleKind::HarmonicMinor,
        ScaleKind::MelodicMinor,
        ScaleKind::PhrygianDominant,
        ScaleKind::DoubleHarmonic,
        ScaleKind::MajorPentatonic,
        ScaleKind::MinorPentatonic,
    ];

    /// Degrees in semitones above the tonic.
    pub fn semitones(self) -> &'static [u8] {
        match self {
            ScaleKind::Ionian => &[0, 2, 4, 5, 7, 9, 11],
            ScaleKind::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            ScaleKind::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            ScaleKind::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            ScaleKind::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            ScaleKind::Aeolian => &[0, 2, 3, 5, 7, 8, 10],
            ScaleKind::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            ScaleKind::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            ScaleKind::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            ScaleKind::PhrygianDominant => &[0, 1, 4, 5, 7, 8, 10],
            ScaleKind::DoubleHarmonic => &[0, 1, 4, 5, 7, 8, 11],
            ScaleKind::MajorPentatonic => &[0, 2, 4, 7, 9],
            ScaleKind::MinorPentatonic => &[0, 3, 5, 7, 10],
        }
    }
}

/// A concrete scale: tonic pitch class and degrees in cents above it (sorted,
/// within [0, 1200)). Tempered kinds use multiples of 100; custom scales may not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scale {
    pub tonic_pc: u8,
    pub degrees_cents: Vec<f64>,
    pub kind: Option<ScaleKind>,
}

/// Result of locating a pitch in a scale.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DegreeMatch {
    /// 0-based degree index.
    pub degree: usize,
    /// Signed distance from that degree, in cents (negative = below).
    pub cents_off: f64,
}

impl Scale {
    pub fn new(tonic_pc: u8, kind: ScaleKind) -> Self {
        Self {
            tonic_pc: tonic_pc % 12,
            degrees_cents: kind.semitones().iter().map(|&s| s as f64 * 100.0).collect(),
            kind: Some(kind),
        }
    }

    /// User-defined scale from cents above the tonic. Values are reduced into one
    /// octave, sorted and de-duplicated; 0 (the tonic) is always present.
    pub fn custom(tonic_pc: u8, mut degrees_cents: Vec<f64>) -> Self {
        degrees_cents.retain(|c| c.is_finite());
        for c in degrees_cents.iter_mut() {
            *c = c.rem_euclid(1200.0);
        }
        degrees_cents.push(0.0);
        degrees_cents.sort_by(|a, b| a.partial_cmp(b).unwrap());
        degrees_cents.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
        Self {
            tonic_pc: tonic_pc % 12,
            degrees_cents,
            kind: None,
        }
    }

    pub fn len(&self) -> usize {
        self.degrees_cents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.degrees_cents.is_empty()
    }

    /// Position of a fractional MIDI number within the octave above the tonic, in cents.
    fn cents_above_tonic(&self, midi: f64) -> f64 {
        ((midi - self.tonic_pc as f64) * 100.0).rem_euclid(1200.0)
    }

    /// Nearest degree to a fractional MIDI number (octave-independent).
    pub fn nearest(&self, midi: f64) -> DegreeMatch {
        let c = self.cents_above_tonic(midi);
        let mut best = DegreeMatch {
            degree: 0,
            cents_off: f64::INFINITY,
        };
        for (i, &d) in self.degrees_cents.iter().enumerate() {
            // Distance on the circle of one octave.
            let mut off = c - d;
            if off > 600.0 {
                off -= 1200.0;
            } else if off < -600.0 {
                off += 1200.0;
            }
            if off.abs() < best.cents_off.abs() {
                best = DegreeMatch {
                    degree: i,
                    cents_off: off,
                };
            }
        }
        best
    }

    /// True if `midi` lies within `tolerance_cents` of a degree.
    pub fn contains(&self, midi: f64, tolerance_cents: f64) -> bool {
        self.nearest(midi).cents_off.abs() <= tolerance_cents
    }

    pub fn contains_pitch(&self, p: &Pitch, tolerance_cents: f64) -> bool {
        self.contains(p.midi_fraction(), tolerance_cents)
    }

    /// Pitch classes of a tempered scale (empty for a scale with non-tempered degrees).
    pub fn pitch_classes(&self) -> Vec<u8> {
        self.degrees_cents
            .iter()
            .filter_map(|&c| {
                let semis = c / 100.0;
                ((semis - semis.round()).abs() < 1e-6)
                    .then_some(((self.tonic_pc as f64 + semis.round()) as u8) % 12)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tempered_membership_with_tolerance() {
        let d_dorian = Scale::new(2, ScaleKind::Dorian); // D E F G A B C
        for pc in [2, 4, 5, 7, 9, 11, 0] {
            assert!(d_dorian.contains(60.0 + pc as f64, 10.0), "pc {pc}");
        }
        assert!(!d_dorian.contains(61.0, 10.0), "C# is not in D dorian");
        assert!(
            d_dorian.contains(62.0 + 0.08, 10.0),
            "8 cents sharp D is in"
        );
        assert!(
            !d_dorian.contains(62.0 + 0.2, 10.0),
            "20 cents sharp D is out at 10"
        );
        assert_eq!(d_dorian.pitch_classes(), vec![2, 4, 5, 7, 9, 11, 0]);
    }

    #[test]
    fn nearest_degree_is_signed_and_wraps_at_the_octave() {
        let s = Scale::new(0, ScaleKind::Ionian);
        let m = s.nearest(71.0 + 0.3); // B4 + 30 cents
        assert_eq!(m.degree, 6);
        assert!((m.cents_off - 30.0).abs() < 1e-9);
        let m = s.nearest(71.0 + 0.6); // closer to C5 (degree 0) than to B
        assert_eq!(m.degree, 0);
        assert!((m.cents_off + 40.0).abs() < 1e-9);
    }

    #[test]
    fn hijaz_has_the_augmented_second() {
        let e_hijaz = Scale::new(4, ScaleKind::PhrygianDominant); // E F G# A B C D
        assert_eq!(e_hijaz.pitch_classes(), vec![4, 5, 8, 9, 11, 0, 2]);
        assert!(!e_hijaz.contains(67.0, 10.0), "G natural is out");
        assert!(e_hijaz.contains(68.0, 10.0), "G# is in");
    }

    #[test]
    fn custom_scale_accepts_non_tempered_degrees() {
        // Tonic, neutral third (350 c), fifth, neutral seventh (1050 c), plus junk input.
        let s = Scale::custom(9, vec![350.0, 700.0, 1050.0, 1200.0 + 350.0, f64::NAN]);
        assert_eq!(s.degrees_cents, vec![0.0, 350.0, 700.0, 1050.0]);
        assert!(
            s.pitch_classes().len() == 2,
            "only tonic and fifth are tempered"
        );
        assert!(s.contains(69.0 + 3.5, 5.0), "neutral third above A");
        assert!(!s.contains(69.0 + 4.0, 5.0), "major third is 50 cents off");
        let m = s.nearest(69.0 + 3.4);
        assert_eq!(m.degree, 1);
        assert!((m.cents_off + 10.0).abs() < 1e-9);
    }

    #[test]
    fn every_kind_starts_on_the_tonic_and_is_sorted() {
        for k in ScaleKind::ALL {
            let s = k.semitones();
            assert_eq!(s[0], 0);
            assert!(s.windows(2).all(|w| w[0] < w[1]), "{k:?}");
            assert!(*s.last().unwrap() < 12);
        }
    }
}
