//! Project and take commands. A project must be open for anything that touches takes.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use audio_engine::{
    compute_peaks, decode_file, linear_to_dbfs, read_wav, write_wav, AudioBuffer, WavFormat,
    WaveformPeaks, IMPORT_EXTENSIONS,
};
use project::{
    OpenReport, Project, Take, TakeHealth, TakeSource, TakeState, TempoPoint, TimeSignaturePoint,
};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::commands::{err, finish_recording, CmdResult, Timing};
use crate::settings;
use crate::state::{AppState, Inner, LoadedTake};

// ---------- views sent to the UI ----------

#[derive(Debug, Clone, Serialize)]
pub struct TakeView {
    pub id: String,
    pub number: u32,
    pub label: Option<String>,
    pub source: TakeSource,
    pub state: TakeState,
    pub created_at_ms: i64,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
    pub duration_seconds: Option<f64>,
    pub trim_in_seconds: f64,
    pub trim_out_seconds: Option<f64>,
    pub original_name: Option<String>,
    pub device: Option<String>,
    pub overrun_samples: u64,
    pub clipped_samples: u64,
    /// Result of the last checksum check (None = not checked yet).
    pub health: Option<TakeHealth>,
    pub derived_from: Option<String>,
}

pub fn take_view(s: &Inner, t: &Take) -> TakeView {
    let sr = t.sample_rate.unwrap_or(0).max(1) as f64;
    TakeView {
        id: t.id.clone(),
        number: t.number,
        label: t.label.clone(),
        source: t.source,
        state: t.state,
        created_at_ms: t.created_at_ms,
        sample_rate: t.sample_rate,
        channels: t.channels,
        duration_seconds: t.frames.map(|f| f as f64 / sr),
        trim_in_seconds: t.trim_in_frames as f64 / sr,
        trim_out_seconds: t.trim_out_frames.map(|f| f as f64 / sr),
        original_name: t.original_name.clone(),
        device: t.device.clone(),
        overrun_samples: t.overrun_samples,
        clipped_samples: t.clipped_samples,
        health: s.health.get(&t.id).copied(),
        derived_from: t.derived_from.clone(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectView {
    pub name: String,
    pub root: String,
    pub timing: Timing,
    pub takes: Vec<TakeView>,
    pub loaded_take: Option<String>,
    /// Scale the user chose, if any (never inferred silently).
    pub scale: Option<music_core::Scale>,
    /// Tuning reference in effect (project value or the 440 Hz default).
    pub a4_hz: f64,
    /// The project's instrument profile (from the global store), if attached.
    pub instrument: Option<project::InstrumentProfile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectOpened {
    pub project: ProjectView,
    pub report: OpenReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoadedTakeInfo {
    pub id: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: usize,
    pub duration_seconds: f64,
    pub peak_dbfs: f32,
    pub rms_dbfs: f32,
    pub clipped_samples: usize,
    /// Seconds cut from the start of the original by the (non-destructive) trim.
    pub offset_seconds: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecentProject {
    pub path: String,
    pub name: String,
    pub exists: bool,
}

fn project_mut(s: &mut Inner) -> CmdResult<&mut Project> {
    s.project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))
}

fn project_ref(s: &Inner) -> CmdResult<&Project> {
    s.project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))
}

fn view(s: &Inner) -> CmdResult<ProjectView> {
    let p = project_ref(s)?;
    let takes = p.takes()?.iter().map(|t| take_view(s, t)).collect();
    Ok(ProjectView {
        name: p.manifest().name.clone(),
        root: p.root().to_string_lossy().into_owned(),
        timing: Timing::from_manifest(p.manifest()),
        takes,
        loaded_take: s.loaded.as_ref().map(|l| l.id.clone()),
        scale: p.manifest().scale.clone(),
        a4_hz: p
            .manifest()
            .a4_hz
            .unwrap_or(crate::analysis_cmds::DEFAULT_A4_HZ),
        instrument: crate::instrument_cmds::project_profile(s),
    })
}

/// The project as the UI sees it (for other command modules).
pub fn project_view(s: &Inner) -> CmdResult<ProjectView> {
    view(s)
}

/// Read a take's original into memory with its trim applied.
pub fn load_take_into(s: &mut Inner, id: &str) -> CmdResult<LoadedTakeInfo> {
    let p = project_ref(s)?;
    let take = p.take(id)?;
    if take.state == TakeState::Recording {
        return Err(err("invalid_state", "This take is still being recorded"));
    }
    let path = p.take_path(&take);
    if !path.exists() {
        s.health.insert(take.id.clone(), TakeHealth::Missing);
        return Err(err(
            "missing",
            format!("Original file is missing: {}", path.display()),
        ));
    }
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("mid"))
    {
        let a4 = p
            .manifest()
            .a4_hz
            .unwrap_or(crate::analysis_cmds::DEFAULT_A4_HZ);
        let loaded = crate::midi_cmds::load_midi_take(&take.id, &path, a4)?;
        let buf = &loaded.buffer;
        let info = LoadedTakeInfo {
            id: take.id.clone(),
            sample_rate: buf.sample_rate,
            channels: buf.channels,
            frames: buf.frames(),
            duration_seconds: buf.duration_seconds(),
            peak_dbfs: linear_to_dbfs(buf.peak()),
            rms_dbfs: linear_to_dbfs(buf.rms()),
            clipped_samples: 0,
            offset_seconds: 0.0,
        };
        s.stop_playback();
        s.loaded = Some(loaded);
        return Ok(info);
    }
    let is_wav = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("wav"));
    let full = if is_wav {
        read_wav(&path)?
    } else {
        decode_file(&path)?
    };
    let end = take
        .trim_out_frames
        .map(|f| f as usize)
        .unwrap_or(full.frames());
    let start = take.trim_in_frames as usize;
    let buf = if start > 0 || end < full.frames() {
        full.slice_frames(start, end)
    } else {
        full
    };
    let info = LoadedTakeInfo {
        id: take.id.clone(),
        sample_rate: buf.sample_rate,
        channels: buf.channels,
        frames: buf.frames(),
        duration_seconds: buf.duration_seconds(),
        peak_dbfs: linear_to_dbfs(buf.peak()),
        rms_dbfs: linear_to_dbfs(buf.rms()),
        clipped_samples: buf.clipped_sample_count(),
        offset_seconds: start as f64 / buf.sample_rate.max(1) as f64,
    };
    s.stop_playback();
    s.loaded = Some(LoadedTake {
        id: take.id,
        offset_seconds: info.offset_seconds,
        buffer: Arc::new(buf),
        midi: None,
    });
    Ok(info)
}

