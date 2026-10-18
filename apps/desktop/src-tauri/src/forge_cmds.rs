//! Instrument Forge (Program P1, R1/R2): instruments as presets with an identity, a
//! browser over factory + user presets, an audition path on the live graph (slot 3) that
//! plays a short pattern through the exact engine voice while the designer's knobs move,
//! and an offline render of the same for evidence. User presets are library entries of
//! kind `instrument` (search, tags, notes come for free); factory presets live in
//! `project::forge` and are read-only ("Edit copy" forks them).

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::{
    write_wav, AudioBuffer, BassNoteEvent, BassParams, BassPatternData, BassSource, BeatClock,
    ChordEvent, ChordPadParams, ChordPadSource, ChordPatternData, DrumEvent, DrumKind, DrumParams,
    DrumPatternData, DrumSource, Graph, GraphCmd, GraphParams, LeadParams, LeadSource,
    NotePatternData, PatternNote, PatternParams, PatternSynthSource, TapanParams, TapanSource,
    WavFormat,
};
use project::{factory_instruments, InstrumentPreset, InstrumentVoice};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::graph_cmds::ensure_graph;
use crate::library_cmds::{self, NewEntry};
use crate::state::{AppState, Inner};

/// Graph slot used for auditioning an instrument alone (slot 3 was reserved).
pub const AUDITION_SLOT: usize = 3;
const LIBRARY_KIND: &str = "instrument";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentSummary {
    pub id: String,
    pub name: String,
    pub category: String,
    pub tags: Vec<String>,
    pub factory: bool,
    pub parent_id: Option<String>,
    pub updated_ms: i64,
}

impl From<&InstrumentPreset> for InstrumentSummary {
    fn from(p: &InstrumentPreset) -> Self {
        Self {
            id: p.id.clone(),
            name: p.name.clone(),
            category: p.category.clone(),
            tags: p.tags.clone(),
            factory: p.factory,
            parent_id: p.parent_id.clone(),
            updated_ms: p.updated_ms,
        }
    }
}

fn library_dir(s: &Inner) -> CmdResult<std::path::PathBuf> {
    s.presets_dir
        .as_ref()
        .map(|d| d.parent().unwrap_or(d).join("library"))
        .ok_or_else(|| err("no_config", "config folder not set"))
}

fn user_presets(s: &Inner) -> Vec<InstrumentPreset> {
    let Ok(dir) = library_dir(s) else {
        return Vec::new();
    };
    library_cmds::list_in(
        &dir,
        Some(LIBRARY_KIND),
        None,
        "",
        library_cmds::SortBy::Name,
    )
    .into_iter()
    .filter_map(|e| {
        let full = library_cmds::get_in(&dir, &e.id).ok()?;
        let mut p: InstrumentPreset = serde_json::from_value(full.payload).ok()?;
        p = p.migrate();
        p.id = full.id;
        p.name = full.name;
        p.tags = full.tags;
        p.notes = full.notes;
        p.updated_ms = full.updated_ms;
        p.factory = false;
        Some(p)
    })
    .collect()
}

/// Every preset the app knows: factory first, then the user's.
pub fn all_presets(s: &Inner) -> Vec<InstrumentPreset> {
    let mut v = factory_instruments();
    v.extend(user_presets(s));
    v
}

pub fn find_preset(s: &Inner, id: &str) -> CmdResult<InstrumentPreset> {
    if let Some(p) = factory_instruments().into_iter().find(|p| p.id == id) {
        return Ok(p);
    }
    let dir = library_dir(s)?;
    let full = library_cmds::get_in(&dir, id)?;
    let mut p: InstrumentPreset =
        serde_json::from_value(full.payload).map_err(|e| err("invalid_preset", e.to_string()))?;
    p = p.migrate();
    p.id = full.id;
    p.name = full.name;
    p.tags = full.tags;
    p.notes = full.notes;
    p.updated_ms = full.updated_ms;
    p.factory = false;
    Ok(p)
}

/// The voice an instrument id resolves to, if the id is known (missing → None, so the
/// part falls back to its own legacy settings instead of failing).
pub fn resolve_voice(s: &Inner, id: Option<&str>) -> Option<InstrumentVoice> {
    let id = id?;
    find_preset(s, id).ok().map(|p| p.voice)
}

