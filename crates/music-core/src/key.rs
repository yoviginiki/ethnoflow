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

/// Share of sounding time on one pitch class.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PcShare {
    pub pc: u8,
    pub share: f64,
}

/// The pitch class that separates a hypothesis from its nearest rival with a
/// different note set (e.g. B♭ vs B for D aeolian vs D dorian).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Decisive {
    /// In this hypothesis, not in the rival.
    pub pc_for: u8,
    pub share_for: f64,
    /// In the rival, not in this hypothesis.
    pub pc_against: u8,
    pub share_against: f64,
    pub rival_tonic_pc: u8,
    pub rival_kind: ScaleKind,
}

/// Plain facts behind a hypothesis, for the UI to word in the user's language. Shares
/// are fractions of the total sounding time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub tonic_share: f64,
    pub fifth_share: f64,
    /// Share of time on any degree of the scale.
    pub coverage: f64,
    pub decisive: Option<Decisive>,
    /// Loudest pitch classes outside the scale (at most two, share > 1 %).
    pub off_scale: Vec<PcShare>,
}

/// Explain `hyp` against the ranked list it came from: the rival is the first later
/// hypothesis whose pitch-class set differs. Returns `None` on an empty histogram.
pub fn explain(
    h: &PitchClassHistogram,
    hyp: &Hypothesis,
    ranked: &[Hypothesis],
) -> Option<Evidence> {
    let total = h.total();
    if total <= 0.0 {
        return None;
    }
    let share = |pc: u8| h.weights[(pc % 12) as usize] / total;
    let scale = Scale::new(hyp.tonic_pc, hyp.kind);
    let pcs = scale.pitch_classes();
    let coverage: f64 = pcs.iter().map(|&pc| share(pc)).sum();
    let pos = ranked
        .iter()
        .position(|x| x.tonic_pc == hyp.tonic_pc && x.kind == hyp.kind)
        .unwrap_or(0);
    let decisive = ranked[pos + 1..].iter().find_map(|r| {
        let rp = Scale::new(r.tonic_pc, r.kind).pitch_classes();
        let pc_for = pcs
            .iter()
            .copied()
            .filter(|pc| !rp.contains(pc))
            .max_by(|a, b| share(*a).partial_cmp(&share(*b)).unwrap())?;
        let pc_against = rp
            .iter()
            .copied()
            .filter(|pc| !pcs.contains(pc))
            .max_by(|a, b| share(*a).partial_cmp(&share(*b)).unwrap())?;
        Some(Decisive {
            pc_for,
            share_for: share(pc_for),
            pc_against,
            share_against: share(pc_against),
            rival_tonic_pc: r.tonic_pc,
            rival_kind: r.kind,
        })
    });
    let mut off_scale: Vec<PcShare> = (0..12u8)
        .filter(|pc| !pcs.contains(pc))
        .map(|pc| PcShare {
            pc,
            share: share(pc),
        })
        .filter(|p| p.share > 0.01)
        .collect();
    off_scale.sort_by(|a, b| b.share.partial_cmp(&a.share).unwrap());
    off_scale.truncate(2);
    Some(Evidence {
        tonic_share: share(hyp.tonic_pc),
        fifth_share: share(hyp.tonic_pc + 7),
        coverage,
        decisive,
        off_scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explanation_names_the_decisive_note_and_the_tonic_share() {
        // D aeolian with a short F♯ ornament: B♭ (2 s) vs B (0 s) separates it from D dorian.
        let h = PitchClassHistogram::from_midi([
            (62.0, 4.0),
            (64.0, 1.0),
            (65.0, 1.5),
            (67.0, 1.0),
            (69.0, 2.0),
            (70.0, 2.0),
            (72.0, 1.0),
            (66.0, 0.2),
        ]);
        let r = rank_hypotheses(&h, &ScaleKind::ALL);
        assert_eq!((r[0].tonic_pc, r[0].kind), (2, ScaleKind::Aeolian));
        let e = explain(&h, &r[0], &r).unwrap();
        assert!((e.tonic_share - 4.0 / 12.7).abs() < 1e-9);
        assert!(e.coverage > 0.98 && e.coverage < 1.0);
        assert_eq!(e.off_scale.len(), 1);
        assert_eq!(e.off_scale[0].pc, 6);
        let d = e.decisive.expect("a rival with a different set exists");
        // The nearest different-set rival is D phrygian (E♭): E speaks for aeolian. Whatever
        // the rival, the decisive note must be ours, absent from the rival, and heavier.
        assert!(d.rival_tonic_pc == 2 && d.rival_kind != ScaleKind::Aeolian);
        assert!(matches!(d.pc_for, 4 | 10), "pc_for {}", d.pc_for);
        assert!(d.share_for > d.share_against);
        assert!(explain(&PitchClassHistogram::default(), &r[0], &r).is_none());
    }

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
