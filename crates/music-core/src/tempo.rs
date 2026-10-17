//! Tempo from a free performance (F4.3): estimate the beat of a phrase from its note
//! onsets so a kaval take can be aligned to the grid by a tempo map and an offset,
//! never by moving the notes. Returns candidates with confidence; the user decides.

use serde::{Deserialize, Serialize};

use crate::notes::Note;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempoCandidate {
    pub bpm: f64,
    /// Seconds from the take start to the first beat.
    pub offset_s: f64,
    /// 0..1 closeness of the onsets to the grid (1 = all exactly on a beat).
    pub fit: f64,
}

/// Candidates in `bpm_lo..=bpm_hi` ranked by how many onsets sit on the grid; up to
/// `max` entries, best first. Fewer than 4 onsets → empty (nothing to say).
pub fn estimate_tempo(notes: &[Note], bpm_lo: f64, bpm_hi: f64, max: usize) -> Vec<TempoCandidate> {
    let mut onsets: Vec<f64> = notes.iter().map(|n| n.start).collect();
    onsets.sort_by(|a, b| a.partial_cmp(b).unwrap());
    onsets.dedup_by(|a, b| (*a - *b).abs() < 0.03);
    if onsets.len() < 4 {
        return Vec::new();
    }
    let (lo, hi) = (bpm_lo.clamp(30.0, 300.0), bpm_hi.clamp(30.0, 300.0));
    let (lo, hi) = (lo.min(hi), hi.max(lo));
    let mut out = Vec::new();
    // Coarse-to-fine search in 0.5 BPM steps: score = onsets near a grid line
    // (tolerance 12 % of the beat), with the best phase offset per tempo.
    let mut bpm = lo;
    while bpm <= hi + 1e-9 {
        let beat = 60.0 / bpm;
        let tol = beat * 0.12;
        // Offsets: try each of the first onsets as "a beat".
        let mut best_fit = 0.0;
        let mut best_off = 0.0;
        for &cand in onsets.iter().take(6) {
            let off = cand % beat;
            // Closeness-weighted: an onset exactly on the grid counts 1, at the
            // tolerance edge 0 — so 96 BPM beats 95 on a 96 BPM phrase.
            let fit = onsets
                .iter()
                .map(|&t| {
                    let rel = (t - off) / beat;
                    let dev = (rel - rel.round()).abs() * beat;
                    (1.0 - dev / tol).max(0.0)
                })
                .sum::<f64>()
                / onsets.len() as f64;
            if fit > best_fit {
                best_fit = fit;
                best_off = off;
            }
        }
        out.push(TempoCandidate {
            bpm,
            offset_s: best_off,
            fit: best_fit,
        });
        bpm += 0.5;
    }
    // Prefer the slower of equally good tempi (fewer spurious grid lines), then merge
    // neighbours within 3 BPM.
    out.sort_by(|a, b| {
        b.fit
            .partial_cmp(&a.fit)
            .unwrap()
            .then(a.bpm.partial_cmp(&b.bpm).unwrap())
    });
    let mut merged: Vec<TempoCandidate> = Vec::new();
    for c in out {
        if merged.iter().any(|m| {
            (m.bpm - c.bpm).abs() < 3.0 || (m.bpm * 2.0 - c.bpm).abs() < 3.0 && m.fit >= c.fit
        }) {
            continue;
        }
        merged.push(c);
        if merged.len() >= max.max(1) {
            break;
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(start: f64, len: f64) -> Note {
        Note {
            start,
            end: start + len,
            midi: 62,
            cents: 0.0,
            confidence: 1.0,
            velocity: 0.8,
            bend: Vec::new(),
        }
    }

    #[test]
    fn finds_the_tempo_of_a_played_phrase_with_rubato_and_rests() {
        // 96 BPM (beat 0.625 s), onsets on beats and some eighths, ±20 ms rubato,
        // starting 0.4 s into the take, with a rest.
        let beat = 60.0 / 96.0;
        let pattern = [
            0.0, 1.0, 1.5, 2.0, 3.0, 4.0, 4.5, 5.0, 6.0, 8.0, 9.0, 9.5, 10.0, 11.0, 12.0,
        ];
        let jitter = [
            0.01, -0.02, 0.015, -0.01, 0.02, 0.0, -0.015, 0.01, 0.02, -0.01, 0.005, 0.0, -0.02,
            0.01, 0.0,
        ];
        let notes: Vec<Note> = pattern
            .iter()
            .zip(jitter.iter())
            .map(|(b, j)| note(0.4 + b * beat + j, 0.3))
            .collect();
        let c = estimate_tempo(&notes, 60.0, 180.0, 3);
        assert!(!c.is_empty());
        let best = &c[0];
        assert!(
            (best.bpm - 96.0).abs() <= 1.0 || (best.bpm - 48.0).abs() <= 1.0,
            "best {best:?}"
        );
        assert!(best.fit > 0.6, "{best:?}");
        assert!((best.offset_s - 0.4).abs() < 0.05 || (best.offset_s - (0.4 % beat)).abs() < 0.05);
        assert!(estimate_tempo(&notes[..3], 60.0, 180.0, 3).is_empty());
    }
}
