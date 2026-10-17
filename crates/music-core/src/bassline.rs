//! Bass foundation (F4.2, the note side): a line that follows chord spans — roots on
//! changes, fifths/octaves and scale passing notes as density grows, syncopation
//! from the seed, a register, glides between stepwise notes. The engine plays it
//! with a sub / pluck / Reese voice and ducks it under the kick.

use serde::{Deserialize, Serialize};

use crate::pattern::{swing_offset, BeatNote, Meter, Rng};
use crate::scale::Scale;

/// A chord to play under, in beats from the pattern start.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordSpan {
    pub start_beat: f64,
    pub end_beat: f64,
    pub root_pc: u8,
    /// Chord tones as (pitch class, cents); the root is among them.
    pub tones: Vec<(u8, f64)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BassParams {
    /// 0..1: notes per bar (0 = roots on changes only, 1 = busy sixteenth figures).
    pub density: f64,
    /// 0..1: how many onsets move off the beat.
    pub syncopation: f64,
    /// 0..1: share of passing notes from the scale between chord tones.
    pub passing: f64,
    /// MIDI register (sub basses live around 28..48).
    pub register: (i32, i32),
    pub swing: f64,
    pub seed: u32,
}

impl Default for BassParams {
    fn default() -> Self {
        Self {
            density: 0.4,
            syncopation: 0.3,
            passing: 0.3,
            register: (31, 43),
            swing: 0.0,
            seed: 1,
        }
    }
}

/// Put a pitch class into the register, nearest to `near` (or the register's lower
/// half when `near` is None).
fn place(pc: u8, register: (i32, i32), near: Option<i32>) -> i32 {
    let (lo, hi) = (register.0.min(register.1), register.1.max(register.0));
    let target = near.unwrap_or(lo + (hi - lo) / 3);
    let mut best = lo;
    let mut best_d = i32::MAX;
    for midi in lo..=hi {
        if midi.rem_euclid(12) as u8 == pc % 12 {
            let d = (midi - target).abs();
            if d < best_d {
                best = midi;
                best_d = d;
            }
        }
    }
    best
}

fn cents_of(tones: &[(u8, f64)], pc: u8) -> f64 {
    tones
        .iter()
        .find(|(p, _)| *p == pc % 12)
        .map(|(_, c)| *c)
        .unwrap_or(0.0)
}