#[tauri::command]
pub async fn instrument_list(
    state: State<'_, AppState>,
    category: Option<String>,
    query: Option<String>,
) -> CmdResult<Vec<InstrumentSummary>> {
    let s = state.lock();
    let q = query.unwrap_or_default().trim().to_lowercase();
    Ok(all_presets(&s)
        .iter()
        .filter(|p| category.as_deref().is_none_or(|c| p.category == c))
        .filter(|p| {
            q.is_empty()
                || p.name.to_lowercase().contains(&q)
                || p.tags.iter().any(|t| t.contains(&q))
                || p.notes.to_lowercase().contains(&q)
        })
        .map(InstrumentSummary::from)
        .collect())
}

#[tauri::command]
pub async fn instrument_get(state: State<'_, AppState>, id: String) -> CmdResult<InstrumentPreset> {
    let s = state.lock();
    find_preset(&s, &id)
}

/// Save a new user preset (Save As). The id is assigned by the library.
#[tauri::command]
pub async fn instrument_save(
    state: State<'_, AppState>,
    preset: InstrumentPreset,
) -> CmdResult<InstrumentPreset> {
    let s = state.lock();
    let mut p = preset.migrate();
    p.factory = false;
    p.author = "user".into();
    p.category = p.voice.category().into();
    p.validate()
        .map_err(|e| err("invalid_argument", e.to_string()))?;
    let dir = library_dir(&s)?;
    let project_name = s
        .project
        .as_ref()
        .map(|x| x.manifest().name.clone())
        .unwrap_or_default();
    let entry = library_cmds::save_in(
        &dir,
        NewEntry {
            kind: LIBRARY_KIND.into(),
            name: p.name.clone(),
            tags: p.tags.clone(),
            notes: p.notes.clone(),
            payload: serde_json::to_value(&p).map_err(|e| err("io", e.to_string()))?,
        },
        &project_name,
    )?;
    p.id = entry.id;
    p.created_ms = entry.created_ms;
    p.updated_ms = entry.updated_ms;
    // Store the final id inside the document too (lineage of later forks).
    library_cmds::replace_payload_in(
        &dir,
        &p.id,
        serde_json::to_value(&p).map_err(|e| err("io", e.to_string()))?,
    )?;
    Ok(p)
}

/// Overwrite an existing user preset (factory presets are read-only: fork them).
#[tauri::command]
pub async fn instrument_update(
    state: State<'_, AppState>,
    preset: InstrumentPreset,
) -> CmdResult<InstrumentPreset> {
    let s = state.lock();
    if preset.id.starts_with("factory:") {
        return Err(err(
            "read_only",
            "Factory presets cannot be changed: save a copy",
        ));
    }
    let mut p = preset.migrate();
    p.factory = false;
    p.category = p.voice.category().into();
    p.validate()
        .map_err(|e| err("invalid_argument", e.to_string()))?;
    let dir = library_dir(&s)?;
    library_cmds::update_in(&dir, &p.id, &p.name, &p.tags, &p.notes)?;
    let e = library_cmds::replace_payload_in(
        &dir,
        &p.id,
        serde_json::to_value(&p).map_err(|e| err("io", e.to_string()))?,
    )?;
    p.updated_ms = e.updated_ms;
    Ok(p)
}

#[tauri::command]
pub async fn instrument_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let s = state.lock();
    if id.starts_with("factory:") {
        return Err(err("read_only", "Factory presets cannot be deleted"));
    }
    let dir = library_dir(&s)?;
    library_cmds::delete_in(&dir, &id)
}

// ---------- audition ----------

/// Live parameter handles of the auditioned voice.
pub enum AuditionParams {
    Drums(Arc<DrumParams>),
    Bass(Arc<BassParams>),
    Synth(Arc<PatternParams>),
    Tapan(Arc<TapanParams>),
    ChordPad(Arc<ChordPadParams>),
    Lead(Arc<LeadParams>),
}

pub struct AuditionLive {
    pub params: AuditionParams,
    pub category: &'static str,
}

fn midi_hz(midi: f64) -> f32 {
    (440.0 * 2f64.powf((midi - 69.0) / 12.0)) as f32
}

