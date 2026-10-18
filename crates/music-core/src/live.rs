//! Live follower (FEATURE_SPEC F8.1, ERROR_TOLERANT_PERFORMANCE_SPEC): the offline
//! chain — pitch samples → notes → phrases → classes → intent → tonal context with
//! hysteresis → harmony decision — fed incrementally while the musician plays.
//!
//! Rules it enforces (the spec's "mandatory functionality"):
//! * harmony changes only at **phrase boundaries** (a phrase is closed once a breath-long
//!   silence follows it, or the next phrase has started), never on an ornament;
//! * the tonal centre moves only through the tracker's hysteresis (N phrases of evidence);
//! * below the confidence gate the previous harmony is **held** and the reason is
//!   reported; before any harmony exists it falls back to a safe drone on the tonic;
//! * locks (tonic / mode / chord / drone) and an emergency bypass are honoured.
//!
//! Pure: no audio, no I/O, no clocks. The shell feeds it samples from the input monitor.

use serde::{Deserialize, Serialize};

use crate::harmony::{propose, ChordTone, HarmonyParams, Shape};
use crate::intent::{
    classify, estimate_intent, Calibration, Classified, ClassifierParams, ClassifyInput,
    IntentEstimate, Mode, ModeParams, NoteClass, PerformanceAnalysis,
};
use crate::key::{Hypothesis, PitchClassHistogram};
use crate::notes::{extract_notes, Note, NoteParams, PitchSample};
use crate::phrase::{segment_phrases, Phrase, PhraseParams};
use crate::scale::{Scale, ScaleKind};
use crate::theory::ChordKind;
use crate::tracker::{Locks, TonalContext, TonalTracker};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveParams {
    pub mode: Mode,
    pub mode_params: ModeParams,
    pub locks: Locks,
    /// The project's chosen scale, if any: seeds the tracker and hints the harmony.
    pub scale: Option<Scale>,
    /// (midi, cents offset) of the instrument profile's measured notes.
    pub profile: Vec<(i32, f64)>,
    pub phrase_params: PhraseParams,
    pub note_params: NoteParams,
    pub harmony: HarmonyParams,
    pub kinds: Vec<ScaleKind>,
    pub calibration: Calibration,
    /// Closed phrases the harmony engine looks back over when choosing the next chord.
    pub window_phrases: usize,
}

impl LiveParams {
    pub fn new(mode: Mode, harmony: HarmonyParams) -> Self {
        Self {
            mode,
            mode_params: mode.default_params(),
            locks: Locks::default(),
            scale: None,
            profile: Vec::new(),
            phrase_params: PhraseParams::for_wind(),
            note_params: NoteParams::default(),
            harmony,
            kinds: ScaleKind::ALL.to_vec(),
            calibration: Calibration::default(),
            window_phrases: 4,
        }
    }

    /// The confidence floor below which harmony is held: the user's harmony setting,
    /// raised by the mode (Stable holds more readily than Creative).
    pub fn min_confidence(&self) -> f64 {
        let floor = match self.mode {
            Mode::Off => 0.0,
            Mode::Stable => 0.55,
            Mode::Assisted => 0.4,
            Mode::Creative => 0.25,
        };
        self.harmony.min_confidence.max(floor)
    }
}

