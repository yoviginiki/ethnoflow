//! Rhythm section (Phase 4: F4.1 drums, F4.2 bass, F4.5 ostinato, F4.6 arpeggio) on
//! graph slots 12..16, driven by one shared beat clock so everything lines up with
//! the project's tempo and meter. Patterns come from `music_core` (pure, seeded);
//! a pattern-affecting change rebuilds that one slot in place (the clock keeps the
//! position), a level/voice change goes through atomics. Tonal parts follow the
//! chord the UI hands over (an armed harmony proposal) or the project's mode.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::{
    BassNoteEvent, BassParams, BassPatternData, BassSource, BeatClock, DrumEvent, DrumKind,
    DrumParams, DrumPatternData, DrumSource, GraphCmd, NotePatternData, PatternNote, PatternParams,
    PatternSynthSource,
};
use music_core::{
    generate_arpeggio, generate_bass, generate_groove, generate_ostinato, static_spans,
    tones_in_register, ArpDirection, ArpParams, BassParams as BassGenParams, Drum, GrooveParams,
    GrooveStyle, Meter, Motif, OstinatoParams, TempoCandidate,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::graph_cmds::{ensure_graph, note_hz, Intonation};
use crate::pad_cmds::PadNote;
use crate::state::{AppState, Inner};

pub const DRUMS_SLOT: usize = 12;
pub const BASS_SLOT: usize = 13;
pub const OSTINATO_SLOT: usize = 14;
pub const ARP_SLOT: usize = 15;

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
    /// Root when the project has no scale (the drone's root).
    #[serde(default)]
    pub root_midi: Option<u8>,
    pub intonation: Intonation,
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
}

pub struct RhythmLive {
    pub clock: Arc<BeatClock>,
    pub drums: Option<Arc<DrumParams>>,
    pub bass: Option<Arc<BassParams>>,
    pub ostinato: Option<Arc<PatternParams>>,
    pub arp: Option<Arc<PatternParams>>,
    pub settings: RhythmSettings,
    /// Build keys per slot (pattern-affecting inputs).
    pub keys: [String; 4],
    pub motif: Option<Motif>,
}