/// The short pattern each category auditions with (two bars, loops at the tempo given).
fn audition_pattern(voice: &InstrumentVoice, midi: f64) -> AuditionPattern {
    match voice {
        InstrumentVoice::Drums { .. } => AuditionPattern::Drums(Arc::new(DrumPatternData::new(
            vec![
                DrumEvent {
                    beat: 0.0,
                    kind: DrumKind::Kick,
                    velocity: 1.0,
                },
                DrumEvent {
                    beat: 0.0,
                    kind: DrumKind::HatClosed,
                    velocity: 0.6,
                },
                DrumEvent {
                    beat: 0.5,
                    kind: DrumKind::HatClosed,
                    velocity: 0.45,
                },
                DrumEvent {
                    beat: 1.0,
                    kind: DrumKind::Snare,
                    velocity: 0.95,
                },
                DrumEvent {
                    beat: 1.0,
                    kind: DrumKind::HatClosed,
                    velocity: 0.6,
                },
                DrumEvent {
                    beat: 1.5,
                    kind: DrumKind::HatOpen,
                    velocity: 0.5,
                },
                DrumEvent {
                    beat: 1.75,
                    kind: DrumKind::Ghost,
                    velocity: 0.5,
                },
                DrumEvent {
                    beat: 2.0,
                    kind: DrumKind::HatClosed,
                    velocity: 0.6,
                },
                DrumEvent {
                    beat: 2.5,
                    kind: DrumKind::Kick,
                    velocity: 0.9,
                },
                DrumEvent {
                    beat: 2.5,
                    kind: DrumKind::HatClosed,
                    velocity: 0.45,
                },
                DrumEvent {
                    beat: 3.0,
                    kind: DrumKind::Snare,
                    velocity: 1.0,
                },
                DrumEvent {
                    beat: 3.0,
                    kind: DrumKind::HatClosed,
                    velocity: 0.6,
                },
                DrumEvent {
                    beat: 3.5,
                    kind: DrumKind::Shaker,
                    velocity: 0.7,
                },
                DrumEvent {
                    beat: 3.75,
                    kind: DrumKind::Shaker,
                    velocity: 0.5,
                },
                DrumEvent {
                    beat: 4.0,
                    kind: DrumKind::Kick,
                    velocity: 1.0,
                },
                DrumEvent {
                    beat: 5.0,
                    kind: DrumKind::Snare,
                    velocity: 0.95,
                },
                DrumEvent {
                    beat: 6.5,
                    kind: DrumKind::Kick,
                    velocity: 0.9,
                },
                DrumEvent {
                    beat: 7.0,
                    kind: DrumKind::Snare,
                    velocity: 1.0,
                },
                DrumEvent {
                    beat: 7.5,
                    kind: DrumKind::Ghost,
                    velocity: 0.45,
                },
            ],
            8.0,
        ))),
        InstrumentVoice::Bass { .. } => AuditionPattern::Bass(Arc::new(BassPatternData::new(
            vec![
                BassNoteEvent {
                    beat: 0.0,
                    length_beats: 1.5,
                    hz: midi_hz(midi),
                    velocity: 1.0,
                    glide: false,
                },
                BassNoteEvent {
                    beat: 2.0,
                    length_beats: 0.75,
                    hz: midi_hz(midi + 7.0),
                    velocity: 0.8,
                    glide: true,
                },
                BassNoteEvent {
                    beat: 3.0,
                    length_beats: 0.5,
                    hz: midi_hz(midi + 5.0),
                    velocity: 0.7,
                    glide: false,
                },
                BassNoteEvent {
                    beat: 4.0,
                    length_beats: 3.0,
                    hz: midi_hz(midi),
                    velocity: 1.0,
                    glide: true,
                },
            ],
            8.0,
        ))),
        InstrumentVoice::Synth { .. } => {
            let steps = [0.0, 4.0, 7.0, 12.0, 7.0, 4.0, 0.0, -5.0];
            let events = steps
                .iter()
                .enumerate()
                .map(|(i, st)| PatternNote {
                    beat: i as f64 * 0.5,
                    length_beats: 0.4,
                    hz: midi_hz(midi + 24.0 + st),
                    velocity: if i % 4 == 0 { 1.0 } else { 0.7 },
                })
                .collect();
            AuditionPattern::Synth(Arc::new(NotePatternData::new(events, 4.0)))
        }
        InstrumentVoice::Tapan { .. } => {
            // Two bars of 7/8 (2+2+3): the drum's home ground.
            let meter = music_core::Meter {
                numerator: 7,
                unit: 8,
                grouping: vec![2, 2, 3],
            };
            let hits = music_core::generate_tapan(
                &meter,
                &music_core::TapanPatternParams {
                    density: 0.5,
                    humanize: 0.15,
                    fill_every_bars: 2,
                    seed: 1,
                    bars: 2,
                },
            );
            AuditionPattern::Tapan(tapan_pattern(&hits, 2.0 * meter.bar_beats()))
        }
        InstrumentVoice::ChordPad { .. } => {
            // Root, fifth, octave, ninth (no third: the mode stays the user's), 4 beats
            // on, 2 off, then a fifth higher.
            let chord = |base: f64| {
                vec![
                    midi_hz(base),
                    midi_hz(base + 7.0),
                    midi_hz(base + 12.0),
                    midi_hz(base + 14.0),
                ]
            };
            AuditionPattern::Chord(Arc::new(ChordPatternData::new(
                vec![
                    ChordEvent {
                        beat: 0.0,
                        length_beats: 4.0,
                        hz: chord(midi),
                        velocity: 0.9,
                    },
                    ChordEvent {
                        beat: 6.0,
                        length_beats: 4.0,
                        hz: chord(midi + 5.0),
                        velocity: 0.8,
                    },
                ],
                12.0,
            )))
        }
        InstrumentVoice::Lead { .. } => {
            // A short phrase without thirds (root, 2nd, 4th, 5th, 6th) with one long note
            // for the vibrato to show and two legato pairs for the glide.
            let steps: [(f64, f64, f64); 9] = [
                (0.0, 0.45, 0.0),
                (0.5, 0.45, 2.0),
                (1.0, 0.45, 5.0),
                (1.5, 1.4, 7.0),
                (3.0, 0.5, 9.0),
                (3.5, 0.45, 7.0),
                (4.0, 0.95, 5.0),
                (5.0, 0.45, 2.0),
                (5.5, 2.4, 0.0),
            ];
            let events = steps
                .iter()
                .enumerate()
                .map(|(i, (beat, len, st))| PatternNote {
                    beat: *beat,
                    length_beats: *len,
                    hz: midi_hz(midi + st),
                    velocity: if i % 3 == 0 { 1.0 } else { 0.75 },
                })
                .collect();
            AuditionPattern::Lead(Arc::new(NotePatternData::new(events, 8.0)))
        }
    }
}

