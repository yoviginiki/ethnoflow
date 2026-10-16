//! Tauri commands = the typed boundary between the UI and the audio engine.
//! Raw audio frames never go through here; only reduced data (peaks, status, info).

use std::path::PathBuf;
use std::sync::Arc;

use audio_engine::{
    compute_peaks, linear_to_dbfs, list_devices, read_wav, write_wav, AudioBuffer, DeviceInfo,
    Direction, Player, PlayerStatus, Recorder, RecorderStatus, WavFormat, WaveformPeaks,
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};

use crate::state::{AppState, TakeSource};

/// Serialisable error shape for the frontend.
#[derive(Debug, Serialize)]
pub struct CmdError {
    pub code: String,
    pub message: String,
}

impl From<audio_engine::AudioError> for CmdError {
    fn from(e: audio_engine::AudioError) -> Self {
        use audio_engine::AudioError as E;
        let code = match &e {
            E::NoHost => "no_host",
            E::DeviceNotFound(_) => "device_not_found",
            E::NoDefaultDevice(_) => "no_default_device",
            E::UnsupportedFormat(_) => "unsupported_format",
            E::DeviceConfig(_) => "device_config",
            E::Stream(_) => "stream",
            E::NotRecording => "not_recording",
            E::NotPlaying => "not_playing",
            E::Wav(_) => "wav",
            E::Io(_) => "io",
            E::InvalidArgument(_) => "invalid_argument",
        };
        CmdError {
            code: code.into(),
            message: e.to_string(),
        }
    }
}

fn err(code: &str, message: impl Into<String>) -> CmdError {
    CmdError {
        code: code.into(),
        message: message.into(),
    }
}

type CmdResult<T> = Result<T, CmdError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TakeInfo {
    pub source: TakeSource,
    pub label: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: usize,
    pub duration_seconds: f64,
    pub peak_dbfs: f32,
    pub rms_dbfs: f32,
    pub clipped_samples: usize,
}

