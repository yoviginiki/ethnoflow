//! Academy (F6.1–F6.3): lessons from `crates/lessons`, exercise instances inside the
//! project's scale and the instrument's range, demonstrations through the built-in
//! synth, scoring, and local progress (`<config>/academy/progress.json`, no streaks).

use std::collections::BTreeMap;
use std::path::PathBuf;

use audio_engine::{render_notes, Player, PlayerStatus, SynthConfig, SynthNote, Voice};
use lessons::{Context, Instance, Lesson, PlayNote, RepeatScore, RhythmScore, ScaleKind};
use music_core::Note;
use project::InstrumentType;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::analysis_cmds::{NotesDoc, DEFAULT_A4_HZ};
use crate::commands::{err, CmdResult};
use crate::midi_cmds::RENDER_SAMPLE_RATE;
use crate::state::{AppState, Inner};
use crate::trainer_cmds::Level;

// ---------------------------------------------------------------- progress

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LessonProgress {
    pub attempts: u32,
    pub best_score: f64,
    pub passed_at_ms: Option<i64>,
    pub last_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    #[serde(default)]
    pub lessons: BTreeMap<String, LessonProgress>,
}

fn progress_path(s: &Inner) -> CmdResult<PathBuf> {
    s.presets_dir
        .as_ref()
        .and_then(|p| p.parent().map(|d| d.join("academy").join("progress.json")))
        .ok_or_else(|| err("no_library_dir", "Library folder is not available"))
}

fn load_progress(s: &Inner) -> CmdResult<Progress> {
    let path = progress_path(s)?;
    Ok(crate::reference_cmds::read_json(&path)?.unwrap_or_default())
}

fn save_progress(s: &Inner, p: &Progress) -> CmdResult<()> {
    let path = progress_path(s)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| err("io", e.to_string()))?;
    }
    crate::reference_cmds::write_json(&path, p)
}

// ---------------------------------------------------------------- context

/// Playable MIDI range by instrument type when the profile has no calibrated notes.
fn default_range(kind: Option<InstrumentType>) -> (i32, i32) {
    match kind {
        Some(InstrumentType::Kaval) => (62, 86),
        Some(InstrumentType::Shakuhachi) => (62, 86),
        Some(InstrumentType::Flute) => (60, 96),
        Some(InstrumentType::Recorder) => (72, 96),
        Some(InstrumentType::Gaida) => (62, 74),
        Some(InstrumentType::Voice) => (55, 76),
        Some(InstrumentType::Monochord) => (48, 72),
        Some(InstrumentType::Piano) | Some(InstrumentType::MidiKeyboard) => (48, 84),
        Some(InstrumentType::Other) | None => (57, 81),
    }
}

/// The exercise context: the project's scale (tonic + kind; custom scales fall back to
/// the nearest tempered kind the UI knows, Ionian) and the instrument's range (profile
/// notes when calibrated, else by instrument type). Without a project the context is
/// neutral (C, Ionian, a middle range) and the UI says so.
pub fn context(s: &Inner) -> Context {
    let scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
    let profile = crate::instrument_cmds::project_profile(s);
    let (tonic_pc, kind) = match &scale {
        Some(sc) => (sc.tonic_pc, sc.kind.unwrap_or(ScaleKind::Ionian)),
        None => match &profile {
            // No scale chosen: at least start on the instrument's lowest calibrated note.
            Some(p) if !p.notes.is_empty() => (p.notes[0].midi % 12, ScaleKind::Ionian),
            _ => (0, ScaleKind::Ionian),
        },
    };
    let (lo, hi) = match &profile {
        Some(p) if p.notes.len() >= 2 => {
            let lo = p.notes.iter().map(|n| n.midi as i32).min().unwrap_or(60);
            let hi = p.notes.iter().map(|n| n.midi as i32).max().unwrap_or(72);
            (lo, hi.max(lo + 7))
        }
        Some(p) => default_range(Some(p.instrument)),
        None => default_range(None),
    };
    Context {
        tonic_pc,
        kind,
        lo_midi: lo,
        hi_midi: hi,
    }
}

// ---------------------------------------------------------------- views

