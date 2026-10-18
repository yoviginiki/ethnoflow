//! Performance intent (F2.11): classify every note as stable / ornament / transition /
//! noise / probable error with rules and confidences, estimate what the musician meant
//! per phrase, and run the tonal tracker on *intended* tones only — so single wrong
//! notes never move the harmony, while consistent, musically grounded changes do.
//!
//! A note outside the scale is never automatically an error: short and next to a held
//! tone → ornament; held and repeated in the next phrase → candidate for a mode change
//! (the tracker decides); single, held, unrepeated → probable error with low weight.

use serde::{Deserialize, Serialize};

use crate::key::PitchClassHistogram;
use crate::notes::Note;
use crate::phrase::{segment_phrases, Phrase, PhraseParams, StableTone};
use crate::scale::{Scale, ScaleKind};
use crate::tracker::{Locks, TonalContext, TonalTracker, TrackerParams};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteClass {
    Stable,
    Ornament,
    Transition,
    Noise,
    ProbableError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    Short,
    Held,
    NextToHeldTone,
    InScale,
    OutOfScale,
    LowConfidence,
    Slide,
    RepeatsNextPhrase,
    OffProfile,
    MatchesExpected,
    DiffersFromExpected,
    Isolated,
    UserMarked,
    LiteralMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Classified {
    pub index: usize,
    pub phrase: usize,
    pub class: NoteClass,
    pub confidence: f64,
    pub cents_from_scale: Option<f64>,
    pub cents_from_profile: Option<f64>,
    pub expected_midi: Option<i32>,
    /// Deviation from the expected note in cents (trainer input).
    pub cents_from_expected: Option<f64>,
    /// True when the deviation lasts (a held note), false when it is brief.
    pub sustained: bool,
    pub reasons: Vec<Reason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentEstimate {
    pub phrase: usize,
    /// What the musician most likely meant: stable tones after removing noise and
    /// probable errors (ornaments count a little).
    pub tones: Vec<StableTone>,
    /// Share of the phrase's weight that is stable and in context, 0..1.
    pub confidence: f64,
    pub stable_notes: usize,
    pub ornaments: usize,
    pub transitions: usize,
    pub noise: usize,
    pub probable_errors: usize,
}

/// How harmony and the trainer consume intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Literal: every note is taken as meant; the context follows at once.
    Off,
    Stable,
    Assisted,
    Creative,
}

/// The two user sliders, 0..1.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ModeParams {
    /// "Tolerance to errors": higher = deviations are forgiven more readily.
    pub tolerance: f64,
    /// "Sensitivity to changes": higher = the context follows sooner.
    pub sensitivity: f64,
}

impl Mode {
    pub fn default_params(self) -> ModeParams {
        match self {
            Mode::Off => ModeParams {
                tolerance: 0.0,
                sensitivity: 1.0,
            },
            Mode::Stable => ModeParams {
                tolerance: 0.8,
                sensitivity: 0.2,
            },
            Mode::Assisted => ModeParams {
                tolerance: 0.5,
                sensitivity: 0.5,
            },
            Mode::Creative => ModeParams {
                tolerance: 0.3,
                sensitivity: 0.8,
            },
        }
    }

