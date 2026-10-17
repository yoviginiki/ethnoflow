//! Countermelody (F4.7): call-and-response answers in the rests of the lead melody.
//! An answer echoes the rhythm of the phrase it follows on chord/mode tones in a
//! register below or above the lead, never while the lead sounds (at tension 0 no
//! countermelody note overlaps a melody note at all), and never a second or a
//! tritone against a concurrent melody note when overlaps are allowed.

use serde::{Deserialize, Serialize};

use crate::notes::Note;
use crate::pattern::Rng;
use crate::phrase::Phrase;
use crate::scale::Scale;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CounterRegister {
    Below,
    Above,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CounterParams {
    pub register: CounterRegister,
    /// 0..1: how far the answer wanders (leaps, more notes, longer answers).
    pub adventure: f64,
    /// 0..1: 0 = strictly in the rests; above 0 answers may start under the last
    /// held melody note (never within a semitone, never a second/tritone against it).
    pub tension: f64,
    /// Shortest rest that gets an answer, seconds.
    pub min_rest_s: f64,
    pub seed: u32,
}

impl Default for CounterParams {
    fn default() -> Self {
        Self {
            register: CounterRegister::Below,
            adventure: 0.4,
            tension: 0.0,
            min_rest_s: 0.6,
            seed: 1,
        }
    }
}

/// A chord tone set active over a time span (seconds); from a harmony proposal or a
/// static chord. Tones as (pitch class, cents).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToneSpan {
    pub start: f64,
    pub end: f64,
    pub tones: Vec<(u8, f64)>,
}

fn tones_at(spans: &[ToneSpan], scale: Option<&Scale>, t: f64) -> Vec<(u8, f64)> {
    if let Some(s) = spans.iter().find(|s| t >= s.start && t < s.end) {
        if !s.tones.is_empty() {
            return s.tones.clone();
        }
    }
    scale
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
        .unwrap_or_default()
}

/// Nearest pitch of one of `pcs` to `target` inside [lo, hi].
fn nearest_tone(
    pcs: &[(u8, f64)],
    target: i32,
    lo: i32,
    hi: i32,
    avoid: &[i32],
) -> Option<(i32, f64)> {
    let mut best: Option<(i32, f64, i32)> = None;
    for midi in lo..=hi {
        let Some(&(_, cents)) = pcs.iter().find(|(p, _)| *p == midi.rem_euclid(12) as u8) else {
            continue;
        };
        if avoid.iter().any(|a| (a - midi).abs() <= 1) {
            continue;
        }
        let d = (midi - target).abs();
        if best.map(|b| d < b.2).unwrap_or(true) {
            best = Some((midi, cents, d));
        }
    }
    best.map(|b| (b.0, b.1))
}

fn melody_sounding(melody: &[Note], t0: f64, t1: f64) -> Vec<i32> {
    melody
        .iter()
        .filter(|n| n.start < t1 && n.end > t0)
        .map(|n| n.midi)
        .collect()
}

/// Dissonant against a sounding melody note: minor/major second or tritone.
fn clashes(midi: i32, sounding: &[i32]) -> bool {
    sounding.iter().any(|m| {
        let ic = (m - midi).rem_euclid(12);
        matches!(ic, 1 | 2 | 6 | 10 | 11) || (m - midi).abs() <= 1
    })
}

