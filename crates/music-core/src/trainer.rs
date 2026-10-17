//! Trainer evaluation (FEATURE_SPEC F2.8.1): compare what was expected with what was
//! played. Pure, no UI, no scoring of "beauty" — only facts per note (master plan §1).
//! Microtonal deviations are reported in cents; whether they are "wrong" in an ethno
//! mode is for the UI to say.

use serde::{Deserialize, Serialize};

use crate::notes::Note;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Tolerance {
    /// Pitch tolerance for a hit.
    pub cents: f64,
    /// Timing tolerance for a hit.
    pub ms: f64,
}

impl Tolerance {
    pub fn beginner() -> Self {
        Self {
            cents: 30.0,
            ms: 80.0,
        }
    }
    pub fn intermediate() -> Self {
        Self {
            cents: 20.0,
            ms: 50.0,
        }
    }
    pub fn advanced() -> Self {
        Self {
            cents: 10.0,
            ms: 30.0,
        }
    }

    /// How far (ms) from the expected onset a played note may still be "the attempt".
    pub fn window_ms(&self) -> f64 {
        (self.ms * 3.0).max(250.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Verdict {
    Hit { cents: f64, ms: f64 },
    WrongPitch { cents: f64, ms: f64 },
    Early { cents: f64, ms: f64 },
    Late { cents: f64, ms: f64 },
    Missed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteResult {
    /// Index into `expected`.
    pub index: usize,
    /// Index into `played`, when a note was matched.
    pub played: Option<usize>,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Summary {
    pub expected: usize,
    pub hits: usize,
    pub wrong_pitch: usize,
    pub early: usize,
    pub late: usize,
    pub missed: usize,
    /// Played notes that match nothing expected.
    pub extra: usize,
    /// Mean |cents| over matched notes with the right pitch.
    pub mean_abs_cents: f64,
    /// Mean |ms| over matched notes with the right pitch.
    pub mean_abs_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub per_note: Vec<NoteResult>,
    pub extra_played: Vec<usize>,
    pub summary: Summary,
}

fn cents_between(expected: &Note, played: &Note) -> f64 {
    ((played.midi as f64 + played.cents / 100.0) - (expected.midi as f64 + expected.cents / 100.0))
        * 100.0
}

/// Match each expected note to the played note nearest in time within the window,
/// preferring one with the right pitch; classify; collect leftovers as extras.
/// `latency_ms` is subtracted from played onsets (input chain delay, F1.4 calibration).
pub fn evaluate(expected: &[Note], played: &[Note], tol: &Tolerance, latency_ms: f64) -> Report {
    let mut used = vec![false; played.len()];
    let mut per_note = Vec::with_capacity(expected.len());
    let window = tol.window_ms();
    let mut order: Vec<usize> = (0..expected.len()).collect();
    order.sort_by(|&a, &b| expected[a].start.partial_cmp(&expected[b].start).unwrap());

    for &i in &order {
        let e = &expected[i];
        let mut best: Option<(usize, f64, bool)> = None; // (index, |dt|, right pitch)
        for (j, p) in played.iter().enumerate() {
            if used[j] {
                continue;
            }
            let dt = (p.start - latency_ms / 1000.0 - e.start) * 1000.0;
            if dt.abs() > window {
                continue;
            }
            let right = cents_between(e, p).abs() <= tol.cents;
            let better = match best {
                None => true,
                Some((_, bdt, bright)) => (right && !bright) || (right == bright && dt.abs() < bdt),
            };
            if better {
                best = Some((j, dt.abs(), right));
            }
        }
        let verdict = match best {
            None => Verdict::Missed,
            Some((j, _, right)) => {
                used[j] = true;
                let p = &played[j];
                let cents = cents_between(e, p);
                let ms = (p.start - latency_ms / 1000.0 - e.start) * 1000.0;
                if !right {
                    Verdict::WrongPitch { cents, ms }
                } else if ms < -tol.ms {
                    Verdict::Early { cents, ms }
                } else if ms > tol.ms {
                    Verdict::Late { cents, ms }
                } else {
                    Verdict::Hit { cents, ms }
                }
            }
        };
        per_note.push(NoteResult {
            index: i,
            played: best.map(|b| b.0),
            verdict,
        });
    }
    per_note.sort_by_key(|r| r.index);
    let extra_played: Vec<usize> = (0..played.len()).filter(|&j| !used[j]).collect();

    let mut s = Summary {
        expected: expected.len(),
        extra: extra_played.len(),
        ..Summary::default()
    };
    let (mut c_sum, mut ms_sum, mut n_right) = (0.0, 0.0, 0usize);
    for r in &per_note {
        match r.verdict {
            Verdict::Hit { cents, ms } => {
                s.hits += 1;
                c_sum += cents.abs();
                ms_sum += ms.abs();
                n_right += 1;
            }
            Verdict::Early { cents, ms } => {
                s.early += 1;
                c_sum += cents.abs();
                ms_sum += ms.abs();
                n_right += 1;
            }
            Verdict::Late { cents, ms } => {
                s.late += 1;
                c_sum += cents.abs();
                ms_sum += ms.abs();
                n_right += 1;
            }
            Verdict::WrongPitch { .. } => s.wrong_pitch += 1,
            Verdict::Missed => s.missed += 1,
        }
    }
    if n_right > 0 {
        s.mean_abs_cents = c_sum / n_right as f64;
        s.mean_abs_ms = ms_sum / n_right as f64;
    }
    Report {
        per_note,
        extra_played,
        summary: s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(start: f64, midi: i32, cents: f64) -> Note {
        Note {
            start,
            end: start + 0.4,
            midi,
            cents,
            confidence: 1.0,
            velocity: 0.7,
            bend: vec![],
        }
    }

    fn melody() -> Vec<Note> {
        vec![
            n(0.0, 62, 0.0),
            n(0.5, 64, 0.0),
            n(1.0, 66, 0.0),
            n(1.5, 69, 0.0),
        ]
    }

    #[test]
    fn exact_performance_is_all_hits() {
        let r = evaluate(&melody(), &melody(), &Tolerance::beginner(), 0.0);
        assert_eq!(r.summary.hits, 4);
        assert!(r.extra_played.is_empty());
        assert_eq!(r.summary.mean_abs_cents, 0.0);
        assert!(r
            .per_note
            .iter()
            .all(|x| matches!(x.verdict, Verdict::Hit { .. })));
    }

    #[test]
    fn wrong_pitch_early_late_missed_and_extra_are_told_apart() {
        let played = vec![
            n(0.0, 62, 40.0),  // 40 cents sharp → wrong pitch at beginner (30)
            n(0.38, 64, 0.0),  // 120 ms early
            n(1.12, 66, -5.0), // 120 ms late, in tune
            // 69 never played; an extra note at 2.5 s
            n(2.5, 71, 0.0),
        ];
        let r = evaluate(&melody(), &played, &Tolerance::beginner(), 0.0);
        let v: Vec<&Verdict> = r.per_note.iter().map(|x| &x.verdict).collect();
        assert!(matches!(v[0], Verdict::WrongPitch { cents, .. } if (cents - 40.0).abs() < 1e-9));
        assert!(matches!(v[1], Verdict::Early { ms, .. } if (ms + 120.0).abs() < 1e-6));
        assert!(
            matches!(v[2], Verdict::Late { ms, cents } if (ms - 120.0).abs() < 1e-6 && (cents + 5.0).abs() < 1e-9)
        );
        assert!(matches!(v[3], Verdict::Missed));
        assert_eq!(r.extra_played, vec![3]);
        let s = &r.summary;
        assert_eq!(
            (s.hits, s.wrong_pitch, s.early, s.late, s.missed, s.extra),
            (0, 1, 1, 1, 1, 1)
        );
        assert!((s.mean_abs_ms - 120.0).abs() < 1e-6);
        assert!((s.mean_abs_cents - 2.5).abs() < 1e-9);
    }

    #[test]
    fn prefers_the_right_pitch_within_the_window_and_uses_each_played_note_once() {
        // A wrong note right on time, then the right one 100 ms later.
        let played = vec![n(0.0, 60, 0.0), n(0.1, 62, 0.0)];
        let r = evaluate(&[n(0.0, 62, 0.0)], &played, &Tolerance::beginner(), 0.0);
        assert_eq!(r.per_note[0].played, Some(1));
        assert!(matches!(r.per_note[0].verdict, Verdict::Late { .. }));
        assert_eq!(r.extra_played, vec![0]);
        // Two expected notes, one played: the second is missed, not double-matched.
        let r = evaluate(
            &[n(0.0, 62, 0.0), n(0.2, 62, 0.0)],
            &[n(0.05, 62, 0.0)],
            &Tolerance::beginner(),
            0.0,
        );
        assert!(matches!(r.per_note[0].verdict, Verdict::Hit { .. }));
        assert!(matches!(r.per_note[1].verdict, Verdict::Missed));
    }

    #[test]
    fn latency_is_subtracted_and_levels_differ() {
        let played = vec![n(0.06, 62, 15.0)];
        let exp = vec![n(0.0, 62, 0.0)];
        // 60 ms late at face value; with 60 ms input latency it is on time.
        assert!(matches!(
            evaluate(&exp, &played, &Tolerance::advanced(), 0.0).per_note[0].verdict,
            Verdict::Late { .. }
        ));
        assert!(
            matches!(
                evaluate(&exp, &played, &Tolerance::advanced(), 60.0).per_note[0].verdict,
                Verdict::WrongPitch { .. }
            ),
            "15 cents is too much for advanced (10)"
        );
        assert!(matches!(
            evaluate(&exp, &played, &Tolerance::beginner(), 60.0).per_note[0].verdict,
            Verdict::Hit { .. }
        ));
        assert!(Tolerance::beginner().window_ms() >= 250.0);
    }
}