/// Generate the bass line for the chord spans. Every note is a chord tone or a scale
/// passing tone; a chord change always gets its root on the downbeat of the span.
pub fn generate_bass(
    spans: &[ChordSpan],
    scale: Option<&Scale>,
    params: &BassParams,
    meter: &Meter,
) -> Vec<BeatNote> {
    let mut rng = Rng::new(params.seed);
    let density = params.density.clamp(0.0, 1.0);
    let sync = params.syncopation.clamp(0.0, 1.0);
    let passing = params.passing.clamp(0.0, 1.0);
    let mut out: Vec<BeatNote> = Vec::new();
    let mut prev_midi: Option<i32> = None;
    let scale_pcs: Vec<(u8, f64)> = scale
        .map(|s| {
            s.degrees_cents
                .iter()
                .map(|&c| {
                    let semis = (c / 100.0).round() as i32;
                    (
                        ((s.tonic_pc as i32 + semis).rem_euclid(12)) as u8,
                        c - semis as f64 * 100.0,
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let step = if density > 0.7 {
        0.25
    } else if density > 0.35 {
        0.5
    } else {
        1.0
    };
    for (si, span) in spans.iter().enumerate() {
        if span.end_beat <= span.start_beat {
            continue;
        }
        let root = place(span.root_pc, params.register, prev_midi);
        let fifth_pc = (span.root_pc + 7) % 12;
        let has_fifth = span.tones.iter().any(|(p, _)| *p == fifth_pc);
        // Root on the change, held a beat (or the whole span when sparse).
        let first_len = if density < 0.15 {
            span.end_beat - span.start_beat
        } else {
            (meter.unit_beats().max(1.0)).min(span.end_beat - span.start_beat)
        };
        out.push(BeatNote {
            start_beat: span.start_beat,
            length_beats: first_len,
            midi: root,
            cents: cents_of(&span.tones, span.root_pc),
            velocity: 0.95,
            glide: false,
        });
        prev_midi = Some(root);
        if density < 0.15 {
            continue;
        }
        // Figures inside the span.
        let mut t = span.start_beat + first_len;
        let next_root = spans
            .get(si + 1)
            .map(|n| place(n.root_pc, params.register, Some(root)));
        while t < span.end_beat - 1e-9 {
            let remaining = span.end_beat - t;
            let on_beat = (t - t.round()).abs() < 1e-9;
            let play = if on_beat {
                rng.chance(0.35 + density * 0.6)
            } else {
                rng.chance(density * 0.7)
            };
            if play {
                // Choice: root (common), fifth/octave, or a passing tone towards the
                // next root in the last beat of the span.
                let last_beat = remaining <= 1.0 + 1e-9;
                let (midi, cents) = if last_beat && passing > 0.0 && rng.chance(passing) {
                    if let Some(nr) = next_root {
                        // Scale tone between here and the next root (or a semitone below it).
                        let dir = (nr - root).signum();
                        let candidate = (1..=4)
                            .map(|k| nr - dir * k)
                            .find(|m| {
                                *m != nr
                                    && (scale_pcs.is_empty()
                                        || scale_pcs
                                            .iter()
                                            .any(|(p, _)| *p == m.rem_euclid(12) as u8))
                            })
                            .unwrap_or(root);
                        let c = scale_pcs
                            .iter()
                            .find(|(p, _)| *p == candidate.rem_euclid(12) as u8)
                            .map(|(_, c)| *c)
                            .unwrap_or(0.0);
                        (candidate.clamp(params.register.0, params.register.1), c)
                    } else {
                        (root, cents_of(&span.tones, span.root_pc))
                    }
                } else if has_fifth && rng.chance(0.3 + density * 0.3) {
                    let f = place(fifth_pc, params.register, Some(root));
                    (f, cents_of(&span.tones, fifth_pc))
                } else if rng.chance(0.2 * density) && root + 12 <= params.register.1 {
                    (root + 12, cents_of(&span.tones, span.root_pc))
                } else {
                    (root, cents_of(&span.tones, span.root_pc))
                };
                // Syncopation: push some onsets a sixteenth/eighth late.
                let shift = if !on_beat || !rng.chance(sync) {
                    0.0
                } else if step <= 0.25 {
                    0.25
                } else {
                    0.5
                };
                let start = t + shift + swing_offset(t, 0.5, params.swing);
                if start < span.end_beat - 1e-9 {
                    let factor: f64 = if rng.chance(0.5) { 1.0 } else { 1.8 };
                    let len = (step * factor).min(span.end_beat - start);
                    let glide = prev_midi
                        .map(|p| (p - midi).abs() <= 2 && (p - midi) != 0)
                        .unwrap_or(false)
                        && rng.chance(0.6);
                    out.push(BeatNote {
                        start_beat: start,
                        length_beats: len.max(0.1),
                        midi,
                        cents,
                        velocity: 0.6 + rng.unit() * 0.3,
                        glide,
                    });
                    prev_midi = Some(midi);
                }
            }
            t += step;
        }
    }
    out.sort_by(|a, b| a.start_beat.partial_cmp(&b.start_beat).unwrap());
    // A syncopated onset that lands on the next step: keep one note there.
    out.dedup_by(|b, a| (a.start_beat - b.start_beat).abs() < 1e-6);
    // No overlaps in a monophonic line: a note ends where the next one starts.
    for i in 0..out.len().saturating_sub(1) {
        let next_start = out[i + 1].start_beat;
        if out[i].start_beat + out[i].length_beats > next_start {
            out[i].length_beats = (next_start - out[i].start_beat).max(0.05);
        }
    }
    out
}

/// Chord spans for a static chord (Compose First without a proposal): one span per
/// `bars` bars on the given root with the given tones.
pub fn static_spans(root_pc: u8, tones: &[(u8, f64)], bars: u32, meter: &Meter) -> Vec<ChordSpan> {
    let bar = meter.bar_beats();
    (0..bars.max(1))
        .map(|b| ChordSpan {
            start_beat: b as f64 * bar,
            end_beat: (b + 1) as f64 * bar,
            root_pc,
            tones: tones.to_vec(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::ScaleKind;

    fn spans() -> Vec<ChordSpan> {
        vec![
            ChordSpan {
                start_beat: 0.0,
                end_beat: 8.0,
                root_pc: 2,
                tones: vec![(2, 0.0), (5, 0.0), (9, 0.0)],
            },
            ChordSpan {
                start_beat: 8.0,
                end_beat: 16.0,
                root_pc: 7,
                tones: vec![(7, 0.0), (10, 0.0), (2, 0.0)],
            },
            ChordSpan {
                start_beat: 16.0,
                end_beat: 24.0,
                root_pc: 0,
                tones: vec![(0, 0.0), (4, 0.0), (7, 0.0)],
            },
        ]
    }

    #[test]
    fn roots_on_changes_everything_in_scale_and_register_deterministic() {
        let scale = Scale::new(2, ScaleKind::Dorian);
        let m = Meter::new(4, 4);
        let p = BassParams {
            density: 0.8,
            syncopation: 0.5,
            passing: 0.8,
            seed: 7,
            ..Default::default()
        };
        let a = generate_bass(&spans(), Some(&scale), &p, &m);
        let b = generate_bass(&spans(), Some(&scale), &p, &m);
        assert_eq!(a, b);
        let c = generate_bass(
            &spans(),
            Some(&scale),
            &BassParams {
                seed: 8,
                ..p.clone()
            },
            &m,
        );
        assert_ne!(a, c);
        for sp in spans() {
            let first = a
                .iter()
                .find(|n| (n.start_beat - sp.start_beat).abs() < 1e-9)
                .unwrap();
            assert_eq!(
                first.midi.rem_euclid(12) as u8,
                sp.root_pc,
                "root on the change"
            );
        }
        let pcs = scale.pitch_classes();
        for n in &a {
            assert!(
                pcs.contains(&(n.midi.rem_euclid(12) as u8)),
                "{} in D dorian",
                n.midi
            );
            assert!((31..=43).contains(&n.midi), "register {}", n.midi);
            assert!(n.length_beats > 0.0 && (0.0..=1.0).contains(&n.velocity));
        }
        for w in a.windows(2) {
            assert!(
                w[0].start_beat + w[0].length_beats <= w[1].start_beat + 1e-9,
                "monophonic"
            );
        }
        assert!(a.len() > 12, "busy at density 0.8: {}", a.len());
        let sparse = generate_bass(
            &spans(),
            Some(&scale),
            &BassParams { density: 0.0, ..p },
            &m,
        );
        assert_eq!(sparse.len(), 3, "roots only");
        assert_eq!(sparse[0].length_beats, 8.0);
    }

    #[test]
    fn static_spans_cover_the_bars_and_bass_follows_the_fifth() {
        let m = Meter::new(7, 8).with_grouping(vec![2, 2, 3]);
        let s = static_spans(9, &[(9, 0.0), (4, 0.0), (0, 0.0)], 4, &m);
        assert_eq!(s.len(), 4);
        assert_eq!(s[3].end_beat, 14.0);
        let line = generate_bass(
            &s,
            None,
            &BassParams {
                density: 0.6,
                seed: 2,
                ..Default::default()
            },
            &m,
        );
        assert!(line
            .iter()
            .all(|n| matches!(n.midi.rem_euclid(12), 9 | 4 | 0)));
        assert!(
            line.iter().any(|n| n.midi.rem_euclid(12) == 4),
            "the fifth appears"
        );
    }
}