/// Why the harmony did not move at a phrase boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HoldReason {
    /// Context or intent confidence under the gate (values 0..1).
    LowConfidence { context: f64, intent: f64 },
    /// Chord and drone are both locked by the user.
    Locked,
    /// Emergency bypass: the follower observes but never acts.
    Bypass,
    /// The engine chose the same chord again.
    SameChord,
    /// The phrase had no stable tones to decide from.
    NoTones,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveDecision {
    pub phrase: usize,
    /// Time (seconds on the live clock) the decision was made: the phrase boundary.
    pub at_s: f64,
    pub phrase_start: f64,
    pub phrase_end: f64,
    pub tonic_pc: u8,
    pub kind: ScaleKind,
    pub context_confidence: f64,
    pub intent_confidence: f64,
    /// True when the tonal context switched on this phrase.
    pub switched: bool,
    pub chord: Option<Vec<ChordTone>>,
    pub chord_root_pc: Option<u8>,
    pub chord_kind: Option<ChordKind>,
    pub chord_shape: Option<Shape>,
    pub drone_root: Option<i32>,
    /// Chord or drone differ from before this decision.
    pub changed: bool,
    pub held: Option<HoldReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentNote {
    pub start: f64,
    pub end: f64,
    pub midi: i32,
    pub cents: f64,
    pub class: Option<NoteClass>,
    /// Index of the (open or closed) phrase the note belongs to.
    pub phrase: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveStatus {
    pub seconds: f64,
    pub notes: usize,
    /// Closed phrases (decided); the open one is not counted.
    pub phrases: usize,
    pub open_phrase_notes: usize,
    pub tonic_pc: Option<u8>,
    pub kind: Option<ScaleKind>,
    pub context_confidence: f64,
    pub alternatives: Vec<Hypothesis>,
    pub chord: Option<Vec<ChordTone>>,
    pub chord_root_pc: Option<u8>,
    pub chord_kind: Option<ChordKind>,
    pub chord_shape: Option<Shape>,
    pub drone_root: Option<i32>,
    pub last_change_s: Option<f64>,
    pub last_hold: Option<HoldReason>,
    pub switches: usize,
    pub holds: usize,
    pub changes: usize,
    pub bypass: bool,
    pub locks: Locks,
    pub mode: Mode,
    pub recent: Vec<RecentNote>,
}

pub struct LiveFollower {
    params: LiveParams,
    samples: Vec<PitchSample>,
    notes: Vec<Note>,
    phrases: Vec<Phrase>,
    classes: Vec<Classified>,
    closed: usize,
    intents: Vec<IntentEstimate>,
    contexts: Vec<TonalContext>,
    tracker: TonalTracker,
    decisions: Vec<LiveDecision>,
    chord: Option<Vec<ChordTone>>,
    chord_root_pc: Option<u8>,
    chord_kind: Option<ChordKind>,
    chord_shape: Option<Shape>,
    drone_root: Option<i32>,
    now: f64,
    bypass: bool,
    last_change_s: Option<f64>,
    last_hold: Option<HoldReason>,
    holds: usize,
    changes: usize,
}

const RECENT_NOTES: usize = 12;

impl LiveFollower {
    pub fn new(params: LiveParams) -> Self {
        let mut tracker = TonalTracker::new(
            params.mode.tracker_params(params.mode_params),
            &params.kinds,
        );
        tracker.set_locks(params.locks);
        if let Some(s) = &params.scale {
            if let Some(k) = s.kind {
                tracker.set_context(s.tonic_pc, k);
            }
        }
        Self {
            params,
            samples: Vec::new(),
            notes: Vec::new(),
            phrases: Vec::new(),
            classes: Vec::new(),
            closed: 0,
            intents: Vec::new(),
            contexts: Vec::new(),
            tracker,
            decisions: Vec::new(),
            chord: None,
            chord_root_pc: None,
            chord_kind: None,
            chord_shape: None,
            drone_root: None,
            now: 0.0,
            bypass: false,
            last_change_s: None,
            last_hold: None,
            holds: 0,
            changes: 0,
        }
    }

    pub fn params(&self) -> &LiveParams {
        &self.params
    }

    pub fn set_locks(&mut self, locks: Locks) {
        self.params.locks = locks;
        self.params.harmony.locks = locks;
        self.tracker.set_locks(locks);
    }

    pub fn set_bypass(&mut self, on: bool) {
        self.bypass = on;
    }

    pub fn bypass(&self) -> bool {
        self.bypass
    }

    pub fn decisions(&self) -> &[LiveDecision] {
        &self.decisions
    }

    /// Feed new pitch samples (monotonic time); returns the decisions taken at phrase
    /// boundaries that these samples closed (usually none, sometimes one).
    pub fn push(&mut self, samples: &[PitchSample]) -> Vec<LiveDecision> {
        if samples.is_empty() {
            return Vec::new();
        }
        for s in samples {
            if self.samples.last().is_none_or(|l| s.t >= l.t) {
                self.samples.push(*s);
            }
        }
        self.now = self.samples.last().map(|s| s.t).unwrap_or(0.0);
        self.reanalyse()
    }

    fn reanalyse(&mut self) -> Vec<LiveDecision> {
        self.notes = extract_notes(&self.samples, &self.params.note_params);
        self.phrases = segment_phrases(&self.notes, &self.params.phrase_params);
        if self.phrases.is_empty() {
            return Vec::new();
        }
        let literal = self.params.mode == Mode::Off;
        let cparams = self
            .params
            .calibration
            .apply(ClassifierParams::default().with_tolerance(self.params.mode_params.tolerance));
        self.classes = classify(&ClassifyInput {
            notes: &self.notes,
            phrases: &self.phrases,
            scale: self.params.scale.as_ref(),
            profile: &self.params.profile,
            expected: &[],
            annotations: &[],
            params: &cparams,
            literal,
        });
        // A phrase is closed when another phrase follows it or a breath of silence has
        // passed since its last note; the open phrase is never decided on.
        let gap = self.params.phrase_params.gap_seconds;
        let closed_now = self
            .phrases
            .iter()
            .enumerate()
            .filter(|(i, p)| *i + 1 < self.phrases.len() || self.now - p.end >= gap)
            .count();
        let mut out = Vec::new();
        while self.closed < closed_now {
            let pi = self.closed;
            let ph = self.phrases[pi].clone();
            let intent = estimate_intent(&self.notes, &ph, pi, &self.classes);
            let hist = PitchClassHistogram::from_midi(
                intent
                    .tones
                    .iter()
                    .map(|t| (t.midi as f64, t.weight * ph.notes.len().max(1) as f64)),
            );
            let ctx = self.tracker.push_phrase(hist);
            self.intents.push(intent.clone());
            self.contexts.push(ctx.clone());
            self.closed += 1;
            let d = self.decide(pi, &ph, &ctx, &intent);
            self.decisions.push(d.clone());
            out.push(d);
        }
        out
    }

    fn decide(
        &mut self,
        pi: usize,
        ph: &Phrase,
        ctx: &TonalContext,
        intent: &IntentEstimate,
    ) -> LiveDecision {
        let mut d = LiveDecision {
            phrase: pi,
            at_s: self.now,
            phrase_start: ph.start,
            phrase_end: ph.end,
            tonic_pc: ctx.tonic_pc,
            kind: ctx.kind,
            context_confidence: ctx.confidence,
            intent_confidence: intent.confidence,
            switched: ctx.switched,
            chord: self.chord.clone(),
            chord_root_pc: self.chord_root_pc,
            chord_kind: self.chord_kind,
            chord_shape: self.chord_shape,
            drone_root: self.drone_root,
            changed: false,
            held: None,
        };
        if self.bypass {
            return self.hold(d, HoldReason::Bypass);
        }
        let locks = self.params.locks;
        if locks.chord && locks.drone {
            return self.hold(d, HoldReason::Locked);
        }
        let gate = self.params.min_confidence();
        let confident = ctx.confidence >= gate && intent.confidence >= gate;
        if !confident {
            // Nothing sounding yet: fall back to a safe drone on the tonic rather than silence.
            if self.chord.is_none() && self.drone_root.is_none() && !locks.drone {
                let root = self.tonic_in_register(ctx.tonic_pc);
                self.drone_root = Some(root);
                d.drone_root = Some(root);
                d.changed = true;
                d.held = Some(HoldReason::LowConfidence {
                    context: ctx.confidence,
                    intent: intent.confidence,
                });
                self.changes += 1;
                self.last_change_s = Some(self.now);
                self.last_hold = d.held.clone();
                return d;
            }
            return self.hold(
                d,
                HoldReason::LowConfidence {
                    context: ctx.confidence,
                    intent: intent.confidence,
                },
            );
        }
        if intent.tones.is_empty() {
            return self.hold(d, HoldReason::NoTones);
        }
        // Harmony over the recent window of closed phrases, re-indexed from zero.
        let from = self
            .closed
            .saturating_sub(self.params.window_phrases.max(1));
        let analysis = PerformanceAnalysis {
            mode: self.params.mode,
            phrases: self.phrases[from..self.closed].to_vec(),
            classes: self.classes.clone(),
            intents: self.intents[from..self.closed].to_vec(),
            contexts: self.contexts[from..self.closed].to_vec(),
            switches: self.tracker.switches(),
        };
        let mut hp = self.params.harmony.clone();
        hp.locks = locks;
        let proposal = propose(&analysis, self.params.scale.as_ref(), &hp);
        let last_chord = proposal.chords.last();
        let new_chord: Option<Vec<ChordTone>> = if locks.chord {
            self.chord.clone()
        } else {
            last_chord.map(|c| c.tones.clone())
        };
        let new_drone = if locks.drone {
            self.drone_root
        } else {
            proposal
                .drone
                .first()
                .map(|t| t.midi)
                .or_else(|| Some(self.tonic_in_register(ctx.tonic_pc)))
        };
        let same_chord = match (&new_chord, &self.chord) {
            (Some(a), Some(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.midi == y.midi)
            }
            (None, None) => true,
            _ => false,
        };
        if same_chord && new_drone == self.drone_root {
            return self.hold(d, HoldReason::SameChord);
        }
        self.chord = new_chord.clone();
        if !locks.chord {
            self.chord_root_pc = last_chord.map(|c| c.root_pc);
            self.chord_kind = last_chord.and_then(|c| c.kind);
            self.chord_shape = last_chord.map(|c| c.shape);
        }
        self.drone_root = new_drone;
        self.changes += 1;
        self.last_change_s = Some(self.now);
        self.last_hold = None;
        d.chord = new_chord;
        d.chord_root_pc = self.chord_root_pc;
        d.chord_kind = self.chord_kind;
        d.chord_shape = self.chord_shape;
        d.drone_root = new_drone;
        d.changed = true;
        d
    }

    fn hold(&mut self, mut d: LiveDecision, why: HoldReason) -> LiveDecision {
        self.holds += 1;
        self.last_hold = Some(why.clone());
        d.held = Some(why);
        d
    }

    fn tonic_in_register(&self, tonic_pc: u8) -> i32 {
        let lo = self.params.harmony.register_low;
        let mut m = lo;
        while m.rem_euclid(12) != tonic_pc as i32 {
            m += 1;
        }
        m
    }

    pub fn status(&self) -> LiveStatus {
        let ctx = self.contexts.last();
        let open_notes = self
            .phrases
            .get(self.closed)
            .map(|p| p.notes.len())
            .unwrap_or(0);
        let recent = self
            .notes
            .iter()
            .enumerate()
            .rev()
            .take(RECENT_NOTES)
            .map(|(i, n)| RecentNote {
                start: n.start,
                end: n.end,
                midi: n.midi,
                cents: n.cents,
                class: self.classes.iter().find(|c| c.index == i).map(|c| c.class),
                phrase: self.phrases.iter().position(|p| p.notes.contains(&i)),
            })
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        LiveStatus {
            seconds: self.now,
            notes: self.notes.len(),
            phrases: self.closed,
            open_phrase_notes: open_notes,
            tonic_pc: ctx
                .map(|c| c.tonic_pc)
                .or_else(|| self.tracker.current().map(|c| c.0)),
            kind: ctx
                .map(|c| c.kind)
                .or_else(|| self.tracker.current().map(|c| c.1)),
            context_confidence: ctx.map(|c| c.confidence).unwrap_or(0.0),
            alternatives: ctx.map(|c| c.alternatives.clone()).unwrap_or_default(),
            chord: self.chord.clone(),
            chord_root_pc: self.chord_root_pc,
            chord_kind: self.chord_kind,
            chord_shape: self.chord_shape,
            drone_root: self.drone_root,
            last_change_s: self.last_change_s,
            last_hold: self.last_hold.clone(),
            switches: self.tracker.switches(),
            holds: self.holds,
            changes: self.changes,
            bypass: self.bypass,
            locks: self.params.locks,
            mode: self.params.mode,
            recent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harmony::Strategy;

    const HOP: f64 = 0.02;

    /// Synthetic pitch stream builder: tones (midi, seconds) and silences.
    struct Stream {
        t: f64,
        out: Vec<PitchSample>,
    }

    impl Stream {
        fn new() -> Self {
            Self {
                t: 0.0,
                out: Vec::new(),
            }
        }
        fn tone(&mut self, midi: f64, seconds: f64) -> &mut Self {
            self.tone_conf(midi, seconds, 0.92)
        }
        fn tone_conf(&mut self, midi: f64, seconds: f64, conf: f64) -> &mut Self {
            let n = (seconds / HOP).round() as usize;
            for _ in 0..n {
                self.t += HOP;
                self.out.push(PitchSample {
                    t: self.t,
                    midi: Some(midi),
                    confidence: conf,
                    rms: 0.2,
                });
            }
            self
        }
        fn rest(&mut self, seconds: f64) -> &mut Self {
            let n = (seconds / HOP).round() as usize;
            for _ in 0..n {
                self.t += HOP;
                self.out.push(PitchSample {
                    t: self.t,
                    midi: None,
                    confidence: 0.05,
                    rms: 0.001,
                });
            }
            self
        }
        /// An ornamented D-dorian phrase on a kaval-like register (D4 = 62).
        fn dorian_phrase(&mut self) -> &mut Self {
            self.tone(62.0, 0.6)
                .tone(64.0, 0.05) // grace note
                .tone(65.0, 0.5)
                .tone(67.0, 0.3)
                .tone(69.0, 0.04) // mordent
                .tone(67.0, 0.3)
                .tone(65.0, 0.4)
                .tone(64.0, 0.05)
                .tone(62.0, 0.8)
        }
        /// A D-hijaz (Phrygian dominant) phrase: E♭, F♯, B♭ against the same tonic.
        fn hijaz_phrase(&mut self) -> &mut Self {
            // The characteristic tones repeat inside the phrase (a single held
            // out-of-scale note is a probable error; a repeated one is evidence).
            self.tone(62.0, 0.4)
                .tone(63.0, 0.4)
                .tone(66.0, 0.5)
                .tone(63.0, 0.3)
                .tone(66.0, 0.4)
                .tone(67.0, 0.3)
                .tone(69.0, 0.4)
                .tone(70.0, 0.4)
                .tone(72.0, 0.3)
                .tone(70.0, 0.4)
                .tone(69.0, 0.3)
                .tone(66.0, 0.4)
                .tone(63.0, 0.3)
                .tone(62.0, 0.8)
        }
    }

    fn params(mode: Mode) -> LiveParams {
        let mut p = LiveParams::new(mode, HarmonyParams::for_strategy(Strategy::ModalPads, 3));
        p.scale = Some(Scale::new(2, ScaleKind::Dorian));
        p.harmony.register_low = 48;
        p.harmony.register_high = 72;
        p
    }

    /// Feed in 100 ms chunks, like the shell's poll, collecting decisions with the time
    /// they were taken.
    fn run(f: &mut LiveFollower, samples: &[PitchSample]) -> Vec<LiveDecision> {
        let mut out = Vec::new();
        for chunk in samples.chunks(5) {
            out.extend(f.push(chunk));
        }
        out
    }

    #[test]
    fn ornamented_phrase_changes_harmony_once_and_only_after_it_ends() {
        let mut s = Stream::new();
        s.dorian_phrase().rest(0.6);
        let phrase_end = s.t - 0.6;
        s.dorian_phrase().rest(0.6);
        let mut f = LiveFollower::new(params(Mode::Assisted));
        let decisions = run(&mut f, &s.out);
        assert!(!decisions.is_empty(), "no decision at all");
        // Nothing is decided inside the first phrase; the first decision comes after its end.
        assert!(
            decisions[0].at_s >= phrase_end,
            "decided at {} before the phrase ended at {phrase_end}",
            decisions[0].at_s
        );
        assert_eq!(decisions[0].phrase, 0);
        // Grace notes and the mordent did not create extra phrases/decisions.
        assert_eq!(decisions.len(), 2, "{decisions:#?}");
        let st = f.status();
        assert_eq!(st.phrases, 2);
        assert_eq!(st.tonic_pc, Some(2));
        assert_eq!(st.kind, Some(ScaleKind::Dorian));
        assert!(st.drone_root.is_some(), "a drone root was chosen");
        assert!(st.changes >= 1 && st.changes <= 2, "changes {}", st.changes);
        assert_eq!(st.switches, 0);
        // The chord, when there is one, lives in the requested register.
        if let Some(ch) = &st.chord {
            assert!(ch.iter().all(|t| (48..=72).contains(&t.midi)), "{ch:?}");
        }
    }

    #[test]
    fn low_confidence_phrase_holds_the_harmony_and_says_why() {
        let mut s = Stream::new();
        s.dorian_phrase().rest(0.6);
        // A phrase of short, out-of-scale, wobbling notes at marginal confidence.
        for _ in 0..6 {
            s.tone_conf(63.3, 0.09, 0.65)
                .tone_conf(68.4, 0.08, 0.65)
                .tone_conf(61.6, 0.1, 0.65);
        }
        s.rest(0.6);
        let mut f = LiveFollower::new(params(Mode::Stable));
        let decisions = run(&mut f, &s.out);
        assert_eq!(decisions.len(), 2, "{decisions:#?}");
        let before = (f.decisions()[0].chord.clone(), f.decisions()[0].drone_root);
        let second = &decisions[1];
        assert!(
            matches!(second.held, Some(HoldReason::LowConfidence { .. })),
            "{second:#?}"
        );
        assert!(!second.changed);
        assert_eq!((second.chord.clone(), second.drone_root), before);
        assert_eq!(f.status().holds, 1);
    }

    #[test]
    fn mode_change_switches_the_context_only_after_hysteresis() {
        let mut s = Stream::new();
        for _ in 0..3 {
            s.dorian_phrase().rest(0.6);
        }
        for _ in 0..4 {
            s.hijaz_phrase().rest(0.6);
        }
        let mut f = LiveFollower::new(params(Mode::Assisted));
        let decisions = run(&mut f, &s.out);
        assert_eq!(decisions.len(), 7);
        let switched: Vec<usize> = decisions
            .iter()
            .filter(|d| d.switched)
            .map(|d| d.phrase)
            .collect();
        assert_eq!(switched.len(), 1, "switches {switched:?}");
        // Not on the first hijaz phrase (index 3): hysteresis wants more than one phrase.
        assert!(
            switched[0] >= 4,
            "switched too early at phrase {}",
            switched[0]
        );
        let st = f.status();
        assert_eq!(st.kind, Some(ScaleKind::PhrygianDominant));
        assert_eq!(st.tonic_pc, Some(2));
        assert_eq!(st.switches, 1);
    }

    #[test]
    fn bypass_and_locks_hold_everything() {
        let mut s = Stream::new();
        s.dorian_phrase().rest(0.6);
        let mut f = LiveFollower::new(params(Mode::Assisted));
        run(&mut f, &s.out);
        let before = f.status();
        f.set_bypass(true);
        let mut s2 = Stream::new();
        s2.t = s.t;
        s2.hijaz_phrase().rest(0.6);
        let d = run(&mut f, &s2.out);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].held, Some(HoldReason::Bypass));
        assert_eq!(f.status().chord, before.chord);
        f.set_bypass(false);
        f.set_locks(Locks {
            tonic: false,
            mode: false,
            chord: true,
            drone: true,
        });
        let mut s3 = Stream::new();
        s3.t = s2.t;
        s3.hijaz_phrase().rest(0.6);
        let d = run(&mut f, &s3.out);
        assert_eq!(d[0].held, Some(HoldReason::Locked));
        assert_eq!(f.status().drone_root, before.drone_root);
    }
}