#[derive(Debug, Clone, Serialize)]
pub struct LessonView {
    #[serde(flatten)]
    pub lesson: Lesson,
    pub progress: LessonProgress,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcademyView {
    pub lessons: Vec<LessonView>,
    pub context: Context,
    pub has_project: bool,
    pub has_scale: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstanceView {
    pub seed: u64,
    pub context: Context,
    pub instance: Instance,
    /// Demo notes when the lesson has a demonstration.
    pub demo: Option<Vec<PlayNote>>,
}

fn all_lessons() -> CmdResult<Vec<Lesson>> {
    lessons::builtin().map_err(|e| err("lessons", e))
}

fn find(id: &str) -> CmdResult<Lesson> {
    all_lessons()?
        .into_iter()
        .find(|l| l.id == id)
        .ok_or_else(|| err("not_found", format!("no lesson {id}")))
}

#[tauri::command]
pub async fn academy_lessons(state: State<'_, AppState>) -> CmdResult<AcademyView> {
    let s = state.lock();
    let progress = load_progress(&s)?;
    let lessons = all_lessons()?
        .into_iter()
        .map(|l| {
            let p = progress.lessons.get(&l.id).cloned().unwrap_or_default();
            LessonView {
                lesson: l,
                progress: p,
            }
        })
        .collect();
    Ok(AcademyView {
        lessons,
        context: context(&s),
        has_project: s.project.is_some(),
        has_scale: s
            .project
            .as_ref()
            .is_some_and(|p| p.manifest().scale.is_some()),
    })
}

#[tauri::command]
pub async fn academy_instance(
    state: State<'_, AppState>,
    id: String,
    seed: Option<u64>,
) -> CmdResult<InstanceView> {
    let lesson = find(&id)?;
    let s = state.lock();
    let ctx = context(&s);
    let seed = seed.unwrap_or_else(|| crate::reference_cmds::now_ms() as u64);
    let instance = lessons::instance(&lesson, seed, &ctx);
    let demo = lesson.demo.as_ref().map(|d| {
        // Demos follow the lesson's range override too (e.g. the kaba lesson).
        let dctx = match lesson.range {
            Some([lo, hi]) => Context {
                lo_midi: lo,
                hi_midi: hi,
                ..ctx
            },
            None => ctx,
        };
        lessons::demo_notes(d, &dctx)
    });
    Ok(InstanceView {
        seed,
        context: ctx,
        instance,
        demo,
    })
}

/// Render a note list through the built-in synth and play it on the output device
/// (the Studio's "Play the notes" player, so status/stop commands are shared).
#[tauri::command]
pub async fn academy_play(
    state: State<'_, AppState>,
    notes: Vec<PlayNote>,
    device: Option<String>,
    voice: Option<Voice>,
) -> CmdResult<PlayerStatus> {
    if notes.is_empty() || notes.len() > 512 {
        return Err(err("invalid_argument", "1..512 notes"));
    }
    for n in &notes {
        if !(n.start.is_finite() && n.end.is_finite() && n.end > n.start && n.end <= 120.0)
            || !(0..=127).contains(&n.midi)
            || !(0.0..=1.0).contains(&n.velocity)
        {
            return Err(err("invalid_argument", "bad note"));
        }
    }
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording first"));
    }
    s.stop_playback();
    let a4 = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(DEFAULT_A4_HZ);
    let cfg = SynthConfig {
        a4_hz: a4,
        voice: voice.unwrap_or(Voice::Flute),
        ..SynthConfig::new(RENDER_SAMPLE_RATE)
    };
    let synth: Vec<SynthNote> = notes
        .iter()
        .map(|n| SynthNote {
            start: n.start,
            end: n.end,
            midi: n.midi,
            cents: 0.0,
            velocity: n.velocity,
            bend: Vec::new(),
        })
        .collect();
    let buf = render_notes(&synth, &cfg);
    let player = Player::start(&buf, device.as_deref(), 0)?;
    let st = player.status();
    s.notes_player = Some(player);
    Ok(st)
}

// ---------------------------------------------------------------- scoring

#[tauri::command]
pub async fn academy_score_rhythm(
    onsets: Vec<f64>,
    grid: Vec<f64>,
    level: Level,
) -> CmdResult<RhythmScore> {
    if onsets.len() > 512 || grid.len() > 512 || onsets.iter().chain(&grid).any(|v| !v.is_finite())
    {
        return Err(err("invalid_argument", "bad onsets"));
    }
    Ok(lessons::score_rhythm(&onsets, &grid, level.tolerance().ms))
}

#[tauri::command]
pub async fn academy_score_repeat(
    expected: Vec<PlayNote>,
    played: Vec<Note>,
    level: Level,
) -> CmdResult<RepeatScore> {
    if expected.is_empty() || expected.len() > 64 || played.len() > 512 {
        return Err(err("invalid_argument", "bad note lists"));
    }
    Ok(lessons::score_repeat(
        &expected,
        &played,
        &level.tolerance(),
    ))
}

/// Record an attempt; `passed` is decided by the caller from the lesson's pass score.
#[tauri::command]
pub async fn academy_record(
    state: State<'_, AppState>,
    id: String,
    score: f64,
    passed: bool,
) -> CmdResult<LessonProgress> {
    if !(0.0..=1.0).contains(&score) {
        return Err(err("invalid_argument", "score 0..1"));
    }
    find(&id)?;
    let s = state.lock();
    let mut p = load_progress(&s)?;
    let now = crate::reference_cmds::now_ms();
    let e = p.lessons.entry(id).or_default();
    e.attempts += 1;
    e.best_score = e.best_score.max(score);
    e.last_at_ms = Some(now);
    if passed && e.passed_at_ms.is_none() {
        e.passed_at_ms = Some(now);
    }
    let out = e.clone();
    save_progress(&s, &p)?;
    Ok(out)
}

#[tauri::command]
pub async fn academy_reset(state: State<'_, AppState>, id: Option<String>) -> CmdResult<()> {
    let s = state.lock();
    let mut p = load_progress(&s)?;
    match id {
        Some(id) => {
            p.lessons.remove(&id);
        }
        None => p.lessons.clear(),
    }
    save_progress(&s, &p)
}

/// Guided check "new take": a take recorded after `since_ms` whose stored notes (edited
/// or automatic) number at least `min_notes`. Returns the take id when satisfied.
#[tauri::command]
pub async fn academy_new_take(
    state: State<'_, AppState>,
    since_ms: i64,
    min_notes: u32,
) -> CmdResult<Option<String>> {
    let s = state.lock();
    let Some(p) = s.project.as_ref() else {
        return Ok(None);
    };
    let takes = p.takes()?;
    for t in takes.iter().filter(|t| t.created_at_ms >= since_ms) {
        let edited = p.load_analysis::<NotesDoc>(&t.id, "notes.edited")?;
        let auto = p.load_analysis::<NotesDoc>(&t.id, "notes")?;
        let n = edited
            .map(|d| d.notes.len())
            .or_else(|| auto.map(|d| d.notes.len()))
            .unwrap_or(0);
        if n as u32 >= min_notes {
            return Ok(Some(t.id.clone()));
        }
    }
    Ok(None)
}

/// "Play along": the lesson phrase becomes a MIDI take in the project (source
/// `midi_file`, like an imported .mid) and is loaded, so the Studio trainer's Wait /
/// Rhythm modes drive it with the falling notes.
#[tauri::command]
pub async fn academy_play_along(
    state: State<'_, AppState>,
    notes: Vec<PlayNote>,
    label: String,
) -> CmdResult<crate::project_cmds::ProjectView> {
    if notes.is_empty() || notes.len() > 256 {
        return Err(err("invalid_argument", "1..256 notes"));
    }
    let mut s = state.lock();
    if s.project.is_none() {
        return Err(err("no_project", "No project is open"));
    }
    if s.mode_busy() {
        return Err(err("busy", "Stop recording or playback first"));
    }
    let core: Vec<Note> = notes
        .iter()
        .map(|n| Note {
            start: n.start,
            end: n.end,
            midi: n.midi,
            cents: 0.0,
            confidence: 1.0,
            velocity: n.velocity,
            bend: Vec::new(),
        })
        .collect();
    let bytes = music_core::write_smf(&core, &music_core::SmfOptions::default());
    let safe: String = label
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(40)
        .collect();
    let path = std::env::temp_dir().join(format!(
        "ethnoflow-academy-{}-{}.mid",
        if safe.is_empty() {
            "phrase"
        } else {
            safe.as_str()
        },
        crate::reference_cmds::now_ms()
    ));
    std::fs::write(&path, &bytes).map_err(|e| err("io", e.to_string()))?;
    let out = crate::midi_cmds::import_midi(&mut s, &path);
    let _ = std::fs::remove_file(&path);
    out
}

// ---------------------------------------------------------------- trainer history (F6.3)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainerLogEntry {
    #[serde(default)]
    pub at_ms: i64,
    pub take_id: Option<String>,
    /// listen | wait | rhythm | drill | academy
    pub mode: String,
    pub level: String,
    pub tempo_percent: u32,
    pub expected: u32,
    pub hits: u32,
    pub wrong_pitch: u32,
    pub early: u32,
    pub late: u32,
    pub missed: u32,
    pub extra: u32,
    pub mean_abs_cents: f64,
    pub mean_abs_ms: f64,
    pub latency_ms: f64,
}

