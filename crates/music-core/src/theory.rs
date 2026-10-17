//! Intervals, chord detection and transposition (F2.1). Pure functions; names are
//! short codes the UI localises.

use serde::{Deserialize, Serialize};

use crate::notes::Note;

/// Interval class within an octave, from a semitone distance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interval {
    pub semitones: i32,
    /// P1, m2, M2, m3, M3, P4, TT, P5, m6, M6, m7, M7, P8 (plus octaves as "+n").
    pub code: &'static str,
    pub octaves: i32,
}

const CODES: [&str; 12] = [
    "P1", "m2", "M2", "m3", "M3", "P4", "TT", "P5", "m6", "M6", "m7", "M7",
];

pub fn interval(from_midi: i32, to_midi: i32) -> Interval {
    let semitones = to_midi - from_midi;
    let abs = semitones.abs();
    let octaves = abs / 12;
    let class = (abs % 12) as usize;
    let code = if abs > 0 && class == 0 {
        "P8"
    } else {
        CODES[class]
    };
    Interval {
        semitones,
        code,
        octaves: if abs > 0 && class == 0 {
            octaves - 1
        } else {
            octaves
        },
    }
}

/// Shift every note by `semitones`, clamping to MIDI range; bends and times unchanged.
pub fn transpose(notes: &[Note], semitones: i32) -> Vec<Note> {
    notes
        .iter()
        .map(|n| Note {
            midi: (n.midi + semitones).clamp(0, 127),
            ..n.clone()
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChordKind {
    Major,
    Minor,
    Diminished,
    Augmented,
    Sus2,
    Sus4,
    Power,
    Major7,
    Minor7,
    Dominant7,
    MinorMajor7,
    HalfDiminished7,
    Diminished7,
}

impl ChordKind {
    pub const ALL: [ChordKind; 13] = [
        ChordKind::Major,
        ChordKind::Minor,
        ChordKind::Diminished,
        ChordKind::Augmented,
        ChordKind::Sus2,
        ChordKind::Sus4,
        ChordKind::Power,
        ChordKind::Major7,
        ChordKind::Minor7,
        ChordKind::Dominant7,
        ChordKind::MinorMajor7,
        ChordKind::HalfDiminished7,
        ChordKind::Diminished7,
    ];

    pub fn intervals(self) -> &'static [u8] {
        match self {
            ChordKind::Major => &[0, 4, 7],
            ChordKind::Minor => &[0, 3, 7],
            ChordKind::Diminished => &[0, 3, 6],
            ChordKind::Augmented => &[0, 4, 8],
            ChordKind::Sus2 => &[0, 2, 7],
            ChordKind::Sus4 => &[0, 5, 7],
            ChordKind::Power => &[0, 7],
            ChordKind::Major7 => &[0, 4, 7, 11],
            ChordKind::Minor7 => &[0, 3, 7, 10],
            ChordKind::Dominant7 => &[0, 4, 7, 10],
            ChordKind::MinorMajor7 => &[0, 3, 7, 11],
            ChordKind::HalfDiminished7 => &[0, 3, 6, 10],
            ChordKind::Diminished7 => &[0, 3, 6, 9],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordMatch {
    pub root_pc: u8,
    pub kind: ChordKind,
    /// 1.0 = every chord tone present and nothing else; lower for missing/extra tones.
    pub score: f64,
    /// Lowest sounding pitch class, if it is a chord tone other than the root (inversion).
    pub bass_pc: Option<u8>,
}

/// Rank chords that explain a set of sounding MIDI notes (any octaves). Returns the
/// best matches first; empty for fewer than two distinct pitch classes.
pub fn detect_chords(midis: &[i32]) -> Vec<ChordMatch> {
    let mut present = [false; 12];
    for &m in midis {
        present[m.rem_euclid(12) as usize] = true;
    }
    let n_present = present.iter().filter(|&&p| p).count();
    if n_present < 2 {
        return Vec::new();
    }
    let bass_pc = midis.iter().min().map(|m| m.rem_euclid(12) as u8);
    let mut out = Vec::new();
    for root in 0..12u8 {
        for kind in ChordKind::ALL {
            let tones: Vec<u8> = kind.intervals().iter().map(|i| (root + i) % 12).collect();
            let matched = tones.iter().filter(|&&t| present[t as usize]).count();
            if matched < 2 {
                continue;
            }
            let missing = tones.len() - matched;
            let extra = n_present - matched;
            // Root missing hurts more than a fifth; extras cost a little each.
            let root_pen = if present[root as usize] { 0.0 } else { 0.35 };
            let score = (matched as f64 / tones.len() as f64)
                - 0.15 * missing as f64
                - 0.12 * extra as f64
                - root_pen;
            if score <= 0.0 {
                continue;
            }
            out.push(ChordMatch {
                root_pc: root,
                kind,
                score: score.min(1.0),
                bass_pc: bass_pc.filter(|&b| b != root && tones.contains(&b)),
            });
        }
    }
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap()
            .then(a.kind.intervals().len().cmp(&b.kind.intervals().len()))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_codes_and_octaves() {
        assert_eq!(interval(60, 60).code, "P1");
        assert_eq!(interval(60, 67).code, "P5");
        assert_eq!(interval(60, 66).code, "TT");
        assert_eq!(interval(67, 60).semitones, -7);
        assert_eq!(interval(67, 60).code, "P5");
        let o = interval(60, 72);
        assert_eq!((o.code, o.octaves), ("P8", 0));
        let m10 = interval(60, 75);
        assert_eq!((m10.code, m10.octaves), ("m3", 1));
    }

    #[test]
    fn transpose_clamps_and_keeps_timing() {
        let n = Note {
            start: 1.0,
            end: 2.0,
            midi: 125,
            cents: 10.0,
            confidence: 1.0,
            velocity: 0.5,
            bend: vec![(0.0, 5.0)],
        };
        let up = transpose(std::slice::from_ref(&n), 5);
        assert_eq!(up[0].midi, 127);
        assert_eq!((up[0].start, up[0].end, up[0].cents), (1.0, 2.0, 10.0));
        assert_eq!(transpose(&[n], -130)[0].midi, 0);
    }

    #[test]
    fn detects_triads_sevenths_and_inversions() {
        let best = |m: &[i32]| {
            let r = detect_chords(m);
            (r[0].root_pc, r[0].kind, r[0].bass_pc)
        };
        assert_eq!(best(&[60, 64, 67]), (0, ChordKind::Major, None));
        assert_eq!(best(&[62, 65, 69]), (2, ChordKind::Minor, None));
        assert_eq!(
            best(&[64, 67, 72]),
            (0, ChordKind::Major, Some(4)),
            "first inversion"
        );
        assert_eq!(best(&[55, 59, 62, 65]), (7, ChordKind::Dominant7, None));
        assert_eq!(best(&[57, 60, 64, 67]), (9, ChordKind::Minor7, None));
        assert_eq!(best(&[62, 69]), (2, ChordKind::Power, None));
        assert_eq!(best(&[60, 65, 67]).1, ChordKind::Sus4);
        assert!(detect_chords(&[60]).is_empty());
        // D hijaz tones D E♭ F♯ A: the best explanation is D major-ish, not nonsense.
        let r = detect_chords(&[62, 63, 66, 69]);
        assert_eq!(r[0].root_pc, 2);
    }
}