fn timing(s: &Inner) -> CmdResult<(f64, Meter)> {
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
fn chord_pcs(s: &Inner, set: &RhythmSettings) -> CmdResult<(u8, Vec<(u8, f64)>)> {
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

fn apply_live(s: &Inner, live: &RhythmLive) {
    let Some(g) = s.graph.as_ref() else { return };
    let set = &live.settings;
    let layer = |slot: usize| &g.params().layers[slot];
    if let Some(p) = live.drums.as_ref() {
        p.kick_hz.set(set.drums.kick_hz);
        p.decay.set(set.drums.decay);
        p.brightness.set(set.drums.brightness);
        layer(DRUMS_SLOT).gain_db.set(set.drums.level_db);
    }
    if let Some(p) = live.bass.as_ref() {
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
        layer(BASS_SLOT).gain_db.set(b.level_db);
    }
    if let Some(p) = live.ostinato.as_ref() {
        p.timbre
            .store(set.ostinato.timbre.min(2), Ordering::Relaxed);
        p.release_s.set(set.ostinato.release_s);
        layer(OSTINATO_SLOT).gain_db.set(set.ostinato.level_db);
    }
    if let Some(p) = live.arp.as_ref() {
        p.timbre.store(set.arp.timbre.min(2), Ordering::Relaxed);
        p.release_s.set(set.arp.release_s);
        layer(ARP_SLOT).gain_db.set(set.arp.level_db);
    }
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
    let mut live = RhythmLive {
        clock: Arc::clone(&clock),
        drums: live_prev.drums.clone(),
        bass: live_prev.bass.clone(),
        ostinato: live_prev.ostinato.clone(),
        arp: live_prev.arp.clone(),
        settings: set.clone(),
        keys: live_prev.keys.clone(),
        motif: live_prev.motif.clone(),
    };
    // Drums.
    if set.drums.on {
        if live.drums.is_none() || live.keys[0] != new_keys[0] {
            let pat = build_drums(&set, &meter);
            let p = live
                .drums
                .clone()
                .unwrap_or_else(|| Arc::new(DrumParams::default()));
            let g = s.graph.as_mut().expect("graph");
            g.params().layers[DRUMS_SLOT].gain_db.set(-60.0);
            g.send(GraphCmd::SetSource {
                slot: DRUMS_SLOT,
                source: Box::new(DrumSource::new(Arc::clone(&p), Arc::clone(&clock), pat)),
            });
            live.drums = Some(p);
            live.keys[0] = new_keys[0].clone();
        }
    } else if live.drums.is_some() {
        clear(s, DRUMS_SLOT);
        live.drums = None;
        live.keys[0].clear();
    }
    // Bass (ducked by the drums when they run).
    if set.bass.on {
        let drums_changed = live_prev.drums.is_some() != live.drums.is_some();
        if live.bass.is_none() || live.keys[1] != new_keys[1] || drums_changed {
            let pat = build_bass(s, &set, &meter, root, &pcs);
            let p = live
                .bass
                .clone()
                .unwrap_or_else(|| Arc::new(BassParams::default()));
            let drums = live.drums.clone();
            let g = s.graph.as_mut().expect("graph");
            g.params().layers[BASS_SLOT].gain_db.set(-60.0);
            g.send(GraphCmd::SetSource {
                slot: BASS_SLOT,
                source: Box::new(BassSource::new(
                    Arc::clone(&p),
                    Arc::clone(&clock),
                    pat,
                    drums,
                )),
            });
            live.bass = Some(p);
            live.keys[1] = new_keys[1].clone();
        }
    } else if live.bass.is_some() {
        clear(s, BASS_SLOT);
        live.bass = None;
        live.keys[1].clear();
    }
    // Ostinato.
    if set.ostinato.on {
        if live.ostinato.is_none() || live.keys[2] != new_keys[2] {
            let (pat, motif) = build_ostinato(s, &set, &meter, &pcs);
            let p = live
                .ostinato
                .clone()
                .unwrap_or_else(|| Arc::new(PatternParams::default()));
            let g = s.graph.as_mut().expect("graph");
            g.params().layers[OSTINATO_SLOT].gain_db.set(-60.0);
            g.send(GraphCmd::SetSource {
                slot: OSTINATO_SLOT,
                source: Box::new(PatternSynthSource::new(
                    Arc::clone(&p),
                    Arc::clone(&clock),
                    pat,
                )),
            });
            live.ostinato = Some(p);
            live.keys[2] = new_keys[2].clone();
            live.motif = Some(motif);
        }
    } else if live.ostinato.is_some() {
        clear(s, OSTINATO_SLOT);
        live.ostinato = None;
        live.keys[2].clear();
    }
    // Arpeggio.
    if set.arp.on {
        if live.arp.is_none() || live.keys[3] != new_keys[3] {
            let pat = build_arp(s, &set, &meter, &pcs);
            let p = live
                .arp
                .clone()
                .unwrap_or_else(|| Arc::new(PatternParams::default()));
            let g = s.graph.as_mut().expect("graph");
            g.params().layers[ARP_SLOT].gain_db.set(-60.0);
            g.send(GraphCmd::SetSource {
                slot: ARP_SLOT,
                source: Box::new(PatternSynthSource::new(
                    Arc::clone(&p),
                    Arc::clone(&clock),
                    pat,
                )),
            });
            live.arp = Some(p);
            live.keys[3] = new_keys[3].clone();
        }
    } else if live.arp.is_some() {
        clear(s, ARP_SLOT);
        live.arp = None;
        live.keys[3].clear();
    }
    if let Some(g) = s.graph.as_ref() {
        for slot in [DRUMS_SLOT, BASS_SLOT, OSTINATO_SLOT, ARP_SLOT] {
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
            for slot in [DRUMS_SLOT, BASS_SLOT, OSTINATO_SLOT, ARP_SLOT] {
                g.params().layers[slot].gain_db.set(-60.0);
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
            for slot in [DRUMS_SLOT, BASS_SLOT, OSTINATO_SLOT, ARP_SLOT] {
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
            drums: None,
            bass: None,
            ostinato: None,
            arp: None,
            settings: settings.clone(),
            keys: Default::default(),
            motif: None,
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
    Ok(set)
}

/// Tempo candidates from the loaded take's notes (F4.3: align a free phrase to the
/// grid by a tempo map and an offset, never by moving the notes).
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
