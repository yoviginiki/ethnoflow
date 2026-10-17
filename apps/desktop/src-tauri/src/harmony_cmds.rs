//! Harmonic strategies (F3.4) on the loaded take: three proposals from the F2.11
//! analysis, and MIDI export of one proposal (chords + bass) on its own tracks.

use std::path::PathBuf;

use music_core::{
    generate_countermelody, propose_all, write_smf, CounterParams, CounterRegister, HarmonyParams,
    HarmonyProposal, Locks, Note, SmfOptions, Strategy, ToneSpan, TECHNICAL_TEMPO_BPM,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::intent_cmds::{analyze_for, IntentSettings};
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmonyUiParams {
    pub tension: f64,
    pub movement: f64,
    pub richness: f64,
    pub seed: u32,
    pub locks: Locks,
    pub register_low: i32,
    pub register_high: i32,
}

#[tauri::command]
pub async fn harmony_proposals(
    state: State<'_, AppState>,
    intent: IntentSettings,
    params: HarmonyUiParams,
) -> CmdResult<Vec<HarmonyProposal>> {
    let s = state.lock();
    let Some(analysis) = analyze_for(&s, &intent)? else {
        return Ok(Vec::new());
    };
    if !(0.0..=1.0).contains(&params.tension)
        || !(0.0..=1.0).contains(&params.movement)
        || !(0.0..=1.0).contains(&params.richness)
        || params.register_low < 24
        || params.register_high > 96
        || params.register_high - params.register_low < 12
    {
        return Err(err("invalid_argument", "harmony parameters out of range"));
    }
    let scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
    let mut base = HarmonyParams::for_strategy(Strategy::ModalPads, params.seed);
    base.tension = params.tension;
    base.movement = params.movement;
    base.richness = params.richness;
    base.locks = params.locks;
    base.register_low = params.register_low;
    base.register_high = params.register_high;
    Ok(propose_all(&analysis, scale.as_ref(), &base))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarmonyExportResult {
    pub path: String,
    pub chords: usize,
    pub bass_notes: usize,
}

/// Export a proposal as a MIDI file: chord tones (with their cents as pitch bend is
/// not possible per voice, so cents are dropped) and bass roots.
#[tauri::command]
pub async fn export_harmony_midi(
    state: State<'_, AppState>,
    proposal: HarmonyProposal,
    path: String,
) -> CmdResult<HarmonyExportResult> {
    let s = state.lock();
    let m = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?
        .manifest();
    let opts = SmfOptions {
        tempo_bpm: m
            .tempo_map
            .first()
            .map(|t| t.bpm)
            .unwrap_or(TECHNICAL_TEMPO_BPM),
        time_signature: m
            .time_signatures
            .first()
            .map(|t| (t.numerator as u8, t.unit as u8)),
        ..SmfOptions::default()
    };
    let mut notes: Vec<Note> = Vec::new();
    for c in &proposal.chords {
        for t in &c.tones {
            notes.push(Note {
                start: c.start,
                end: c.end,
                midi: t.midi,
                cents: 0.0,
                confidence: 1.0,
                velocity: 0.6,
                bend: Vec::new(),
            });
        }
    }
    let chords = proposal.chords.len();
    if proposal.chords.is_empty() {
        for d in &proposal.drone {
            if let (Some(first), Some(last)) = (proposal.bass.first(), proposal.bass.last()) {
                notes.push(Note {
                    start: first.start,
                    end: last.end,
                    midi: d.midi,
                    cents: 0.0,
                    confidence: 1.0,
                    velocity: 0.5,
                    bend: Vec::new(),
                });
            }
        }
    }
    for b in &proposal.bass {
        notes.push(Note {
            start: b.start,
            end: b.end,
            midi: b.midi,
            cents: 0.0,
            confidence: 1.0,
            velocity: 0.7,
            bend: Vec::new(),
        });
    }
    if notes.is_empty() {
        return Err(err("no_notes", "The proposal has nothing to export"));
    }
    notes.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());
    let bytes = write_smf(&notes, &opts);
    let mut p = PathBuf::from(&path);
    if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("mid")) {
        p.set_extension("mid");
    }
    std::fs::write(&p, &bytes).map_err(|e| err("io", e.to_string()))?;
    Ok(HarmonyExportResult {
        path: p.to_string_lossy().into_owned(),
        chords,
        bass_notes: proposal.bass.len(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterUiParams {
    pub register: CounterRegister,
    pub adventure: f64,
    pub tension: f64,
    pub min_rest_s: f64,
    pub seed: u32,
}

/// Countermelody (F4.7): answers in the rests of the loaded take, on the tones of
/// the given proposal (or the mode), rendered as a new MIDI take in the player so it
/// can be heard, edited and exported like any other take.
#[tauri::command]
pub async fn countermelody_to_take(
    state: State<'_, AppState>,
    intent: IntentSettings,
    params: CounterUiParams,
    proposal: Option<HarmonyProposal>,
    label: Option<String>,
) -> CmdResult<crate::midi_cmds::SentToPlayer> {
    if !(0.0..=1.0).contains(&params.adventure)
        || !(0.0..=1.0).contains(&params.tension)
        || !(0.1..=10.0).contains(&params.min_rest_s)
    {
        return Err(err(
            "invalid_argument",
            "countermelody parameters out of range",
        ));
    }
    let mut s = state.lock();
    if s.mode_busy() {
        return Err(err("busy", "Stop recording or playback first"));
    }
    let Some(analysis) = analyze_for(&s, &intent)? else {
        return Err(err("no_notes", "Analyse the take first"));
    };
    let melody = crate::analysis_cmds::current_notes(&s)?;
    if melody.is_empty() {
        return Err(err("no_notes", "There are no notes to answer"));
    }
    let spans: Vec<ToneSpan> = proposal
        .as_ref()
        .map(|p| {
            p.chords
                .iter()
                .map(|c| ToneSpan {
                    start: c.start,
                    end: c.end,
                    tones: c
                        .tones
                        .iter()
                        .map(|t| (t.midi.rem_euclid(12) as u8, t.cents))
                        .collect(),
                })
                .collect()
        })
        .unwrap_or_default();
    let scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
    let answer = generate_countermelody(
        &melody,
        &analysis.phrases,
        &spans,
        scale.as_ref(),
        &CounterParams {
            register: params.register,
            adventure: params.adventure,
            tension: params.tension,
            min_rest_s: params.min_rest_s,
            seed: params.seed,
        },
    );
    if answer.is_empty() {
        return Err(err(
            "no_rests",
            "No rests long enough for an answer (lower the minimum rest)",
        ));
    }
    crate::midi_cmds::notes_to_take(&mut s, &answer, label.as_deref())
}