/// Close whatever is open: stop and finalize a running take first, so it ends
/// `complete` rather than `recovered`.
pub fn close_inner(s: &mut Inner) {
    if s.recording.is_some() {
        if let Err(e) = finish_recording(s) {
            log::error!("could not finish recording on close: {}", e.message);
        }
    }
    s.stop_playback();
    s.stop_metronome();
    s.stop_monitor();
    s.stop_live_synth();
    s.midi_input = None;
    s.loaded = None;
    s.health.clear();
    s.project = None;
}

fn install(
    app: &AppHandle,
    s: &mut Inner,
    project: Project,
    report: Option<&OpenReport>,
) -> CmdResult<ProjectView> {
    close_inner(s);
    settings::remember_project(app, project.root());
    s.health.clear();
    if let Some(r) = report {
        for c in &r.checks {
            s.health.insert(c.id.clone(), c.health);
        }
    }
    s.project = Some(project);
    view(s)
}

// ---------- project commands ----------

#[tauri::command]
pub async fn default_projects_dir(app: AppHandle) -> CmdResult<String> {
    let docs = app
        .path()
        .document_dir()
        .map_err(|e| err("no_documents_dir", e.to_string()))?;
    Ok(docs.join("EthnoFlow").to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn recent_projects(app: AppHandle) -> Vec<RecentProject> {
    settings::load(&app)
        .recent_projects
        .into_iter()
        .map(|p| {
            let path = PathBuf::from(&p);
            let name = project::Manifest::load(&path)
                .map(|m| m.name)
                .unwrap_or_else(|_| {
                    path.file_stem()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default()
                });
            RecentProject {
                exists: Project::is_project_dir(&path),
                path: p,
                name,
            }
        })
        .collect()
}

#[tauri::command]
pub async fn create_project(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    parent: Option<String>,
) -> CmdResult<ProjectView> {
    let parent = match parent {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(default_projects_dir(app.clone()).await?),
    };
    let project = Project::create(&parent, &name)?;
    install(&app, &mut state.lock(), project, None)
}

#[tauri::command]
pub async fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> CmdResult<ProjectOpened> {
    let mut path = PathBuf::from(path);
    // Accept a click on project.json as well as on the folder.
    if path.file_name().is_some_and(|n| n == "project.json") {
        path.pop();
    }
    let (project, report) = Project::open(&path)?;
    let mut s = state.lock();
    let pv = install(&app, &mut s, project, Some(&report))?;
    Ok(ProjectOpened {
        project: pv,
        report,
    })
}

#[tauri::command]
pub async fn close_project(state: State<'_, AppState>) -> CmdResult<()> {
    close_inner(&mut state.lock());
    Ok(())
}

/// Write `[in, out)` seconds of the loaded (trimmed) take as a new take in the project.
#[tauri::command]
pub async fn save_selection_as_take(
    state: State<'_, AppState>,
    in_seconds: f64,
    out_seconds: f64,
    label: Option<String>,
) -> CmdResult<ProjectView> {
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording first"));
    }
    let (parent, buf) = s
        .loaded
        .as_ref()
        .map(|l| (l.id.clone(), Arc::clone(&l.buffer)))
        .ok_or_else(|| err("no_take", "Nothing is loaded"))?;
    let sr = buf.sample_rate as f64;
    let a = (in_seconds.max(0.0) * sr).round() as usize;
    let b = (out_seconds.max(0.0) * sr).round() as usize;
    if b <= a || a >= buf.frames() {
        return Err(err("invalid_argument", "Empty selection"));
    }
    let piece = buf.slice_frames(a, b.min(buf.frames()));
    let take = project_mut(&mut s)?.create_derived_take(&parent, label.as_deref(), |path| {
        write_wav(path, &piece, WavFormat::Float32)
    })?;
    s.health.insert(take.id.clone(), TakeHealth::Ok);
    view(&s)
}

#[tauri::command]
pub async fn project_state(state: State<'_, AppState>) -> CmdResult<Option<ProjectView>> {
    let s = state.lock();
    if s.project.is_none() {
        return Ok(None);
    }
    view(&s).map(Some)
}

#[tauri::command]
pub async fn set_timing(state: State<'_, AppState>, timing: Timing) -> CmdResult<ProjectView> {
    timing.validate()?;
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err(
            "recording",
            "Stop recording before changing tempo or meter",
        ));
    }
    project_mut(&mut s)?.update_manifest(|m| {
        m.tempo_map = timing
            .bpm
            .map(|bpm| {
                vec![TempoPoint {
                    at_seconds: 0.0,
                    bpm,
                }]
            })
            .unwrap_or_default();
        m.time_signatures = match (timing.numerator, timing.unit) {
            (Some(numerator), Some(unit)) => vec![TimeSignaturePoint {
                from_bar: 1,
                numerator,
                unit,
                grouping: timing.grouping.clone(),
            }],
            _ => Vec::new(),
        };
    })?;
    view(&s)
}

