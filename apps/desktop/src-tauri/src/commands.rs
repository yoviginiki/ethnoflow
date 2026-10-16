//! Tauri commands = the typed boundary between the UI and the engine.
//! Raw audio frames never go through here; only reduced data (peaks, status, info).
//! Project/take commands live in `project_cmds.rs`.

use std::sync::Arc;

use audio_engine::{
    list_devices, AudioBuffer, ClickConfig, DeviceInfo, Direction, Metronome, MetronomeStatus,
    PlayOptions, Player, PlayerStatus, Recorder, RecorderStatus, StreamRequest, WaveformPeaks,
};
use project::{Manifest, ProjectError};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::project_cmds::{load_take_into, take_view, TakeView};
use crate::state::{ActiveRecording, AppState};

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
        err(code, e.to_string())
    }
}

impl From<ProjectError> for CmdError {
    fn from(e: ProjectError) -> Self {
        use ProjectError as E;
        let code = match &e {
            E::Io(_) => "io",
            E::Db(_) => "database",
            E::Json(_) => "project_json",
            E::Audio(_) => "audio",
            E::NotAProject(_) => "not_a_project",
            E::AlreadyExists(_) => "already_exists",
            E::InvalidName => "invalid_name",
            E::TakeNotFound(_) => "take_not_found",
            E::NewerSchema { .. } => "newer_schema",
            E::ChecksumMismatch(_) => "checksum_mismatch",
            E::InvalidState(_) => "invalid_state",
        };
        err(code, e.to_string())
    }
}

pub fn err(code: &str, message: impl Into<String>) -> CmdError {
    CmdError {
        code: code.into(),
        message: message.into(),
    }
}

pub type CmdResult<T> = Result<T, CmdError>;

/// Tempo and meter as the UI edits them. All optional: nothing is assumed.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Timing {
    pub bpm: Option<f64>,
    pub numerator: Option<u32>,
    pub unit: Option<u32>,
    #[serde(default)]
    pub grouping: Vec<u32>,
}

impl Timing {
    pub fn from_manifest(m: &Manifest) -> Self {
        let ts = m.time_signatures.first();
        Self {
            bpm: m.tempo_map.first().map(|t| t.bpm),
            numerator: ts.map(|t| t.numerator),
            unit: ts.map(|t| t.unit),
            grouping: ts.map(|t| t.grouping.clone()).unwrap_or_default(),
        }
    }

    /// A click needs both tempo and meter; otherwise there is no metronome.
    pub fn click(&self, level: f32) -> CmdResult<ClickConfig> {
        match (self.bpm, self.numerator, self.unit) {
            (Some(bpm), Some(numerator), Some(unit)) => {
                let c = ClickConfig {
                    bpm,
                    numerator,
                    unit,
                    grouping: self.grouping.clone(),
                    level,
                };
                c.validate()?;
                Ok(c)
            }
            _ => Err(err("no_timing", "Set tempo and meter first")),
        }
    }

    /// Each part is optional, but what is given must be valid.
    pub fn validate(&self) -> CmdResult<()> {
        let bad = |m: String| Err(err("invalid_argument", m));
        if let Some(b) = self.bpm {
            if !(b.is_finite() && (20.0..=600.0).contains(&b)) {
                return bad(format!("tempo {b} outside 20..600"));
            }
        }
        match (self.numerator, self.unit) {
            (None, None) => {}
            (Some(n), Some(u)) => {
                if n == 0 || n > 32 {
                    return bad(format!("numerator {n} outside 1..32"));
                }
                if !matches!(u, 1 | 2 | 4 | 8 | 16 | 32) {
                    return bad(format!("unit {u} is not a power of two up to 32"));
                }
                if !self.grouping.is_empty()
                    && (self.grouping.contains(&0) || self.grouping.iter().sum::<u32>() != n)
                {
                    return bad(format!(
                        "grouping {:?} does not add up to {n}",
                        self.grouping
                    ));
                }
            }
            _ => return bad("meter needs both numerator and unit".into()),
        }
        Ok(())
    }

    pub fn bar_seconds(&self) -> Option<f64> {
        Some(self.numerator? as f64 * 60.0 / self.bpm?)
    }
}

const CLICK_LEVEL: f32 = 0.5;

