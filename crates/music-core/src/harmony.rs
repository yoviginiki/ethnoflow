//! Harmonic strategies (F3.4): from a performance analysis (phrases, intent, tracked
//! tonal context) to three different proposals — drone only, modal pads (sparse, one
//! chord per phrase), contemporary liquid chords (7ths, 9ths, sus, quartal) — each
//! scored on melodic compatibility, modal fidelity, voice leading, tension target,
//! rhythmic fit, register separation and variety, with an explanation per chord. The
//! harmony consumes `IntentEstimate` and `TonalContext`, never raw notes; when the
//! context or the intent is unsure it keeps the chord and says why. Seeded and
//! deterministic.

use serde::{Deserialize, Serialize};

use crate::intent::PerformanceAnalysis;
use crate::phrase::StableTone;
use crate::scale::{Scale, ScaleKind};
use crate::theory::{detect_chords, ChordKind};
use crate::tracker::Locks;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    DroneOnly,
    ModalPads,
    LiquidChords,
}

impl Strategy {
    pub const ALL: [Strategy; 3] = [
        Strategy::DroneOnly,
        Strategy::ModalPads,
        Strategy::LiquidChords,
    ];
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarmonyParams {
    pub strategy: Strategy,
    /// 0..1 target dissonance (liquid chords lean on it).
    pub tension: f64,
    /// 0..1 how often chords may change (0 = every ~4 phrases, 1 = every phrase).
    pub movement: f64,
    /// 0..1 voicing richness (number of voices, extensions).
    pub richness: f64,
    /// Chord register in MIDI (inclusive).
    pub register_low: i32,
    pub register_high: i32,
    pub seed: u32,
    pub locks: Locks,
    /// Below this context or intent confidence the harmony keeps its chord.
    pub min_confidence: f64,
}

impl HarmonyParams {
    pub fn for_strategy(strategy: Strategy, seed: u32) -> Self {
        Self {
            strategy,
            tension: match strategy {
                Strategy::LiquidChords => 0.45,
                _ => 0.15,
            },
            movement: match strategy {
                Strategy::ModalPads => 0.4,
                Strategy::LiquidChords => 0.6,
                Strategy::DroneOnly => 0.0,
            },
            richness: match strategy {
                Strategy::LiquidChords => 0.7,
                _ => 0.35,
            },
            register_low: 48,
            register_high: 72,
            seed,
            locks: Locks::default(),
            min_confidence: 0.3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Open5,
    Triad,
    Seventh,
    Add9,
    Sus2,
    Sus4,
    Quartal,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChordTone {
    pub midi: i32,
    /// Cents from the tempered note, from the scale's own degree.
    pub cents: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HarmonyReason {
    /// The chord sits on the context's tonic.
    Tonic,
    /// Which stable melody tones (MIDI) the chord contains and their weight share.
    CoversMelody {
        tones: Vec<i32>,
        share: f64,
    },
    SmoothVoiceLeading {
        semitones: f64,
    },
    KeptLowConfidence {
        confidence: f64,
    },
    KeptChordLock,
    KeptHold {
        phrases_left: usize,
    },
    ChangedForClash {
        share: f64,
    },
    TensionTarget {
        tension: f64,
    },
    AvoidsClash {
        pc: u8,
    },
    FirstChord,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordEvent {
    pub phrase: usize,
    pub start: f64,
    pub end: f64,
    pub root_pc: u8,
    pub kind: Option<ChordKind>,
    pub shape: Shape,
    pub tones: Vec<ChordTone>,
    pub tension: f64,
    pub melody_share: f64,
    pub why: Vec<HarmonyReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BassEvent {
    pub phrase: usize,
    pub start: f64,
    pub end: f64,
    pub midi: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ScoreBreakdown {
    pub melodic_compatibility: f64,
    pub modal_fidelity: f64,
    pub voice_leading: f64,
    pub tension_fit: f64,
    pub rhythmic_fit: f64,
    pub register_separation: f64,
    pub variety: f64,
    pub total: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarmonyProposal {
    pub strategy: Strategy,
    pub seed: u32,
    pub tonic_pc: u8,
    pub kind: ScaleKind,
    pub drone: Vec<ChordTone>,
    pub chords: Vec<ChordEvent>,
    pub bass: Vec<BassEvent>,
    pub score: ScoreBreakdown,
    /// Mean context × intent confidence over the phrases.
    pub confidence: f64,
}

struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        let mut s = self.0.max(1);
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.0 = s;
        s
    }
}

/// A degree of a scale as a semitone offset with its cents.
fn degree(scale: &Scale, i: i32) -> (i32, f64) {
    let n = scale.degrees_cents.len() as i32;
    let oct = i.div_euclid(n);
    let c = scale.degrees_cents[i.rem_euclid(n) as usize] + 1200.0 * oct as f64;
    let semis = (c / 100.0).round() as i32;
    (semis, c - semis as f64 * 100.0)
}

struct Candidate {
    root_degree: i32,
    shape: Shape,
    /// (semitone offset from the tonic, cents) per tone, ascending.
    tones: Vec<(i32, f64)>,
}

/// A voiced candidate with its score for one phrase.
struct Scored {
    total: f64,
    breakdown: ScoreBreakdown,
    tones: Vec<ChordTone>,
    shape: Shape,
    root_degree: i32,
    share: f64,
    covered: Vec<i32>,
    avoid: Option<u8>,
}

/// Chord vocabulary of a scale for a strategy: stacked degrees (triads, sevenths),
/// open fifths, sus and quartal shapes — always from the scale's own degrees.
fn vocabulary(scale: &Scale, strategy: Strategy, richness: f64) -> Vec<Candidate> {
    let n = scale.degrees_cents.len() as i32;
    let mut out = Vec::new();
    if n < 5 {
        return out;
    }
    let has_fifth = |root: i32| {
        let (r, _) = degree(scale, root);
        (0..n).any(|k| {
            let (s, _) = degree(scale, root + k);
            (s - r).rem_euclid(12) == 7
        })
    };
    for root in 0..n {
        let stack = |idx: &[i32]| -> Vec<(i32, f64)> {
            idx.iter().map(|&k| degree(scale, root + k)).collect()
        };
        // Open fifth (root + fifth + octave) when the mode has a perfect fifth there.
        if has_fifth(root) {
            let (r, rc) = degree(scale, root);
            let fifth = (0..n)
                .map(|k| degree(scale, root + k))
                .find(|(s, _)| (s - r).rem_euclid(12) == 7)
                .unwrap();
            out.push(Candidate {
                root_degree: root,
                shape: Shape::Open5,
                tones: vec![(r, rc), (fifth.0, fifth.1), (r + 12, rc)],
            });
        }
        if n >= 7 {
            out.push(Candidate {
                root_degree: root,
                shape: Shape::Triad,
                tones: stack(&[0, 2, 4]),
            });
            if strategy == Strategy::LiquidChords || richness > 0.6 {
                out.push(Candidate {
                    root_degree: root,
                    shape: Shape::Seventh,
                    tones: stack(&[0, 2, 4, 6]),
                });
                out.push(Candidate {
                    root_degree: root,
                    shape: Shape::Add9,
                    tones: stack(&[0, 2, 4, 8]),
                });
            }
            if strategy == Strategy::LiquidChords {
                out.push(Candidate {
                    root_degree: root,
                    shape: Shape::Sus2,
                    tones: stack(&[0, 1, 4]),
                });
                out.push(Candidate {
                    root_degree: root,
                    shape: Shape::Sus4,
                    tones: stack(&[0, 3, 4]),
                });
                out.push(Candidate {
                    root_degree: root,
                    shape: Shape::Quartal,
                    tones: stack(&[0, 3, 6]),
                });
            }
        } else if n >= 5 {
            // Pentatonic: stacked "thirds" are the scale's own skips.
            out.push(Candidate {
                root_degree: root,
                shape: Shape::Triad,
                tones: stack(&[0, 2, 4]),
            });
        }
    }
    out
}

/// Place a candidate's tones in the register as an open voicing (root lowest).
fn voice(
    tonic_pc: u8,
    c: &Candidate,
    low: i32,
    high: i32,
    previous: Option<&[ChordTone]>,
) -> Vec<ChordTone> {
    let base = tonic_pc as i32;
    let mut tones: Vec<ChordTone> = c
        .tones
        .iter()
        .map(|&(s, cents)| ChordTone {
            midi: base + s,
            cents,
        })
        .collect();
    // Root into the lower third of the register.
    let root_target = low + (high - low) / 4;
    let shift = ((root_target - tones[0].midi) as f64 / 12.0).round() as i32 * 12;
    for t in tones.iter_mut() {
        t.midi += shift;
    }
    // Upper tones: choose the octave closest to the previous voicing (voice leading),
    // kept inside the register and above the root.
    for k in 1..tones.len() {
        let mut best = tones[k].midi;
        let mut best_cost = f64::INFINITY;
        for oct in -2..=2 {
            let m = tones[k].midi + 12 * oct;
            if m <= tones[0].midi || m < low || m > high {
                continue;
            }
            let cost = match previous {
                Some(p) if !p.is_empty() => p
                    .iter()
                    .map(|pt| (pt.midi - m).abs() as f64)
                    .fold(f64::INFINITY, f64::min),
                _ => (m - (low + high) / 2).abs() as f64,
            };
            if cost < best_cost {
                best_cost = cost;
                best = m;
            }
        }
        tones[k].midi = best;
    }
    tones.sort_by_key(|t| t.midi);
    tones.dedup_by_key(|t| t.midi);
    tones
}

fn tension_of(tones: &[ChordTone]) -> f64 {
    let mut pairs = 0.0f64;
    let mut tense = 0.0f64;
    for i in 0..tones.len() {
        for j in i + 1..tones.len() {
            pairs += 1.0;
            let iv = (tones[j].midi - tones[i].midi).rem_euclid(12);
            tense += match iv {
                1 | 11 => 1.0,
                2 | 10 => 0.5,
                6 => 0.8,
                _ => 0.0,
            };
        }
    }
    if pairs > 0.0 {
        (tense / pairs).min(1.0)
    } else {
        0.0
    }
}

fn melody_share(tones: &[ChordTone], intent: &[StableTone]) -> (f64, Vec<i32>) {
    let mut share = 0.0;
    let mut covered = Vec::new();
    for t in intent {
        let hit = tones
            .iter()
            .any(|c| (c.midi - t.midi).rem_euclid(12) == 0 && (c.cents - t.cents).abs() <= 35.0);
        if hit {
            share += t.weight;
            covered.push(t.midi);
        }
    }
    (share, covered)
}

/// Soft avoid-note penalty: a chord tone a semitone above a weighty melody tone.
fn clash(tones: &[ChordTone], intent: &[StableTone]) -> Option<u8> {
    for t in intent.iter().filter(|t| t.weight >= 0.2) {
        if let Some(c) = tones.iter().find(|c| (c.midi - t.midi).rem_euclid(12) == 1) {
            return Some(c.midi.rem_euclid(12) as u8);
        }
    }
    None
}

fn voice_leading_cost(prev: &[ChordTone], next: &[ChordTone]) -> f64 {
    if prev.is_empty() || next.is_empty() {
        return 0.0;
    }
    let total: f64 = next
        .iter()
        .map(|n| {
            prev.iter()
                .map(|p| (p.midi - n.midi).abs() as f64)
                .fold(f64::INFINITY, f64::min)
        })
        .sum();
    total / next.len() as f64
}

fn register_separation(tones: &[ChordTone], intent: &[StableTone]) -> f64 {
    let Some(top) = tones.iter().map(|t| t.midi).max() else {
        return 1.0;
    };
    let Some(melody_low) = intent.iter().map(|t| t.midi).min() else {
        return 1.0;
    };
    if top + 2 <= melody_low {
        1.0
    } else {
        (1.0 - (top + 2 - melody_low) as f64 / 12.0).max(0.0)
    }
}

/// Build one proposal. The analysis must have at least one phrase.
pub fn propose(
    analysis: &PerformanceAnalysis,
    scale_hint: Option<&Scale>,
    params: &HarmonyParams,
) -> HarmonyProposal {
    let mut rng = Rng(params.seed);
    let first_ctx = analysis.contexts.first();
    let (tonic_pc, kind) = match (
        scale_hint.and_then(|s| s.kind.map(|k| (s.tonic_pc, k))),
        first_ctx,
    ) {
        (Some(hint), _) => hint,
        (None, Some(c)) => (c.tonic_pc, c.kind),
        (None, None) => (0, ScaleKind::Aeolian), // no phrases: an empty proposal below
    };
    let has_phrases = !analysis.phrases.is_empty();
    let empty_scale;
    let base_scale: &Scale = match scale_hint {
        Some(s) if s.kind.is_none() || true => s,
        _ => {
            empty_scale = Scale::new(tonic_pc, kind);
            &empty_scale
        }
    };
    // Drone: tonic (+ fifth when the mode has one).
    let drone_root =
        params.register_low - 12 + ((tonic_pc as i32 - (params.register_low - 12)).rem_euclid(12));
    let mut drone = vec![ChordTone {
        midi: drone_root,
        cents: 0.0,
    }];
    if let Some((s, c)) = (0..base_scale.degrees_cents.len() as i32)
        .map(|k| degree(base_scale, k))
        .find(|(s, _)| s.rem_euclid(12) == 7)
    {
        drone.push(ChordTone {
            midi: drone_root + s,
            cents: c,
        });
    }
    let mut chords: Vec<ChordEvent> = Vec::new();
    let mut bass = Vec::new();
    let mut sum = ScoreBreakdown::default();
    let mut conf_sum = 0.0;
    let hold_phrases = 1 + (3.0 * (1.0 - params.movement.clamp(0.0, 1.0))).round() as usize;
    let mut last_change: Option<usize> = None;
    let target_tension = match params.strategy {
        Strategy::LiquidChords => 0.2 + 0.5 * params.tension,
        _ => 0.1 + 0.2 * params.tension,
    };

    for (pi, ph) in analysis.phrases.iter().enumerate() {
        let ctx = &analysis.contexts[pi];
        let intent = &analysis.intents[pi];
        let conf = ctx.confidence.max(0.0) * (0.5 + 0.5 * intent.confidence);
        conf_sum += conf;
        bass.push(BassEvent {
            phrase: pi,
            start: ph.start,
            end: ph.end,
            midi: drone_root,
        });
        if params.strategy == Strategy::DroneOnly {
            continue;
        }
        // The scale of this phrase: the user's hint when locked/given, else the context.
        let phrase_scale: Scale = match scale_hint {
            Some(s) if params.locks.mode || params.locks.tonic || s.kind.is_none() => s.clone(),
            _ => Scale::new(ctx.tonic_pc, ctx.kind),
        };
        let vocab = vocabulary(&phrase_scale, params.strategy, params.richness);
        let prev = chords.last();
        let prev_tones: Option<&[ChordTone]> = prev.map(|c| c.tones.as_slice());
        let tonic_degree = 0;

        // Keep the chord when the evidence is weak, when locked, or while holding.
        let mut keep_reason: Option<HarmonyReason> = None;
        if let Some(p) = prev {
            if params.locks.chord {
                keep_reason = Some(HarmonyReason::KeptChordLock);
            } else if ctx.confidence < params.min_confidence
                || intent.confidence < params.min_confidence
            {
                keep_reason = Some(HarmonyReason::KeptLowConfidence {
                    confidence: ctx.confidence.min(intent.confidence),
                });
            } else if let Some(lc) = last_change {
                let (share, _) = melody_share(&p.tones, &intent.tones);
                if pi - lc < hold_phrases && share >= 0.3 {
                    keep_reason = Some(HarmonyReason::KeptHold {
                        phrases_left: hold_phrases - (pi - lc),
                    });
                }
            }
        }
        if let (Some(reason), Some(p)) = (keep_reason, prev) {
            let (share, covered) = melody_share(&p.tones, &intent.tones);
            let mut why = vec![reason];
            if !covered.is_empty() {
                why.push(HarmonyReason::CoversMelody {
                    tones: covered,
                    share,
                });
            }
            let ev = ChordEvent {
                phrase: pi,
                start: ph.start,
                end: ph.end,
                root_pc: p.root_pc,
                kind: p.kind,
                shape: p.shape,
                tones: p.tones.clone(),
                tension: p.tension,
                melody_share: share,
                why,
            };
            sum.melodic_compatibility += share;
            sum.modal_fidelity += 1.0;
            sum.voice_leading += 1.0;
            sum.tension_fit += 1.0 - (p.tension - target_tension).abs();
            sum.rhythmic_fit += 1.0;
            sum.register_separation += register_separation(&p.tones, &intent.tones);
            sum.variety += 0.5;
            chords.push(ev);
            continue;
        }

        // Score every candidate for this phrase.
        let mut scored: Vec<Scored> = Vec::new();
        for c in &vocab {
            let tones = voice(
                phrase_scale.tonic_pc,
                c,
                params.register_low,
                params.register_high,
                prev_tones,
            );
            let (share, covered) = melody_share(&tones, &intent.tones);
            let avoid = clash(&tones, &intent.tones);
            let modal = 1.0; // vocabulary is built from the scale's degrees
            let vl_cost = prev_tones
                .map(|p| voice_leading_cost(p, &tones))
                .unwrap_or(0.0);
            let vl = (1.0 - vl_cost / 6.0).clamp(0.0, 1.0);
            let tens = tension_of(&tones);
            let tfit = 1.0 - (tens - target_tension).abs();
            let reg = register_separation(&tones, &intent.tones);
            let same_as_prev = prev.is_some_and(|p| {
                p.root_pc == tones[0].midi.rem_euclid(12) as u8 && p.shape == c.shape
            });
            let repeats = chords
                .iter()
                .rev()
                .take(3)
                .filter(|p| p.root_pc == tones[0].midi.rem_euclid(12) as u8)
                .count();
            let variety = if same_as_prev {
                0.3
            } else {
                1.0 - 0.2 * repeats as f64
            };
            let tonic_bonus = if c.root_degree == tonic_degree
                && (pi == 0
                    || intent
                        .tones
                        .first()
                        .is_some_and(|t| t.midi.rem_euclid(12) as u8 == phrase_scale.tonic_pc))
            {
                0.08
            } else {
                0.0
            };
            let clash_pen = if avoid.is_some() { 0.15 } else { 0.0 };
            let b = ScoreBreakdown {
                melodic_compatibility: share,
                modal_fidelity: modal,
                voice_leading: vl,
                tension_fit: tfit,
                rhythmic_fit: 1.0,
                register_separation: reg,
                variety,
                total: 0.0,
            };
            let total = 0.35 * share
                + 0.2 * modal
                + 0.15 * vl
                + 0.1 * tfit
                + 0.05
                + 0.1 * reg
                + 0.05 * variety
                + tonic_bonus
                - clash_pen;
            scored.push(Scored {
                total,
                breakdown: b,
                tones,
                shape: c.shape,
                root_degree: c.root_degree,
                share,
                covered,
                avoid,
            });
        }
        if scored.is_empty() {
            continue;
        }
        scored.sort_by(|a, b| b.total.partial_cmp(&a.total).unwrap());
        // Seeded choice among near-equal candidates (variety without randomness in the result).
        let best = scored[0].total;
        let near: Vec<usize> = scored
            .iter()
            .enumerate()
            .filter(|(_, s)| s.total >= best - 0.03)
            .map(|(i, _)| i)
            .collect();
        let pick = if near.len() > 1 && params.strategy == Strategy::LiquidChords {
            near[(rng.next() as usize) % near.len()]
        } else {
            0
        };
        let Scored {
            total,
            breakdown: b,
            tones,
            shape: cand_shape,
            root_degree: cand_root,
            share,
            covered,
            avoid,
        } = &scored[pick];
        let root_pc = tones[0].midi.rem_euclid(12) as u8;
        let pcs: Vec<i32> = tones.iter().map(|t| t.midi).collect();
        let kind = detect_chords(&pcs)
            .into_iter()
            .find(|m| m.root_pc == root_pc && m.score >= 0.99)
            .map(|m| m.kind);
        let mut why = Vec::new();
        if prev.is_none() {
            why.push(HarmonyReason::FirstChord);
        } else if let Some(p) = prev {
            let (ps, _) = melody_share(&p.tones, &intent.tones);
            if ps < 0.3 {
                why.push(HarmonyReason::ChangedForClash { share: ps });
            }
        }
        if *cand_root == tonic_degree {
            why.push(HarmonyReason::Tonic);
        }
        if !covered.is_empty() {
            why.push(HarmonyReason::CoversMelody {
                tones: covered.clone(),
                share: *share,
            });
        }
        if let Some(p) = prev_tones {
            let vl = voice_leading_cost(p, tones);
            if vl <= 2.0 {
                why.push(HarmonyReason::SmoothVoiceLeading { semitones: vl });
            }
        }
        if params.strategy == Strategy::LiquidChords {
            why.push(HarmonyReason::TensionTarget {
                tension: tension_of(tones),
            });
        }
        if avoid.is_none() {
            if let Some(pc) = scored.iter().find_map(|s| s.avoid) {
                if scored
                    .iter()
                    .any(|s| s.avoid.is_some() && s.total > *total - 0.2)
                {
                    why.push(HarmonyReason::AvoidsClash { pc });
                }
            }
        }
        sum.melodic_compatibility += b.melodic_compatibility;
        sum.modal_fidelity += b.modal_fidelity;
        sum.voice_leading += b.voice_leading;
        sum.tension_fit += b.tension_fit;
        sum.rhythmic_fit += b.rhythmic_fit;
        sum.register_separation += b.register_separation;
        sum.variety += b.variety;
        chords.push(ChordEvent {
            phrase: pi,
            start: ph.start,
            end: ph.end,
            root_pc,
            kind,
            shape: *cand_shape,
            tones: tones.clone(),
            tension: tension_of(tones),
            melody_share: *share,
            why,
        });
        last_change = Some(pi);
    }

    let n = analysis.phrases.len().max(1) as f64;
    let mut score = ScoreBreakdown {
        melodic_compatibility: sum.melodic_compatibility / n,
        modal_fidelity: sum.modal_fidelity / n,
        voice_leading: sum.voice_leading / n,
        tension_fit: sum.tension_fit / n,
        rhythmic_fit: sum.rhythmic_fit / n,
        register_separation: sum.register_separation / n,
        variety: sum.variety / n,
        total: 0.0,
    };
    if params.strategy == Strategy::DroneOnly {
        // A drone has no chords to score: its fit is the share of the intent on tonic/fifth.
        let share: f64 = analysis
            .intents
            .iter()
            .map(|i| melody_share(&drone, &i.tones).0)
            .sum::<f64>()
            / n;
        score = ScoreBreakdown {
            melodic_compatibility: share,
            modal_fidelity: 1.0,
            voice_leading: 1.0,
            tension_fit: 1.0,
            rhythmic_fit: 1.0,
            register_separation: 1.0,
            variety: 0.0,
            total: 0.0,
        };
    }
    score.total = 0.35 * score.melodic_compatibility
        + 0.2 * score.modal_fidelity
        + 0.15 * score.voice_leading
        + 0.1 * score.tension_fit
        + 0.05 * score.rhythmic_fit
        + 0.1 * score.register_separation
        + 0.05 * score.variety;
    HarmonyProposal {
        strategy: params.strategy,
        seed: params.seed,
        tonic_pc,
        kind,
        drone,
        chords,
        bass,
        score,
        confidence: if has_phrases { conf_sum / n } else { 0.0 },
    }
}

/// The three strategies side by side, same seed and sliders.
pub fn propose_all(
    analysis: &PerformanceAnalysis,
    scale_hint: Option<&Scale>,
    base: &HarmonyParams,
) -> Vec<HarmonyProposal> {
    Strategy::ALL
        .iter()
        .map(|&s| {
            let mut p = HarmonyParams::for_strategy(s, base.seed);
            p.locks = base.locks;
            p.register_low = base.register_low;
            p.register_high = base.register_high;
            p.min_confidence = base.min_confidence;
            if s != Strategy::DroneOnly {
                p.tension = base.tension;
                p.movement = base.movement;
                p.richness = base.richness;
            }
            propose(analysis, scale_hint, &p)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intent::{analyze_performance, AnalysisInput, Calibration, Mode};
    use crate::notes::Note;
    use crate::phrase::PhraseParams;

    fn note(start: f64, dur: f64, midi: i32) -> Note {
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

    /// Four phrases in G mixolydian (any mode would do), breaths between.
    fn performance(damage: bool) -> (Vec<Note>, Scale) {
        let scale = Scale::new(7, ScaleKind::Mixolydian);
        let phrases: [&[(i32, f64)]; 4] = [
            &[
                (67, 0.5),
                (69, 0.5),
                (71, 0.5),
                (74, 1.5),
                (72, 0.5),
                (71, 1.5),
            ],
            &[(72, 0.5), (74, 0.5), (76, 1.0), (77, 0.5), (76, 1.5)],
            &[(69, 0.5), (71, 0.5), (72, 1.0), (69, 0.5), (67, 2.0)],
            &[(74, 0.5), (72, 0.5), (71, 0.5), (69, 0.5), (67, 2.5)],
        ];
        let mut t = 0.0;
        let mut notes = Vec::new();
        let mut k = 0;
        for ph in phrases {
            for &(m, d) in ph {
                let mut m = m;
                if damage && k % 5 == 3 {
                    m += 1; // a wrong held note now and then
                }
                notes.push(note(t, d * 0.95, m));
                t += d;
                k += 1;
            }
            t += 0.6;
        }
        (notes, scale)
    }

    fn analysis(notes: &[Note], scale: &Scale) -> PerformanceAnalysis {
        analyze_performance(&AnalysisInput {
            notes,
            scale: Some(scale),
            profile: &[],
            expected: &[],
            annotations: &[],
            mode: Mode::Assisted,
            mode_params: Mode::Assisted.default_params(),
            locks: Locks::default(),
            phrase_params: PhraseParams::for_wind(),
            calibration: Calibration::default(),
            kinds: &ScaleKind::ALL,
        })
    }

    #[test]
    fn three_strategies_differ_and_stay_in_the_mode() {
        let (notes, scale) = performance(false);
        let a = analysis(&notes, &scale);
        assert_eq!(a.phrases.len(), 4);
        let all = propose_all(
            &a,
            Some(&scale),
            &HarmonyParams::for_strategy(Strategy::ModalPads, 3),
        );
        assert_eq!(all.len(), 3);
        let drone = &all[0];
        assert_eq!(drone.strategy, Strategy::DroneOnly);
        assert!(drone.chords.is_empty() && drone.bass.len() == 4);
        assert_eq!(
            drone.drone[0].midi.rem_euclid(12),
            7,
            "drone on the tonic G"
        );
        assert_eq!(
            drone.drone[1].midi - drone.drone[0].midi,
            7,
            "and its fifth"
        );
        let modal = &all[1];
        let liquid = &all[2];
        assert_eq!(modal.chords.len(), 4);
        for c in modal.chords.iter().chain(&liquid.chords) {
            for t in &c.tones {
                assert!(
                    scale.contains(t.midi as f64 + t.cents / 100.0, 1.0),
                    "chord tone {} outside the mode",
                    t.midi
                );
            }
            assert!(c.tones.iter().all(|t| t.midi >= 48 && t.midi <= 72));
            assert!(!c.why.is_empty());
        }
        // Every modal chord explains itself with the melody tones it covers or why it stayed.
        assert!(modal.chords.iter().all(|c| c.why.iter().any(|w| matches!(
            w,
            HarmonyReason::CoversMelody { .. }
                | HarmonyReason::KeptHold { .. }
                | HarmonyReason::KeptLowConfidence { .. }
        ))));
        assert!(modal.chords[0].why.contains(&HarmonyReason::FirstChord));
        // Liquid chords are richer and tenser.
        let mean = |p: &HarmonyProposal| {
            p.chords.iter().map(|c| c.tension).sum::<f64>() / p.chords.len() as f64
        };
        assert!(liquid.chords.iter().any(|c| matches!(
            c.shape,
            Shape::Seventh | Shape::Add9 | Shape::Sus2 | Shape::Sus4 | Shape::Quartal
        )));
        assert!(
            mean(liquid) >= mean(modal),
            "liquid {} modal {}",
            mean(liquid),
            mean(modal)
        );
        assert!(modal.score.melodic_compatibility > 0.4, "{:?}", modal.score);
        assert!(modal.score.total > 0.0 && modal.score.total <= 1.0);
    }

    #[test]
    fn seed_reproduces_and_errors_do_not_change_the_chords() {
        let (notes, scale) = performance(false);
        let a = analysis(&notes, &scale);
        let p1 = propose(
            &a,
            Some(&scale),
            &HarmonyParams::for_strategy(Strategy::LiquidChords, 9),
        );
        let p2 = propose(
            &a,
            Some(&scale),
            &HarmonyParams::for_strategy(Strategy::LiquidChords, 9),
        );
        assert_eq!(p1, p2);
        let (damaged, _) = performance(true);
        let ad = analysis(&damaged, &scale);
        let pm = propose(
            &a,
            Some(&scale),
            &HarmonyParams::for_strategy(Strategy::ModalPads, 1),
        );
        let pd = propose(
            &ad,
            Some(&scale),
            &HarmonyParams::for_strategy(Strategy::ModalPads, 1),
        );
        let roots = |p: &HarmonyProposal| {
            p.chords
                .iter()
                .map(|c| (c.root_pc, c.shape))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            roots(&pm),
            roots(&pd),
            "wrong notes must not move the harmony"
        );
    }

    #[test]
    fn locks_and_low_confidence_keep_the_chord_and_say_so() {
        let (notes, scale) = performance(false);
        let a = analysis(&notes, &scale);
        let mut p = HarmonyParams::for_strategy(Strategy::ModalPads, 1);
        p.locks.chord = true;
        let locked = propose(&a, Some(&scale), &p);
        assert!(locked
            .chords
            .windows(2)
            .all(|w| w[0].root_pc == w[1].root_pc && w[0].shape == w[1].shape));
        assert!(locked.chords[1].why.contains(&HarmonyReason::KeptChordLock));
        let mut p = HarmonyParams::for_strategy(Strategy::ModalPads, 1);
        p.min_confidence = 1.01; // nothing is ever confident enough
        let kept = propose(&a, Some(&scale), &p);
        assert!(kept.chords[1..].iter().all(|c| c
            .why
            .iter()
            .any(|w| matches!(w, HarmonyReason::KeptLowConfidence { .. }))));
        // Movement 1 = may change every phrase; movement 0 = holds for four.
        let mut fast = HarmonyParams::for_strategy(Strategy::ModalPads, 1);
        fast.movement = 1.0;
        let mut slow = fast.clone();
        slow.movement = 0.0;
        let f = propose(&a, Some(&scale), &fast);
        let s = propose(&a, Some(&scale), &slow);
        let changes = |p: &HarmonyProposal| {
            p.chords
                .windows(2)
                .filter(|w| w[0].root_pc != w[1].root_pc || w[0].shape != w[1].shape)
                .count()
        };
        assert!(changes(&f) >= changes(&s));
        // The hold rule: while the previous chord still covers ≥ 30 % of the phrase's
        // intent and fewer than four phrases passed, the chord is kept and says so.
        let mut last_change = 0;
        for i in 1..s.chords.len() {
            let (share, _) = melody_share(&s.chords[i - 1].tones, &a.intents[i].tones);
            let changed = s.chords[i].root_pc != s.chords[i - 1].root_pc
                || s.chords[i].shape != s.chords[i - 1].shape;
            if share >= 0.3 && i - last_change < 4 {
                assert!(!changed, "phrase {i} should hold");
                // Kept for the hold, or earlier in the rule order for low confidence.
                assert!(
                    s.chords[i].why.iter().any(|w| matches!(
                        w,
                        HarmonyReason::KeptHold { .. } | HarmonyReason::KeptLowConfidence { .. }
                    )),
                    "{:?}",
                    s.chords[i].why
                );
            }
            if changed {
                last_change = i;
            }
        }
    }

    #[test]
    fn empty_analysis_gives_an_empty_proposal() {
        let (notes, scale) = performance(false);
        let mut a = analysis(&notes, &scale);
        a.phrases.clear();
        a.intents.clear();
        a.contexts.clear();
        let p = propose(
            &a,
            Some(&scale),
            &HarmonyParams::for_strategy(Strategy::ModalPads, 1),
        );
        assert!(p.chords.is_empty() && p.bass.is_empty() && p.confidence == 0.0);
    }
}