    /// Tracker thresholds from the sliders. Even Creative never switches inside a phrase.
    pub fn tracker_params(self, p: ModeParams) -> TrackerParams {
        if self == Mode::Off {
            return TrackerParams {
                window_phrases: 1,
                forgetting: 1.0,
                switch_phrases: 1,
                switch_margin: 0.0,
                ..TrackerParams::default()
            };
        }
        let sens = p.sensitivity.clamp(0.0, 1.0);
        TrackerParams {
            window_phrases: 4 + (4.0 * (1.0 - sens)).round() as usize,
            forgetting: 0.55 + 0.3 * (1.0 - sens),
            switch_phrases: 1 + (2.0 * (1.0 - sens)).round() as usize,
            switch_margin: 0.05 + 0.25 * (1.0 - sens),
            top_k: 5,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassifierParams {
    /// Notes up to this long are "short" (ornament/transition candidates).
    pub ornament_max_seconds: f64,
    /// Frame confidence below this is noise.
    pub noise_confidence: f64,
    pub noise_min_seconds: f64,
    /// Pitch travel within a note that makes it a slide.
    pub slide_cents: f64,
    /// Cents from the nearest scale degree still counted as in the scale.
    pub in_scale_cents: f64,
    /// A short note must start within this of a held neighbour to be an ornament.
    pub adjacency_seconds: f64,
}

impl Default for ClassifierParams {
    fn default() -> Self {
        Self {
            ornament_max_seconds: 0.18,
            noise_confidence: 0.4,
            noise_min_seconds: 0.04,
            slide_cents: 60.0,
            in_scale_cents: 35.0,
            adjacency_seconds: 0.12,
        }
    }
}

impl ClassifierParams {
    /// Scale the thresholds by the tolerance slider.
    pub fn with_tolerance(mut self, tolerance: f64) -> Self {
        let t = tolerance.clamp(0.0, 1.0);
        self.ornament_max_seconds *= 0.7 + 0.8 * t;
        self.in_scale_cents = 25.0 + 25.0 * t;
        self
    }
}

/// A user's mark on a note (saved per take); calibrates the classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Annotation {
    pub index: usize,
    pub class: NoteClass,
}

/// Per-user calibration from annotations: deterministic Bayesian-style update of two
/// thresholds with pseudo-counts, so a few marks nudge and many marks move.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    /// Multiplier on `ornament_max_seconds` (0.5..2).
    pub ornament_scale: f64,
    /// Multiplier on `in_scale_cents` (0.6..1.6).
    pub intonation_scale: f64,
}

impl Default for Calibration {
    fn default() -> Self {
        Self {
            ornament_scale: 1.0,
            intonation_scale: 1.0,
        }
    }
}

impl Calibration {
    /// `disagreements` are (what the classifier said, what the user marked) pairs.
    pub fn from_marks(disagreements: &[(NoteClass, NoteClass)]) -> Self {
        const PRIOR: f64 = 4.0;
        let mut more_ornament = 0.0;
        let mut fewer_ornament = 0.0;
        let mut forgive_pitch = 0.0;
        let mut strict_pitch = 0.0;
        for &(said, marked) in disagreements {
            match (said, marked) {
                (NoteClass::ProbableError, NoteClass::Ornament)
                | (NoteClass::Transition, NoteClass::Ornament)
                | (NoteClass::Stable, NoteClass::Ornament) => more_ornament += 1.0,
                (NoteClass::Ornament, NoteClass::Stable)
                | (NoteClass::Ornament, NoteClass::ProbableError) => fewer_ornament += 1.0,
                (NoteClass::ProbableError, NoteClass::Stable) => forgive_pitch += 1.0,
                (NoteClass::Stable, NoteClass::ProbableError) => strict_pitch += 1.0,
                _ => {}
            }
        }
        let ratio = |a: f64, b: f64| (PRIOR + a) / (PRIOR + b);
        Self {
            ornament_scale: ratio(more_ornament, fewer_ornament).clamp(0.5, 2.0),
            intonation_scale: ratio(forgive_pitch, strict_pitch).clamp(0.6, 1.6),
        }
    }

    pub fn apply(&self, mut p: ClassifierParams) -> ClassifierParams {
        p.ornament_max_seconds *= self.ornament_scale;
        p.in_scale_cents *= self.intonation_scale;
        p
    }
}

/// Expected note of a prepared melody (trainer input): what should sound when.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Expected {
    pub midi: i32,
    pub start: f64,
    pub end: f64,
}

pub struct ClassifyInput<'a> {
    pub notes: &'a [Note],
    pub phrases: &'a [Phrase],
    pub scale: Option<&'a Scale>,
    /// (midi, cents offset) of the instrument profile's measured notes, if any.
    pub profile: &'a [(i32, f64)],
    pub expected: &'a [Expected],
    pub annotations: &'a [Annotation],
    pub params: &'a ClassifierParams,
    pub literal: bool,
}