enum AuditionPattern {
    Drums(Arc<DrumPatternData>),
    Bass(Arc<BassPatternData>),
    Synth(Arc<NotePatternData>),
    Tapan(Arc<DrumPatternData>),
    Chord(Arc<ChordPatternData>),
    Lead(Arc<NotePatternData>),
}

/// Tapan strokes → the drum pattern type the engine's tapan plays (boom = kick, rim =
/// snare, ghost = ghost).
pub(crate) fn tapan_pattern(
    hits: &[music_core::TapanHit],
    length_beats: f64,
) -> Arc<DrumPatternData> {
    Arc::new(DrumPatternData::new(
        hits.iter()
            .map(|h| DrumEvent {
                beat: h.start_beat,
                kind: match h.stroke {
                    music_core::TapanStroke::Boom => DrumKind::Kick,
                    music_core::TapanStroke::Rim => DrumKind::Snare,
                    music_core::TapanStroke::Ghost => DrumKind::Ghost,
                },
                velocity: h.velocity as f32,
            })
            .collect(),
        length_beats,
    ))
}

/// Push a voice document into live params (the designer's knobs while auditioning, and
/// the rhythm parts when they reference a preset).
pub fn apply_voice(voice: &InstrumentVoice, params: &AuditionParams) {
    match (voice, params) {
        (
            InstrumentVoice::Drums {
                kick_hz,
                decay,
                brightness,
                levels_db,
            },
            AuditionParams::Drums(p),
        ) => {
            p.kick_hz.set(*kick_hz as f32);
            p.decay.set(*decay as f32);
            p.brightness.set(*brightness as f32);
            for (k, l) in levels_db.iter().enumerate() {
                p.level_db[k].set(*l as f32);
            }
        }
        (
            InstrumentVoice::Bass {
                voice,
                cutoff_hz,
                resonance,
                drive,
                glide_s,
                sidechain,
                wobble_div,
                wobble_depth,
                warmth,
                attack_s,
                decay_s,
                sustain,
                release_s,
                filter_env,
                pitch_env_st,
                pitch_env_s,
                microvar,
            },
            AuditionParams::Bass(p),
        ) => {
            p.voice.store((*voice).min(2), Ordering::Relaxed);
            p.cutoff_hz.set(*cutoff_hz as f32);
            p.resonance.set(*resonance as f32);
            p.drive.set(*drive as f32);
            p.glide_s.set(*glide_s as f32);
            p.sidechain.set(*sidechain as f32);
            p.wobble_div.store(*wobble_div, Ordering::Relaxed);
            p.wobble_depth.set(*wobble_depth as f32);
            p.warmth.set(*warmth as f32);
            p.attack_s.set(*attack_s as f32);
            p.decay_s.set(*decay_s as f32);
            p.sustain.set(*sustain as f32);
            p.release_s.set(*release_s as f32);
            p.filter_env.set(*filter_env as f32);
            p.pitch_env_st.set(*pitch_env_st as f32);
            p.pitch_env_s.set(*pitch_env_s as f32);
            p.microvar.set(*microvar as f32);
        }
        (
            InstrumentVoice::Synth {
                timbre,
                attack_s,
                release_s,
                tone_hz,
                dynamics,
            },
            AuditionParams::Synth(p),
        ) => {
            p.timbre.store((*timbre).min(2), Ordering::Relaxed);
            p.attack_s.set(*attack_s as f32);
            p.release_s.set(*release_s as f32);
            p.tone_hz.set(*tone_hz as f32);
            p.dynamics.set(*dynamics as f32);
        }
        (
            InstrumentVoice::Tapan {
                tune_hz,
                decay,
                skin,
                rim_tone,
                levels_db,
            },
            AuditionParams::Tapan(p),
        ) => {
            p.tune_hz.set(*tune_hz as f32);
            p.decay.set(*decay as f32);
            p.skin.set(*skin as f32);
            p.rim_tone.set(*rim_tone as f32);
            for (k, l) in levels_db.iter().enumerate() {
                p.level_db[k].set(*l as f32);
            }
        }
        (
            InstrumentVoice::ChordPad {
                wave,
                attack_s,
                release_s,
                detune_cents,
                cutoff_hz,
                movement,
                dynamics,
            },
            AuditionParams::ChordPad(p),
        ) => {
            p.wave.store((*wave).min(1), Ordering::Relaxed);
            p.attack_s.set(*attack_s as f32);
            p.release_s.set(*release_s as f32);
            p.detune_cents.set(*detune_cents as f32);
            p.cutoff_hz.set(*cutoff_hz as f32);
            p.movement.set(*movement as f32);
            p.dynamics.set(*dynamics as f32);
        }
        (
            InstrumentVoice::Lead {
                wave,
                glide_s,
                vibrato_hz,
                vibrato_cents,
                cutoff_hz,
                resonance,
                env_amount,
                attack_s,
                release_s,
                drive,
            },
            AuditionParams::Lead(p),
        ) => {
            p.wave.store((*wave).min(2), Ordering::Relaxed);
            p.glide_s.set(*glide_s as f32);
            p.vibrato_hz.set(*vibrato_hz as f32);
            p.vibrato_cents.set(*vibrato_cents as f32);
            p.cutoff_hz.set(*cutoff_hz as f32);
            p.resonance.set(*resonance as f32);
            p.env_amount.set(*env_amount as f32);
            p.attack_s.set(*attack_s as f32);
            p.release_s.set(*release_s as f32);
            p.drive.set(*drive as f32);
        }
        _ => {}
    }
}