// ---------- take commands ----------

#[tauri::command]
pub async fn load_take(state: State<'_, AppState>, id: String) -> CmdResult<LoadedTakeInfo> {
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording first"));
    }
    load_take_into(&mut s, &id)
}

#[tauri::command]
pub async fn get_waveform(
    state: State<'_, AppState>,
    bins: Option<usize>,
) -> CmdResult<Option<WaveformPeaks>> {
    let s = state.lock();
    let bins = bins.unwrap_or(1200).clamp(16, 20_000);
    Ok(s.loaded.as_ref().map(|l| compute_peaks(&l.buffer, bins)))
}

#[tauri::command]
pub async fn rename_take(
    state: State<'_, AppState>,
    id: String,
    label: Option<String>,
) -> CmdResult<ProjectView> {
    let mut s = state.lock();
    project_mut(&mut s)?.rename_take(&id, label.as_deref())?;
    view(&s)
}

#[tauri::command]
pub async fn delete_take(state: State<'_, AppState>, id: String) -> CmdResult<ProjectView> {
    let mut s = state.lock();
    if s.loaded.as_ref().is_some_and(|l| l.id == id) {
        s.stop_playback();
        s.loaded = None;
    }
    project_mut(&mut s)?.delete_take(&id)?;
    s.health.remove(&id);
    view(&s)
}

