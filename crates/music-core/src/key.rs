//! Tonic/mode hypotheses from a pitch-class histogram.
//!
//! Output is always a ranked list with probabilities, never a single answer
//! (master plan §5.2). The caller decides what to show and the user decides what to
//! use. No default key exists anywhere.

use serde::{Deserialize, Serialize};

use crate::scale::{Scale, ScaleKind};

/// Weighted pitch-class histogram (index 0 = C). Weights are typically seconds of
/// sounding time, optionally scaled by confidence.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct PitchClassHistogram {
    pub weights: [f64; 12],
}

impl PitchClassHistogram {
    /// Accumulate fractional MIDI numbers. A pitch between two classes is split
    /// proportionally, so a 50-cent-off note does not vote for one side only.
    pub fn from_midi(samples: impl IntoIterator<Item = (f64, f64)>) -> Self {
        let mut h = Self::default();
        for (midi, w) in samples {
            if !(midi.is_finite() && w.is_finite()) || w <= 0.0 {
                continue;
            }
            let lo = midi.floor();
            let frac = midi - lo;
            let lo_pc = (lo as i64).rem_euclid(12) as usize;
            h.weights[lo_pc] += w * (1.0 - frac);
            h.weights[(lo_pc + 1) % 12] += w * frac;
        }
        h
    }

    pub fn total(&self) -> f64 {
        self.weights.iter().sum()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hypothesis {
    pub tonic_pc: u8,
    pub kind: ScaleKind,
    /// Raw fit in [0, 1]: how much of the sounding time lies on scale degrees,
    /// weighted by how prominent the tonic and fifth are.
    pub score: f64,
    /// Share of the probability mass among all evaluated hypotheses, in [0, 1].
    pub probability: f64,
}

/// Softmax temperature: smaller = more decisive. Chosen so that a clearly diatonic
/// melody with a strong tonic gives the top hypothesis well over 50 %.
const TEMPERATURE: f64 = 0.04;

/// Rank every (tonic, kind) pair over `kinds`. Returns hypotheses sorted by
/// probability, descending; empty if the histogram is empty.
pub fn rank_hypotheses(h: &PitchClassHistogram, kinds: &[ScaleKind]) -> Vec<Hypothesis> {
    let total = h.total();
    if total <= 0.0 || kinds.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(12 * kinds.len());
    for &kind in kinds {
        for tonic in 0..12u8 {
            let scale = Scale::new(tonic, kind);
            let in_scale: f64 = scale
                .pitch_classes()
                .iter()
                .map(|&pc| h.weights[pc as usize])
                .sum();
            let coverage = in_scale / total;
            let tonic_w = h.weights[tonic as usize] / total;
            let fifth_w = h.weights[((tonic + 7) % 12) as usize] / total;
            // A scale that merely contains the notes is not enough: the tonic should
            // carry weight, the fifth helps. Penalise scales with more degrees slightly
            // so a pentatonic melody is not explained equally well by every superset.
            let prominence = (2.0 * tonic_w + fifth_w).min(1.0);
            let size_penalty = 1.0 - 0.02 * (scale.len() as f64 - 5.0);
            let score = coverage * (0.5 + 0.5 * prominence) * size_penalty;
            out.push(Hypothesis {
                tonic_pc: tonic,
                kind,
                score,
                probability: 0.0,
            });
        }
    }
    let max = out.iter().map(|x| x.score).fold(f64::MIN, f64::max);
    let mut z = 0.0;
    for x in out.iter_mut() {
        x.probability = ((x.score - max) / TEMPERATURE).exp();
        z += x.probability;
    }
    for x in out.iter_mut() {
        x.probability /= z;
    }
    out.sort_by(|a, b| b.probability.partial_cmp(&a.probability).unwrap());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Durations per pitch class of a melody; tonic and fifth longer, as in practice.
    fn melody(tonic: u8, kind: ScaleKind) -> PitchClassHistogram {
        let mut samples = Vec::new();
        for (i, &s) in kind.semitones().iter().enumerate() {
            let midi = 60.0 + ((tonic + s) % 12) as f64;
            let w = match i {
                0 => 4.0,
                4 if kind.semitones().len() == 7 => 2.5, // fifth in heptatonic kinds
                _ => 1.0,
            };
            samples.push((midi, w));
        }
        PitchClassHistogram::from_midi(samples)
    }

    #[test]
    fn clear_major_is_ranked_first_with_high_probability() {
        let h = melody(2, ScaleKind::Ionian); // D major
        let r = rank_hypotheses(&h, &ScaleKind::ALL);
        assert_eq!((r[0].tonic_pc, r[0].kind), (2, ScaleKind::Ionian));
        assert!(
            r[0].probability > 0.5,
            "top probability {}",
            r[0].probability
        );
        let sum: f64 = r.iter().map(|x| x.probability).sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn hijaz_beats_its_relatives() {
        let h = melody(4, ScaleKind::PhrygianDominant); // E hijaz
        let r = rank_hypotheses(&h, &ScaleKind::ALL);
        assert_eq!((r[0].tonic_pc, r[0].kind), (4, ScaleKind::PhrygianDominant));
        // A harmonic minor shares the pitch set; it must come second-ish, not first.
        let am = r
            .iter()
            .position(|x| x.tonic_pc == 9 && x.kind == ScaleKind::HarmonicMinor)
            .unwrap();
        assert!(am > 0 && am < 6, "A harmonic minor at position {am}");
    }

    #[test]
    fn ambiguous_material_is_not_overconfident() {
        // All twelve classes equally: nothing should dominate.
        let h = PitchClassHistogram { weights: [1.0; 12] };
        let r = rank_hypotheses(&h, &ScaleKind::ALL);
        assert!(r[0].probability < 0.1, "top {}", r[0].probability);
        // Two notes only (C and G): many scales fit; top < 50 %.
        let h = PitchClassHistogram::from_midi([(60.0, 1.0), (67.0, 1.0)]);
        let r = rank_hypotheses(&h, &ScaleKind::ALL);
        assert!(r[0].probability < 0.5, "top {}", r[0].probability);
    }

    #[test]
    fn histogram_splits_between_neighbours_and_ignores_junk() {
        let h = PitchClassHistogram::from_midi([(60.5, 2.0), (f64::NAN, 1.0), (62.0, -1.0)]);
        assert!((h.weights[0] - 1.0).abs() < 1e-9);
        assert!((h.weights[1] - 1.0).abs() < 1e-9);
        assert!((h.total() - 2.0).abs() < 1e-9);
        assert!(rank_hypotheses(&PitchClassHistogram::default(), &ScaleKind::ALL).is_empty());
    }
}