const HISTORY_CAP: usize = 500;

fn history_path(s: &Inner) -> CmdResult<PathBuf> {
    Ok(progress_path(s)?.with_file_name("trainer-history.json"))
}

/// Append a trainer result (Studio trainer modes and drills) to the local history.
#[tauri::command]
pub async fn academy_trainer_log(
    state: State<'_, AppState>,
    entry: TrainerLogEntry,
) -> CmdResult<usize> {
    if entry.mode.len() > 20 || entry.level.len() > 20 || entry.tempo_percent > 400 {
        return Err(err("invalid_argument", "bad trainer entry"));
    }
    let s = state.lock();
    let path = history_path(&s)?;
    let mut list: Vec<TrainerLogEntry> =
        crate::reference_cmds::read_json(&path)?.unwrap_or_default();
    let mut e = entry;
    if e.at_ms == 0 {
        e.at_ms = crate::reference_cmds::now_ms();
    }
    list.push(e);
    if list.len() > HISTORY_CAP {
        let drop = list.len() - HISTORY_CAP;
        list.drain(..drop);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| err("io", e.to_string()))?;
    }
    crate::reference_cmds::write_json(&path, &list)?;
    Ok(list.len())
}

/// Most recent trainer results, newest first.
#[tauri::command]
pub async fn academy_trainer_history(
    state: State<'_, AppState>,
    limit: Option<usize>,
) -> CmdResult<Vec<TrainerLogEntry>> {
    let s = state.lock();
    let path = history_path(&s)?;
    let mut list: Vec<TrainerLogEntry> =
        crate::reference_cmds::read_json(&path)?.unwrap_or_default();
    list.reverse();
    list.truncate(limit.unwrap_or(20).min(HISTORY_CAP));
    Ok(list)
}

