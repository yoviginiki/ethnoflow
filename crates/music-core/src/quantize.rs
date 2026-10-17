//! Quantisation to a grid (F2.1): a separate, reversible step that needs an explicit
//! grid — nothing here assumes a tempo. Notes outside the tolerance are left alone.

use serde::{Deserialize, Serialize};

use crate::notes::Note;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    /// Seconds between grid lines (e.g. a 16th at 120 BPM = 0.125).
    pub step_seconds: f64,
    /// Seconds of the first grid line.
    pub origin_seconds: f64,
}

impl Grid {
    /// Grid from a tempo: `division` lines per quarter note (4 = 16ths).
    pub fn from_tempo(bpm: f64, division: u32, origin_seconds: f64) -> Self {
        Self {
            step_seconds: 60.0 / bpm / division.max(1) as f64,
            origin_seconds,
        }
    }

    pub fn nearest(&self, t: f64) -> f64 {
        self.origin_seconds
            + ((t - self.origin_seconds) / self.step_seconds).round() * self.step_seconds
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QuantizeParams {
    /// Snap only if the note is within this fraction of a step from a grid line.
    pub tolerance: f64,
    /// Also snap the end (durations become multiples of the step).
    pub ends: bool,
    /// Shortest note kept after quantisation, in steps.
    pub min_steps: u32,
}

impl Default for QuantizeParams {
    fn default() -> Self {
        Self {
            tolerance: 0.5,
            ends: true,
            min_steps: 1,
        }
    }
}

/// Quantised copy of `notes` (input untouched). Returns the notes plus how many were
/// moved, so the UI can say "7 of 12 notes snapped".
pub fn quantize(notes: &[Note], grid: &Grid, p: &QuantizeParams) -> (Vec<Note>, usize) {
    let tol = p.tolerance.clamp(0.0, 0.5) * grid.step_seconds;
    let mut moved = 0;
    let out = notes
        .iter()
        .map(|n| {
            let mut m = n.clone();
            let s = grid.nearest(n.start);
            if (s - n.start).abs() <= tol && s != n.start {
                m.start = s;
                moved += 1;
            }
            if p.ends {
                let e = grid.nearest(n.end);
                let min_end = m.start + p.min_steps.max(1) as f64 * grid.step_seconds;
                if (e - n.end).abs() <= tol {
                    m.end = e.max(min_end);
                } else if m.end <= m.start {
                    m.end = min_end;
                }
            } else if m.end <= m.start {
                m.end = n.end.max(m.start + 1e-3);
            }
            m
        })
        .collect();
    (out, moved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(start: f64, end: f64) -> Note {
        Note {
            start,
            end,
            midi: 60,
            cents: 0.0,
            confidence: 1.0,
            velocity: 0.5,
            bend: vec![],
        }
    }

    #[test]
    fn grid_from_tempo_and_nearest() {
        let g = Grid::from_tempo(120.0, 4, 0.0); // 16ths = 125 ms
        assert!((g.step_seconds - 0.125).abs() < 1e-12);
        assert!((g.nearest(0.13) - 0.125).abs() < 1e-12);
        assert!((g.nearest(0.19) - 0.25).abs() < 1e-12);
        let g2 = Grid::from_tempo(120.0, 4, 0.05);
        assert!((g2.nearest(0.05) - 0.05).abs() < 1e-12);
    }

    #[test]
    fn snaps_within_tolerance_only_and_keeps_input() {
        let g = Grid::from_tempo(120.0, 4, 0.0);
        let src = vec![n(0.02, 0.26), n(0.44, 0.60), n(1.0, 1.02)];
        let (q, moved) = quantize(
            &src,
            &g,
            &QuantizeParams {
                tolerance: 0.25,
                ..QuantizeParams::default()
            },
        );
        assert_eq!(
            moved, 1,
            "0.02 snaps (20 ms ≤ 31 ms); 0.44 is 65 ms from 0.375/0.5: left alone"
        );
        assert_eq!(q[0].start, 0.0);
        assert!((q[0].end - 0.25).abs() < 1e-12);
        assert_eq!(q[1].start, 0.44);
        // Tiny note: end snaps onto its start → extended to one step.
        assert!((q[2].end - 1.125).abs() < 1e-12);
        assert_eq!(src[0].start, 0.02, "input untouched");
    }
}
