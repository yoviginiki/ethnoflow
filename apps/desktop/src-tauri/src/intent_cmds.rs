//! Error-tolerant performance intelligence (F2.9–F2.11) on the loaded take: phrases,
//! note classes, intent per phrase and the tracked tonal context. The user switches it
//! off (literal notes) or picks Stable / Assisted / Creative; marks on notes are saved
//! with the take and calibrate the classifier per user.

use music_core::{
    analyze_performance, classify, segment_phrases, AnalysisInput, Annotation, Calibration,
    ClassifierParams, ClassifyInput, Locks, Mode, ModeParams, NoteClass, PerformanceAnalysis,
    PhraseParams, ScaleKind,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::analysis_cmds::current_notes;
use crate::commands::{err, CmdResult};
use crate::state::{AppState, Inner};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentSettings {
    /// Off = literal: every note is taken as meant.
    pub enabled: bool,
    pub mode: Mode,
    /// 0..1 sliders ("tolerance to errors", "sensitivity to changes").
    pub tolerance: f64,
    pub sensitivity: f64,
    pub locks: Locks,
}

impl IntentSettings {
    pub(crate) fn mode(&self) -> Mode {
        if self.enabled {
            self.mode
        } else {
            Mode::Off
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnnotationsDoc {
    pub annotations: Vec<Annotation>,
}

/// Per-user calibration pairs (what the classifier said, what the user marked).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CalibrationDoc {
    pub(crate) pairs: Vec<(NoteClass, NoteClass)>,
}

const MAX_CALIBRATION_PAIRS: usize = 500;

fn calibration_path(s: &Inner) -> Option<std::path::PathBuf> {
    s.profiles_dir
        .as_ref()
        .and_then(|d| d.parent().map(|p| p.join("intent_calibration.json")))
}

pub(crate) fn load_calibration(s: &Inner) -> CalibrationDoc {
    calibration_path(s)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_calibration(s: &Inner, doc: &CalibrationDoc) {
    if let Some(p) = calibration_path(s) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(text) = serde_json::to_string_pretty(doc) {
            if let Err(e) = std::fs::write(&p, text) {
                log::warn!("could not save intent calibration: {e}");
            }
        }
    }
}

pub(crate) fn profile_offsets(s: &Inner) -> Vec<(i32, f64)> {
    crate::instrument_cmds::project_profile(s)
        .map(|p| {
            p.notes
                .iter()
                .map(|n| (n.midi as i32, n.cents_offset))
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn phrase_params(s: &Inner) -> PhraseParams {
    let wind = crate::instrument_cmds::project_profile(s).is_some_and(|p| {
        matches!(
            p.instrument,
            project::InstrumentType::Kaval
                | project::InstrumentType::Shakuhachi
                | project::InstrumentType::Flute
                | project::InstrumentType::Recorder
                | project::InstrumentType::Gaida
                | project::InstrumentType::Voice
        )
    });
    let mut p = if wind {
        PhraseParams::for_wind()
    } else {
        PhraseParams::default()
    };
    if let Some(pr) = s.project.as_ref() {
        let m = pr.manifest();
        if let (Some(t), Some(ts)) = (m.tempo_map.first(), m.time_signatures.first()) {
            if t.bpm > 0.0 {
                p.bar_seconds = Some(60.0 / t.bpm * ts.numerator as f64 * 4.0 / ts.unit as f64);
            }
        }
    }
    p
}

fn load_annotations(s: &Inner, take_id: &str) -> CmdResult<Vec<Annotation>> {
    let p = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    Ok(p.load_analysis::<AnnotationsDoc>(take_id, "annotations")?
        .map(|d| d.annotations)
        .unwrap_or_default())
}

/// The F2.11 analysis for other commands (harmony).
pub fn analyze_for(s: &Inner, settings: &IntentSettings) -> CmdResult<Option<PerformanceAnalysis>> {
    analyze(s, settings)
}

fn analyze(s: &Inner, settings: &IntentSettings) -> CmdResult<Option<PerformanceAnalysis>> {
    let Some(loaded) = s.loaded.as_ref() else {
        return Ok(None);
    };
    let notes = current_notes(s)?;
    if notes.is_empty() {
        return Ok(None);
    }
    let annotations = load_annotations(s, &loaded.id)?;
    let scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
    let profile = profile_offsets(s);
    let mode = settings.mode();
    let mut mode_params = mode.default_params();
    if mode != Mode::Off {
        mode_params = ModeParams {
            tolerance: settings.tolerance.clamp(0.0, 1.0),
            sensitivity: settings.sensitivity.clamp(0.0, 1.0),
        };
    }
    let calibration = Calibration::from_marks(&load_calibration(s).pairs);
    Ok(Some(analyze_performance(&AnalysisInput {
        notes: &notes,
        scale: scale.as_ref(),
        profile: &profile,
        expected: &[],
        annotations: &annotations,
        mode,
        mode_params,
        locks: settings.locks,
        phrase_params: phrase_params(s),
        calibration,
        kinds: &ScaleKind::ALL,
    })))
}

#[tauri::command]
pub async fn performance_analysis(
    state: State<'_, AppState>,
    settings: IntentSettings,
) -> CmdResult<Option<PerformanceAnalysis>> {
    let s = state.lock();
    analyze(&s, &settings)
}

/// Mark a note (or clear the mark with `class = None`); saved with the take, and the
/// disagreement with the classifier feeds the per-user calibration.
#[tauri::command]
pub async fn annotate_note(
    state: State<'_, AppState>,
    settings: IntentSettings,
    index: usize,
    class: Option<NoteClass>,
) -> CmdResult<Option<PerformanceAnalysis>> {
    let s = state.lock();
    let loaded = s
        .loaded
        .as_ref()
        .ok_or_else(|| err("no_take", "Nothing is loaded"))?;
    let take_id = loaded.id.clone();
    let notes = current_notes(&s)?;
    if index >= notes.len() {
        return Err(err("invalid_argument", "note index out of range"));
    }
    let mut annotations = load_annotations(&s, &take_id)?;
    annotations.retain(|a| a.index != index);
    if let Some(class) = class {
        // What would the classifier have said on its own? That pair calibrates.
        let scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
        let profile = profile_offsets(&s);
        let phrases = segment_phrases(&notes, &phrase_params(&s));
        let said = classify(&ClassifyInput {
            notes: &notes,
            phrases: &phrases,
            scale: scale.as_ref(),
            profile: &profile,
            expected: &[],
            annotations: &[],
            params: &ClassifierParams::default().with_tolerance(settings.tolerance),
            literal: false,
        })
        .into_iter()
        .find(|c| c.index == index)
        .map(|c| c.class);
        if let Some(said) = said {
            if said != class {
                let mut doc = load_calibration(&s);
                doc.pairs.push((said, class));
                if doc.pairs.len() > MAX_CALIBRATION_PAIRS {
                    let drop = doc.pairs.len() - MAX_CALIBRATION_PAIRS;
                    doc.pairs.drain(0..drop);
                }
                save_calibration(&s, &doc);
            }
        }
        annotations.push(Annotation { index, class });
    }
    annotations.sort_by_key(|a| a.index);
    let p = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    p.save_analysis(&take_id, "annotations", &AnnotationsDoc { annotations })?;
    analyze(&s, &settings)
}

#[tauri::command]
pub async fn intent_annotations(state: State<'_, AppState>) -> CmdResult<Vec<Annotation>> {
    let s = state.lock();
    let Some(loaded) = s.loaded.as_ref() else {
        return Ok(Vec::new());
    };
    load_annotations(&s, &loaded.id)
}