// ---------------------------------------------------------------- fingering overrides (F2.6)

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingeringOverride {
    pub midi: u8,
    pub register: u8,
    /// 8 hole states in the order T 2 3 4 | 5 6 7 8: c closed, o open, x optional, h half.
    pub holes: String,
    #[serde(default)]
    pub alt: bool,
    #[serde(default)]
    pub note: Option<lessons::Text>,
}

fn overrides_path(s: &Inner, instrument: &str) -> CmdResult<PathBuf> {
    if instrument.is_empty()
        || !instrument
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(err("invalid_argument", "bad instrument id"));
    }
    Ok(progress_path(s)?
        .with_file_name("fingerings")
        .join(format!("{instrument}.json")))
}

/// The user's corrections to a fingering chart (a copy next to the progress file; the
/// factory table in the repository never changes).
#[tauri::command]
pub async fn fingering_overrides_get(
    state: State<'_, AppState>,
    instrument: String,
) -> CmdResult<Vec<FingeringOverride>> {
    let s = state.lock();
    let path = overrides_path(&s, &instrument)?;
    Ok(crate::reference_cmds::read_json(&path)?.unwrap_or_default())
}

#[tauri::command]
pub async fn fingering_overrides_set(
    state: State<'_, AppState>,
    instrument: String,
    overrides: Vec<FingeringOverride>,
) -> CmdResult<Vec<FingeringOverride>> {
    if overrides.len() > 256 {
        return Err(err("invalid_argument", "too many overrides"));
    }
    for o in &overrides {
        if o.register > 4
            || o.holes.len() != 8
            || !o.holes.chars().all(|c| matches!(c, 'c' | 'o' | 'x' | 'h'))
        {
            return Err(err(
                "invalid_argument",
                format!("bad fingering for midi {}", o.midi),
            ));
        }
    }
    let s = state.lock();
    let path = overrides_path(&s, &instrument)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| err("io", e.to_string()))?;
    }
    crate::reference_cmds::write_json(&path, &overrides)?;
    Ok(overrides)
}
