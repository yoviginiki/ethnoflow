//! Phrase recognition (F2.9): boundaries from pauses (breath), a long tone with an
//! energy drop, or a leap after a held tone — optionally helped by bar lines. Works in
//! free time. Each phrase carries its stable tones weighted by duration, contour
//! stability and metric position.

use serde::{Deserialize, Serialize};

use crate::notes::Note;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhraseParams {
    /// A silence at least this long ends a phrase (winds: a breath). Seconds.
    pub gap_seconds: f64,
    /// A note at least this long counts as "held".
    pub long_tone_seconds: f64,
    /// A jump of at least this many semitones after a held tone starts a new phrase.
    pub leap_semitones: f64,
    /// The next note must be this fraction quieter than a held tone to count as a drop.
    pub drop_ratio: f64,
    /// Bar length in seconds when the take has a tempo map (boundaries near bar lines
    /// are trusted more); `None` in free time.
    pub bar_seconds: Option<f64>,
}

impl Default for PhraseParams {
    fn default() -> Self {
        Self {
            gap_seconds: 0.35,
            long_tone_seconds: 0.8,
            leap_semitones: 5.0,
            drop_ratio: 0.6,
            bar_seconds: None,
        }
    }
}

impl PhraseParams {
    /// Wind instruments breathe: a longer gap before a boundary is called.
    pub fn for_wind() -> Self {
        Self {
            gap_seconds: 0.45,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContourShape {
    Rising,
    Falling,
    Arch,
    Valley,
    Flat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StableTone {
    pub midi: i32,
    /// Weighted mean deviation from `midi` in cents.
    pub cents: f64,
    /// Share of the phrase's stable weight, 0..1 (sums to 1 over the phrase).
    pub weight: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryCause {
    Start,
    Gap,
    LongToneDrop,
    Leap,
    End,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Phrase {
    pub start: f64,
    pub end: f64,
    /// Indices into the note list, in time order.
    pub notes: Vec<usize>,
    pub stable_tones: Vec<StableTone>,
    pub shape: ContourShape,
    pub breath_before: bool,
    pub cause: BoundaryCause,
    /// How sure the phrase boundaries are, 0..1.
    pub confidence: f64,
}

/// Contour stability of a note: 1 for a flat tone, towards 0 for ±50 cents of wobble.
fn stability(n: &Note) -> f64 {
    if n.bend.len() < 2 {
        return 1.0;
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(_, c) in &n.bend {
        lo = lo.min(c);
        hi = hi.max(c);
    }
    1.0 / (1.0 + (hi - lo) / 50.0)
}

/// Metric weight: tones starting near a beat count more when a tempo is known.
fn metric_weight(start: f64, bar: Option<f64>) -> f64 {
    match bar {
        Some(b) if b > 0.0 => {
            let beat = b / 4.0;
            let off = (start / beat).fract();
            let d = off.min(1.0 - off) * beat;
            if d < 0.06 {
                1.3
            } else {
                1.0
            }
        }
        _ => 1.0,
    }
}

fn near_bar(t: f64, bar: Option<f64>) -> bool {
    match bar {
        Some(b) if b > 0.0 => {
            let off = (t / b).fract();
            off.min(1.0 - off) * b < 0.1
        }
        _ => false,
    }
}

/// Weighted stable tones of a set of notes.
pub fn stable_tones(notes: &[Note], idx: &[usize], bar: Option<f64>) -> Vec<StableTone> {
    let mut acc: Vec<(i32, f64, f64)> = Vec::new(); // midi, weight, weight×cents
    for &i in idx {
        let n = &notes[i];
        let w = n.duration() * stability(n) * metric_weight(n.start, bar) * n.confidence.max(0.0);
        if w <= 0.0 {
            continue;
        }
        match acc.iter_mut().find(|a| a.0 == n.midi) {
            Some(a) => {
                a.1 += w;
                a.2 += w * n.cents;
            }
            None => acc.push((n.midi, w, w * n.cents)),
        }
    }
    let total: f64 = acc.iter().map(|a| a.1).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let mut out: Vec<StableTone> = acc
        .into_iter()
        .map(|(midi, w, wc)| StableTone {
            midi,
            cents: wc / w,
            weight: w / total,
        })
        .collect();
    out.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());
    out
}

fn shape(notes: &[Note], idx: &[usize]) -> ContourShape {
    if idx.len() < 2 {
        return ContourShape::Flat;
    }
    let first = notes[idx[0]].midi as f64;
    let last = notes[idx[idx.len() - 1]].midi as f64;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &i in idx {
        lo = lo.min(notes[i].midi as f64);
        hi = hi.max(notes[i].midi as f64);
    }
    let ends_max = first.max(last);
    let ends_min = first.min(last);
    if hi - ends_max >= 2.0 && (last - first).abs() < 2.0 {
        ContourShape::Arch
    } else if ends_min - lo >= 2.0 && (last - first).abs() < 2.0 {
        ContourShape::Valley
    } else if last - first >= 2.0 {
        ContourShape::Rising
    } else if first - last >= 2.0 {
        ContourShape::Falling
    } else {
        ContourShape::Flat
    }
}

/// Split notes (time-ordered) into phrases. Never empty when `notes` is not empty.
pub fn segment_phrases(notes: &[Note], p: &PhraseParams) -> Vec<Phrase> {
    if notes.is_empty() {
        return Vec::new();
    }
    // (cause, confidence) of the boundary *before* note i; index 0 is the start.
    let mut boundaries: Vec<Option<(BoundaryCause, f64)>> = vec![None; notes.len()];
    boundaries[0] = Some((BoundaryCause::Start, 1.0));
    for i in 1..notes.len() {
        let prev = &notes[i - 1];
        let cur = &notes[i];
        let gap = cur.start - prev.end;
        let bar_bonus = if near_bar(cur.start, p.bar_seconds) {
            0.1
        } else {
            0.0
        };
        let mut best: Option<(BoundaryCause, f64)> = None;
        if gap >= p.gap_seconds {
            let c = (0.7 + 0.3 * ((gap / p.gap_seconds) - 1.0).min(1.0) + bar_bonus).min(1.0);
            best = Some((BoundaryCause::Gap, c));
        }
        let held = prev.duration() >= p.long_tone_seconds;
        if best.is_none() && held && cur.velocity < prev.velocity * p.drop_ratio {
            best = Some((BoundaryCause::LongToneDrop, (0.65 + bar_bonus).min(1.0)));
        }
        if best.is_none()
            && prev.duration() >= p.long_tone_seconds
            && ((cur.midi - prev.midi).abs() as f64) >= p.leap_semitones
            && gap >= 0.0
        {
            best = Some((BoundaryCause::Leap, (0.55 + bar_bonus).min(1.0)));
        }
        boundaries[i] = best;
    }
    let mut phrases = Vec::new();
    let mut i = 0;
    while i < notes.len() {
        let (cause, start_conf) = boundaries[i].unwrap_or((BoundaryCause::Start, 1.0));
        let mut j = i + 1;
        while j < notes.len() && boundaries[j].is_none() {
            j += 1;
        }
        let end_conf = if j < notes.len() {
            boundaries[j].map(|b| b.1).unwrap_or(1.0)
        } else {
            1.0
        };
        let idx: Vec<usize> = (i..j).collect();
        phrases.push(Phrase {
            start: notes[i].start,
            end: notes[j - 1].end,
            stable_tones: stable_tones(notes, &idx, p.bar_seconds),
            shape: shape(notes, &idx),
            breath_before: cause == BoundaryCause::Gap,
            cause,
            confidence: start_conf.min(end_conf),
            notes: idx,
        });
        i = j;
    }
    phrases
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn note(start: f64, dur: f64, midi: i32) -> Note {
        Note {
            start,
            end: start + dur,
            midi,
            cents: 0.0,
            confidence: 0.9,
            velocity: 0.8,
            bend: Vec::new(),
        }
    }

    /// Notes from (midi, duration, gap-after) triples, back to back.
    pub(crate) fn melody(spec: &[(i32, f64, f64)]) -> Vec<Note> {
        let mut t = 0.0;
        let mut out = Vec::new();
        for &(m, d, g) in spec {
            out.push(note(t, d, m));
            t += d + g;
        }
        out
    }

    #[test]
    fn breath_gaps_split_phrases_and_mark_breath() {
        let notes = melody(&[
            (62, 0.3, 0.0),
            (64, 0.3, 0.0),
            (65, 0.6, 0.5), // breath
            (67, 0.3, 0.0),
            (65, 0.3, 0.0),
            (62, 0.8, 0.0),
        ]);
        let ph = segment_phrases(&notes, &PhraseParams::default());
        assert_eq!(ph.len(), 2);
        assert_eq!(ph[0].notes, vec![0, 1, 2]);
        assert!(!ph[0].breath_before && ph[1].breath_before);
        assert_eq!(ph[1].cause, BoundaryCause::Gap);
        assert!((ph[1].start - 1.7).abs() < 1e-9);
        assert_eq!(ph[0].shape, ContourShape::Rising);
        assert_eq!(ph[1].shape, ContourShape::Falling);
        // The long final D carries the most weight in phrase 2.
        assert_eq!(ph[1].stable_tones[0].midi, 62);
        let sum: f64 = ph[1].stable_tones.iter().map(|t| t.weight).sum();
        assert!((sum - 1.0).abs() < 1e-9);
    }

    #[test]
    fn held_tone_with_drop_or_leap_splits_without_a_gap() {
        let mut notes = melody(&[(60, 1.0, 0.0), (62, 0.3, 0.0), (64, 0.3, 0.0)]);
        notes[1].velocity = 0.3; // energy drop after the held C
        let ph = segment_phrases(&notes, &PhraseParams::default());
        assert_eq!(ph.len(), 2);
        assert_eq!(ph[1].cause, BoundaryCause::LongToneDrop);
        let notes = melody(&[(60, 0.9, 0.0), (69, 0.3, 0.0), (71, 0.3, 0.0)]);
        let ph = segment_phrases(&notes, &PhraseParams::default());
        assert_eq!(ph.len(), 2);
        assert_eq!(ph[1].cause, BoundaryCause::Leap);
        // A small step after a held tone is not a boundary.
        let notes = melody(&[(60, 0.9, 0.0), (62, 0.3, 0.0)]);
        assert_eq!(segment_phrases(&notes, &PhraseParams::default()).len(), 1);
    }

    #[test]
    fn free_time_and_tempo_agree_on_boundaries_and_beats_weigh_more() {
        let notes = melody(&[
            (62, 0.5, 0.0),
            (64, 0.5, 0.0),
            (65, 1.0, 0.4),
            (67, 0.5, 0.0),
            (62, 0.5, 0.0),
        ]);
        let free = segment_phrases(&notes, &PhraseParams::default());
        let timed = segment_phrases(
            &notes,
            &PhraseParams {
                bar_seconds: Some(2.0),
                ..PhraseParams::default()
            },
        );
        assert_eq!(free.len(), timed.len());
        assert!(timed[1].confidence >= free[1].confidence);
        // Wobbly tones weigh less than flat ones of the same length.
        let mut wobbly = note(0.0, 1.0, 62);
        wobbly.bend = vec![(0.0, -30.0), (0.5, 30.0)];
        let flat = note(1.0, 1.0, 64);
        let tones = stable_tones(&[wobbly, flat], &[0, 1], None);
        assert_eq!(tones[0].midi, 64);
        assert!(tones[0].weight > tones[1].weight);
        assert!(segment_phrases(&[], &PhraseParams::default()).is_empty());
    }
}
