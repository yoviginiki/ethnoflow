//! Phase 2 commands: pitch analysis of the loaded take, user-edited notes, scale and
//! tuning reference, MIDI export, and the input monitor (tuner without recording).
//!
//! Everything here is a proposal: the audio and the raw contour are never changed;
//! user edits live in a separate document; the scale is chosen by the user.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use audio_engine::{
    contour, ContourPoint, InputMonitor, MonitorStatus, PitchConfig, StreamRequest,
};
use music_core::{
    explain, extract_notes, rank_hypotheses, write_smf, Evidence, Hypothesis, Note, NoteParams,
    PitchClassHistogram, PitchSample, Reference, Scale, ScaleKind, SmfOptions, TECHNICAL_TEMPO_BPM,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::project_cmds::{project_view, ProjectView};
use crate::state::{AppState, Inner};

const CONTOUR_HOP_SECONDS: f64 = 0.01;
const MAX_VIEW_POINTS: usize = 4000;
const MAX_HYPOTHESES: usize = 10;
pub const DEFAULT_A4_HZ: f64 = 440.0;

// ---------- documents stored under analysis/ ----------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContourDoc {
    pub hop_seconds: f64,
    /// Trim-in of the take when analysed: notes/contour are relative to the trimmed audio.
    pub trim_in_seconds: f64,
    pub points: Vec<ContourPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotesDoc {
    pub a4_hz: f64,
    pub trim_in_seconds: f64,
    pub params: NoteParams,
    pub notes: Vec<Note>,
}

// ---------- what the UI sees ----------

#[derive(Debug, Clone, Serialize)]
pub struct ContourView {
    pub t: f64,
    /// Fractional MIDI at the project's reference, `None` when unvoiced.
    pub midi: Option<f64>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnalysisView {
    pub take_id: String,
    pub a4_hz: f64,
    pub contour: Vec<ContourView>,
    pub notes: Vec<Note>,
    /// True when `notes` are the user's edited version.
    pub edited: bool,
    pub hypotheses: Vec<HypothesisView>,
}

/// A ranked hypothesis with the facts behind it (F3.1); the UI words the explanation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HypothesisView {
    #[serde(flatten)]
    pub hypothesis: Hypothesis,
    pub evidence: Option<Evidence>,
}

fn rank_views(h: &PitchClassHistogram) -> Vec<HypothesisView> {
    let ranked = rank_hypotheses(h, &ScaleKind::ALL);
    ranked
        .iter()
        .take(MAX_HYPOTHESES)
        .map(|hyp| HypothesisView {
            hypothesis: hyp.clone(),
            evidence: explain(h, hyp, &ranked),
        })
        .collect()
}

fn a4_of(s: &Inner) -> f64 {
    s.project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(DEFAULT_A4_HZ)
}

fn loaded_id(s: &Inner) -> CmdResult<(String, f64)> {
    s.loaded
        .as_ref()
        .map(|l| (l.id.clone(), l.offset_seconds))
        .ok_or_else(|| err("no_take", "Nothing is loaded"))
}

fn to_view(doc: &ContourDoc, a4: f64) -> Vec<ContourView> {
    let r = Reference::new(a4);
    let step = doc.points.len().div_ceil(MAX_VIEW_POINTS).max(1);
    doc.points
        .iter()
        .step_by(step)
        .map(|p| ContourView {
            t: p.t,
            midi: p.hz.map(|hz| r.hz_to_midi(hz)),
            confidence: p.confidence,
        })
        .collect()
}

fn hypotheses_of(doc: &ContourDoc, a4: f64) -> Vec<HypothesisView> {
    let r = Reference::new(a4);
    let h = PitchClassHistogram::from_midi(doc.points.iter().filter_map(|p| {
        p.hz.map(|hz| (r.hz_to_midi(hz), doc.hop_seconds * p.confidence))
    }));
    rank_views(&h)
}

fn notes_from_contour(doc: &ContourDoc, a4: f64) -> NotesDoc {
    let r = Reference::new(a4);
    let samples: Vec<PitchSample> = doc
        .points
        .iter()
        .map(|p| PitchSample {
            t: p.t,
            midi: p.hz.map(|hz| r.hz_to_midi(hz)),
            confidence: p.confidence,
            rms: p.rms,
        })
        .collect();
    let params = NoteParams::default();
    NotesDoc {
        a4_hz: a4,
        trim_in_seconds: doc.trim_in_seconds,
        params,
        notes: extract_notes(&samples, &params),
    }
}

/// The notes the user is working with: edited document, else automatic, else the
/// MIDI clip's own notes. Empty when nothing has been analysed.
pub fn current_notes(s: &Inner) -> CmdResult<Vec<Note>> {
    Ok(view_from_docs(s)?.map(|v| v.notes).unwrap_or_default())
}

/// Assemble the view from stored documents, if they match the loaded trim.
/// MIDI takes need no analysis: their notes come straight from the file.
pub fn view_from_docs(s: &Inner) -> CmdResult<Option<AnalysisView>> {
    let (id, offset) = loaded_id(s)?;
    let p = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    if let Some(m) = s.loaded.as_ref().and_then(|l| l.midi.as_ref()) {
        let a4 = a4_of(s);
        let auto = crate::midi_cmds::clip_notes(&m.clip, &m.render);
        let edited = p.load_analysis::<NotesDoc>(&id, "notes.edited")?;
        let (notes, is_edited) = match edited {
            Some(e) => (e.notes, true),
            None => (auto, false),
        };
        let h =
            PitchClassHistogram::from_midi(notes.iter().map(|n| (n.midi as f64, n.end - n.start)));
        let hypotheses = rank_views(&h);
        return Ok(Some(AnalysisView {
            take_id: id,
            a4_hz: a4,
            contour: Vec::new(),
            notes,
            edited: is_edited,
            hypotheses,
        }));
    }
    let Some(c) = p.load_analysis::<ContourDoc>(&id, "contour")? else {
        return Ok(None);
    };
    if (c.trim_in_seconds - offset).abs() > 1e-6 {
        return Ok(None); // trim changed since the analysis: it must be redone
    }
    let a4 = a4_of(s);
    let edited = p.load_analysis::<NotesDoc>(&id, "notes.edited")?;
    let auto = p.load_analysis::<NotesDoc>(&id, "notes")?;
    let (notes, is_edited) = match (edited, auto) {
        (Some(e), _) => (e.notes, true),
        (None, Some(a)) => (a.notes, false),
        (None, None) => (notes_from_contour(&c, a4).notes, false),
    };
    Ok(Some(AnalysisView {
        take_id: id,
        a4_hz: a4,
        contour: to_view(&c, a4),
        notes,
        edited: is_edited,
        hypotheses: hypotheses_of(&c, a4),
    }))
}

// ---------- commands ----------

/// Analyse the loaded take: contour → notes → hypotheses. Keeps the user's edited
/// notes if they exist for the same trim.
#[tauri::command]
pub async fn analyze_take(state: State<'_, AppState>) -> CmdResult<AnalysisView> {
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording first"));
    }
    if s.loaded.as_ref().is_some_and(|l| l.midi.is_some()) {
        return view_from_docs(&s)?.ok_or_else(|| err("analysis", "no notes"));
    }
    let (id, offset, buf) = s
        .loaded
        .as_ref()
        .map(|l| (l.id.clone(), l.offset_seconds, Arc::clone(&l.buffer)))
        .ok_or_else(|| err("no_take", "Nothing is loaded"))?;
    let a4 = a4_of(&s);
    let cfg = PitchConfig::new(buf.sample_rate);
    let doc = ContourDoc {
        hop_seconds: CONTOUR_HOP_SECONDS,
        trim_in_seconds: offset,
        points: contour(&buf, &cfg, CONTOUR_HOP_SECONDS),
    };
    let notes = notes_from_contour(&doc, a4);
    let p = s
        .project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    p.save_analysis(&id, "contour", &doc)?;
    p.save_analysis(&id, "notes", &notes)?;
    // An edited document from a different trim is stale: drop it.
    if let Some(e) = p.load_analysis::<NotesDoc>(&id, "notes.edited")? {
        if (e.trim_in_seconds - offset).abs() > 1e-6 {
            let _ = std::fs::remove_file(p.analysis_path(&id, "notes.edited"));
        }
    }
    view_from_docs(&s)?.ok_or_else(|| err("analysis", "Analysis could not be read back"))
}