/// Build the audition source for a voice on a clock; returns the params handle too.
fn audition_source(
    voice: &InstrumentVoice,
    clock: &Arc<BeatClock>,
    midi: f64,
) -> (AuditionParams, Box<dyn audio_engine::Source>) {
    match audition_pattern(voice, midi) {
        AuditionPattern::Drums(pat) => {
            let p = Arc::new(DrumParams::default());
            let src = DrumSource::new(Arc::clone(&p), Arc::clone(clock), pat);
            let params = AuditionParams::Drums(p);
            apply_voice(voice, &params);
            (params, Box::new(src))
        }
        AuditionPattern::Bass(pat) => {
            let p = Arc::new(BassParams::default());
            let src = BassSource::new(Arc::clone(&p), Arc::clone(clock), pat, None);
            let params = AuditionParams::Bass(p);
            apply_voice(voice, &params);
            (params, Box::new(src))
        }
        AuditionPattern::Synth(pat) => {
            let p = Arc::new(PatternParams::default());
            let src = PatternSynthSource::new(Arc::clone(&p), Arc::clone(clock), pat);
            let params = AuditionParams::Synth(p);
            apply_voice(voice, &params);
            (params, Box::new(src))
        }
        AuditionPattern::Tapan(pat) => {
            let p = Arc::new(TapanParams::default());
            let src = TapanSource::new(Arc::clone(&p), Arc::clone(clock), pat);
            let params = AuditionParams::Tapan(p);
            apply_voice(voice, &params);
            (params, Box::new(src))
        }
        AuditionPattern::Chord(pat) => {
            let p = Arc::new(ChordPadParams::default());
            let src = ChordPadSource::new(Arc::clone(&p), Arc::clone(clock), pat);
            let params = AuditionParams::ChordPad(p);
            apply_voice(voice, &params);
            (params, Box::new(src))
        }
        AuditionPattern::Lead(pat) => {
            let p = Arc::new(LeadParams::default());
            let src = LeadSource::new(Arc::clone(&p), Arc::clone(clock), pat);
            let params = AuditionParams::Lead(p);
            apply_voice(voice, &params);
            (params, Box::new(src))
        }
    }
}

