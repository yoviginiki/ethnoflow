//! Melodic phrases from a reference (P2 Phase B, melody step): a pitch track of the
//! song's melody band → notes (the same note extractor the take analysis uses) → phrases
//! split at rests, scored by confidence, length and pitch variety. Everything is a
//! *proposal*: the user listens to a phrase through the audition slot and takes it into
//! the project as a MIDI take if they like it. On a dense mix the tracker follows the
//! most periodic voice in 110–1600 Hz, so phrases from busy sections are marked with a
//! lower confidence rather than hidden.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::{
    pitch::{contour, PitchConfig},
    resample_linear, BeatClock, GraphCmd, NotePatternData, PatternNote, PatternParams,
    PatternSynthSource,
};
use music_core::{extract_notes, Note, NoteParams, PitchSample, Reference};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::forge_cmds::AUDITION_SLOT;
use crate::graph_cmds::ensure_graph;
use crate::reference_cmds::{
    decoded, folder, original_path, read_json, write_json, ReferenceMeta, ReferenceReport,
};
use crate::state::{AppState, Inner};

const TRACK_RATE: u32 = 22_050;
const HOP_S: f64 = 0.02;
const MAX_PHRASES: usize = 12;
/// A rest at least this long ends a phrase.
const PHRASE_GAP_S: f64 = 0.45;
const MIN_NOTES: usize = 4;
const MIN_LEN_S: f64 = 1.2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseNote {
    pub start_s: f64,
    pub dur_s: f64,
    pub midi: i32,
    pub cents: f64,
    pub velocity: f64,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phrase {
    pub index: usize,
    pub start_s: f64,
    pub end_s: f64,
    /// Structure segment (1-based) the phrase starts in, when the song was analysed.
    pub section: Option<usize>,
    pub notes: Vec<PhraseNote>,
    /// Mean note confidence 0..1.
    pub confidence: f64,
    /// Share of notes on the top key hypothesis's pitch classes (None without a key).
    pub in_key: Option<f64>,
    pub span_semitones: i32,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhraseReport {
    pub id: String,
    pub a4_hz: f64,
    pub extracted_ms: i64,
    pub notes_total: usize,
    /// Share of frames with a pitch, 0..1 (low = the tracker found little to follow).
    pub voiced_share: f64,
    pub phrases: Vec<Phrase>,
}

/// One-pole band limiting (≈150 Hz high-pass, ≈3 kHz low-pass) so the bass and the hats
/// pull the tracker less; pure and cheap, applied once off-thread.
fn melody_band(samples: &[f32], sr: f32) -> Vec<f32> {
    // Two cascaded one-pole high-passes (12 dB/oct) and one low-pass.
    let hp_a = (-2.0 * std::f32::consts::PI * 150.0 / sr).exp();
    let lp_a = 1.0 - (-2.0 * std::f32::consts::PI * 3000.0 / sr).exp();
    let (mut in1, mut out1, mut in2, mut out2, mut lp) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    samples
        .iter()
        .map(|&x| {
            out1 = hp_a * (out1 + x - in1);
            in1 = x;
            out2 = hp_a * (out2 + out1 - in2);
            in2 = out1;
            lp += lp_a * (out2 - lp);
            lp
        })
        .collect()
}

/// Index ranges of phrases: runs of notes separated by rests shorter than `gap_s`,
/// keeping runs with at least `min_notes` notes and `min_len_s` seconds.
pub(crate) fn segment_phrases(
    notes: &[Note],
    gap_s: f64,
    min_notes: usize,
    min_len_s: f64,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for i in 1..=notes.len() {
        let split = i == notes.len() || notes[i].start - notes[i - 1].end >= gap_s;
        if split {
            let len = notes[i - 1].end - notes[start].start;
            if i - start >= min_notes && len >= min_len_s {
                out.push((start, i));
            }
            start = i;
        }
    }
    out
}

/// 0..1: confidence × length (saturating at 6 notes) × pitch variety (≥ 3 pitches).
pub(crate) fn phrase_score(notes: &[Note]) -> f64 {
    if notes.is_empty() {
        return 0.0;
    }
    let conf = notes.iter().map(|n| n.confidence).sum::<f64>() / notes.len() as f64;
    let len = (notes.len() as f64 / 6.0).min(1.0);
    let mut pitches: Vec<i32> = notes.iter().map(|n| n.midi).collect();
    pitches.sort_unstable();
    pitches.dedup();
    let variety = if pitches.len() >= 3 { 1.0 } else { 0.5 };
    (conf * len * variety).clamp(0.0, 1.0)
}

fn build_report(
    id: &str,
    a4: f64,
    notes: &[Note],
    voiced_share: f64,
    song: Option<&ReferenceReport>,
) -> PhraseReport {
    let key_pcs: Option<Vec<u8>> = song.and_then(|r| r.key.first()).map(|h| {
        music_core::Scale::new(h.tonic_pc, h.kind)
            .pitch_classes()
            .into_iter()
            .collect()
    });
    let mut scored: Vec<Phrase> = segment_phrases(notes, PHRASE_GAP_S, MIN_NOTES, MIN_LEN_S)
        .into_iter()
        .map(|(a, b)| {
            let slice = &notes[a..b];
            let start_s = slice[0].start;
            let end_s = slice[slice.len() - 1].end;
            let lo = slice.iter().map(|n| n.midi).min().unwrap_or(0);
            let hi = slice.iter().map(|n| n.midi).max().unwrap_or(0);
            let in_key = key_pcs.as_ref().map(|pcs| {
                slice
                    .iter()
                    .filter(|n| pcs.contains(&((n.midi.rem_euclid(12)) as u8)))
                    .count() as f64
                    / slice.len() as f64
            });
            let section = song.and_then(|r| {
                r.segments
                    .iter()
                    .position(|s| start_s >= s.start_s && start_s < s.end_s)
                    .map(|i| i + 1)
            });
            Phrase {
                index: 0,
                start_s,
                end_s,
                section,
                notes: slice
                    .iter()
                    .map(|n| PhraseNote {
                        start_s: n.start,
                        dur_s: n.end - n.start,
                        midi: n.midi,
                        cents: n.cents,
                        velocity: n.velocity,
                        confidence: n.confidence,
                    })
                    .collect(),
                confidence: slice.iter().map(|n| n.confidence).sum::<f64>() / slice.len() as f64,
                in_key,
                span_semitones: hi - lo,
                score: phrase_score(slice),
            }
        })
        .collect();
    scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
    scored.truncate(MAX_PHRASES);
    scored.sort_by(|a, b| a.start_s.partial_cmp(&b.start_s).unwrap());
    for (i, p) in scored.iter_mut().enumerate() {
        p.index = i;
    }
    PhraseReport {
        id: id.to_string(),
        a4_hz: a4,
        extracted_ms: crate::reference_cmds::now_ms(),
        notes_total: notes.len(),
        voiced_share,
        phrases: scored,
    }
}

fn a4_of(s: &Inner) -> f64 {
    s.project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(crate::analysis_cmds::DEFAULT_A4_HZ)
}

// ---------- commands ----------

/// Extract melodic phrases (on request, off-thread): decode → mono → 22.05 kHz → melody
/// band → pitch track → notes → phrases. Saved as `phrases.json` next to the reference.
#[tauri::command]
pub async fn reference_phrases(state: State<'_, AppState>, id: String) -> CmdResult<PhraseReport> {
    let (dir, meta, a4) = {
        let s = state.lock();
        let dir = folder(&s, &id)?;
        let meta: ReferenceMeta = read_json(&dir.join("meta.json"))?
            .ok_or_else(|| err("corrupt", "meta.json is missing"))?;
        (dir, meta, a4_of(&s))
    };
    let id2 = id.clone();
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<PhraseReport> {
        let buf = decoded(&id2, &original_path(&dir, &meta))?;
        let mono = buf.to_mono();
        let mono = if mono.sample_rate == TRACK_RATE {
            mono
        } else {
            resample_linear(&mono, TRACK_RATE)
        };
        let band = audio_engine::AudioBuffer::new(
            TRACK_RATE,
            1,
            melody_band(&mono.samples, TRACK_RATE as f32),
        );
        let mut cfg = PitchConfig::new(TRACK_RATE);
        cfg.min_hz = 110.0;
        cfg.max_hz = 1600.0;
        let points = contour(&band, &cfg, HOP_S);
        let voiced = points.iter().filter(|p| p.hz.is_some()).count();
        let voiced_share = if points.is_empty() {
            0.0
        } else {
            voiced as f64 / points.len() as f64
        };
        let r = Reference::new(a4);
        let samples: Vec<PitchSample> = points
            .iter()
            .map(|p| PitchSample {
                t: p.t,
                midi: p.hz.map(|hz| r.hz_to_midi(hz)),
                confidence: p.confidence,
                rms: p.rms,
            })
            .collect();
        let notes = extract_notes(&samples, &NoteParams::default());
        let song: Option<ReferenceReport> = read_json(&dir.join("report.json"))?;
        let report = build_report(&id2, a4, &notes, voiced_share, song.as_ref());
        write_json(&dir.join("phrases.json"), &report)?;
        Ok(report)
    })
    .await
    .map_err(|e| err("analysis", e.to_string()))?
}

#[tauri::command]
pub async fn reference_phrases_get(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<Option<PhraseReport>> {
    let dir = {
        let s = state.lock();
        folder(&s, &id)?
    };
    read_json(&dir.join("phrases.json"))
}

fn phrase_of(s: &Inner, id: &str, index: usize) -> CmdResult<(Phrase, f64)> {
    let dir = folder(s, id)?;
    let rep: PhraseReport = read_json(&dir.join("phrases.json"))?
        .ok_or_else(|| err("not_found", "Extract the phrases first"))?;
    let p = rep
        .phrases
        .get(index)
        .cloned()
        .ok_or_else(|| err("not_found", "No such phrase"))?;
    Ok((p, rep.a4_hz))
}

/// Play a phrase through the audition slot with the pattern synth (flute-like voice),
/// at its original timing (a 60 BPM clock: one beat = one second).
#[tauri::command]
pub async fn reference_phrase_play(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    index: usize,
    device: Option<String>,
) -> CmdResult<f64> {
    let mut s = state.lock();
    let (phrase, a4) = phrase_of(&s, &id, index)?;
    let r = Reference::new(a4);
    let t0 = phrase.start_s;
    let events: Vec<PatternNote> = phrase
        .notes
        .iter()
        .map(|n| PatternNote {
            beat: n.start_s - t0,
            length_beats: n.dur_s.max(0.05),
            hz: r.midi_to_hz(n.midi as f64 + n.cents / 100.0) as f32,
            velocity: (0.4 + 0.6 * n.velocity) as f32,
        })
        .collect();
    let length = (phrase.end_s - t0) + 1.0;
    ensure_graph(&mut s, device.as_deref())?;
    let clock = Arc::new(BeatClock::new(60.0));
    let params = Arc::new(PatternParams::default());
    params.release_s.set(0.12);
    let token = s.preview_token.wrapping_add(1);
    s.preview_token = token;
    s.audition = None;
    let g = s.graph.as_mut().expect("graph just started");
    let frame = g.params().frame.load(Ordering::Relaxed);
    clock.start(frame, 0.0);
    g.params().layers[AUDITION_SLOT].gain_db.set(-60.0);
    g.params().layers[AUDITION_SLOT].pan.set(0.0);
    if !g.send(GraphCmd::SetSource {
        slot: AUDITION_SLOT,
        source: Box::new(PatternSynthSource::new(
            params,
            Arc::clone(&clock),
            Arc::new(NotePatternData::new(events, length)),
        )),
    }) {
        return Err(err("graph_busy", "Graph command queue is full"));
    }
    g.params().layers[AUDITION_SLOT].gain_db.set(-6.0);
    s.previewing = true;
    drop(s);
    std::thread::spawn(move || {
        use tauri::Manager;
        std::thread::sleep(std::time::Duration::from_secs_f64(length));
        let st = app.state::<AppState>();
        let mut s = st.lock();
        if s.preview_token == token && s.previewing {
            s.previewing = false;
            crate::graph_cmds::release_slot(&mut s, AUDITION_SLOT);
        }
    });
    Ok(length)
}

/// Take a phrase into the open project as a MIDI take (timing kept in seconds, the
/// project's tempo written as the file's tempo). The take is named after the reference.
#[tauri::command]
pub async fn reference_phrase_to_take(
    state: State<'_, AppState>,
    id: String,
    index: usize,
) -> CmdResult<crate::project_cmds::ProjectView> {
    let mut s = state.lock();
    if s.project.is_none() {
        return Err(err("no_project", "No project is open"));
    }
    if s.mode_busy() {
        return Err(err("busy", "Stop recording or playback first"));
    }
    let (phrase, _a4) = phrase_of(&s, &id, index)?;
    let name = {
        let dir = folder(&s, &id)?;
        read_json::<ReferenceMeta>(&dir.join("meta.json"))?
            .map(|m| m.name)
            .unwrap_or_else(|| id.clone())
    };
    let t0 = phrase.start_s;
    let notes: Vec<Note> = phrase
        .notes
        .iter()
        .map(|n| Note {
            start: n.start_s - t0,
            end: n.start_s - t0 + n.dur_s.max(0.05),
            midi: n.midi,
            cents: n.cents,
            confidence: n.confidence,
            velocity: n.velocity.clamp(0.05, 1.0),
            bend: Vec::new(),
        })
        .collect();
    let bpm = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().tempo_map.first().map(|t| t.bpm))
        .unwrap_or(120.0);
    let bytes = music_core::write_smf(
        &notes,
        &music_core::SmfOptions {
            tempo_bpm: bpm,
            ..music_core::SmfOptions::default()
        },
    );
    let tmp_dir = std::env::temp_dir().join("ethnoflow-phrases");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| err("io", e.to_string()))?;
    let path = tmp_dir.join(format!(
        "phrase-{}-{}.mid",
        index + 1,
        project::now_ms_pub()
    ));
    std::fs::write(&path, &bytes).map_err(|e| err("io", e.to_string()))?;
    let pv = crate::midi_cmds::import_midi(&mut s, &path);
    let _ = std::fs::remove_file(&path);
    let mut pv = pv?;
    if let Some(t) = pv.takes.iter().max_by_key(|t| t.number) {
        let label = format!("{} · фраза {}", name, index + 1);
        if let Some(p) = s.project.as_mut() {
            let _ = p.rename_take(&t.id, Some(&label));
        }
        pv = crate::project_cmds::view(&s)?;
    }
    Ok(pv)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(start: f64, end: f64, midi: i32, conf: f64) -> Note {
        Note {
            start,
            end,
            midi,
            cents: 0.0,
            confidence: conf,
            velocity: 0.8,
            bend: Vec::new(),
        }
    }

    #[test]
    fn phrases_split_at_rests_and_short_runs_are_dropped() {
        let notes = vec![
            note(0.0, 0.4, 62, 0.9),
            note(0.45, 0.9, 64, 0.9),
            note(0.95, 1.4, 65, 0.8),
            note(1.45, 2.0, 67, 0.9),
            // rest of 1 s
            note(3.0, 3.3, 69, 0.7),
            note(3.35, 3.6, 69, 0.7), // only two notes: dropped
            // rest
            note(5.0, 5.5, 62, 0.95),
            note(5.55, 6.0, 62, 0.95),
            note(6.05, 6.5, 62, 0.95),
            note(6.55, 7.2, 62, 0.95),
        ];
        let ranges = segment_phrases(&notes, PHRASE_GAP_S, MIN_NOTES, MIN_LEN_S);
        assert_eq!(ranges, vec![(0, 4), (6, 10)]);
        // The varied phrase outscores the one-pitch phrase despite lower confidence.
        assert!(phrase_score(&notes[0..4]) > phrase_score(&notes[6..10]));
        let rep = build_report("r1", 440.0, &notes, 0.8, None);
        assert_eq!(rep.phrases.len(), 2);
        assert_eq!(rep.phrases[0].span_semitones, 5);
        assert_eq!(rep.phrases[1].span_semitones, 0);
        assert!(rep.phrases[0].in_key.is_none());
        assert_eq!(
            rep.phrases.iter().map(|p| p.index).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn melody_band_keeps_the_middle_and_tames_the_extremes() {
        let sr = 22_050.0f32;
        let tone = |hz: f32| -> Vec<f32> {
            (0..22_050)
                .map(|i| (std::f32::consts::TAU * hz * i as f32 / sr).sin())
                .collect()
        };
        let rms = |x: &[f32]| {
            (x.iter().skip(4000).map(|v| v * v).sum::<f32>() / (x.len() - 4000) as f32).sqrt()
        };
        let mid = rms(&melody_band(&tone(440.0), sr));
        let low = rms(&melody_band(&tone(50.0), sr));
        let high = rms(&melody_band(&tone(8000.0), sr));
        assert!(mid > low * 3.0, "mid {mid} vs low {low}");
        assert!(mid > high * 2.0, "mid {mid} vs high {high}");
    }
}