/// Set the trim in seconds of the *original* take. `out_seconds = None` = to the end.
#[tauri::command]
pub async fn set_trim(
    state: State<'_, AppState>,
    id: String,
    in_seconds: f64,
    out_seconds: Option<f64>,
) -> CmdResult<ProjectView> {
    let mut s = state.lock();
    let p = project_mut(&mut s)?;
    let sr = p.take(&id)?.sample_rate.unwrap_or(0) as f64;
    if sr <= 0.0 {
        return Err(err("invalid_state", "Take has no sample rate"));
    }
    let to_frames = |x: f64| (x.max(0.0) * sr).round() as u64;
    p.set_trim(&id, to_frames(in_seconds), out_seconds.map(to_frames))?;
    if s.loaded.as_ref().is_some_and(|l| l.id == id) {
        load_take_into(&mut s, &id)?;
    }
    view(&s)
}

#[tauri::command]
pub async fn relink_take(
    state: State<'_, AppState>,
    id: String,
    path: String,
) -> CmdResult<ProjectView> {
    let mut s = state.lock();
    project_mut(&mut s)?.relink_take(&id, Path::new(&path))?;
    s.health.insert(id, TakeHealth::Ok);
    view(&s)
}

#[tauri::command]
pub async fn verify_takes(state: State<'_, AppState>) -> CmdResult<ProjectView> {
    let mut s = state.lock();
    let checks = project_ref(&s)?.verify_takes()?;
    for c in checks {
        s.health.insert(c.id, c.health);
    }
    view(&s)
}

#[tauri::command]
pub async fn import_audio(state: State<'_, AppState>, path: String) -> CmdResult<ProjectView> {
    let src = PathBuf::from(&path);
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if ext == "mid" {
        let mut s = state.lock();
        if s.recording.is_some() {
            return Err(err("recording", "Stop recording first"));
        }
        return crate::midi_cmds::import_midi(&mut s, &src);
    }
    if !IMPORT_EXTENSIONS.contains(&ext.as_str()) {
        return Err(err(
            "unsupported_format",
            format!("Supported: {}, mid", IMPORT_EXTENSIONS.join(", ")),
        ));
    }
    // Decode before copying: a file we cannot read never enters the project.
    let buf: AudioBuffer = decode_file(&src)?;
    if buf.is_empty() {
        return Err(err("empty_file", "The file contains no audio"));
    }
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording first"));
    }
    let take = project_mut(&mut s)?.import_original(
        &src,
        buf.sample_rate,
        buf.channels,
        buf.frames() as u64,
    )?;
    s.health.insert(take.id.clone(), TakeHealth::Ok);
    s.stop_playback();
    s.loaded = Some(LoadedTake {
        id: take.id.clone(),
        offset_seconds: 0.0,
        buffer: Arc::new(buf),
        midi: None,
    });
    view(&s)
}

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub path: String,
    pub bytes: u64,
    pub format: WavFormat,
}

/// Export the loaded take (trim applied) to a new WAV outside the project.
#[tauri::command]
pub async fn export_wav(
    state: State<'_, AppState>,
    path: String,
    format: Option<WavFormat>,
) -> CmdResult<ExportResult> {
    let buf = {
        let s = state.lock();
        s.loaded
            .as_ref()
            .map(|l| Arc::clone(&l.buffer))
            .ok_or_else(|| err("no_take", "Nothing to export"))?
    };
    let mut p = PathBuf::from(&path);
    if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("wav")) {
        p.set_extension("wav");
    }
    let format = format.unwrap_or_default();
    write_wav(&p, &buf, format)?;
    let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
    Ok(ExportResult {
        path: p.to_string_lossy().into_owned(),
        bytes,
        format,
    })
}
