//! Note extraction from a pitch contour (offline, a proposal only).
//!
//! The contour is never modified; the result is a separate list of notes, each with
//! the raw pitch deviation inside it (vibrato, slides, grace notes) kept as bend data.
//! No grid, tempo or key is involved; quantisation is a separate step elsewhere.

use serde::{Deserialize, Serialize};

/// One contour frame, already converted to fractional MIDI (see `Reference`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PitchSample {
    pub t: f64,
    /// Fractional MIDI number, `None` when unvoiced.
    pub midi: Option<f64>,
    pub confidence: f64,
    /// Linear RMS of the frame.
    pub rms: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct NoteParams {
    /// Frames below this confidence count as unvoiced.
    pub min_confidence: f64,
    /// Shorter segments are ornaments: attached to a neighbour as bend data.
    pub min_note_seconds: f64,
    /// A sustained deviation from the note's centre beyond this starts a new note.
    pub split_cents: f64,
    /// How long the deviation must persist (keeps vibrato and ornaments inside).
    pub hold_seconds: f64,
    /// An unvoiced gap at least this long ends the note.
    pub rest_seconds: f64,
    /// An RMS rise of at least this many dB within two frames re-articulates the
    /// same pitch (tongued repeated notes).
    pub onset_db: f64,
}