fn audition_bpm(s: &Inner) -> f64 {
    s.project
        .as_ref()
        .and_then(|p| p.manifest().tempo_map.first().map(|t| t.bpm))
        .unwrap_or(120.0)
}

/// Start (or restart) auditioning a voice on slot 3 at the project's tempo.
#[tauri::command]
pub async fn instrument_audition_start(
    state: State<'_, AppState>,
    voice: InstrumentVoice,
    device: Option<String>,
    midi: Option<f64>,
    level_db: Option<f32>,
) -> CmdResult<()> {
    voice
        .validate()
        .map_err(|e| err("invalid_argument", e.to_string()))?;
    let mut s = state.lock();
    ensure_graph(&mut s, device.as_deref())?;
    let bpm = audition_bpm(&s);
    let clock = Arc::new(BeatClock::new(bpm as f32));
    let midi = midi.unwrap_or(match voice {
        InstrumentVoice::Bass { .. } => 38.0,
        InstrumentVoice::Synth { .. } => 48.0,
        InstrumentVoice::Drums { .. } | InstrumentVoice::Tapan { .. } => 36.0,
        InstrumentVoice::ChordPad { .. } => 55.0,
        InstrumentVoice::Lead { .. } => 74.0,
    });
    let (params, src) = audition_source(&voice, &clock, midi);
    let g = s.graph.as_mut().expect("graph just started");
    let frame = g.params().frame.load(Ordering::Relaxed);
    clock.start(frame, 0.0);
    g.params().layers[AUDITION_SLOT].gain_db.set(-60.0);
    g.params().layers[AUDITION_SLOT].pan.set(0.0);
    if !g.send(GraphCmd::SetSource {
        slot: AUDITION_SLOT,
        source: src,
    }) {
        return Err(err("graph_busy", "Graph command queue is full"));
    }
    g.params().layers[AUDITION_SLOT]
        .gain_db
        .set(level_db.unwrap_or(-6.0).clamp(-60.0, 6.0));
    s.audition = Some(AuditionLive {
        params,
        category: voice.category(),
    });
    Ok(())
}

/// Push knob changes into the auditioned voice (no rebuild, no click).
#[tauri::command]
pub async fn instrument_audition_set(
    state: State<'_, AppState>,
    voice: InstrumentVoice,
) -> CmdResult<bool> {
    let s = state.lock();
    let Some(a) = s.audition.as_ref() else {
        return Ok(false);
    };
    if a.category != voice.category() {
        return Ok(false);
    }
    voice
        .validate()
        .map_err(|e| err("invalid_argument", e.to_string()))?;
    apply_voice(&voice, &a.params);
    Ok(true)
}