#[tauri::command]
pub async fn get_analysis(state: State<'_, AppState>) -> CmdResult<Option<AnalysisView>> {
    let s = state.lock();
    if s.loaded.is_none() {
        return Ok(None);
    }
    view_from_docs(&s)
}

/// Replace the user's notes (a full list; the UI keeps its own undo stack).
#[tauri::command]
pub async fn update_notes(state: State<'_, AppState>, notes: Vec<Note>) -> CmdResult<AnalysisView> {
    let mut s = state.lock();
    let (id, offset) = loaded_id(&s)?;
    for n in &notes {
        let positive = n.end.partial_cmp(&n.start) == Some(std::cmp::Ordering::Greater);
        if !positive || !(0..=127).contains(&n.midi) {
            return Err(err(
                "invalid_argument",
                "A note has an empty range or a pitch outside MIDI",
            ));
        }
    }
    let a4 = a4_of(&s);
    let p = s
        .project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    let doc = NotesDoc {
        a4_hz: a4,
        trim_in_seconds: offset,
        params: NoteParams::default(),
        notes,
    };
    p.save_analysis(&id, "notes.edited", &doc)?;
    view_from_docs(&s)?.ok_or_else(|| err("analysis", "Analyse the take first"))
}

/// Discard the user's edits and go back to the automatic notes.
#[tauri::command]
pub async fn reset_notes(state: State<'_, AppState>) -> CmdResult<AnalysisView> {
    let s = state.lock();
    let (id, _) = loaded_id(&s)?;
    let p = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    let path = p.analysis_path(&id, "notes.edited");
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| err("io", e.to_string()))?;
    }
    view_from_docs(&s)?.ok_or_else(|| err("analysis", "Analyse the take first"))
}