pub fn project_timing(s: &crate::state::Inner) -> CmdResult<Timing> {
    let p = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    Ok(Timing::from_manifest(p.manifest()))
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

/// 1 s of 440 Hz at -18 dBFS on the chosen output, to check routing and level.
#[tauri::command]
pub fn play_test_tone(state: State<AppState>, device: Option<String>) -> CmdResult<()> {
    let mut s = state.lock();
    if let Some(t) = s.tone.take() {
        t.stop();
    }
    let tone = AudioBuffer::sine(48_000, 440.0, 1.0, 10f32.powf(-18.0 / 20.0));
    s.tone = Some(Player::start(&tone, device.as_deref(), 0)?);
    Ok(())
}

// ---------- recording ----------

#[derive(Debug, Clone, Serialize)]
pub struct RecordingStarted {
    pub take_id: String,
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub count_in_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RecordRequest {
    pub input: Option<String>,
    pub output: Option<String>,
    pub sample_rate: Option<u32>,
    /// Play the project click while recording.
    #[serde(default)]
    pub click: bool,
    /// Bars of click before the music starts; trimmed off (non-destructively) on stop.
    #[serde(default)]
    pub count_in_bars: u32,
}

#[tauri::command]
pub fn start_recording(state: State<AppState>, req: RecordRequest) -> CmdResult<RecordingStarted> {
    let mut guard = state.lock();
    let s = &mut *guard;
    if s.recording.is_some() {
        return Err(err("already_recording", "Recording is already in progress"));
    }
    let timing = project_timing(s)?;
    let click = if req.click || req.count_in_bars > 0 {
        Some(timing.click(CLICK_LEVEL)?)
    } else {
        None
    };
    let count_in_seconds = match timing.bar_seconds() {
        Some(bar) if click.is_some() => req.count_in_bars as f64 * bar,
        _ => 0.0,
    };
    s.stop_playback();
    s.stop_metronome();
    let project = s
        .project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    let take = project.begin_recording(req.input.as_deref())?;
    let path = project.take_path(&take);
    let request = StreamRequest {
        sample_rate: req.sample_rate,
        channels: None,
    };
    // Click first, so the count-in is audible from the start of the take.
    let metronome = match &click {
        Some(cfg) => match Metronome::start(req.output.as_deref(), cfg) {
            Ok(m) => Some(m),
            Err(e) => {
                let _ = project.abort_recording(&take.id);
                return Err(e.into());
            }
        },
        None => None,
    };
    let rec = match Recorder::start_with(req.input.as_deref(), &path, request) {
        Ok(r) => r,
        Err(e) => {
            if let Some(m) = metronome {
                m.stop();
            }
            let _ = project.abort_recording(&take.id);
            return Err(e.into());
        }
    };
    s.metronome = metronome;
    let started = RecordingStarted {
        take_id: take.id.clone(),
        device: rec.device_name().to_string(),
        sample_rate: rec.sample_rate(),
        channels: rec.channels(),
        count_in_seconds,
    };
    s.recording = Some(ActiveRecording {
        recorder: rec,
        take_id: take.id,
        count_in_seconds,
    });
    Ok(started)
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingStatusView {
    #[serde(flatten)]
    pub recorder: RecorderStatus,
    pub metronome: Option<MetronomeStatus>,
    pub count_in_seconds: f64,
}

#[tauri::command]
pub fn recording_status(state: State<AppState>) -> CmdResult<Option<RecordingStatusView>> {
    let s = state.lock();
    Ok(s.recording.as_ref().map(|r| RecordingStatusView {
        recorder: r.recorder.status(),
        metronome: s.metronome.as_ref().map(|m| m.status()),
        count_in_seconds: r.count_in_seconds,
    }))
}

/// Waveform of the take being recorded (reduced peaks only).
#[tauri::command]
pub fn recording_waveform(
    state: State<AppState>,
    bins: Option<usize>,
) -> CmdResult<Option<WaveformPeaks>> {
    let s = state.lock();
    let bins = bins.unwrap_or(1200).clamp(16, 20_000);
    Ok(s.recording.as_ref().map(|r| r.recorder.live_peaks(bins)))
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordingStopped {
    pub take: TakeView,
    pub frames_lost: u64,
    pub write_error: Option<String>,
    pub device_lost: bool,
}

/// Stop, finalize the file, record checksum and facts, and select the new take.
pub fn finish_recording(s: &mut crate::state::Inner) -> CmdResult<RecordingStopped> {
    let active = s
        .recording
        .take()
        .ok_or_else(|| err("not_recording", "No recording in progress"))?;
    s.stop_metronome();
    let recorded = active.recorder.stop()?;
    if let Some(e) = &recorded.write_error {
        log::error!("take {} incomplete: {e}", active.take_id);
    }
    let project = s
        .project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    let mut take = project.finish_recording(
        &active.take_id,
        recorded.overrun_samples,
        recorded.clipped_samples,
    )?;
    let count_in = (active.count_in_seconds * recorded.sample_rate as f64).round() as u64;
    if count_in > 0 && count_in < recorded.frames {
        take = project.set_trim(&take.id, count_in, None)?;
    }
    s.health.insert(take.id.clone(), project::TakeHealth::Ok);
    load_take_into(s, &take.id)?;
    let view = take_view(s, &take);
    Ok(RecordingStopped {
        take: view,
        frames_lost: recorded.frames_captured.saturating_sub(recorded.frames),
        write_error: recorded.write_error,
        device_lost: recorded.device_lost,
    })
}

#[tauri::command]
pub fn stop_recording(state: State<AppState>) -> CmdResult<RecordingStopped> {
    finish_recording(&mut state.lock())
}

// ---------- playback ----------

#[derive(Debug, Clone, Deserialize)]
pub struct PlayRequest {
    pub device: Option<String>,
    pub start_seconds: f64,
    /// Loop `[start, end)` seconds of the loaded (trimmed) take.
    pub loop_seconds: Option<(f64, f64)>,
    #[serde(default)]
    pub click: bool,
}

#[tauri::command]
pub fn start_playback(state: State<AppState>, req: PlayRequest) -> CmdResult<PlayerStatus> {
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording before playback"));
    }
    s.stop_playback();
    let buf: Arc<AudioBuffer> = s
        .loaded
        .as_ref()
        .map(|l| Arc::clone(&l.buffer))
        .ok_or_else(|| err("no_take", "Nothing to play"))?;
    let sr = buf.sample_rate as f64;
    let opts = PlayOptions {
        loop_frames: req
            .loop_seconds
            .map(|(a, b)| ((a.max(0.0) * sr) as usize, (b.max(0.0) * sr) as usize)),
        click: if req.click {
            Some(project_timing(&s)?.click(CLICK_LEVEL)?)
        } else {
            None
        },
    };
    let start = (req.start_seconds.max(0.0) * sr) as usize;
    let player = Player::start_with(&buf, req.device.as_deref(), start, &opts)?;
    let st = player.status();
    s.player = Some(player);
    Ok(st)
}

#[tauri::command]
pub fn playback_status(state: State<AppState>) -> CmdResult<Option<PlayerStatus>> {
    let mut s = state.lock();
    let st = s.player.as_ref().map(|p| p.status());
    if st.as_ref().is_some_and(|st| st.finished) {
        s.stop_playback();
    }
    Ok(st)
}

#[tauri::command]
pub fn set_playback_loop(
    state: State<AppState>,
    loop_seconds: Option<(f64, f64)>,
) -> CmdResult<()> {
    let s = state.lock();
    let (Some(p), Some(l)) = (s.player.as_ref(), s.loaded.as_ref()) else {
        return Ok(());
    };
    let sr = l.buffer.sample_rate as f64;
    p.set_loop(loop_seconds.map(|(a, b)| ((a.max(0.0) * sr) as usize, (b.max(0.0) * sr) as usize)));
    Ok(())
}

#[tauri::command]
pub fn set_playback_click(state: State<AppState>, on: bool) -> CmdResult<()> {
    if let Some(p) = state.lock().player.as_ref() {
        p.set_click(on);
    }
    Ok(())
}

#[tauri::command]
pub fn stop_playback(state: State<AppState>) -> CmdResult<()> {
    state.lock().stop_playback();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timing(bpm: Option<f64>, n: Option<u32>, u: Option<u32>, g: &[u32]) -> Timing {
        Timing {
            bpm,
            numerator: n,
            unit: u,
            grouping: g.to_vec(),
        }
    }

    #[test]
    fn empty_timing_is_valid_but_has_no_click() {
        let t = timing(None, None, None, &[]);
        assert!(t.validate().is_ok());
        assert_eq!(t.click(0.5).unwrap_err().code, "no_timing");
        assert_eq!(t.bar_seconds(), None);
    }

    #[test]
    fn odd_meter_with_grouping_gives_a_click_and_bar_length() {
        let t = timing(Some(280.0), Some(7), Some(8), &[2, 2, 3]);
        assert!(t.validate().is_ok());
        assert_eq!(t.click(0.5).unwrap().grouping, vec![2, 2, 3]);
        assert!((t.bar_seconds().unwrap() - 1.5).abs() < 1e-12);
    }

    #[test]
    fn invalid_timing_is_rejected() {
        assert!(timing(Some(5.0), None, None, &[]).validate().is_err());
        assert!(timing(None, Some(7), None, &[]).validate().is_err());
        assert!(timing(None, Some(7), Some(6), &[]).validate().is_err());
        assert!(timing(None, Some(7), Some(8), &[2, 2]).validate().is_err());
        assert!(timing(None, Some(0), Some(4), &[]).validate().is_err());
    }

    #[test]
    fn manifest_roundtrip_keeps_user_timing_only() {
        let mut m = Manifest::new("x", 0);
        assert!(Timing::from_manifest(&m).bpm.is_none());
        m.tempo_map.push(project::TempoPoint {
            at_seconds: 0.0,
            bpm: 96.0,
        });
        m.time_signatures.push(project::TimeSignaturePoint {
            from_bar: 1,
            numerator: 9,
            unit: 8,
            grouping: vec![2, 2, 2, 3],
        });
        let t = Timing::from_manifest(&m);
        assert_eq!((t.bpm, t.numerator, t.unit), (Some(96.0), Some(9), Some(8)));
        assert_eq!(t.grouping, vec![2, 2, 2, 3]);
    }
}