fn take_info_for(buf: &AudioBuffer, source: TakeSource, label: Option<String>) -> TakeInfo {
    TakeInfo {
        source,
        label,
        sample_rate: buf.sample_rate,
        channels: buf.channels,
        frames: buf.frames(),
        duration_seconds: buf.duration_seconds(),
        peak_dbfs: linear_to_dbfs(buf.peak()),
        rms_dbfs: linear_to_dbfs(buf.rms()),
        clipped_samples: buf.clipped_sample_count(),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioHostInfo {
    pub host: String,
    pub os: String,
}

// ---------- devices ----------

#[tauri::command]
pub fn audio_host() -> AudioHostInfo {
    AudioHostInfo {
        host: audio_engine::host_name(),
        os: std::env::consts::OS.to_string(),
    }
}

#[tauri::command]
pub fn list_input_devices() -> CmdResult<Vec<DeviceInfo>> {
    Ok(list_devices(Direction::Input)?)
}

#[tauri::command]
pub fn list_output_devices() -> CmdResult<Vec<DeviceInfo>> {
    Ok(list_devices(Direction::Output)?)
}

// ---------- recording ----------

#[derive(Debug, Clone, Serialize)]
pub struct RecordingStarted {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Where new takes are written until the project system (Phase 1.2) owns them.
fn recordings_dir(app: &AppHandle) -> CmdResult<PathBuf> {
    let docs = app
        .path()
        .document_dir()
        .map_err(|e| err("no_documents_dir", e.to_string()))?;
    let dir = docs.join("EthnoFlow").join("recordings");
    std::fs::create_dir_all(&dir).map_err(|e| err("io", e.to_string()))?;
    Ok(dir)
}

#[tauri::command]
pub fn start_recording(
    app: AppHandle,
    state: State<AppState>,
    device: Option<String>,
) -> CmdResult<RecordingStarted> {
    let mut s = state.lock();
    if s.recorder.is_some() {
        return Err(err("already_recording", "Recording is already in progress"));
    }
    if let Some(p) = s.player.take() {
        p.stop();
    }
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let path = recordings_dir(&app)?.join(format!("take-{ms}.wav"));
    let rec = Recorder::start(device.as_deref(), &path)?;
    let started = RecordingStarted {
        device: rec.device_name().to_string(),
        sample_rate: rec.sample_rate(),
        channels: rec.channels(),
    };
    s.recorder = Some(rec);
    Ok(started)
}

#[tauri::command]
pub fn recording_status(state: State<AppState>) -> CmdResult<Option<RecorderStatus>> {
    let s = state.lock();
    Ok(s.recorder.as_ref().map(|r| r.status()))
}

#[tauri::command]
pub fn stop_recording(state: State<AppState>) -> CmdResult<TakeInfo> {
    let mut s = state.lock();
    let rec = s
        .recorder
        .take()
        .ok_or_else(|| err("not_recording", "No recording in progress"))?;
    let recorded = rec.stop()?;
    if let Some(e) = &recorded.write_error {
        log::error!("take {} incomplete: {e}", recorded.path.display());
    }
    let buf = read_wav(&recorded.path)?;
    let label = recorded
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let info = take_info_for(&buf, TakeSource::Microphone, label.clone());
    s.take = Some(Arc::new(buf));
    s.take_source = Some(TakeSource::Microphone);
    s.take_label = label;
    Ok(info)
}

/// Waveform of the take being recorded (reduced peaks only).
#[tauri::command]
pub fn recording_waveform(
    state: State<AppState>,
    bins: Option<usize>,
) -> CmdResult<Option<WaveformPeaks>> {
    let s = state.lock();
    let bins = bins.unwrap_or(1200).clamp(16, 20_000);
    Ok(s.recorder.as_ref().map(|r| r.live_peaks(bins)))
}

// ---------- take / waveform ----------

#[tauri::command]
pub fn take_info(state: State<AppState>) -> CmdResult<Option<TakeInfo>> {
    let s = state.lock();
    Ok(s.take.as_ref().map(|b| {
        take_info_for(
            b,
            s.take_source.clone().unwrap_or(TakeSource::None),
            s.take_label.clone(),
        )
    }))
}

#[tauri::command]
pub fn get_waveform(
    state: State<AppState>,
    bins: Option<usize>,
) -> CmdResult<Option<WaveformPeaks>> {
    let s = state.lock();
    let bins = bins.unwrap_or(1200).clamp(16, 20_000);
    Ok(s.take.as_ref().map(|b| compute_peaks(b, bins)))
}

#[tauri::command]
pub fn clear_take(state: State<AppState>) -> CmdResult<()> {
    let mut s = state.lock();
    if let Some(p) = s.player.take() {
        p.stop();
    }
    s.take = None;
    s.take_source = None;
    s.take_label = None;
    Ok(())
}

// ---------- playback ----------

#[tauri::command]
pub fn start_playback(
    state: State<AppState>,
    device: Option<String>,
    start_seconds: Option<f64>,
) -> CmdResult<PlayerStatus> {
    let mut s = state.lock();
    if s.recorder.is_some() {
        return Err(err("recording", "Stop recording before playback"));
    }
    if let Some(p) = s.player.take() {
        p.stop();
    }
    let buf = s
        .take
        .clone()
        .ok_or_else(|| err("no_take", "Nothing to play"))?;
    let start_frame = (start_seconds.unwrap_or(0.0).max(0.0) * buf.sample_rate as f64) as usize;
    let player = Player::start(&buf, device.as_deref(), start_frame)?;
    let st = player.status();
    s.player = Some(player);
    Ok(st)
}

#[tauri::command]
pub fn playback_status(state: State<AppState>) -> CmdResult<Option<PlayerStatus>> {
    let mut s = state.lock();
    let st = s.player.as_ref().map(|p| p.status());
    if let Some(st) = &st {
        if st.finished {
            if let Some(p) = s.player.take() {
                p.stop();
            }
        }
    }
    Ok(st)
}

#[tauri::command]
pub fn stop_playback(state: State<AppState>) -> CmdResult<()> {
    let mut s = state.lock();
    if let Some(p) = s.player.take() {
        p.stop();
    }
    Ok(())
}

// ---------- files ----------

#[derive(Debug, Clone, Serialize)]
pub struct ExportResult {
    pub path: String,
    pub bytes: u64,
    pub format: WavFormat,
}

#[tauri::command]
pub fn export_wav(
    state: State<AppState>,
    path: String,
    format: Option<WavFormat>,
) -> CmdResult<ExportResult> {
    let buf = {
        let s = state.lock();
        s.take
            .clone()
            .ok_or_else(|| err("no_take", "Nothing to export"))?
    };
    let mut p = PathBuf::from(&path);
    if p.extension()
        .map(|e| !e.eq_ignore_ascii_case("wav"))
        .unwrap_or(true)
    {
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

#[tauri::command]
pub fn import_wav(state: State<AppState>, path: String) -> CmdResult<TakeInfo> {
    let buf = read_wav(&path)?;
    if buf.is_empty() {
        return Err(err("empty_file", "The WAV file contains no audio"));
    }
    let label = PathBuf::from(&path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    let mut s = state.lock();
    if let Some(p) = s.player.take() {
        p.stop();
    }
    let info = take_info_for(&buf, TakeSource::ImportedFile, label.clone());
    s.take = Some(Arc::new(buf));
    s.take_source = Some(TakeSource::ImportedFile);
    s.take_label = label;
    Ok(info)
}