fn slide_cents(n: &Note) -> f64 {
    if n.bend.len() < 2 {
        return 0.0;
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for &(_, c) in &n.bend {
        lo = lo.min(c);
        hi = hi.max(c);
    }
    hi - lo
}

fn expected_at(expected: &[Expected], n: &Note) -> Option<Expected> {
    let mid = (n.start + n.end) / 2.0;
    expected
        .iter()
        .copied()
        .find(|e| mid >= e.start - 0.05 && mid < e.end + 0.05)
        .or_else(|| {
            expected
                .iter()
                .copied()
                .min_by(|a, b| {
                    let da = (a.start - n.start).abs();
                    let db = (b.start - n.start).abs();
                    da.partial_cmp(&db).unwrap()
                })
                .filter(|e| (e.start - n.start).abs() < 0.3)
        })
}

/// Classify every note. Deterministic; one pass with lookahead into the next phrase.
pub fn classify(input: &ClassifyInput) -> Vec<Classified> {
    let ClassifyInput {
        notes,
        phrases,
        scale,
        profile,
        expected,
        annotations,
        params: p,
        literal,
    } = input;
    let mut out = Vec::with_capacity(notes.len());
    for (pi, ph) in phrases.iter().enumerate() {
        let next_phrase = phrases.get(pi + 1);
        for (k, &i) in ph.notes.iter().enumerate() {
            let n = &notes[i];
            let d = n.duration();
            let midi_frac = n.midi as f64 + n.cents / 100.0;
            let cents_from_scale = scale.map(|s| s.nearest(midi_frac).cents_off);
            let cents_from_profile = profile
                .iter()
                .find(|(m, _)| *m == n.midi)
                .map(|(_, off)| n.cents - off);
            let exp = expected_at(expected, n);
            let cents_from_expected = exp.map(|e| (midi_frac - e.midi as f64) * 100.0);
            let mut reasons = Vec::new();
            let short = d <= p.ornament_max_seconds;
            let in_scale = cents_from_scale.map(|c| c.abs() <= p.in_scale_cents);
            reasons.push(if short { Reason::Short } else { Reason::Held });
            match in_scale {
                Some(true) => reasons.push(Reason::InScale),
                Some(false) => reasons.push(Reason::OutOfScale),
                None => {}
            }
            if let Some(c) = cents_from_profile {
                if c.abs() > 30.0 {
                    reasons.push(Reason::OffProfile);
                }
            }
            let prev = k.checked_sub(1).map(|j| &notes[ph.notes[j]]);
            let next = ph.notes.get(k + 1).map(|&j| &notes[j]);
            let held_neighbour = |m: Option<&Note>, after: bool| {
                m.is_some_and(|m| {
                    let gap = if after {
                        m.start - n.end
                    } else {
                        n.start - m.end
                    };
                    m.duration() >= 2.5 * d && gap.abs() <= p.adjacency_seconds
                })
            };
            let next_to_held = held_neighbour(prev, false) || held_neighbour(next, true);
            if next_to_held {
                reasons.push(Reason::NextToHeldTone);
            }
            // "Comes back": the same pitch class held in the next phrase (offline lookahead)
            // or in the previous one (what a live follower can know at the boundary).
            let prev_phrase = pi.checked_sub(1).and_then(|j| phrases.get(j));
            let held_in = |ph: &Phrase| {
                ph.notes.iter().any(|&j| {
                    let m = &notes[j];
                    (m.midi - n.midi).rem_euclid(12) == 0 && m.duration() > p.ornament_max_seconds
                })
            };
            let repeats = next_phrase.is_some_and(held_in) || prev_phrase.is_some_and(held_in);
            if repeats && in_scale == Some(false) {
                reasons.push(Reason::RepeatsNextPhrase);
            }
            let sl = slide_cents(n);

            let (class, confidence) = if let Some(a) = annotations.iter().find(|a| a.index == i) {
                reasons.push(Reason::UserMarked);
                (a.class, 1.0)
            } else if *literal {
                reasons.push(Reason::LiteralMode);
                (NoteClass::Stable, 1.0)
            } else if n.confidence < p.noise_confidence || d < p.noise_min_seconds {
                reasons.push(Reason::LowConfidence);
                (NoteClass::Noise, 0.8)
            } else if sl >= p.slide_cents && d < 0.4 {
                reasons.push(Reason::Slide);
                (NoteClass::Transition, 0.7)
            } else if let Some(e) = exp.filter(|_| !short) {
                // Trainer input: the expected note decides for held notes.
                let dev = cents_from_expected.unwrap_or(0.0);
                if dev.abs() <= 50.0 {
                    reasons.push(Reason::MatchesExpected);
                    (NoteClass::Stable, 0.6 + 0.4 * (1.0 - dev.abs() / 50.0))
                } else {
                    reasons.push(Reason::DiffersFromExpected);
                    let _ = e;
                    (
                        NoteClass::ProbableError,
                        (0.5 + dev.abs() / 400.0).min(0.95),
                    )
                }
            } else if short && next_to_held {
                (NoteClass::Ornament, 0.75)
            } else if short {
                let passing = match (prev, next) {
                    (Some(a), Some(b)) => {
                        (a.midi < n.midi && n.midi < b.midi) || (a.midi > n.midi && n.midi > b.midi)
                    }
                    _ => false,
                };
                (NoteClass::Transition, if passing { 0.7 } else { 0.55 })
            } else if in_scale != Some(false) {
                let tune = cents_from_scale
                    .map(|c| 1.0 - c.abs() / 100.0)
                    .unwrap_or(0.8);
                (
                    NoteClass::Stable,
                    (0.6 + 0.4 * tune.clamp(0.0, 1.0)).min(1.0),
                )
            } else if repeats {
                // Held, out of scale, comes back: the musician means it (mode change candidate).
                (NoteClass::Stable, 0.55)
            } else {
                reasons.push(Reason::Isolated);
                let off = cents_from_scale.map(|c| c.abs()).unwrap_or(100.0);
                (NoteClass::ProbableError, (0.4 + off / 300.0).min(0.9))
            };
            out.push(Classified {
                index: i,
                phrase: pi,
                class,
                confidence,
                cents_from_scale,
                cents_from_profile,
                expected_midi: exp.map(|e| e.midi),
                cents_from_expected,
                sustained: !short,
                reasons,
            });
        }
    }
    out
}

/// What the musician meant in a phrase, from the classes.
pub fn estimate_intent(
    notes: &[Note],
    phrase: &Phrase,
    phrase_index: usize,
    classes: &[Classified],
) -> IntentEstimate {
    let mut acc: Vec<(i32, f64, f64)> = Vec::new();
    let mut total = 0.0;
    let mut stable_w = 0.0;
    let (mut st, mut orn, mut tr, mut no, mut pe) = (0, 0, 0, 0, 0);
    for &i in &phrase.notes {
        let n = &notes[i];
        let Some(c) = classes.iter().find(|c| c.index == i) else {
            continue;
        };
        let base = n.duration() * n.confidence.max(0.0);
        total += base;
        let w = match c.class {
            NoteClass::Stable => {
                st += 1;
                stable_w += base * c.confidence;
                base * c.confidence
            }
            NoteClass::Ornament => {
                orn += 1;
                base * 0.2
            }
            NoteClass::Transition => {
                tr += 1;
                base * 0.1
            }
            NoteClass::Noise => {
                no += 1;
                0.0
            }
            NoteClass::ProbableError => {
                pe += 1;
                0.0
            }
        };
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
    let sum: f64 = acc.iter().map(|a| a.1).sum();
    let mut tones: Vec<StableTone> = acc
        .into_iter()
        .map(|(midi, w, wc)| StableTone {
            midi,
            cents: wc / w,
            weight: if sum > 0.0 { w / sum } else { 0.0 },
        })
        .collect();
    tones.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());
    IntentEstimate {
        phrase: phrase_index,
        tones,
        confidence: if total > 0.0 { stable_w / total } else { 0.0 },
        stable_notes: st,
        ornaments: orn,
        transitions: tr,
        noise: no,
        probable_errors: pe,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerformanceAnalysis {
    pub mode: Mode,
    pub phrases: Vec<Phrase>,
    pub classes: Vec<Classified>,
    pub intents: Vec<IntentEstimate>,
    /// One per phrase, after it.
    pub contexts: Vec<TonalContext>,
    pub switches: usize,
}

pub struct AnalysisInput<'a> {
    pub notes: &'a [Note],
    pub scale: Option<&'a Scale>,
    pub profile: &'a [(i32, f64)],
    pub expected: &'a [Expected],
    pub annotations: &'a [Annotation],
    pub mode: Mode,
    pub mode_params: ModeParams,
    pub locks: Locks,
    pub phrase_params: PhraseParams,
    pub calibration: Calibration,
    /// Kinds the tracker considers (e.g. `ScaleKind::ALL`).
    pub kinds: &'a [ScaleKind],
}

/// The whole offline chain: phrases → classes → intent → tracked context.
pub fn analyze_performance(input: &AnalysisInput) -> PerformanceAnalysis {
    let phrases = segment_phrases(input.notes, &input.phrase_params);
    let literal = input.mode == Mode::Off;
    let params = input
        .calibration
        .apply(ClassifierParams::default().with_tolerance(input.mode_params.tolerance));
    let classes = classify(&ClassifyInput {
        notes: input.notes,
        phrases: &phrases,
        scale: input.scale,
        profile: input.profile,
        expected: input.expected,
        annotations: input.annotations,
        params: &params,
        literal,
    });
    let mut tracker = TonalTracker::new(input.mode.tracker_params(input.mode_params), input.kinds);
    tracker.set_locks(input.locks);
    if let Some(s) = input.scale {
        if let Some(k) = s.kind {
            tracker.set_context(s.tonic_pc, k);
        }
    }
    let mut intents = Vec::with_capacity(phrases.len());
    let mut contexts = Vec::with_capacity(phrases.len());
    for (pi, ph) in phrases.iter().enumerate() {
        let intent = estimate_intent(input.notes, ph, pi, &classes);
        // The tonal context cares about the intended pitch class, not the intonation:
        // a +30-cent tone must not vote for its upper neighbour.
        let hist = PitchClassHistogram::from_midi(
            intent
                .tones
                .iter()
                .map(|t| (t.midi as f64, t.weight * ph.notes.len().max(1) as f64)),
        );
        contexts.push(tracker.push_phrase(hist));
        intents.push(intent);
    }
    PerformanceAnalysis {
        mode: input.mode,
        phrases,
        classes,
        intents,
        contexts,
        switches: tracker.switches(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(start: f64, dur: f64, midi: i32, cents: f64) -> Note {
        Note {
            start,
            end: start + dur,
            midi,
            cents,
            confidence: 0.9,
            velocity: 0.8,
            bend: Vec::new(),
        }
    }

    #[test]
    fn short_note_by_a_held_tone_is_an_ornament_even_outside_the_scale() {
        let scale = Scale::new(2, ScaleKind::Dorian);
        let notes = vec![
            note(0.0, 0.08, 63, 0.0),
            note(0.08, 1.0, 62, 0.0),
            note(1.1, 0.5, 65, 0.0),
        ];
        let phrases = segment_phrases(&notes, &PhraseParams::default());
        let c = classify(&ClassifyInput {
            notes: &notes,
            phrases: &phrases,
            scale: Some(&scale),
            profile: &[],
            expected: &[],
            annotations: &[],
            params: &ClassifierParams::default(),
            literal: false,
        });
        assert_eq!(c[0].class, NoteClass::Ornament);
        assert!(
            c[0].reasons.contains(&Reason::OutOfScale)
                && c[0].reasons.contains(&Reason::NextToHeldTone)
        );
        assert_eq!(c[1].class, NoteClass::Stable);
        assert!(!c[0].sustained && c[1].sustained);
    }

    #[test]
    fn held_isolated_out_of_scale_note_is_a_probable_error_but_a_repeated_one_is_meant() {
        let scale = Scale::new(2, ScaleKind::Dorian);
        // Phrase 1 holds a B♭ (not in D dorian), phrase 2 brings it back.
        let notes = vec![
            note(0.0, 0.5, 62, 0.0),
            note(0.5, 0.6, 70, 0.0),
            note(1.1, 0.5, 69, 0.0),
            note(2.0, 0.5, 70, 0.0),
            note(2.5, 0.6, 62, 0.0),
        ];
        let phrases = segment_phrases(&notes, &PhraseParams::default());
        assert_eq!(phrases.len(), 2);
        let mk = |ns: &[Note], ph: &[Phrase]| {
            classify(&ClassifyInput {
                notes: ns,
                phrases: ph,
                scale: Some(&scale),
                profile: &[],
                expected: &[],
                annotations: &[],
                params: &ClassifierParams::default(),
                literal: false,
            })
        };
        let c = mk(&notes, &phrases);
        assert_eq!(
            c[1].class,
            NoteClass::Stable,
            "repeated next phrase: {:?}",
            c[1].reasons
        );
        assert!(c[1].reasons.contains(&Reason::RepeatsNextPhrase));
        // The second B♭ comes back from the previous phrase: meant as well (a live
        // follower only knows the past at a phrase boundary, so the rule is symmetric).
        assert_eq!(c[3].class, NoteClass::Stable, "{:?}", c[3].reasons);
        // Alone in its phrase (nothing before or after brings it back): a probable error.
        let lone = vec![
            note(0.0, 0.5, 62, 0.0),
            note(0.5, 0.6, 70, 0.0),
            note(1.1, 0.5, 69, 0.0),
        ];
        let lone_ph = segment_phrases(&lone, &PhraseParams::default());
        assert_eq!(mk(&lone, &lone_ph)[1].class, NoteClass::ProbableError);
        // The user says the second one was meant: a mark overrides, and calibration moves.
        let marked = classify(&ClassifyInput {
            notes: &notes,
            phrases: &phrases,
            scale: Some(&scale),
            profile: &[],
            expected: &[],
            annotations: &[Annotation {
                index: 3,
                class: NoteClass::Stable,
            }],
            params: &ClassifierParams::default(),
            literal: false,
        });
        assert_eq!(marked[3].class, NoteClass::Stable);
        let cal = Calibration::from_marks(&[(NoteClass::ProbableError, NoteClass::Stable); 4]);
        assert!(cal.intonation_scale > 1.5 && cal.ornament_scale == 1.0);
        assert!(Calibration::from_marks(&[]).intonation_scale == 1.0);
    }

    #[test]
    fn expected_melody_decides_for_held_notes_and_reports_the_deviation() {
        let notes = vec![note(0.0, 0.5, 62, 20.0), note(0.5, 0.5, 66, 0.0)];
        let expected = [
            Expected {
                midi: 62,
                start: 0.0,
                end: 0.5,
            },
            Expected {
                midi: 64,
                start: 0.5,
                end: 1.0,
            },
        ];
        let phrases = segment_phrases(&notes, &PhraseParams::default());
        let c = classify(&ClassifyInput {
            notes: &notes,
            phrases: &phrases,
            scale: None,
            profile: &[(62, 15.0)],
            expected: &expected,
            annotations: &[],
            params: &ClassifierParams::default(),
            literal: false,
        });
        assert_eq!(c[0].class, NoteClass::Stable);
        assert!((c[0].cents_from_expected.unwrap() - 20.0).abs() < 1e-9);
        assert!((c[0].cents_from_profile.unwrap() - 5.0).abs() < 1e-9);
        assert_eq!(c[1].class, NoteClass::ProbableError);
        assert_eq!(c[1].expected_midi, Some(64));
        assert!((c[1].cents_from_expected.unwrap() - 200.0).abs() < 1e-9);
    }

    #[test]
    fn literal_mode_takes_every_note_as_meant() {
        let scale = Scale::new(2, ScaleKind::Dorian);
        let notes = vec![note(0.0, 0.05, 63, 0.0), note(0.05, 0.5, 70, 0.0)];
        let phrases = segment_phrases(&notes, &PhraseParams::default());
        let c = classify(&ClassifyInput {
            notes: &notes,
            phrases: &phrases,
            scale: Some(&scale),
            profile: &[],
            expected: &[],
            annotations: &[],
            params: &ClassifierParams::default(),
            literal: true,
        });
        assert!(c
            .iter()
            .all(|x| x.class == NoteClass::Stable && x.reasons.contains(&Reason::LiteralMode)));
        let p = Mode::Off.tracker_params(Mode::Off.default_params());
        assert_eq!(p.switch_phrases, 1);
        assert!(
            Mode::Stable
                .tracker_params(Mode::Stable.default_params())
                .switch_phrases
                > Mode::Creative
                    .tracker_params(Mode::Creative.default_params())
                    .switch_phrases
        );
    }
}