#[tauri::command]
pub async fn set_scale(state: State<'_, AppState>, scale: Option<Scale>) -> CmdResult<ProjectView> {
    if let Some(sc) = &scale {
        if sc.is_empty() || sc.tonic_pc > 11 {
            return Err(err("invalid_argument", "Invalid scale"));
        }
    }
    let mut s = state.lock();
    let p = s
        .project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    p.update_manifest(|m| m.scale = scale)?;
    project_view(&s)
}

#[tauri::command]
pub async fn set_reference(
    state: State<'_, AppState>,
    a4_hz: Option<f64>,
) -> CmdResult<ProjectView> {
    if let Some(a4) = a4_hz {
        if !(a4.is_finite() && (400.0..=480.0).contains(&a4)) {
            return Err(err(
                "invalid_argument",
                format!("A4 = {a4} Hz is outside 400..480"),
            ));
        }
    }
    let mut s = state.lock();
    let p = s
        .project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    p.update_manifest(|m| m.a4_hz = a4_hz)?;
    project_view(&s)
}

#[derive(Debug, Clone, Serialize)]
pub struct MidiExportResult {
    pub path: String,
    pub bytes: u64,
    pub notes: usize,
    pub tempo_bpm: f64,
    /// True when no project tempo exists and the technical 60 BPM mapping was used.
    pub technical_tempo: bool,
    pub readme_path: Option<String>,
}

/// Export the current notes (edited if present) as a Standard MIDI File.
#[tauri::command]
pub async fn export_midi(state: State<'_, AppState>, path: String) -> CmdResult<MidiExportResult> {
    let s = state.lock();
    let view = view_from_docs(&s)?.ok_or_else(|| err("analysis", "Analyse the take first"))?;
    if view.notes.is_empty() {
        return Err(err("no_notes", "There are no notes to export"));
    }
    let m = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?
        .manifest();
    let project_tempo = m.tempo_map.first().map(|t| t.bpm);
    let ts = m
        .time_signatures
        .first()
        .map(|t| (t.numerator as u8, t.unit as u8));
    let opts = SmfOptions {
        tempo_bpm: project_tempo.unwrap_or(TECHNICAL_TEMPO_BPM),
        time_signature: ts,
        ..SmfOptions::default()
    };
    let bytes = write_smf(&view.notes, &opts);
    let mut p = PathBuf::from(&path);
    if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("mid")) {
        p.set_extension("mid");
    }
    std::fs::write(&p, &bytes).map_err(|e| err("io", e.to_string()))?;
    let readme_path = if project_tempo.is_none() {
        let rp = readme_for(&p);
        let text = format!(
            "EthnoFlow Studio MIDI export\n\
             This project has no tempo set, so the file uses a TECHNICAL tempo of {bpm} BPM:\n\
             one quarter note = one second, {tpq} ticks per second. Set your DAW's tempo to {bpm}\n\
             to keep the original timing. Pitch bend range: ±{range} semitones (RPN 0).\n\n\
             Проектът няма зададено темпо, затова файлът ползва ТЕХНИЧЕСКО темпо {bpm} BPM:\n\
             една четвъртина = една секунда. Задай {bpm} BPM в DAW-а, за да запазиш времето.\n",
            bpm = TECHNICAL_TEMPO_BPM,
            tpq = opts.ticks_per_quarter,
            range = opts.bend_range_semitones
        );
        std::fs::write(&rp, text).map_err(|e| err("io", e.to_string()))?;
        Some(rp.to_string_lossy().into_owned())
    } else {
        None
    };
    Ok(MidiExportResult {
        path: p.to_string_lossy().into_owned(),
        bytes: bytes.len() as u64,
        notes: view.notes.len(),
        tempo_bpm: opts.tempo_bpm,
        technical_tempo: project_tempo.is_none(),
        readme_path,
    })
}

