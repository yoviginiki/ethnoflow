//! Rhythm section (Phase 4: F4.1 drums, F4.2 bass, F4.5 ostinato, F4.6 arpeggio) on
//! graph slots 12..16, driven by one shared beat clock so everything lines up with
//! the project's tempo and meter. Patterns come from `music_core` (pure, seeded);
//! a pattern-affecting change rebuilds that one slot in place (the clock keeps the
//! position), a level/voice change goes through atomics. Tonal parts follow the
//! chord the UI hands over (an armed harmony proposal) or the project's mode.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::{
    pattern_channel, write_wav, AudioBuffer, BassNoteEvent, BassParams, BassPatternData,
    BassSource, BeatClock, DrumEvent, DrumKind, DrumParams, DrumPatternData, DrumSource, Graph,
    GraphCmd, GraphParams, NotePatternData, PatternNote, PatternParams, PatternSwap,
    PatternSynthSource, WavFormat,
};
use audio_engine::{
    ChordEvent, ChordPadParams, ChordPadSource, ChordPatternData, LeadParams, LeadSource,
    TapanParams, TapanSource,
};
use music_core::{
    generate_arpeggio, generate_bass, generate_chord_pad, generate_groove, generate_lead,
    generate_ostinato, generate_tapan, static_spans, tones_in_register, ArpDirection, ArpParams,
    BassParams as BassGenParams, ChordPadParams as ChordPadGenParams, Drum, GrooveParams,
    GrooveStyle, LeadPhraseParams, Meter, Motif, OstinatoParams, TapanPatternParams,
    TempoCandidate,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::forge_cmds::{apply_voice, resolve_voice, AuditionParams};
use crate::graph_cmds::{ensure_graph, note_hz, Intonation};
use crate::pad_cmds::PadNote;
use crate::state::{AppState, Inner};
use project::InstrumentVoice;

pub const DRUMS_SLOT: usize = 12;
pub const BASS_SLOT: usize = 13;
pub const OSTINATO_SLOT: usize = 14;
pub const ARP_SLOT: usize = 15;
/// First slot of the Instrument Rack's extra parts (tapan, chord pad, lead …); one slot
/// per part, up to `MAX_EXTRAS` (P1 R3).
pub const EXTRA_SLOT0: usize = 16;
pub const MAX_EXTRAS: usize = audio_engine::MAX_LAYERS - EXTRA_SLOT0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumsSettings {
    pub on: bool,
    pub style: GrooveStyle,
    pub complexity: f64,
    pub humanize: f64,
    pub swing: f64,
    pub fill_every_bars: u32,
    pub seed: u32,
    pub level_db: f32,
    pub kick_hz: f32,
    pub decay: f32,
    pub brightness: f32,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BassSettings {
    pub on: bool,
    /// 0 sub, 1 pluck, 2 reese.
    pub voice: u8,
    pub density: f64,
    pub syncopation: f64,
    pub passing: f64,
    pub register_low: i32,
    pub register_high: i32,
    pub seed: u32,
    pub level_db: f32,
    pub cutoff_hz: f32,
    pub resonance: f32,
    pub drive: f32,
    pub glide_s: f32,
    pub sidechain: f32,
    pub wobble_div: u32,
    pub wobble_depth: f32,
    pub warmth: f32,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OstinatoSettings {
    pub on: bool,
    pub steps: u32,
    pub step_beats: f64,
    pub density: f64,
    pub probability: f64,
    pub variation_every_bars: u32,
    pub swing: f64,
    pub seed: u32,
    pub level_db: f32,
    pub register_low: i32,
    pub register_high: i32,
    /// 0 sine, 1 triangle, 2 flute.
    pub timbre: u8,
    pub release_s: f32,
    #[serde(default)]
    pub locked: bool,
    /// A locked motif survives regeneration with any seed.
    #[serde(default)]
    pub motif: Option<Motif>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArpSettings {
    pub on: bool,
    pub direction: ArpDirection,
    pub octaves: u32,
    pub step_beats: f64,
    pub gate: f64,
    pub density: f64,
    pub swing: f64,
    pub seed: u32,
    pub level_db: f32,
    pub register_low: i32,
    pub timbre: u8,
    pub release_s: f32,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RhythmSettings {
    pub drums: DrumsSettings,
    pub bass: BassSettings,
    pub ostinato: OstinatoSettings,
    pub arp: ArpSettings,
    /// Pattern length before it repeats.
    pub bars: u32,
    /// Chord the tonal parts follow (an armed proposal), else the mode's triad.
    #[serde(default)]
    pub chord: Option<Vec<PadNote>>,
    /// Instrument presets per part (Instrument Forge); None = the part's own fields.
    #[serde(default)]
    pub instruments: InstrumentRefs,
    /// Root when the project has no scale (the drone's root).
    #[serde(default)]
    pub root_midi: Option<u8>,
    pub intonation: Intonation,
    /// Extra parts of the Instrument Rack (P1 R3), in slot order from `EXTRA_SLOT0`.
    #[serde(default)]
    pub extras: Vec<ExtraPart>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtraKind {
    Tapan,
    ChordPad,
    Lead,
}

impl ExtraKind {
    pub fn name(self) -> &'static str {
        match self {
            ExtraKind::Tapan => "tapan",
            ExtraKind::ChordPad => "chordpad",
            ExtraKind::Lead => "lead",
        }
    }
}

/// One extra part: what plays (kind + optional instrument preset), how loud, and the
/// generator's knobs (each kind reads the fields it needs; the others are ignored).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtraPart {
    pub id: String,
    pub kind: ExtraKind,
    #[serde(default = "yes")]
    pub on: bool,
    #[serde(default = "extra_level")]
    pub level_db: f32,
    #[serde(default)]
    pub instrument: Option<String>,
    /// Tapan: share of off-positions with strokes; lead: share of steps that sound.
    #[serde(default = "half")]
    pub density: f64,
    /// Tapan: velocity spread.
    #[serde(default = "fifth")]
    pub humanize: f64,
    /// Tapan: a rim roll every N bars (0 = never).
    #[serde(default = "four")]
    pub fill_every_bars: u32,
    /// Chord pad: release gap before the next chord (0..0.9 of its length).
    #[serde(default = "tenth")]
    pub gap: f64,
    /// Chord pad: bars per chord event.
    #[serde(default = "one")]
    pub bars_per_chord: u32,
    /// Lead: step length in beats.
    #[serde(default = "half")]
    pub step_beats: f64,
    /// Lead: phrase length in bars.
    #[serde(default = "two")]
    pub phrase_bars: u32,
    /// Lead: share of leaps.
    #[serde(default = "fifth")]
    pub leaps: f64,
    /// Chord pad / lead register, MIDI.
    #[serde(default = "reg_low")]
    pub register_low: i32,
    #[serde(default = "reg_high")]
    pub register_high: i32,
    #[serde(default = "one")]
    pub seed: u32,
    #[serde(default)]
    pub locked: bool,
}

fn yes() -> bool {
    true
}
fn extra_level() -> f32 {
    -14.0
}
fn half() -> f64 {
    0.5
}
fn fifth() -> f64 {
    0.2
}
fn tenth() -> f64 {
    0.1
}
fn one() -> u32 {
    1
}
fn two() -> u32 {
    2
}
fn four() -> u32 {
    4
}
fn reg_low() -> i32 {
    60
}
fn reg_high() -> i32 {
    84
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RhythmStatus {
    pub playing: bool,
    pub sample_rate: u32,
    pub bpm: f64,
    /// Position in beats since the clock started.
    pub beat: f64,
    pub bar: u32,
    pub beat_in_bar: f64,
    /// Peaks: drums, bass, ostinato, arp.
    pub peaks: [f32; 4],
    pub master_peak: f32,
    pub limiter_gain: f32,
    pub stream_errors: u64,
    /// The ostinato motif in use (so the UI can lock it).
    pub motif: Option<Motif>,
    pub root_pc: Option<u8>,
    pub chord_pcs: Vec<u8>,
    /// Peaks of the extra parts, in `settings.extras` order (0 when a part is off).
    #[serde(default)]
    pub extra_peaks: Vec<f32>,
}

/// Instrument preset ids per part (empty string / None = legacy fields).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct InstrumentRefs {
    #[serde(default)]
    pub drums: Option<String>,
    #[serde(default)]
    pub bass: Option<String>,
    #[serde(default)]
    pub ostinato: Option<String>,
    #[serde(default)]
    pub arp: Option<String>,
}

/// Resolved voices per part (drums, bass, ostinato, arp), None = legacy fields.
pub fn resolve_voices(s: &Inner, refs: &InstrumentRefs) -> [Option<InstrumentVoice>; 4] {
    [
        resolve_voice(s, refs.drums.as_deref()),
        resolve_voice(s, refs.bass.as_deref()),
        resolve_voice(s, refs.ostinato.as_deref()),
        resolve_voice(s, refs.arp.as_deref()),
    ]
}

pub struct RhythmLive {
    pub clock: Arc<BeatClock>,
    /// Instrument voices per part when a preset is referenced.
    pub voices: [Option<InstrumentVoice>; 4],
    pub drums: Option<Arc<DrumParams>>,
    pub bass: Option<Arc<BassParams>>,
    pub ostinato: Option<Arc<PatternParams>>,
    pub arp: Option<Arc<PatternParams>>,
    pub settings: RhythmSettings,
    /// Build keys per slot (pattern-affecting inputs).
    pub keys: [String; 4],
    pub motif: Option<Motif>,
    /// Pattern hand-offs to the live sources (a slider move never restarts a voice).
    pub drum_swap: Option<PatternSwap<DrumPatternData>>,
    pub bass_swap: Option<PatternSwap<BassPatternData>>,
    pub ost_swap: Option<PatternSwap<NotePatternData>>,
    pub arp_swap: Option<PatternSwap<NotePatternData>>,
    /// Extra parts that are running (rack slots).
    pub extras: Vec<ExtraLive>,
}

pub struct ExtraLive {
    pub id: String,
    pub kind: ExtraKind,
    pub slot: usize,
    pub voice: Option<InstrumentVoice>,
    pub params: AuditionParams,
    pub key: String,
    pub swap: ExtraSwap,
}

pub enum ExtraSwap {
    Drum(PatternSwap<DrumPatternData>),
    Chord(PatternSwap<ChordPatternData>),
    Note(PatternSwap<NotePatternData>),
    None,
}

impl ExtraSwap {
    fn send(&mut self, pat: ExtraPattern) {
        match (self, pat) {
            (ExtraSwap::Drum(sw), ExtraPattern::Drum(p)) => {
                sw.send(p);
            }
            (ExtraSwap::Chord(sw), ExtraPattern::Chord(p)) => {
                sw.send(p);
            }
            (ExtraSwap::Note(sw), ExtraPattern::Note(p)) => {
                sw.send(p);
            }
            _ => {}
        }
    }
}

pub enum ExtraPattern {
    Drum(Arc<DrumPatternData>),
    Chord(Arc<ChordPatternData>),
    Note(Arc<NotePatternData>),
}

/// The project's mode (all degrees) for melodic parts; the chord tones when there is
/// no scale.
fn mode_pcs(s: &Inner, pcs: &[(u8, f64)]) -> Vec<(u8, f64)> {
    if let Some(scale) = s.project.as_ref().and_then(|p| p.manifest().scale.clone()) {
        let v: Vec<(u8, f64)> = scale
            .degrees_cents
            .iter()
            .map(|&c| {
                let semis = (c / 100.0).round() as i32;
                (
                    ((scale.tonic_pc as i32 + semis).rem_euclid(12)) as u8,
                    c - semis as f64 * 100.0,
                )
            })
            .collect();
        if !v.is_empty() {
            return v;
        }
    }
    pcs.to_vec()
}

fn build_extra(
    s: &Inner,
    set: &RhythmSettings,
    part: &ExtraPart,
    meter: &Meter,
    pcs: &[(u8, f64)],
) -> ExtraPattern {
    let bars = set.bars.clamp(1, 64);
    let length = bars as f64 * meter.bar_beats();
    match part.kind {
        ExtraKind::Tapan => {
            let hits = generate_tapan(
                meter,
                &TapanPatternParams {
                    density: part.density,
                    humanize: part.humanize,
                    fill_every_bars: part.fill_every_bars,
                    seed: part.seed,
                    bars,
                },
            );
            ExtraPattern::Drum(crate::forge_cmds::tapan_pattern(&hits, length))
        }
        ExtraKind::ChordPad => {
            let steps = generate_chord_pad(
                pcs,
                &ChordPadGenParams {
                    bars_per_chord: part.bars_per_chord,
                    gap: part.gap,
                    register_low: part.register_low,
                    register_high: part.register_high,
                    bars,
                },
                meter,
            );
            let events = steps
                .iter()
                .map(|st| ChordEvent {
                    beat: st.start_beat,
                    length_beats: st.length_beats,
                    hz: st
                        .tones
                        .iter()
                        .map(|(m, c)| hz(s, *m, *c, set.intonation))
                        .collect(),
                    velocity: st.velocity as f32,
                })
                .collect();
            ExtraPattern::Chord(Arc::new(ChordPatternData::new(events, length)))
        }
        ExtraKind::Lead => {
            let tones = tones_in_register(&mode_pcs(s, pcs), part.register_low, part.register_high);
            let notes = generate_lead(
                &tones,
                &LeadPhraseParams {
                    density: part.density,
                    step_beats: part.step_beats,
                    phrase_bars: part.phrase_bars,
                    leaps: part.leaps,
                    register_low: part.register_low,
                    register_high: part.register_high,
                    seed: part.seed,
                    bars,
                },
                meter,
            );
            let events = notes
                .iter()
                .map(|n| PatternNote {
                    beat: n.start_beat,
                    length_beats: n.length_beats,
                    hz: hz(s, n.midi, n.cents, set.intonation),
                    velocity: n.velocity as f32,
                })
                .collect();
            ExtraPattern::Note(Arc::new(NotePatternData::new(events, length)))
        }
    }
}

/// Everything pattern-affecting about an extra part (level, instrument and lock are not).
fn extra_key(part: &ExtraPart, meter: &Meter, pcs: &[(u8, f64)], set: &RhythmSettings) -> String {
    format!(
        "{:?}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{:?}:{:?}:{:?}",
        part.kind,
        part.density,
        part.humanize,
        part.fill_every_bars,
        part.gap,
        part.bars_per_chord,
        part.step_beats,
        part.phrase_bars,
        part.leaps,
        part.register_low,
        part.register_high,
        part.seed,
        set.bars,
        set.intonation,
        pcs,
        meter
    )
}

fn extra_params_default(kind: ExtraKind) -> AuditionParams {
    match kind {
        ExtraKind::Tapan => AuditionParams::Tapan(Arc::new(TapanParams::default())),
        ExtraKind::ChordPad => AuditionParams::ChordPad(Arc::new(ChordPadParams::default())),
        ExtraKind::Lead => AuditionParams::Lead(Arc::new(LeadParams::default())),
    }
}

/// The engine source for an extra part; `live` adds the pattern inbox for in-place swaps.
fn make_extra(
    params: &AuditionParams,
    clock: &Arc<BeatClock>,
    pat: &ExtraPattern,
    live: bool,
) -> (Box<dyn audio_engine::Source>, ExtraSwap) {
    match (params, pat) {
        (AuditionParams::Tapan(p), ExtraPattern::Drum(d)) => {
            let src = TapanSource::new(Arc::clone(p), Arc::clone(clock), Arc::clone(d));
            if live {
                let (swap, inbox) = pattern_channel::<DrumPatternData>();
                (Box::new(src.with_inbox(inbox)), ExtraSwap::Drum(swap))
            } else {
                (Box::new(src), ExtraSwap::None)
            }
        }
        (AuditionParams::ChordPad(p), ExtraPattern::Chord(d)) => {
            let src = ChordPadSource::new(Arc::clone(p), Arc::clone(clock), Arc::clone(d));
            if live {
                let (swap, inbox) = pattern_channel::<ChordPatternData>();
                (Box::new(src.with_inbox(inbox)), ExtraSwap::Chord(swap))
            } else {
                (Box::new(src), ExtraSwap::None)
            }
        }
        (AuditionParams::Lead(p), ExtraPattern::Note(d)) => {
            let src = LeadSource::new(Arc::clone(p), Arc::clone(clock), Arc::clone(d));
            if live {
                let (swap, inbox) = pattern_channel::<NotePatternData>();
                (Box::new(src.with_inbox(inbox)), ExtraSwap::Note(swap))
            } else {
                (Box::new(src), ExtraSwap::None)
            }
        }
        _ => unreachable!("extra kind and pattern always match (build_extra)"),
    }
}

/// All slots the section uses right now (the four parts and the running extras).
fn part_slots(live: &RhythmLive) -> Vec<usize> {
    let mut v = vec![DRUMS_SLOT, BASS_SLOT, OSTINATO_SLOT, ARP_SLOT];
    v.extend(live.extras.iter().map(|e| e.slot));
    v
}

pub(crate) fn timing(s: &Inner) -> CmdResult<(f64, Meter)> {
    let m = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?
        .manifest();
    let bpm = m
        .tempo_map
        .first()
        .map(|t| t.bpm)
        .ok_or_else(|| err("no_timing", "Set tempo and meter first"))?;
    let ts = m
        .time_signatures
        .first()
        .ok_or_else(|| err("no_timing", "Set tempo and meter first"))?;
    Ok((
        bpm,
        Meter::new(ts.numerator, ts.unit).with_grouping(ts.grouping.clone()),
    ))
}

/// Root pitch class and chord tones (pc, cents) the tonal parts play.
pub(crate) fn chord_pcs(s: &Inner, set: &RhythmSettings) -> CmdResult<(u8, Vec<(u8, f64)>)> {
    if let Some(chord) = set.chord.as_ref().filter(|c| !c.is_empty()) {
        let root = chord[0].midi % 12;
        let mut pcs: Vec<(u8, f64)> = chord.iter().map(|n| (n.midi % 12, n.cents)).collect();
        pcs.dedup_by(|a, b| a.0 == b.0);
        return Ok((root, pcs));
    }
    let scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
    if let Some(scale) = scale {
        let root = set.root_midi.map(|m| m % 12).unwrap_or(scale.tonic_pc);
        // Triad on the root from the mode's degrees (1-3-5 counted from the root).
        let degrees: Vec<(u8, f64)> = scale
            .degrees_cents
            .iter()
            .map(|&c| {
                let semis = (c / 100.0).round() as i32;
                (
                    ((scale.tonic_pc as i32 + semis).rem_euclid(12)) as u8,
                    c - semis as f64 * 100.0,
                )
            })
            .collect();
        let start = degrees.iter().position(|(pc, _)| *pc == root).unwrap_or(0);
        let n = degrees.len();
        let pick: Vec<(u8, f64)> = if n >= 5 {
            [0usize, 2, 4]
                .iter()
                .map(|k| degrees[(start + k) % n])
                .collect()
        } else {
            degrees.clone()
        };
        return Ok((root, pick));
    }
    if let Some(r) = set.root_midi {
        let root = r % 12;
        return Ok((root, vec![(root, 0.0), ((root + 7) % 12, 0.0)]));
    }
    Err(err(
        "no_root",
        "Choose a tonal centre or a drone root first",
    ))
}

fn hz(s: &Inner, midi: i32, cents: f64, intonation: Intonation) -> f32 {
    note_hz(s, midi.clamp(0, 127) as u8, cents, intonation).0
}

fn drum_kind(d: Drum) -> DrumKind {
    match d {
        Drum::Kick => DrumKind::Kick,
        Drum::Snare => DrumKind::Snare,
        Drum::Ghost => DrumKind::Ghost,
        Drum::HatClosed => DrumKind::HatClosed,
        Drum::HatOpen => DrumKind::HatOpen,
        Drum::Shaker => DrumKind::Shaker,
    }
}

fn build_drums(set: &RhythmSettings, meter: &Meter) -> Arc<DrumPatternData> {
    let d = &set.drums;
    let pat = generate_groove(
        &GrooveParams {
            style: d.style,
            complexity: d.complexity,
            humanize: d.humanize,
            swing: d.swing,
            fill_every_bars: d.fill_every_bars,
            seed: d.seed,
            bars: set.bars.clamp(1, 64),
        },
        meter,
    );
    let events = pat
        .hits
        .iter()
        .map(|h| DrumEvent {
            beat: h.start_beat,
            kind: drum_kind(h.drum),
            velocity: h.velocity as f32,
        })
        .collect();
    Arc::new(DrumPatternData::new(events, pat.length_beats()))
}

fn build_bass(
    s: &Inner,
    set: &RhythmSettings,
    meter: &Meter,
    root: u8,
    pcs: &[(u8, f64)],
) -> Arc<BassPatternData> {
    let b = &set.bass;
    let spans = static_spans(root, pcs, set.bars.clamp(1, 64), meter);
    let scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
    let notes = generate_bass(
        &spans,
        scale.as_ref(),
        &BassGenParams {
            density: b.density,
            syncopation: b.syncopation,
            passing: b.passing,
            register: (b.register_low, b.register_high),
            swing: 0.0,
            seed: b.seed,
        },
        meter,
    );
    let events = notes
        .iter()
        .map(|n| BassNoteEvent {
            beat: n.start_beat,
            length_beats: n.length_beats,
            hz: hz(s, n.midi, n.cents, set.intonation),
            velocity: n.velocity as f32,
            glide: n.glide,
        })
        .collect();
    Arc::new(BassPatternData::new(
        events,
        set.bars.clamp(1, 64) as f64 * meter.bar_beats(),
    ))
}

fn build_ostinato(
    s: &Inner,
    set: &RhythmSettings,
    meter: &Meter,
    pcs: &[(u8, f64)],
) -> (Arc<NotePatternData>, Motif) {
    let o = &set.ostinato;
    let tones = tones_in_register(pcs, o.register_low, o.register_high);
    let params = OstinatoParams {
        steps: o.steps,
        step_beats: o.step_beats,
        density: o.density,
        probability: o.probability,
        variation_every_bars: o.variation_every_bars,
        swing: o.swing,
        seed: o.seed,
        bars: set.bars.clamp(1, 64),
    };
    let motif = o
        .motif
        .clone()
        .unwrap_or_else(|| Motif::generate(tones.len(), &params));
    let notes = generate_ostinato(&tones, &params, meter, o.motif.as_ref());
    let events = notes
        .iter()
        .map(|n| PatternNote {
            beat: n.start_beat,
            length_beats: n.length_beats,
            hz: hz(s, n.midi, n.cents, set.intonation),
            velocity: n.velocity as f32,
        })
        .collect();
    (
        Arc::new(NotePatternData::new(
            events,
            set.bars.clamp(1, 64) as f64 * meter.bar_beats(),
        )),
        motif,
    )
}

fn build_arp(
    s: &Inner,
    set: &RhythmSettings,
    meter: &Meter,
    pcs: &[(u8, f64)],
) -> Arc<NotePatternData> {
    let a = &set.arp;
    let chord = tones_in_register(pcs, a.register_low, a.register_low + 11);
    let notes = generate_arpeggio(
        &chord,
        &ArpParams {
            direction: a.direction,
            octaves: a.octaves,
            step_beats: a.step_beats,
            gate: a.gate,
            density: a.density,
            swing: a.swing,
            seed: a.seed,
            bars: set.bars.clamp(1, 64),
        },
        meter,
    );
    let events = notes
        .iter()
        .map(|n| PatternNote {
            beat: n.start_beat,
            length_beats: n.length_beats,
            hz: hz(s, n.midi, n.cents, set.intonation),
            velocity: n.velocity as f32,
        })
        .collect();
    Arc::new(NotePatternData::new(
        events,
        set.bars.clamp(1, 64) as f64 * meter.bar_beats(),
    ))
}

fn keys(set: &RhythmSettings, meter: &Meter, root: u8, pcs: &[(u8, f64)]) -> [String; 4] {
    let d = &set.drums;
    let b = &set.bass;
    let o = &set.ostinato;
    let a = &set.arp;
    let tones = format!("{root}:{pcs:?}:{:?}:{}", set.intonation, set.bars);
    [
        format!(
            "{:?}:{}:{}:{}:{}:{}:{}:{:?}",
            d.style, d.complexity, d.humanize, d.swing, d.fill_every_bars, d.seed, set.bars, meter
        ),
        format!(
            "{}:{}:{}:{}:{}:{}:{tones}:{:?}",
            b.density, b.syncopation, b.passing, b.register_low, b.register_high, b.seed, meter
        ),
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{:?}:{tones}:{:?}",
            o.steps,
            o.step_beats,
            o.density,
            o.probability,
            o.variation_every_bars,
            o.swing,
            o.seed,
            o.register_low,
            o.register_high,
            o.motif,
            meter
        ),
        format!(
            "{:?}:{}:{}:{}:{}:{}:{}:{}:{tones}:{:?}",
            a.direction,
            a.octaves,
            a.step_beats,
            a.gate,
            a.density,
            a.swing,
            a.seed,
            a.register_low,
            meter
        ),
    ]
}

fn validate(set: &RhythmSettings) -> CmdResult<()> {
    let unit = |v: f64| (0.0..=1.0).contains(&v);
    if set.extras.len() > MAX_EXTRAS {
        return Err(err(
            "invalid_argument",
            format!("at most {MAX_EXTRAS} extra parts"),
        ));
    }
    for (i, x) in set.extras.iter().enumerate() {
        if x.id.is_empty() || x.id.len() > 32 || set.extras[..i].iter().any(|y| y.id == x.id) {
            return Err(err("invalid_argument", "extra part ids must be unique"));
        }
        if !(-40.0..=6.0).contains(&x.level_db)
            || !unit(x.density)
            || !unit(x.humanize)
            || !(0.0..=0.9).contains(&x.gap)
            || !unit(x.leaps)
            || x.fill_every_bars > 64
            || !(1..=16).contains(&x.bars_per_chord)
            || !(0.125..=2.0).contains(&x.step_beats)
            || !(1..=8).contains(&x.phrase_bars)
            || !(24..=108).contains(&x.register_low)
            || !(24..=108).contains(&x.register_high)
            || x.register_low >= x.register_high
        {
            return Err(err(
                "invalid_argument",
                format!("extra part {} out of range", x.id),
            ));
        }
    }
    let d = &set.drums;
    let b = &set.bass;
    let o = &set.ostinato;
    let a = &set.arp;
    if !(1..=64).contains(&set.bars)
        || !unit(d.complexity)
        || !unit(d.humanize)
        || !unit(d.swing)
        || d.fill_every_bars > 32
        || !(-60.0..=6.0).contains(&d.level_db)
        || !(35.0..=90.0).contains(&d.kick_hz)
        || !unit(b.density)
        || !unit(b.syncopation)
        || !unit(b.passing)
        || b.register_low < 20
        || b.register_high > 72
        || b.register_high - b.register_low < 5
        || !(-60.0..=6.0).contains(&b.level_db)
        || !(60.0..=8000.0).contains(&b.cutoff_hz)
        || !(2..=32).contains(&o.steps)
        || !(0.0625..=2.0).contains(&o.step_beats)
        || !unit(o.density)
        || !unit(o.probability)
        || o.register_low < 24
        || o.register_high > 108
        || o.register_high - o.register_low < 5
        || !(-60.0..=6.0).contains(&o.level_db)
        || !(1..=3).contains(&a.octaves)
        || !(0.0625..=2.0).contains(&a.step_beats)
        || !unit(a.gate)
        || !unit(a.density)
        || a.register_low < 24
        || a.register_low > 96
        || !(-60.0..=6.0).contains(&a.level_db)
    {
        return Err(err("invalid_argument", "rhythm settings out of range"));
    }
    if let Some(c) = &set.chord {
        if c.len() > 8 || c.iter().any(|n| n.midi > 127) {
            return Err(err("invalid_argument", "chord out of range"));
        }
    }
    Ok(())
}

/// Re-push the levels and the strip (background mixer bus) into the running section.
pub fn reapply(s: &Inner) {
    if let Some(live) = s.rhythm.as_ref() {
        apply_live(s, live);
    }
}

/// Push the parts' continuous parameters and levels into a set of slot params.
/// `lvl` maps a part's own level (dB) to the slot gain (floor, bus trim, mute).
pub(crate) fn apply_params(
    layers: &[audio_engine::LayerParams],
    live: &RhythmLive,
    lvl: &dyn Fn(f32) -> f32,
) {
    let set = &live.settings;
    let layer = |slot: usize| &layers[slot];
    // Extra parts: the preset shapes the voice, the part's slider sets the level.
    for e in &live.extras {
        if let Some(v) = e.voice.as_ref() {
            apply_voice(v, &e.params);
        }
        let level = set
            .extras
            .iter()
            .find(|p| p.id == e.id)
            .map(|p| p.level_db)
            .unwrap_or(extra_level());
        layer(e.slot).gain_db.set(lvl(level));
    }
    // Parts that reference an instrument preset take their voice from it.
    if let (Some(v), Some(p)) = (live.voices[0].as_ref(), live.drums.as_ref()) {
        apply_voice(v, &AuditionParams::Drums(Arc::clone(p)));
    }
    if let (Some(v), Some(p)) = (live.voices[1].as_ref(), live.bass.as_ref()) {
        apply_voice(v, &AuditionParams::Bass(Arc::clone(p)));
    }
    if let (Some(v), Some(p)) = (live.voices[2].as_ref(), live.ostinato.as_ref()) {
        apply_voice(v, &AuditionParams::Synth(Arc::clone(p)));
    }
    if let (Some(v), Some(p)) = (live.voices[3].as_ref(), live.arp.as_ref()) {
        apply_voice(v, &AuditionParams::Synth(Arc::clone(p)));
    }
    // The part's level is the user's slider whatever the voice: a preset shapes the sound,
    // the level stays with the part (offline renders have no bus strip to fall back on).
    if live.drums.is_some() {
        layer(DRUMS_SLOT).gain_db.set(lvl(set.drums.level_db));
    }
    if live.bass.is_some() {
        layer(BASS_SLOT).gain_db.set(lvl(set.bass.level_db));
    }
    if live.ostinato.is_some() {
        layer(OSTINATO_SLOT).gain_db.set(lvl(set.ostinato.level_db));
    }
    if live.arp.is_some() {
        layer(ARP_SLOT).gain_db.set(lvl(set.arp.level_db));
    }
    if let (None, Some(p)) = (live.voices[0].as_ref(), live.drums.as_ref()) {
        p.kick_hz.set(set.drums.kick_hz);
        p.decay.set(set.drums.decay);
        p.brightness.set(set.drums.brightness);
    }
    if let (None, Some(p)) = (live.voices[1].as_ref(), live.bass.as_ref()) {
        let b = &set.bass;
        p.voice.store(b.voice.min(2), Ordering::Relaxed);
        p.cutoff_hz.set(b.cutoff_hz);
        p.resonance.set(b.resonance);
        p.drive.set(b.drive);
        p.glide_s.set(b.glide_s);
        p.sidechain.set(b.sidechain);
        p.wobble_div.store(b.wobble_div, Ordering::Relaxed);
        p.wobble_depth.set(b.wobble_depth);
        p.warmth.set(b.warmth);
    }
    if let (None, Some(p)) = (live.voices[2].as_ref(), live.ostinato.as_ref()) {
        p.timbre
            .store(set.ostinato.timbre.min(2), Ordering::Relaxed);
        p.release_s.set(set.ostinato.release_s);
    }
    if let (None, Some(p)) = (live.voices[3].as_ref(), live.arp.as_ref()) {
        p.timbre.store(set.arp.timbre.min(2), Ordering::Relaxed);
        p.release_s.set(set.arp.release_s);
    }
}

fn apply_live(s: &Inner, live: &RhythmLive) {
    let Some(g) = s.graph.as_ref() else { return };
    // The slider's floor (−40) is silence; the bus trim and its mute come on top.
    let lvl = |db: f32| {
        let own = if db <= -39.5 { -120.0 } else { db };
        crate::bus_cmds::bus_gain(s, crate::bus_cmds::BUS_RHYTHM, own)
    };
    for slot in part_slots(live) {
        crate::bus_cmds::apply_bus_strip(
            s,
            crate::bus_cmds::BUS_RHYTHM,
            &g.params().layers[slot],
            0.0,
            20_000.0,
            10.0,
        );
    }
    apply_params(&g.params().layers, live, &lvl);
}

fn clear(s: &mut Inner, slot: usize) {
    if let Some(g) = s.graph.as_mut() {
        g.params().layers[slot].gain_db.set(-60.0);
        g.send(GraphCmd::Clear { slot });
    }
}

/// Bring the four slots in line with `set`: (re)build what changed, clear what is
/// off, push the live parameters.
fn sync(s: &mut Inner, set: RhythmSettings) -> CmdResult<()> {
    validate(&set)?;
    let (bpm, meter) = timing(s)?;
    let (root, pcs) = chord_pcs(s, &set)?;
    let new_keys = keys(&set, &meter, root, &pcs);
    let Some(live_prev) = s.rhythm.take() else {
        return Err(err("no_rhythm", "The rhythm section is not running"));
    };
    let clock = Arc::clone(&live_prev.clock);
    let sr = s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000);
    if let Some(g) = s.graph.as_ref() {
        let frame = g.params().frame.load(Ordering::Relaxed);
        if (clock.bpm() as f64 - bpm).abs() > 1e-6 {
            clock.set_bpm(bpm as f32, frame, sr);
        }
    }
    let RhythmLive {
        drum_swap,
        bass_swap,
        ost_swap,
        arp_swap,
        extras: prev_extras,
        ..
    } = live_prev;
    let voices = resolve_voices(s, &set.instruments);
    let mut live = RhythmLive {
        clock: Arc::clone(&clock),
        voices,
        drums: live_prev.drums.clone(),
        bass: live_prev.bass.clone(),
        ostinato: live_prev.ostinato.clone(),
        arp: live_prev.arp.clone(),
        settings: set.clone(),
        keys: live_prev.keys.clone(),
        motif: live_prev.motif.clone(),
        drum_swap,
        bass_swap,
        ost_swap,
        arp_swap,
        extras: Vec::new(),
    };
    // Drums.
    if set.drums.on {
        if live.drums.is_none() {
            let pat = build_drums(&set, &meter);
            let p = Arc::new(DrumParams::default());
            let (swap, inbox) = pattern_channel::<DrumPatternData>();
            let g = s.graph.as_mut().expect("graph");
            g.params().layers[DRUMS_SLOT].gain_db.set(-60.0);
            g.send(GraphCmd::SetSource {
                slot: DRUMS_SLOT,
                source: Box::new(
                    DrumSource::new(Arc::clone(&p), Arc::clone(&clock), pat).with_inbox(inbox),
                ),
            });
            live.drums = Some(p);
            live.drum_swap = Some(swap);
            live.keys[0] = new_keys[0].clone();
        } else if live.keys[0] != new_keys[0] {
            let pat = build_drums(&set, &meter);
            if let Some(sw) = live.drum_swap.as_mut() {
                sw.send(pat);
            }
            live.keys[0] = new_keys[0].clone();
        }
    } else if live.drums.is_some() {
        clear(s, DRUMS_SLOT);
        live.drums = None;
        live.drum_swap = None;
        live.keys[0].clear();
    }
    // Bass (ducked by the drums when they run).
    if set.bass.on {
        let drums_changed = live_prev.drums.is_some() != live.drums.is_some();
        if live.bass.is_none() || drums_changed {
            // (Re)build: first time, or the drums appeared/vanished (the sidechain
            // reads their envelope). Keep the params so the knobs stay where they are.
            let pat = build_bass(s, &set, &meter, root, &pcs);
            let p = live
                .bass
                .clone()
                .unwrap_or_else(|| Arc::new(BassParams::default()));
            let (swap, inbox) = pattern_channel::<BassPatternData>();
            let drums = live.drums.clone();
            let g = s.graph.as_mut().expect("graph");
            if live.bass.is_none() {
                g.params().layers[BASS_SLOT].gain_db.set(-60.0);
            }
            g.send(GraphCmd::SetSource {
                slot: BASS_SLOT,
                source: Box::new(
                    BassSource::new(Arc::clone(&p), Arc::clone(&clock), pat, drums)
                        .with_inbox(inbox),
                ),
            });
            live.bass = Some(p);
            live.bass_swap = Some(swap);
            live.keys[1] = new_keys[1].clone();
        } else if live.keys[1] != new_keys[1] {
            let pat = build_bass(s, &set, &meter, root, &pcs);
            if let Some(sw) = live.bass_swap.as_mut() {
                sw.send(pat);
            }
            live.keys[1] = new_keys[1].clone();
        }
    } else if live.bass.is_some() {
        clear(s, BASS_SLOT);
        live.bass = None;
        live.bass_swap = None;
        live.keys[1].clear();
    }
    // Ostinato.
    if set.ostinato.on {
        if live.ostinato.is_none() {
            let (pat, motif) = build_ostinato(s, &set, &meter, &pcs);
            let p = Arc::new(PatternParams::default());
            let (swap, inbox) = pattern_channel::<NotePatternData>();
            let g = s.graph.as_mut().expect("graph");
            g.params().layers[OSTINATO_SLOT].gain_db.set(-60.0);
            g.send(GraphCmd::SetSource {
                slot: OSTINATO_SLOT,
                source: Box::new(
                    PatternSynthSource::new(Arc::clone(&p), Arc::clone(&clock), pat)
                        .with_inbox(inbox),
                ),
            });
            live.ostinato = Some(p);
            live.ost_swap = Some(swap);
            live.keys[2] = new_keys[2].clone();
            live.motif = Some(motif);
        } else if live.keys[2] != new_keys[2] {
            let (pat, motif) = build_ostinato(s, &set, &meter, &pcs);
            if let Some(sw) = live.ost_swap.as_mut() {
                sw.send(pat);
            }
            live.keys[2] = new_keys[2].clone();
            live.motif = Some(motif);
        }
    } else if live.ostinato.is_some() {
        clear(s, OSTINATO_SLOT);
        live.ostinato = None;
        live.ost_swap = None;
        live.keys[2].clear();
    }
    // Arpeggio.
    if set.arp.on {
        if live.arp.is_none() {
            let pat = build_arp(s, &set, &meter, &pcs);
            let p = Arc::new(PatternParams::default());
            let (swap, inbox) = pattern_channel::<NotePatternData>();
            let g = s.graph.as_mut().expect("graph");
            g.params().layers[ARP_SLOT].gain_db.set(-60.0);
            g.send(GraphCmd::SetSource {
                slot: ARP_SLOT,
                source: Box::new(
                    PatternSynthSource::new(Arc::clone(&p), Arc::clone(&clock), pat)
                        .with_inbox(inbox),
                ),
            });
            live.arp = Some(p);
            live.arp_swap = Some(swap);
            live.keys[3] = new_keys[3].clone();
        } else if live.keys[3] != new_keys[3] {
            let pat = build_arp(s, &set, &meter, &pcs);
            if let Some(sw) = live.arp_swap.as_mut() {
                sw.send(pat);
            }
            live.keys[3] = new_keys[3].clone();
        }
    } else if live.arp.is_some() {
        clear(s, ARP_SLOT);
        live.arp = None;
        live.arp_swap = None;
        live.keys[3].clear();
    }
    // Extra parts (rack slots): reuse a running part with the same id, kind and slot
    // (push a new pattern when its key changed), build the rest, clear what vanished.
    let mut prev_extras = prev_extras;
    let mut next_extras: Vec<ExtraLive> = Vec::new();
    for (i, part) in set.extras.iter().enumerate().take(MAX_EXTRAS) {
        if !part.on {
            continue;
        }
        let slot = EXTRA_SLOT0 + i;
        let key = extra_key(part, &meter, &pcs, &set);
        let voice = resolve_voice(s, part.instrument.as_deref());
        if let Some(pos) = prev_extras
            .iter()
            .position(|e| e.id == part.id && e.kind == part.kind && e.slot == slot)
        {
            let mut e = prev_extras.swap_remove(pos);
            if e.key != key {
                let pat = build_extra(s, &set, part, &meter, &pcs);
                e.swap.send(pat);
                e.key = key;
            }
            e.voice = voice;
            next_extras.push(e);
        } else {
            let pat = build_extra(s, &set, part, &meter, &pcs);
            let params = extra_params_default(part.kind);
            let (src, swap) = make_extra(&params, &clock, &pat, true);
            let g = s.graph.as_mut().expect("graph");
            g.params().layers[slot].gain_db.set(-60.0);
            g.send(GraphCmd::SetSource { slot, source: src });
            next_extras.push(ExtraLive {
                id: part.id.clone(),
                kind: part.kind,
                slot,
                voice,
                params,
                key,
                swap,
            });
        }
    }
    for e in prev_extras {
        clear(s, e.slot);
    }
    live.extras = next_extras;
    if let Some(g) = s.graph.as_ref() {
        for slot in part_slots(&live) {
            g.params().layers[slot].pan.set(0.0);
        }
    }
    apply_live(s, &live);
    s.rhythm = Some(live);
    Ok(())
}

/// Stop the section: fade and clear the slots, stop the clock, stop the stream when
/// nothing else uses it.
pub fn release_all(s: &mut Inner) {
    if let Some(live) = s.rhythm.take() {
        live.clock.stop();
        if let Some(g) = s.graph.as_mut() {
            for slot in DRUMS_SLOT..audio_engine::MAX_LAYERS {
                g.params().layers[slot].gain_db.set(-60.0);
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
            for slot in DRUMS_SLOT..audio_engine::MAX_LAYERS {
                g.send(GraphCmd::Clear { slot });
            }
        }
    }
    if s.graph_idle() {
        s.stop_graph();
    }
}

/// The project's tempo or meter changed: keep the beat and rebuild the patterns.
pub fn on_timing_changed(s: &mut Inner) {
    if let Some(live) = s.rhythm.as_ref() {
        let set = live.settings.clone();
        if let Err(e) = sync(s, set) {
            log::warn!("rhythm after timing change: {}", e.message);
            release_all(s);
        }
    }
}

fn status(s: &mut Inner) -> CmdResult<RhythmStatus> {
    let (bpm, meter) = timing(s).unwrap_or((0.0, Meter::new(4, 4)));
    let mut out = RhythmStatus {
        playing: false,
        sample_rate: 0,
        bpm,
        beat: 0.0,
        bar: 0,
        beat_in_bar: 0.0,
        peaks: [0.0; 4],
        master_peak: 0.0,
        limiter_gain: 1.0,
        stream_errors: 0,
        motif: None,
        root_pc: None,
        chord_pcs: Vec::new(),
        extra_peaks: Vec::new(),
    };
    let Some(live) = s.rhythm.as_ref() else {
        return Ok(out);
    };
    out.motif = live.motif.clone();
    if let Ok((root, pcs)) = chord_pcs(s, &live.settings) {
        out.root_pc = Some(root);
        out.chord_pcs = pcs.iter().map(|p| p.0).collect();
    }
    let clock = Arc::clone(&live.clock);
    let extra_slots: Vec<Option<usize>> = live
        .settings
        .extras
        .iter()
        .map(|p| live.extras.iter().find(|e| e.id == p.id).map(|e| e.slot))
        .collect();
    let Some(graph) = s.graph.as_mut() else {
        return Ok(out);
    };
    let st = graph.status();
    let frame = graph.params().frame.load(Ordering::Relaxed);
    let m = graph.params().meters(audio_engine::MAX_LAYERS);
    let beat = clock.beat_at(frame, st.sample_rate).unwrap_or(0.0);
    let bar_beats = meter.bar_beats();
    out.playing = true;
    out.sample_rate = st.sample_rate;
    out.beat = beat;
    out.bar = (beat / bar_beats).floor() as u32 + 1;
    out.beat_in_bar = beat - (beat / bar_beats).floor() * bar_beats;
    out.peaks = [
        m.layers.get(DRUMS_SLOT).copied().unwrap_or(0.0),
        m.layers.get(BASS_SLOT).copied().unwrap_or(0.0),
        m.layers.get(OSTINATO_SLOT).copied().unwrap_or(0.0),
        m.layers.get(ARP_SLOT).copied().unwrap_or(0.0),
    ];
    out.extra_peaks = extra_slots
        .iter()
        .map(|slot| slot.and_then(|sl| m.layers.get(sl).copied()).unwrap_or(0.0))
        .collect();
    out.master_peak = m.master_peak;
    out.limiter_gain = m.limiter_gain;
    out.stream_errors = st.stream_errors;
    Ok(out)
}

#[tauri::command]
pub async fn start_rhythm(
    state: State<'_, AppState>,
    device: Option<String>,
    settings: RhythmSettings,
) -> CmdResult<RhythmStatus> {
    validate(&settings)?;
    let mut s = state.lock();
    let (bpm, _meter) = timing(&s)?;
    chord_pcs(&s, &settings)?;
    ensure_graph(&mut s, device.as_deref())?;
    if s.rhythm.is_none() {
        let clock = Arc::new(BeatClock::new(bpm as f32));
        let frame = s
            .graph
            .as_ref()
            .map(|g| g.params().frame.load(Ordering::Relaxed))
            .unwrap_or(0);
        clock.start(frame, 0.0);
        s.rhythm = Some(RhythmLive {
            clock,
            voices: [None, None, None, None],
            drums: None,
            bass: None,
            ostinato: None,
            arp: None,
            settings: settings.clone(),
            keys: Default::default(),
            motif: None,
            drum_swap: None,
            bass_swap: None,
            ost_swap: None,
            arp_swap: None,
            extras: Vec::new(),
        });
    }
    if let Err(e) = sync(&mut s, settings) {
        release_all(&mut s);
        return Err(e);
    }
    status(&mut s)
}

#[tauri::command]
pub async fn set_rhythm(
    state: State<'_, AppState>,
    settings: RhythmSettings,
) -> CmdResult<RhythmStatus> {
    let mut s = state.lock();
    if s.rhythm.is_none() {
        return Err(err("no_rhythm", "The rhythm section is not running"));
    }
    sync(&mut s, settings)?;
    status(&mut s)
}

#[tauri::command]
pub async fn stop_rhythm(state: State<'_, AppState>) -> CmdResult<()> {
    release_all(&mut state.lock());
    Ok(())
}

#[tauri::command]
pub async fn rhythm_status(state: State<'_, AppState>) -> CmdResult<RhythmStatus> {
    status(&mut state.lock())
}

/// Restart the beat from bar 1 (keeps the patterns).
#[tauri::command]
pub async fn rhythm_restart(state: State<'_, AppState>) -> CmdResult<RhythmStatus> {
    let mut s = state.lock();
    if let (Some(live), Some(g)) = (s.rhythm.as_ref(), s.graph.as_ref()) {
        let frame = g.params().frame.load(Ordering::Relaxed);
        live.clock.start(frame, 0.0);
    }
    status(&mut s)
}

/// New seeds for every unlocked part ("regenerate only unlocked", F4.3).
#[tauri::command]
pub async fn rhythm_regenerate(settings: RhythmSettings) -> CmdResult<RhythmSettings> {
    let mut set = settings;
    let mut x = (project::now_ms_pub() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) as u32 | 1;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        (x % 1_000_000) + 1
    };
    if !set.drums.locked {
        set.drums.seed = next();
    }
    if !set.bass.locked {
        set.bass.seed = next();
    }
    if !set.ostinato.locked {
        set.ostinato.seed = next();
        set.ostinato.motif = None;
    }
    if !set.arp.locked {
        set.arp.seed = next();
    }
    for x in set.extras.iter_mut() {
        if !x.locked {
            x.seed = next();
        }
    }
    Ok(set)
}

/// Tempo candidates from the loaded take's notes (F4.3: align a free phrase to the
/// grid by a tempo map and an offset, never by moving the notes).
/// Names of the four parts, in slot order (stems are written under these names).
pub const PART_NAMES: [&str; 4] = ["drums", "bass", "ostinato", "arp"];

/// What an offline render of the section returns: the mix and, when asked, one stem per
/// part that was on (post-fader, pre-master), all the same length.
pub struct RhythmOffline {
    pub mix: AudioBuffer,
    pub stems: Vec<(String, AudioBuffer)>,
    pub seconds: f64,
}

/// Stem name of an extra part: its kind and 1-based position ("tapan-1").
pub fn extra_stem_name(part: &ExtraPart, index: usize) -> String {
    format!("{}-{}", part.kind.name(), index + 1)
}

/// Live handles plus (slot, source) pairs for an offline graph.
pub(crate) type OfflineParts = (RhythmLive, Vec<(usize, Box<dyn audio_engine::Source>)>);

/// The section's live handles and sources for an offline graph (shared with the song
/// export): the generators, the voices (preset or legacy) and a fresh clock at bar 1.
pub(crate) fn offline_parts(
    s: &Inner,
    settings: &RhythmSettings,
    ctx: &SectionContext,
) -> CmdResult<OfflineParts> {
    validate(settings)?;
    let (bpm, meter, root, pcs) = (ctx.bpm, ctx.meter, ctx.root, ctx.pcs);
    let clock = Arc::new(BeatClock::new(bpm as f32));
    let mut live = RhythmLive {
        clock: Arc::clone(&clock),
        voices: resolve_voices(s, &settings.instruments),
        drums: None,
        bass: None,
        ostinato: None,
        arp: None,
        settings: settings.clone(),
        keys: Default::default(),
        motif: None,
        drum_swap: None,
        bass_swap: None,
        ost_swap: None,
        arp_swap: None,
        extras: Vec::new(),
    };
    let mut sources: Vec<(usize, Box<dyn audio_engine::Source>)> = Vec::new();
    if settings.drums.on {
        let p = Arc::new(DrumParams::default());
        let pat = build_drums(settings, meter);
        sources.push((
            DRUMS_SLOT,
            Box::new(DrumSource::new(Arc::clone(&p), Arc::clone(&clock), pat)),
        ));
        live.drums = Some(p);
    }
    if settings.bass.on {
        let p = Arc::new(BassParams::default());
        let pat = build_bass(s, settings, meter, root, pcs);
        sources.push((
            BASS_SLOT,
            Box::new(BassSource::new(
                Arc::clone(&p),
                Arc::clone(&clock),
                pat,
                live.drums.clone(),
            )),
        ));
        live.bass = Some(p);
    }
    if settings.ostinato.on {
        let p = Arc::new(PatternParams::default());
        let (pat, _motif) = build_ostinato(s, settings, meter, pcs);
        sources.push((
            OSTINATO_SLOT,
            Box::new(PatternSynthSource::new(
                Arc::clone(&p),
                Arc::clone(&clock),
                pat,
            )),
        ));
        live.ostinato = Some(p);
    }
    if settings.arp.on {
        let p = Arc::new(PatternParams::default());
        let pat = build_arp(s, settings, meter, pcs);
        sources.push((
            ARP_SLOT,
            Box::new(PatternSynthSource::new(
                Arc::clone(&p),
                Arc::clone(&clock),
                pat,
            )),
        ));
        live.arp = Some(p);
    }
    for (i, part) in settings.extras.iter().enumerate().take(MAX_EXTRAS) {
        if !part.on {
            continue;
        }
        let slot = EXTRA_SLOT0 + i;
        let pat = build_extra(s, settings, part, meter, pcs);
        let params = extra_params_default(part.kind);
        let (src, swap) = make_extra(&params, &clock, &pat, false);
        sources.push((slot, src));
        live.extras.push(ExtraLive {
            id: part.id.clone(),
            kind: part.kind,
            slot,
            voice: resolve_voice(s, part.instrument.as_deref()),
            params,
            key: String::new(),
            swap,
        });
    }
    Ok((live, sources))
}

/// Render the section exactly as it would play: the same generators, the same sources on
/// the same kind of clock, `bars` bars at `rate`. Deterministic per seed (no humanize
/// drift at render time), so it serves as audio evidence and as a test oracle
/// (Program P1, R0). `s` supplies the project's scale / intonation for pitches; with no
/// project the pitches are tempered.
pub struct SectionContext<'a> {
    pub bpm: f64,
    pub meter: &'a Meter,
    pub root: u8,
    pub pcs: &'a [(u8, f64)],
}

pub fn render_section(
    s: &Inner,
    settings: &RhythmSettings,
    ctx: &SectionContext,
    bars: u32,
    rate: u32,
    want_stems: bool,
) -> CmdResult<RhythmOffline> {
    let (bpm, meter) = (ctx.bpm, ctx.meter);
    validate(settings)?;
    if !(8_000..=192_000).contains(&rate) {
        return Err(err("invalid_argument", "sample rate outside 8..192 kHz"));
    }
    let bars = bars.clamp(1, 256);
    let seconds = bars as f64 * meter.bar_beats() * 60.0 / bpm;
    let gp = Arc::new(GraphParams::default());
    gp.master.gain_db.set(0.0);
    let (live, sources) = offline_parts(s, settings, ctx)?;
    let clock = Arc::clone(&live.clock);
    // Levels and voices first, so the graph starts at its settled state (no ramp-in).
    let floor = |db: f32| if db <= -39.5 { -120.0 } else { db };
    apply_params(&gp.layers, &live, &floor);
    let mut g = Graph::new(Arc::clone(&gp), rate);
    for (slot, src) in sources {
        g.set_source(slot, src);
    }
    clock.start(0, 0.0);
    let ch = 2usize;
    let frames = (seconds * rate as f64).round() as usize;
    let mut out = vec![0.0f32; frames * ch];
    let mut stem_bufs: Vec<Vec<f32>> = if want_stems {
        (0..audio_engine::MAX_LAYERS)
            .map(|_| vec![0.0f32; frames * ch])
            .collect()
    } else {
        Vec::new()
    };
    let mut done = 0;
    while done < frames {
        let n = (frames - done).min(512);
        let slice = &mut out[done * ch..(done + n) * ch];
        if want_stems {
            let mut parts: Vec<Vec<f32>> = (0..audio_engine::MAX_LAYERS)
                .map(|_| vec![0.0f32; n * ch])
                .collect();
            g.render_with_stems(slice, ch, &mut parts);
            for (sb, part) in stem_bufs.iter_mut().zip(parts.iter()) {
                sb[done * ch..(done + n) * ch].copy_from_slice(part);
            }
        } else {
            g.render(slice, ch);
        }
        done += n;
    }
    let mix = AudioBuffer::new(rate, ch as u16, out);
    let mut stems = Vec::new();
    if want_stems {
        let on = [
            settings.drums.on,
            settings.bass.on,
            settings.ostinato.on,
            settings.arp.on,
        ];
        for (i, slot) in [DRUMS_SLOT, BASS_SLOT, OSTINATO_SLOT, ARP_SLOT]
            .into_iter()
            .enumerate()
        {
            if on[i] {
                stems.push((
                    PART_NAMES[i].to_string(),
                    AudioBuffer::new(rate, ch as u16, std::mem::take(&mut stem_bufs[slot])),
                ));
            }
        }
        for (i, part) in settings.extras.iter().enumerate().take(MAX_EXTRAS) {
            if part.on {
                stems.push((
                    extra_stem_name(part, i),
                    AudioBuffer::new(
                        rate,
                        ch as u16,
                        std::mem::take(&mut stem_bufs[EXTRA_SLOT0 + i]),
                    ),
                ));
            }
        }
    }
    Ok(RhythmOffline {
        mix,
        stems,
        seconds,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct RhythmRenderResult {
    pub path: String,
    pub stems: Vec<String>,
    pub seconds: f64,
    pub bars: u32,
    pub bpm: f64,
    pub sample_rate: u32,
    pub peak: f32,
}

/// Write the section as a WAV (and stems in a folder next to it): the project's tempo and
/// meter, the given settings, `bars` bars (default: the pattern length).
#[tauri::command]
pub async fn rhythm_render(
    state: State<'_, AppState>,
    path: String,
    settings: RhythmSettings,
    bars: Option<u32>,
    sample_rate: Option<u32>,
    stems: Option<bool>,
    format: Option<WavFormat>,
) -> CmdResult<RhythmRenderResult> {
    let rate = sample_rate.unwrap_or(48_000);
    let bars = bars.unwrap_or(settings.bars).clamp(1, 256);
    let (bpm, r) = {
        let s = state.lock();
        let (bpm, meter) = timing(&s)?;
        let (root, pcs) = chord_pcs(&s, &settings)?;
        (
            bpm,
            render_section(
                &s,
                &settings,
                &SectionContext {
                    bpm,
                    meter: &meter,
                    root,
                    pcs: &pcs,
                },
                bars,
                rate,
                stems.unwrap_or(true),
            )?,
        )
    };
    let mut p = std::path::PathBuf::from(&path);
    if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("wav")) {
        p.set_extension("wav");
    }
    let format = format.unwrap_or_default();
    write_wav(&p, &r.mix, format)?;
    let mut stem_paths = Vec::new();
    if !r.stems.is_empty() {
        let dir = p.with_extension("");
        std::fs::create_dir_all(&dir).map_err(|e| err("io", e.to_string()))?;
        for (name, buf) in &r.stems {
            let sp = dir.join(format!("{name}.wav"));
            write_wav(&sp, buf, format)?;
            stem_paths.push(sp.display().to_string());
        }
    }
    let peak = r.mix.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    Ok(RhythmRenderResult {
        path: p.display().to_string(),
        stems: stem_paths,
        seconds: r.seconds,
        bars,
        bpm,
        sample_rate: rate,
        peak,
    })
}

#[tauri::command]
pub async fn estimate_take_tempo(
    state: State<'_, AppState>,
    bpm_low: Option<f64>,
    bpm_high: Option<f64>,
) -> CmdResult<Vec<TempoCandidate>> {
    let s = state.lock();
    let notes = crate::analysis_cmds::current_notes(&s)?;
    Ok(music_core::estimate_tempo(
        &notes,
        bpm_low.unwrap_or(60.0),
        bpm_high.unwrap_or(200.0),
        4,
    ))
}

#[derive(Debug, Clone, Serialize)]
pub struct RhythmTakesResult {
    pub project: crate::project_cmds::ProjectView,
    pub created: Vec<String>,
}

fn beats_to_notes(notes: &[music_core::BeatNote], bpm: f64) -> Vec<music_core::Note> {
    let spb = 60.0 / bpm;
    notes
        .iter()
        .map(|n| music_core::Note {
            start: n.start_beat * spb,
            end: (n.start_beat + n.length_beats) * spb,
            midi: n.midi,
            cents: n.cents,
            confidence: 1.0,
            velocity: n.velocity,
            bend: Vec::new(),
        })
        .collect()
}

/// General MIDI percussion numbers for the synthesized kit.
fn gm_drum(d: Drum) -> i32 {
    match d {
        Drum::Kick => 36,
        Drum::Snare => 38,
        Drum::Ghost => 38,
        Drum::HatClosed => 42,
        Drum::HatOpen => 46,
        Drum::Shaker => 70,
    }
}

/// Write the enabled parts as MIDI takes (editable in the player, placeable as clips
/// in "My studio", exportable as .mid) — the Phase 4 "editable bass/drums" exit.
/// Drums go on channel 10 with GM numbers; pitched parts carry their cents as bends.
#[tauri::command]
pub async fn rhythm_to_takes(
    state: State<'_, AppState>,
    settings: RhythmSettings,
) -> CmdResult<RhythmTakesResult> {
    validate(&settings)?;
    let mut s = state.lock();
    if s.mode_busy() {
        return Err(err("busy", "Stop recording or playback first"));
    }
    let (bpm, meter) = timing(&s)?;
    let (root, pcs) = chord_pcs(&s, &settings)?;
    let ts = (meter.numerator as u8, meter.unit as u8);
    let mut parts: Vec<(&str, Vec<music_core::Note>, u8)> = Vec::new();
    if settings.drums.on {
        let d = &settings.drums;
        let pat = generate_groove(
            &GrooveParams {
                style: d.style,
                complexity: d.complexity,
                humanize: d.humanize,
                swing: d.swing,
                fill_every_bars: d.fill_every_bars,
                seed: d.seed,
                bars: settings.bars.clamp(1, 64),
            },
            &meter,
        );
        let spb = 60.0 / bpm;
        let notes: Vec<music_core::Note> = pat
            .hits
            .iter()
            .map(|h| music_core::Note {
                start: h.start_beat * spb,
                end: h.start_beat * spb + 0.1,
                midi: gm_drum(h.drum),
                cents: 0.0,
                confidence: 1.0,
                velocity: if h.drum == Drum::Ghost {
                    h.velocity * 0.5
                } else {
                    h.velocity
                },
                bend: Vec::new(),
            })
            .collect();
        parts.push(("drums", notes, 9));
    }
    if settings.bass.on {
        let spans = static_spans(root, &pcs, settings.bars.clamp(1, 64), &meter);
        let scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
        let b = &settings.bass;
        let notes = generate_bass(
            &spans,
            scale.as_ref(),
            &BassGenParams {
                density: b.density,
                syncopation: b.syncopation,
                passing: b.passing,
                register: (b.register_low, b.register_high),
                swing: 0.0,
                seed: b.seed,
            },
            &meter,
        );
        parts.push(("bass", beats_to_notes(&notes, bpm), 0));
    }
    if settings.ostinato.on {
        let o = &settings.ostinato;
        let tones = tones_in_register(&pcs, o.register_low, o.register_high);
        let params = OstinatoParams {
            steps: o.steps,
            step_beats: o.step_beats,
            density: o.density,
            probability: o.probability,
            variation_every_bars: o.variation_every_bars,
            swing: o.swing,
            seed: o.seed,
            bars: settings.bars.clamp(1, 64),
        };
        let notes = generate_ostinato(&tones, &params, &meter, o.motif.as_ref());
        parts.push(("ostinato", beats_to_notes(&notes, bpm), 0));
    }
    if settings.arp.on {
        let a = &settings.arp;
        let chord = tones_in_register(&pcs, a.register_low, a.register_low + 11);
        let notes = generate_arpeggio(
            &chord,
            &ArpParams {
                direction: a.direction,
                octaves: a.octaves,
                step_beats: a.step_beats,
                gate: a.gate,
                density: a.density,
                swing: a.swing,
                seed: a.seed,
                bars: settings.bars.clamp(1, 64),
            },
            &meter,
        );
        parts.push(("arp", beats_to_notes(&notes, bpm), 0));
    }
    if parts.is_empty() {
        return Err(err("empty", "No part is switched on"));
    }
    let mut created = Vec::new();
    let tmp_dir = std::env::temp_dir().join("ethnoflow-rhythm");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| err("io", e.to_string()))?;
    let stamp = project::now_ms_pub();
    for (name, notes, channel) in parts {
        if notes.is_empty() {
            continue;
        }
        let bytes = music_core::write_smf(
            &notes,
            &music_core::SmfOptions {
                tempo_bpm: bpm,
                time_signature: Some(ts),
                channel,
                ..music_core::SmfOptions::default()
            },
        );
        let path = tmp_dir.join(format!("{name}-{stamp}.mid"));
        std::fs::write(&path, &bytes).map_err(|e| err("io", e.to_string()))?;
        let pv = crate::midi_cmds::import_midi(&mut s, &path);
        let _ = std::fs::remove_file(&path);
        let pv = pv?;
        if let Some(t) = pv.takes.iter().max_by_key(|t| t.number) {
            let label = match name {
                "drums" => "Drums",
                "bass" => "Bass",
                "ostinato" => "Ostinato",
                _ => "Arpeggio",
            };
            if let Some(p) = s.project.as_mut() {
                let _ = p.rename_take(&t.id, Some(label));
            }
            created.push(t.id.clone());
        }
    }
    Ok(RhythmTakesResult {
        project: crate::project_cmds::project_view(&s)?,
        created,
    })
}

#[cfg(test)]
mod render_tests {
    use super::*;

    fn settings() -> RhythmSettings {
        serde_json::from_value(serde_json::json!({
            "drums": {"on": true, "style": "liquid", "complexity": 0.4, "humanize": 0.3, "swing": 0.0, "fill_every_bars": 4, "seed": 1, "level_db": -10.0, "kick_hz": 50.0, "decay": 0.5, "brightness": 0.5, "locked": false},
            "bass": {"on": true, "voice": 0, "density": 0.4, "syncopation": 0.3, "passing": 0.3, "register_low": 31, "register_high": 43, "seed": 1, "level_db": -12.0, "cutoff_hz": 400.0, "resonance": 0.2, "drive": 0.3, "glide_s": 0.08, "sidechain": 0.6, "wobble_div": 0, "wobble_depth": 0.5, "warmth": 0.5, "locked": false},
            "ostinato": {"on": true, "steps": 8, "step_beats": 0.5, "density": 0.6, "probability": 1.0, "variation_every_bars": 4, "swing": 0.0, "seed": 1, "level_db": -16.0, "register_low": 55, "register_high": 79, "timbre": 2, "release_s": 0.15, "locked": false, "motif": null},
            "arp": {"on": false, "direction": "up", "octaves": 1, "step_beats": 0.25, "gate": 0.6, "density": 1.0, "swing": 0.0, "seed": 1, "level_db": -18.0, "register_low": 60, "timbre": 0, "release_s": 0.12, "locked": false},
            "bars": 2, "chord": null, "root_midi": 50, "intonation": "tempered"
        }))
        .unwrap()
    }

    #[test]
    fn extra_parts_render_their_own_stems_and_follow_presets() {
        let s = Inner::default();
        let meter = Meter {
            numerator: 7,
            unit: 8,
            grouping: vec![2, 2, 3],
        };
        let pcs = vec![(2u8, 0.0), (5u8, 0.0), (9u8, 0.0)];
        let ctx = SectionContext {
            bpm: 120.0,
            meter: &meter,
            root: 2,
            pcs: &pcs,
        };
        let mut set = settings();
        set.drums.on = false;
        set.bass.on = false;
        set.ostinato.on = false;
        set.arp.on = false;
        let extra = |id: &str, kind: ExtraKind| ExtraPart {
            id: id.into(),
            kind,
            on: true,
            level_db: -10.0,
            instrument: None,
            density: 0.5,
            humanize: 0.2,
            fill_every_bars: 2,
            gap: 0.1,
            bars_per_chord: 1,
            step_beats: 0.5,
            phrase_bars: 2,
            leaps: 0.2,
            register_low: 50,
            register_high: 84,
            seed: 3,
            locked: false,
        };
        set.extras = vec![
            extra("x1", ExtraKind::Tapan),
            extra("x2", ExtraKind::ChordPad),
            extra("x3", ExtraKind::Lead),
        ];
        validate(&set).unwrap();
        let r = render_section(&s, &set, &ctx, 2, 48_000, true).unwrap();
        let names: Vec<&str> = r.stems.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, vec!["tapan-1", "chordpad-2", "lead-3"]);
        let peak = |b: &AudioBuffer| b.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        for (name, stem) in &r.stems {
            assert!(peak(stem) > 0.02, "{name} audible: {}", peak(stem));
        }
        assert!(peak(&r.mix) > 0.05 && peak(&r.mix) <= 1.0);
        // A preset changes the sound: the deep tapan differs from the default one.
        let mut with_preset = set.clone();
        with_preset.extras[0].instrument = Some("factory:tapan.deep".into());
        let r2 = render_section(&s, &with_preset, &ctx, 2, 48_000, true).unwrap();
        assert_ne!(r.stems[0].1.samples, r2.stems[0].1.samples);
        // Too many or duplicate ids are refused.
        let mut bad = set.clone();
        bad.extras.push(extra("x1", ExtraKind::Lead));
        assert!(validate(&bad).is_err());
    }

    #[test]
    fn offline_render_with_instrument_presets_is_audible_and_differs_from_legacy() {
        let s = Inner::default();
        let meter = Meter::new(7, 8);
        let pcs = vec![(2u8, 0.0), (5u8, 0.0), (9u8, 0.0)];
        let ctx = SectionContext {
            bpm: 120.0,
            meter: &meter,
            root: 2,
            pcs: &pcs,
        };
        let legacy = render_section(&s, &settings(), &ctx, 2, 48_000, true).unwrap();
        let mut set = settings();
        set.instruments = InstrumentRefs {
            drums: Some("factory:drums.ethno".into()),
            bass: Some("factory:bass.wood".into()),
            ostinato: Some("factory:synth.glass".into()),
            arp: None,
        };
        let preset = render_section(&s, &set, &ctx, 2, 48_000, true).unwrap();
        let peak = |b: &AudioBuffer| b.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(
            peak(&preset.mix) > 0.05,
            "preset voices audible: {}",
            peak(&preset.mix)
        );
        for (name, stem) in &preset.stems {
            assert!(peak(stem) > 0.02, "stem {name} audible: {}", peak(stem));
        }
        assert_ne!(
            legacy.mix.samples, preset.mix.samples,
            "presets change the sound"
        );
    }

    #[test]
    fn offline_render_is_deterministic_audible_and_stems_sum_to_the_mix() {
        let s = Inner::default();
        let set = settings();
        let meter = Meter::new(4, 4);
        let pcs = vec![(2u8, 0.0), (5u8, 0.0), (9u8, 0.0)];
        let a = render_section(
            &s,
            &set,
            &SectionContext {
                bpm: 170.0,
                meter: &meter,
                root: 2,
                pcs: &pcs,
            },
            2,
            48_000,
            true,
        )
        .unwrap();
        let b = render_section(
            &s,
            &set,
            &SectionContext {
                bpm: 170.0,
                meter: &meter,
                root: 2,
                pcs: &pcs,
            },
            2,
            48_000,
            true,
        )
        .unwrap();
        assert_eq!(
            a.mix.samples, b.mix.samples,
            "same seed → identical samples"
        );
        assert!((a.seconds - 2.0 * 4.0 * 60.0 / 170.0).abs() < 1e-9);
        let peak = a.mix.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
        assert!(peak > 0.05 && peak <= 1.0, "audible, not clipping: {peak}");
        assert_eq!(
            a.stems.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(),
            vec!["drums", "bass", "ostinato"]
        );
        // Stems are pre-master; with master at 0 dB and no reverb their sum is the mix
        // (within the limiter's reach).
        let n = a.mix.samples.len();
        let mut max_diff = 0.0f32;
        for i in (n / 2)..n {
            let sum: f32 = a.stems.iter().map(|(_, b)| b.samples[i]).sum();
            max_diff = max_diff.max((sum - a.mix.samples[i]).abs());
        }
        assert!(max_diff < 0.05, "stems sum to the mix: {max_diff}");
        // A different seed changes the audio.
        let mut set2 = set.clone();
        set2.drums.seed = 7;
        let c = render_section(
            &s,
            &set2,
            &SectionContext {
                bpm: 170.0,
                meter: &meter,
                root: 2,
                pcs: &pcs,
            },
            2,
            48_000,
            false,
        )
        .unwrap();
        assert_ne!(a.mix.samples, c.mix.samples);
    }
}
