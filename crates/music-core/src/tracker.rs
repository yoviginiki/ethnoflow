//! Tonal & modal context tracker (F2.10): the (tonic, mode) posterior over the last
//! phrases with exponential forgetting, hysteresis (a new centre needs consecutive
//! phrases of evidence and a margin), and locks (keep measuring, never act).

use serde::{Deserialize, Serialize};

use crate::key::{rank_hypotheses, Hypothesis, PitchClassHistogram};
use crate::scale::ScaleKind;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackerParams {
    /// How many recent phrases take part.
    pub window_phrases: usize,
    /// Weight multiplier per phrase of age (1 = no forgetting).
    pub forgetting: f64,
    /// Consecutive phrases a rival must lead before it becomes the context.
    pub switch_phrases: usize,
    /// The rival's probability must exceed the current context's by this much.
    pub switch_margin: f64,
    /// Alternatives reported.
    pub top_k: usize,
}

impl Default for TrackerParams {
    fn default() -> Self {
        Self {
            window_phrases: 6,
            forgetting: 0.7,
            switch_phrases: 2,
            switch_margin: 0.15,
            top_k: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Locks {
    pub tonic: bool,
    pub mode: bool,
    /// Passed through to the harmony layer (F3.4); the tracker does not use them.
    pub chord: bool,
    pub drone: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChangeEvidence {
    pub tonic_pc: u8,
    pub kind: ScaleKind,
    pub probability: f64,
    /// Phrases in a row this rival has led.
    pub consecutive_phrases: usize,
    /// True when it would have switched but a lock held it.
    pub held_by_lock: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TonalContext {
    pub phrase_index: usize,
    pub tonic_pc: u8,
    pub kind: ScaleKind,
    /// Probability of the current context in the windowed ranking.
    pub confidence: f64,
    pub alternatives: Vec<Hypothesis>,
    pub locked: Locks,
    pub evidence_for_change: Option<ChangeEvidence>,
    /// True on the phrase where the context switched.
    pub switched: bool,
}

pub struct TonalTracker {
    params: TrackerParams,
    kinds: Vec<ScaleKind>,
    locks: Locks,
    history: Vec<PitchClassHistogram>,
    current: Option<(u8, ScaleKind)>,
    candidate: Option<((u8, ScaleKind), usize)>,
    switches: usize,
}

impl TonalTracker {
    pub fn new(params: TrackerParams, kinds: &[ScaleKind]) -> Self {
        Self {
            params,
            kinds: kinds.to_vec(),
            locks: Locks::default(),
            history: Vec::new(),
            current: None,
            candidate: None,
            switches: 0,
        }
    }

    pub fn set_locks(&mut self, locks: Locks) {
        self.locks = locks;
    }

    pub fn locks(&self) -> Locks {
        self.locks
    }

    /// Set the context by hand (the user's choice); counts as locked evidence.
    pub fn set_context(&mut self, tonic_pc: u8, kind: ScaleKind) {
        self.current = Some((tonic_pc % 12, kind));
        self.candidate = None;
    }

    pub fn current(&self) -> Option<(u8, ScaleKind)> {
        self.current
    }

    pub fn switches(&self) -> usize {
        self.switches
    }

    fn windowed(&self) -> PitchClassHistogram {
        let mut h = PitchClassHistogram::default();
        let n = self.history.len();
        let from = n.saturating_sub(self.params.window_phrases);
        for (k, ph) in self.history[from..].iter().enumerate() {
            let age = (n - 1 - (from + k)) as i32;
            let w = self.params.forgetting.powi(age);
            for pc in 0..12 {
                h.weights[pc] += ph.weights[pc] * w;
            }
        }
        h
    }

    /// Feed one phrase (its intended tones as a histogram) and get the context after it.
    pub fn push_phrase(&mut self, phrase: PitchClassHistogram) -> TonalContext {
        self.history.push(phrase);
        let idx = self.history.len() - 1;
        let ranked = rank_hypotheses(&self.windowed(), &self.kinds);
        let prob_of = |t: u8, k: ScaleKind| {
            ranked
                .iter()
                .find(|h| h.tonic_pc == t && h.kind == k)
                .map(|h| h.probability)
                .unwrap_or(0.0)
        };
        let mut switched = false;
        let mut evidence = None;
        match (self.current, ranked.first()) {
            (None, Some(top)) => {
                self.current = Some((top.tonic_pc, top.kind));
            }
            (Some((ct, ck)), Some(top)) if (top.tonic_pc, top.kind) != (ct, ck) => {
                let lead = top.probability - prob_of(ct, ck);
                let rival = (top.tonic_pc, top.kind);
                if lead >= self.params.switch_margin {
                    let count = match self.candidate {
                        Some((c, n)) if c == rival => n + 1,
                        _ => 1,
                    };
                    self.candidate = Some((rival, count));
                    let tonic_change = rival.0 != ct;
                    let mode_change = rival.1 != ck;
                    let locked =
                        (tonic_change && self.locks.tonic) || (mode_change && self.locks.mode);
                    let ready = count >= self.params.switch_phrases;
                    if ready && !locked {
                        self.current = Some(rival);
                        self.candidate = None;
                        self.switches += 1;
                        switched = true;
                    } else {
                        evidence = Some(ChangeEvidence {
                            tonic_pc: rival.0,
                            kind: rival.1,
                            probability: top.probability,
                            consecutive_phrases: count,
                            held_by_lock: ready && locked,
                        });
                    }
                } else {
                    self.candidate = None;
                }
            }
            _ => {
                self.candidate = None;
            }
        }
        let (tonic_pc, kind) = self
            .current
            .expect("a context exists after the first phrase");
        TonalContext {
            phrase_index: idx,
            tonic_pc,
            kind,
            confidence: prob_of(tonic_pc, kind),
            alternatives: ranked.into_iter().take(self.params.top_k).collect(),
            locked: self.locks,
            evidence_for_change: evidence,
            switched,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scale::Scale;

    /// A phrase histogram for a mode: tonic heavy, every degree present.
    fn phrase(tonic: u8, kind: ScaleKind) -> PitchClassHistogram {
        let s = Scale::new(tonic, kind);
        let mut h = PitchClassHistogram::default();
        for (i, pc) in s.pitch_classes().iter().enumerate() {
            h.weights[*pc as usize] += if i == 0 { 3.0 } else { 1.0 };
        }
        h.weights[((tonic + 7) % 12) as usize] += 1.0;
        h
    }

    #[test]
    fn a_deliberate_change_is_accepted_after_two_phrases_and_a_single_phrase_is_not() {
        let mut tr = TonalTracker::new(TrackerParams::default(), &ScaleKind::ALL);
        for _ in 0..4 {
            let c = tr.push_phrase(phrase(2, ScaleKind::Dorian));
            assert_eq!((c.tonic_pc, c.kind), (2, ScaleKind::Dorian));
        }
        // One odd phrase: evidence is reported, nothing moves.
        let c = tr.push_phrase(phrase(9, ScaleKind::PhrygianDominant));
        assert_eq!((c.tonic_pc, c.kind), (2, ScaleKind::Dorian));
        assert!(!c.switched);
        let back = tr.push_phrase(phrase(2, ScaleKind::Dorian));
        assert_eq!((back.tonic_pc, back.kind), (2, ScaleKind::Dorian));
        assert_eq!(tr.switches(), 0);
        // A real change: after ≤ 3 consecutive phrases the context follows.
        let mut when = None;
        for i in 0..4 {
            let c = tr.push_phrase(phrase(9, ScaleKind::PhrygianDominant));
            if c.switched {
                when = Some(i + 1);
                break;
            }
        }
        assert!(
            when.is_some() && when.unwrap() <= 3,
            "switched after {when:?} phrases"
        );
        assert_eq!(tr.current(), Some((9, ScaleKind::PhrygianDominant)));
        assert_eq!(tr.switches(), 1);
    }

    #[test]
    fn locks_hold_the_context_but_report_the_evidence() {
        let mut tr = TonalTracker::new(TrackerParams::default(), &ScaleKind::ALL);
        tr.push_phrase(phrase(7, ScaleKind::Mixolydian));
        tr.set_locks(Locks {
            tonic: true,
            ..Locks::default()
        });
        let mut last = None;
        for _ in 0..5 {
            last = Some(tr.push_phrase(phrase(0, ScaleKind::Ionian)));
        }
        let c = last.unwrap();
        assert_eq!((c.tonic_pc, c.kind), (7, ScaleKind::Mixolydian));
        let e = c.evidence_for_change.expect("evidence while locked");
        assert_eq!((e.tonic_pc, e.kind), (0, ScaleKind::Ionian));
        assert!(e.held_by_lock && e.consecutive_phrases >= 2 && e.probability > 0.3);
        // Unlock: the pending evidence is enough on the next phrase.
        tr.set_locks(Locks::default());
        let c = tr.push_phrase(phrase(0, ScaleKind::Ionian));
        assert!(c.switched);
    }

    #[test]
    fn manual_context_and_forgetting_window() {
        let mut tr = TonalTracker::new(
            TrackerParams {
                window_phrases: 2,
                forgetting: 0.5,
                ..TrackerParams::default()
            },
            &ScaleKind::ALL,
        );
        tr.set_context(4, ScaleKind::Phrygian);
        let c = tr.push_phrase(phrase(4, ScaleKind::Phrygian));
        assert_eq!((c.tonic_pc, c.kind), (4, ScaleKind::Phrygian));
        assert!(c.confidence > 0.3);
        assert_eq!(c.alternatives.len(), 5);
        assert_eq!(c.alternatives[0].tonic_pc, 4);
    }
}