impl Default for NoteParams {
    fn default() -> Self {
        Self {
            min_confidence: 0.6,
            min_note_seconds: 0.06,
            split_cents: 80.0,
            hold_seconds: 0.03,
            rest_seconds: 0.05,
            onset_db: 6.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub start: f64,
    pub end: f64,
    /// Nearest tempered note to the median pitch.
    pub midi: i32,
    /// Median deviation from `midi`, in cents.
    pub cents: f64,
    /// Mean frame confidence.
    pub confidence: f64,
    /// Peak RMS relative to the loudest note in the result, 0..1.
    pub velocity: f64,
    /// (seconds from `start`, cents from `midi`) for every voiced frame of the note.
    pub bend: Vec<(f64, f64)>,
}

impl Note {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

struct Segment {
    frames: Vec<(f64, f64, f32)>, // t, midi, rms
    confidence_sum: f64,
    hop: f64,
}

impl Segment {
    fn start(&self) -> f64 {
        self.frames[0].0
    }
    fn end(&self) -> f64 {
        self.frames[self.frames.len() - 1].0 + self.hop
    }
    fn duration(&self) -> f64 {
        self.end() - self.start()
    }
    fn median_midi(&self) -> f64 {
        let mut v: Vec<f64> = self.frames.iter().map(|f| f.1).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    }
    fn peak_rms(&self) -> f32 {
        self.frames.iter().map(|f| f.2).fold(0.0, f32::max)
    }
    /// A segment whose pitch moves steadily (slide/portamento) rather than staying
    /// around a centre: the first and last thirds differ by more than `split_cents`.
    /// Vibrato is symmetric, so its thirds agree and it stays a note.
    fn is_transitional(&self, split_cents: f64) -> bool {
        let n = self.frames.len();
        if n < 3 {
            return false;
        }
        let third = n / 3;
        let med = |fr: &[(f64, f64, f32)]| {
            let mut v: Vec<f64> = fr.iter().map(|f| f.1).collect();
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[v.len() / 2]
        };
        let first = med(&self.frames[..third.max(1)]);
        let last = med(&self.frames[n - third.max(1)..]);
        (last - first).abs() * 100.0 > split_cents
    }
}

/// Append `seg` to the last merged note if the two are contiguous; otherwise drop it.
fn attach_to_last(merged: &mut [Segment], seg: Segment, rest_seconds: f64) {
    if let Some(prev) = merged.last_mut() {
        if seg.start() - prev.end() < rest_seconds {
            prev.frames.extend(seg.frames);
            prev.confidence_sum += seg.confidence_sum;
        }
    }
}

fn db(x: f32) -> f64 {
    20.0 * (x.max(1e-9) as f64).log10()
}

/// Cut the voiced parts of `samples` into notes.
pub fn extract_notes(samples: &[PitchSample], p: &NoteParams) -> Vec<Note> {
    let hop = samples
        .windows(2)
        .map(|w| w[1].t - w[0].t)
        .find(|&d| d > 0.0)
        .unwrap_or(0.01);
    let hold_frames = (p.hold_seconds / hop).round().max(1.0) as usize;

    let mut segments: Vec<Segment> = Vec::new();
    let mut cur: Option<Segment> = None;
    // Running "anchor" pitch of the current segment and how many consecutive frames
    // have been away from it.
    let mut anchor = 0.0f64;
    let mut away = 0usize;
    let mut last_voiced_t = f64::NEG_INFINITY;

    let close = |cur: &mut Option<Segment>, segments: &mut Vec<Segment>| {
        if let Some(s) = cur.take() {
            segments.push(s);
        }
    };

    for (i, s) in samples.iter().enumerate() {
        let voiced = s.midi.is_some() && s.confidence >= p.min_confidence;
        let Some(midi) = s.midi.filter(|_| voiced) else {
            if cur.is_some() && s.t - last_voiced_t >= p.rest_seconds {
                close(&mut cur, &mut segments);
            }
            continue;
        };
        last_voiced_t = s.t;
        match cur.as_mut() {
            None => {
                cur = Some(Segment {
                    frames: vec![(s.t, midi, s.rms)],
                    confidence_sum: s.confidence,
                    hop,
                });
                anchor = midi;
                away = 0;
            }
            Some(seg) => {
                let dev_cents = (midi - anchor) * 100.0;
                // Re-articulation: a sharp rise in level on the same pitch.
                let onset = i >= 2
                    && db(s.rms) - db(samples[i - 2].rms) >= p.onset_db
                    && seg.frames.len() >= hold_frames;
                if dev_cents.abs() > p.split_cents {
                    away += 1;
                } else {
                    away = 0;
                }
                if onset || away >= hold_frames {
                    // Split: the frames that were already "away" belong to the new note.
                    let keep = if onset {
                        seg.frames.len()
                    } else {
                        seg.frames.len() + 1 - away
                    };
                    let moved: Vec<_> = seg.frames.split_off(keep);
                    let moved_conf = moved.len() as f64 * s.confidence;
                    seg.confidence_sum -= moved_conf.min(seg.confidence_sum);
                    if seg.frames.is_empty() {
                        // Degenerate (should not happen): put everything back.
                        seg.frames = moved;
                        seg.frames.push((s.t, midi, s.rms));
                        seg.confidence_sum += s.confidence;
                        continue;
                    }
                    close(&mut cur, &mut segments);
                    let mut frames = moved;
                    frames.push((s.t, midi, s.rms));
                    let n = frames.len() as f64;
                    cur = Some(Segment {
                        frames,
                        confidence_sum: n * s.confidence,
                        hop,
                    });
                    anchor = midi;
                    away = 0;
                } else {
                    seg.frames.push((s.t, midi, s.rms));
                    seg.confidence_sum += s.confidence;
                    // Let the anchor follow slow drift so a long note with a slight
                    // tuning change is not split.
                    if away == 0 {
                        anchor += (midi - anchor) * 0.05;
                    }
                }
            }
        }
    }
    close(&mut cur, &mut segments);

    // Ornaments and slides: short or transitional segments attach to a contiguous
    // neighbour — a slide prefers the note it leads into, a trailing grace the
    // note before it.
    let mut merged: Vec<Segment> = Vec::new();
    let mut pending_short: Option<Segment> = None;
    for seg in segments {
        let is_short = seg.duration() < p.min_note_seconds;
        let is_transitional = seg.is_transitional(p.split_cents);
        if is_transitional {
            pending_short = Some(match pending_short.take() {
                Some(mut ps) if seg.start() - ps.end() < p.rest_seconds => {
                    ps.frames.extend(seg.frames);
                    ps.confidence_sum += seg.confidence_sum;
                    ps
                }
                Some(ps) => {
                    // Orphaned earlier transition: give it to the previous note if any.
                    attach_to_last(&mut merged, ps, p.rest_seconds);
                    seg
                }
                None => seg,
            });
            continue;
        }
        if is_short {
            if let Some(prev) = merged.last_mut() {
                if seg.start() - prev.end() < p.rest_seconds {
                    prev.frames.extend(seg.frames);
                    prev.confidence_sum += seg.confidence_sum;
                    continue;
                }
            }
            // Try the next note instead.
            pending_short = Some(match pending_short.take() {
                Some(mut ps) if seg.start() - ps.end() < p.rest_seconds => {
                    ps.frames.extend(seg.frames);
                    ps.confidence_sum += seg.confidence_sum;
                    ps
                }
                _ => seg,
            });
            continue;
        }
        let mut seg = seg;
        if let Some(ps) = pending_short.take() {
            if seg.start() - ps.end() < p.rest_seconds {
                let mut frames = ps.frames;
                frames.extend(seg.frames);
                seg.frames = frames;
                seg.confidence_sum += ps.confidence_sum;
            }
        }
        merged.push(seg);
    }
    // A trailing ornament/slide: attach to the last note if contiguous, else drop it
    // (it is still in the contour).
    if let Some(ps) = pending_short.take() {
        attach_to_last(&mut merged, ps, p.rest_seconds);
    }

    let loudest = merged
        .iter()
        .map(|s| s.peak_rms())
        .fold(0.0f32, f32::max)
        .max(1e-9);
    merged
        .into_iter()
        .map(|seg| {
            let med = seg.median_midi();
            let midi = med.round() as i32;
            let start = seg.start();
            Note {
                start,
                end: seg.end(),
                midi,
                cents: (med - midi as f64) * 100.0,
                confidence: seg.confidence_sum / seg.frames.len() as f64,
                velocity: (seg.peak_rms() / loudest) as f64,
                bend: seg
                    .frames
                    .iter()
                    .map(|&(t, m, _)| (t - start, (m - midi as f64) * 100.0))
                    .collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOP: f64 = 0.01;

    /// Build a contour from (midi or None, seconds) steps with optional per-frame pitch
    /// function and rms.
    fn contour(steps: &[(Option<f64>, f64)], jitter: impl Fn(usize) -> f64) -> Vec<PitchSample> {
        let mut out = Vec::new();
        let mut t = 0.0;
        let mut i = 0usize;
        for &(midi, secs) in steps {
            let n = (secs / HOP).round() as usize;
            for _ in 0..n {
                out.push(PitchSample {
                    t,
                    midi: midi.map(|m| m + jitter(i) / 100.0),
                    confidence: if midi.is_some() { 0.95 } else { 0.0 },
                    rms: if midi.is_some() { 0.1 } else { 0.0 },
                });
                t += HOP;
                i += 1;
            }
        }
        out
    }

    fn jitter(i: usize) -> f64 {
        // deterministic ±5 cents
        ((i * 7919) % 11) as f64 - 5.0
    }

    #[test]
    fn simple_melody_gives_exact_notes() {
        let c = contour(
            &[
                (Some(60.0), 0.5),
                (Some(62.0), 0.5),
                (Some(64.0), 0.5),
                (None, 0.2),
                (Some(67.0), 0.8),
            ],
            jitter,
        );
        let notes = extract_notes(&c, &NoteParams::default());
        let midis: Vec<i32> = notes.iter().map(|n| n.midi).collect();
        assert_eq!(midis, vec![60, 62, 64, 67]);
        let expect = [(0.0, 0.5), (0.5, 1.0), (1.0, 1.5), (1.7, 2.5)];
        for (n, (s, e)) in notes.iter().zip(expect) {
            assert!((n.start - s).abs() <= 0.02, "start {} vs {s}", n.start);
            assert!((n.end - e).abs() <= 0.02, "end {} vs {e}", n.end);
            assert!(n.cents.abs() <= 5.0);
            assert!(n.confidence > 0.9);
            assert!((n.velocity - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn vibrato_stays_one_note_with_bend_data() {
        let c = contour(&[(Some(69.0), 1.0)], |i| {
            50.0 * (i as f64 * HOP * 6.0 * std::f64::consts::TAU).sin()
        });
        let notes = extract_notes(&c, &NoteParams::default());
        assert_eq!(notes.len(), 1, "{notes:?}");
        let n = &notes[0];
        assert_eq!(n.midi, 69);
        assert_eq!(n.bend.len(), 100);
        let max = n.bend.iter().fold(0.0f64, |m, b| m.max(b.1.abs()));
        assert!(max > 40.0 && max <= 55.0, "max bend {max}");
    }

    #[test]
    fn grace_note_is_kept_inside_the_main_note() {
        let c = contour(&[(Some(62.0), 0.03), (Some(60.0), 0.5)], jitter);
        let notes = extract_notes(&c, &NoteParams::default());
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert_eq!(notes[0].midi, 60);
        assert!(
            notes[0].start.abs() < 1e-9,
            "the grace note extends the start"
        );
        assert!(
            notes[0].bend[0].1 > 150.0,
            "first bend sample is the grace note"
        );
    }

    #[test]
    fn repeated_pitch_is_split_by_rest_and_by_onset() {
        // By a 60 ms rest.
        let c = contour(
            &[(Some(60.0), 0.3), (None, 0.06), (Some(60.0), 0.3)],
            jitter,
        );
        assert_eq!(extract_notes(&c, &NoteParams::default()).len(), 2);
        // By re-articulation: same pitch, level dips then jumps 12 dB.
        let mut c = contour(&[(Some(60.0), 0.6)], jitter);
        for (i, s) in c.iter_mut().enumerate() {
            s.rms = if (28..30).contains(&i) { 0.02 } else { 0.1 };
        }
        let notes = extract_notes(&c, &NoteParams::default());
        assert_eq!(notes.len(), 2, "{notes:?}");
        assert!((notes[1].start - 0.30).abs() <= 0.02);
    }

    #[test]
    fn slide_ends_on_the_target_note() {
        // Glissando 60 → 67 over 0.3 s, then 67 held 0.5 s.
        let mut c = contour(&[(Some(60.0), 0.3), (Some(67.0), 0.5)], |_| 0.0);
        for (i, s) in c.iter_mut().enumerate().take(30) {
            s.midi = Some(60.0 + 7.0 * i as f64 / 30.0);
        }
        let notes = extract_notes(&c, &NoteParams::default());
        assert_eq!(notes.len(), 1, "{notes:?}");
        let n = &notes[0];
        assert_eq!(n.midi, 67);
        assert!(n.start.abs() < 1e-9, "the slide leads into the note");
        assert!((n.end - 0.8).abs() <= 0.02);
        assert!(n.bend[0].1 < -600.0, "bend starts near -700 cents");
        assert!(n.cents.abs() < 5.0, "median sits on the held note");
    }

    #[test]
    fn vibrato_is_not_mistaken_for_a_slide() {
        let c = contour(&[(Some(60.0), 0.4), (Some(69.0), 0.4)], |i| {
            50.0 * (i as f64 * HOP * 6.0 * std::f64::consts::TAU).sin()
        });
        let notes = extract_notes(&c, &NoteParams::default());
        assert_eq!(
            notes.iter().map(|n| n.midi).collect::<Vec<_>>(),
            vec![60, 69],
            "{notes:?}"
        );
    }

    #[test]
    fn low_confidence_and_empty_input_give_no_notes() {
        let mut c = contour(&[(Some(60.0), 0.5)], jitter);
        for s in c.iter_mut() {
            s.confidence = 0.3;
        }
        assert!(extract_notes(&c, &NoteParams::default()).is_empty());
        assert!(extract_notes(&[], &NoteParams::default()).is_empty());
    }
}