fn readme_for(mid: &Path) -> PathBuf {
    let stem = mid
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "export".into());
    mid.with_file_name(format!("{stem}.README_DAW_IMPORT.txt"))
}

// ---------- input monitor (tuner without recording) ----------

#[tauri::command]
pub async fn start_monitor(
    state: State<'_, AppState>,
    device: Option<String>,
    sample_rate: Option<u32>,
) -> CmdResult<()> {
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err(
            "recording",
            "The tuner follows the recording while it runs",
        ));
    }
    if let Some(m) = s.monitor.take() {
        m.stop();
    }
    let req = StreamRequest {
        sample_rate,
        channels: None,
    };
    s.monitor = Some(InputMonitor::start(device.as_deref(), req)?);
    Ok(())
}

#[tauri::command]
pub async fn stop_monitor(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(m) = state.lock().monitor.take() {
        m.stop();
    }
    Ok(())
}

#[tauri::command]
pub async fn monitor_status(state: State<'_, AppState>) -> CmdResult<Option<MonitorStatus>> {
    let mut s = state.lock();
    let st = s.monitor.as_ref().map(|m| m.status());
    if st.as_ref().is_some_and(|st| st.device_lost) {
        if let Some(m) = s.monitor.take() {
            m.stop();
        }
    }
    Ok(st)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readme_sits_next_to_the_midi_file() {
        let r = readme_for(Path::new("C:/x/Песен - Запис 1.mid"));
        assert_eq!(
            r.file_name().unwrap().to_string_lossy(),
            "Песен - Запис 1.README_DAW_IMPORT.txt"
        );
        assert_eq!(r.parent(), Some(Path::new("C:/x")));
    }

    #[test]
    fn contour_view_is_bounded_and_converted() {
        let doc = ContourDoc {
            hop_seconds: 0.01,
            trim_in_seconds: 0.0,
            points: (0..10_000)
                .map(|i| ContourPoint {
                    t: i as f64 * 0.01,
                    hz: Some(440.0),
                    confidence: 1.0,
                    rms: 0.1,
                })
                .collect(),
        };
        let v = to_view(&doc, 440.0);
        assert!(v.len() <= MAX_VIEW_POINTS && v.len() >= MAX_VIEW_POINTS / 2);
        assert!((v[0].midi.unwrap() - 69.0).abs() < 1e-9);
        let h = hypotheses_of(&doc, 440.0);
        assert_eq!(h.len(), MAX_HYPOTHESES);
        let n = notes_from_contour(&doc, 440.0);
        assert_eq!(n.notes.len(), 1);
        assert_eq!(n.notes[0].midi, 69);
    }
}

// ---------------------------------------------------------------- loudness of rendered files

#[derive(Debug, Clone, Serialize)]
pub struct WavStat {
    pub path: String,
    pub rms_dbfs: f64,
    pub peak_dbfs: f64,
    pub seconds: f64,
}

/// RMS and peak of WAV files over `[from_s, to_s)` (the whole file when omitted): the
/// numbers the auto-balance and the assistant reason with. Mono-summed.
#[tauri::command]
pub async fn wav_stats(
    paths: Vec<String>,
    from_s: Option<f64>,
    to_s: Option<f64>,
) -> CmdResult<Vec<WavStat>> {
    if paths.len() > 32 {
        return Err(err("invalid_argument", "at most 32 files"));
    }
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let buf = audio_engine::read_wav(std::path::Path::new(&p))?;
        let mono = buf.to_mono();
        let sr = mono.sample_rate.max(1) as f64;
        let a = ((from_s.unwrap_or(0.0).max(0.0)) * sr) as usize;
        let z = to_s
            .map(|t| ((t.max(0.0)) * sr) as usize)
            .unwrap_or(usize::MAX)
            .min(mono.samples.len());
        let slice = if a < z {
            &mono.samples[a..z]
        } else {
            &mono.samples[0..0]
        };
        let n = slice.len().max(1) as f64;
        let sum: f64 = slice.iter().map(|x| (*x as f64) * (*x as f64)).sum();
        let peak = slice.iter().fold(0.0f64, |m, x| m.max(x.abs() as f64));
        let db = |x: f64| if x > 0.0 { 20.0 * x.log10() } else { -120.0 };
        out.push(WavStat {
            path: p,
            rms_dbfs: db((sum / n).sqrt()),
            peak_dbfs: db(peak),
            seconds: slice.len() as f64 / sr,
        });
    }
    Ok(out)
}