#[tauri::command]
pub async fn instrument_audition_stop(state: State<'_, AppState>) -> CmdResult<()> {
    let mut s = state.lock();
    if s.audition.take().is_some() {
        crate::graph_cmds::release_slot(&mut s, AUDITION_SLOT);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct InstrumentRenderResult {
    pub path: String,
    pub seconds: f64,
    pub peak: f32,
}

/// Offline render of the audition pattern through the voice: the same code path as the
/// live audition, written to a WAV (evidence for a preset, and the test oracle).
pub fn render_voice(
    voice: &InstrumentVoice,
    bpm: f64,
    seconds: f64,
    rate: u32,
    midi: f64,
) -> AudioBuffer {
    let gp = Arc::new(GraphParams::default());
    gp.master.gain_db.set(0.0);
    gp.layers[AUDITION_SLOT].gain_db.set(-6.0);
    let clock = Arc::new(BeatClock::new(bpm as f32));
    let (_params, src) = audition_source(voice, &clock, midi);
    let mut g = Graph::new(gp, rate);
    g.set_source(AUDITION_SLOT, src);
    clock.start(0, 0.0);
    g.render_offline(seconds, 2)
}

#[tauri::command]
pub async fn instrument_render(
    state: State<'_, AppState>,
    path: String,
    voice: InstrumentVoice,
    seconds: Option<f64>,
    midi: Option<f64>,
) -> CmdResult<InstrumentRenderResult> {
    voice
        .validate()
        .map_err(|e| err("invalid_argument", e.to_string()))?;
    let bpm = {
        let s = state.lock();
        audition_bpm(&s)
    };
    let seconds = seconds.unwrap_or(4.0 * 60.0 / bpm * 2.0).clamp(0.5, 60.0);
    let midi = midi.unwrap_or(match voice {
        InstrumentVoice::Bass { .. } => 38.0,
        InstrumentVoice::Synth { .. } => 48.0,
        InstrumentVoice::Drums { .. } | InstrumentVoice::Tapan { .. } => 36.0,
        InstrumentVoice::ChordPad { .. } => 55.0,
        InstrumentVoice::Lead { .. } => 74.0,
    });
    let buf = render_voice(&voice, bpm, seconds, 48_000, midi);
    let mut p = std::path::PathBuf::from(&path);
    if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("wav")) {
        p.set_extension("wav");
    }
    write_wav(&p, &buf, WavFormat::default())?;
    let peak = buf.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    Ok(InstrumentRenderResult {
        path: p.display().to_string(),
        seconds,
        peak,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(b: &AudioBuffer) -> f32 {
        b.rms()
    }

    #[test]
    fn every_factory_preset_renders_audibly_and_presets_differ() {
        let all = factory_instruments();
        let mut renders: Vec<(String, AudioBuffer)> = Vec::new();
        for p in &all {
            let b = render_voice(&p.voice, 170.0, 2.0, 48_000, 38.0);
            let peak = b.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            assert!(b.samples.iter().all(|x| x.is_finite()), "{}: finite", p.id);
            assert!(
                peak > 0.02 && peak <= 1.0,
                "{}: audible, not clipping ({peak})",
                p.id
            );
            renders.push((p.id.clone(), b));
        }
        // Presets of one category are not the same sound: their renders differ clearly.
        for (i, (ia, a)) in renders.iter().enumerate() {
            for (ib, b) in renders.iter().skip(i + 1) {
                let same_cat = ia.split('.').next() == ib.split('.').next();
                if !same_cat {
                    continue;
                }
                let n = a.samples.len().min(b.samples.len());
                let diff: f32 = (0..n)
                    .map(|k| (a.samples[k] - b.samples[k]).abs())
                    .sum::<f32>()
                    / n as f32;
                let level = (rms(a) + rms(b)) * 0.5;
                assert!(
                    diff > level * 0.2,
                    "{ia} vs {ib} render almost identically ({diff} vs {level})"
                );
            }
        }
    }

    #[test]
    fn apply_voice_moves_the_live_params() {
        let p = Arc::new(BassParams::default());
        let params = AuditionParams::Bass(Arc::clone(&p));
        let mut v = project::legacy_bass(2);
        if let InstrumentVoice::Bass {
            cutoff_hz,
            attack_s,
            ..
        } = &mut v
        {
            *cutoff_hz = 1234.0;
            *attack_s = 0.3;
        }
        apply_voice(&v, &params);
        assert_eq!(p.voice.load(Ordering::Relaxed), 2);
        assert!((p.cutoff_hz.target() - 1234.0).abs() < 1e-3);
        assert!((p.attack_s.target() - 0.3).abs() < 1e-6);
    }
}