/// Generate answers for the rests after each phrase. Times in seconds.
pub fn generate_countermelody(
    melody: &[Note],
    phrases: &[Phrase],
    spans: &[ToneSpan],
    scale: Option<&Scale>,
    params: &CounterParams,
) -> Vec<Note> {
    let mut rng = Rng::new(params.seed ^ 0xC0DE_CAFE);
    let adventure = params.adventure.clamp(0.0, 1.0);
    let tension = params.tension.clamp(0.0, 1.0);
    let mut out: Vec<Note> = Vec::new();
    if melody.is_empty() {
        return out;
    }
    let melody_end = melody.iter().map(|n| n.end).fold(0.0, f64::max);
    // Melody register → answer register a sixth to an octave away.
    let mean_midi = melody.iter().map(|n| n.midi as f64).sum::<f64>() / melody.len() as f64;
    let (lo, hi) = match params.register {
        CounterRegister::Below => ((mean_midi - 17.0) as i32, (mean_midi - 5.0) as i32),
        CounterRegister::Above => ((mean_midi + 5.0) as i32, (mean_midi + 17.0) as i32),
    };
    for (pi, ph) in phrases.iter().enumerate() {
        // The rest: from this phrase's end to the next phrase's start (or the end).
        let next_start = phrases
            .get(pi + 1)
            .map(|n| n.start)
            .unwrap_or(melody_end + 2.0);
        let rest_start = ph.end;
        let rest_end = next_start;
        if rest_end - rest_start < params.min_rest_s {
            continue;
        }
        // Rhythm: echo the last few note durations of the phrase, shortened.
        let last_notes: Vec<&Note> = ph
            .notes
            .iter()
            .rev()
            .take(2 + (adventure * 3.0) as usize)
            .filter_map(|&i| melody.get(i))
            .collect();
        if last_notes.is_empty() {
            continue;
        }
        let mut durations: Vec<f64> = last_notes
            .iter()
            .rev()
            .map(|n| (n.end - n.start).clamp(0.12, 1.2))
            .collect();
        if rng.chance(adventure * 0.5) {
            durations.push(durations[durations.len() - 1] * 1.5);
        }
        let total: f64 = durations.iter().sum::<f64>() + 0.05 * durations.len() as f64;
        let available = rest_end - rest_start - 0.12;
        let scale_t = if total > available {
            available / total
        } else {
            1.0
        };
        if scale_t < 0.3 {
            continue;
        }
        // Start: just after the lead stops (a little earlier with tension, under the
        // held last note, but never clashing).
        let lead_in = if tension > 0.0 {
            let last = last_notes[0];
            (last.end - last.start) * 0.5 * tension
        } else {
            0.0
        };
        let mut t = rest_start - lead_in + 0.08;
        // Contour: an answer to a rising phrase falls, and the other way round.
        let last_midi = last_notes[0].midi;
        let prev_midi = last_notes.get(1).map(|n| n.midi).unwrap_or(last_midi);
        let dir: i32 = if last_midi >= prev_midi { -1 } else { 1 };
        let mut target = (last_midi + dir * 3).clamp(lo, hi);
        for (k, d) in durations.iter().enumerate() {
            let len = d * scale_t;
            let end = t + len;
            if end > rest_end - 0.02 {
                break;
            }
            let pcs = tones_at(spans, scale, t);
            if pcs.is_empty() {
                break;
            }
            let sounding = melody_sounding(melody, t, end);
            if !sounding.is_empty() && tension == 0.0 {
                // Strictly in the rests.
                t = end + 0.05;
                continue;
            }
            let avoid: Vec<i32> = sounding.clone();
            let Some((midi, cents)) = nearest_tone(&pcs, target, lo, hi, &avoid) else {
                break;
            };
            if clashes(midi, &sounding) {
                t = end + 0.05;
                continue;
            }
            out.push(Note {
                start: t,
                end,
                midi,
                cents,
                confidence: 1.0,
                velocity: 0.55 + 0.25 * (1.0 - k as f64 / durations.len() as f64),
                bend: Vec::new(),
            });
            // Next target: a step, sometimes a leap with adventure.
            let leap = if rng.chance(adventure * 0.6) {
                3 + rng.below(4) as i32
            } else {
                1 + rng.below(2) as i32
            };
            let step_dir = if rng.chance(0.7) { dir } else { -dir };
            target = (midi + step_dir * leap).clamp(lo, hi);
            t = end + 0.05;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::phrase::{BoundaryCause, ContourShape};
    use crate::scale::ScaleKind;

    fn note(start: f64, end: f64, midi: i32) -> Note {
        Note {
            start,
            end,
            midi,
            cents: 0.0,
            confidence: 1.0,
            velocity: 0.8,
            bend: Vec::new(),
        }
    }

    fn phrase(start: f64, end: f64, notes: Vec<usize>) -> Phrase {
        Phrase {
            start,
            end,
            notes,
            stable_tones: Vec::new(),
            shape: ContourShape::Arch,
            breath_before: true,
            cause: BoundaryCause::Gap,
            confidence: 0.9,
        }
    }

    #[test]
    fn answers_fall_in_the_rests_on_scale_tones_and_never_overlap_at_tension_zero() {
        // Two kaval phrases with a 2 s rest between them and a 3 s tail.
        let melody = vec![
            note(0.0, 0.5, 74),
            note(0.5, 1.0, 76),
            note(1.0, 2.0, 77),
            note(4.0, 4.4, 79),
            note(4.4, 4.8, 77),
            note(4.8, 6.0, 74),
        ];
        let phrases = vec![
            phrase(0.0, 2.0, vec![0, 1, 2]),
            phrase(4.0, 6.0, vec![3, 4, 5]),
        ];
        let scale = Scale::new(2, ScaleKind::Dorian);
        let p = CounterParams {
            seed: 4,
            ..Default::default()
        };
        let a = generate_countermelody(&melody, &phrases, &[], Some(&scale), &p);
        let b = generate_countermelody(&melody, &phrases, &[], Some(&scale), &p);
        assert_eq!(a, b);
        assert!(a.len() >= 3, "answers exist: {}", a.len());
        let pcs = scale.pitch_classes();
        for n in &a {
            assert!(
                pcs.contains(&(n.midi.rem_euclid(12) as u8)),
                "{} in D dorian",
                n.midi
            );
            assert!(n.midi < 72, "below the lead: {}", n.midi);
            // Zero overlaps with the melody within a semitone (actually zero overlaps at all).
            for m in &melody {
                let overlap = n.start < m.end && n.end > m.start;
                assert!(
                    !overlap,
                    "note {:?} overlaps melody {:?}",
                    (n.start, n.end, n.midi),
                    (m.start, m.end, m.midi)
                );
            }
        }
        // Above register option and spans.
        let spans = vec![ToneSpan {
            start: 0.0,
            end: 10.0,
            tones: vec![(2, 0.0), (5, 0.0), (9, 0.0)],
        }];
        let up = generate_countermelody(
            &melody,
            &phrases,
            &spans,
            Some(&scale),
            &CounterParams {
                register: CounterRegister::Above,
                ..p.clone()
            },
        );
        assert!(up
            .iter()
            .all(|n| n.midi > 78 && matches!(n.midi.rem_euclid(12), 2 | 5 | 9)));
        // With tension an answer may start under the held last note, but never clashing.
        let tense = generate_countermelody(
            &melody,
            &phrases,
            &[],
            Some(&scale),
            &CounterParams { tension: 1.0, ..p },
        );
        for n in &tense {
            for m in &melody {
                if n.start < m.end && n.end > m.start {
                    let ic = (m.midi - n.midi).rem_euclid(12);
                    assert!(
                        !matches!(ic, 1 | 2 | 6 | 10 | 11),
                        "clash {} vs {}",
                        n.midi,
                        m.midi
                    );
                }
            }
        }
    }
}
