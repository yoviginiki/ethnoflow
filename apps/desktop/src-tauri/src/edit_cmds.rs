//! Audio editor (home-studio item 5 = AUDIO_EDITOR_PLAN E1, 2026-11-01): one take in
//! memory as a working copy, operations on a selection, a clipboard, and a history of
//! versions (undo / redo / jump to any step). The result is saved as a NEW take derived
//! from the original; the original file never changes (hard rule 3).
//!
//! Memory: every version is a whole buffer (simple and exact). The history keeps at most
//! `MAX_VERSIONS` steps and about `MAX_BYTES` of audio; the oldest steps are dropped
//! first (the original is always on disk, so nothing is lost).

use std::sync::Arc;

use audio_engine::{
    apply_recipe, compute_peaks, decode_file, fade, gain, linear_to_dbfs, nearest_zero_crossing,
    normalize, read_wav, reverse, write_wav, AudioBuffer, PlayOptions, Player, PlayerStatus,
    WavFormat, WaveformPeaks,
};
use project::{TakeHealth, TakeState};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::project_cmds::{view, ProjectView};
use crate::state::{AppState, Inner};

const MAX_VERSIONS: usize = 200;
const MAX_BYTES: usize = 1 << 30;

pub struct EditVersion {
    /// Operation key that produced this version ("open", "cut", "paste", …).
    pub op: String,
    pub buffer: Arc<AudioBuffer>,
}

pub struct EditSession {
    pub take_id: String,
    pub versions: Vec<EditVersion>,
    pub index: usize,
    pub clipboard: Option<Arc<AudioBuffer>>,
    /// Number of versions the budget dropped from the front (so step numbers stay stable).
    pub dropped: usize,
    /// Index of the version last saved as a take (None = never).
    pub saved_index: Option<usize>,
}

impl EditSession {
    fn current(&self) -> &Arc<AudioBuffer> {
        &self.versions[self.index].buffer
    }

    /// Append a version after the current one (drops the redo branch), within the budget.
    fn push(&mut self, op: &str, buf: AudioBuffer) {
        self.versions.truncate(self.index + 1);
        self.versions.push(EditVersion {
            op: op.to_string(),
            buffer: Arc::new(buf),
        });
        self.index = self.versions.len() - 1;
        let bytes = |v: &EditVersion| v.buffer.samples.len() * 4;
        while self.versions.len() > 1
            && (self.versions.len() > MAX_VERSIONS
                || self.versions.iter().map(bytes).sum::<usize>() > MAX_BYTES)
        {
            self.versions.remove(0);
            self.index -= 1;
            self.dropped += 1;
            self.saved_index = self.saved_index.and_then(|i| i.checked_sub(1));
        }
    }
}

/// One operation on the working copy. Seconds are absolute in the current version.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOp {
    /// Remove [a, b) and put it on the clipboard.
    Cut { a: f64, b: f64 },
    /// Copy [a, b) to the clipboard (no change; no history step).
    Copy { a: f64, b: f64 },
    /// Remove [a, b); what follows moves left.
    Delete { a: f64, b: f64 },
    /// Insert the clipboard at `at`.
    Paste { at: f64 },
    /// Replace [a, b) with silence of the same length.
    Silence { a: f64, b: f64 },
    /// Keep only [a, b).
    Trim { a: f64, b: f64 },
    /// Insert `seconds` of silence at `at`.
    InsertSilence { at: f64, seconds: f64 },
    /// Gain in dB on [a, b).
    Gain { a: f64, b: f64, db: f64 },
    /// Peak-normalise [a, b) to `target_dbfs`.
    Normalize { a: f64, b: f64, target_dbfs: f64 },
    /// Linear fade in over [a, b).
    FadeIn { a: f64, b: f64 },
    /// Linear fade out over [a, b).
    FadeOut { a: f64, b: f64 },
    /// Reverse [a, b).
    Reverse { a: f64, b: f64 },
}

impl EditOp {
    fn key(&self) -> &'static str {
        match self {
            EditOp::Cut { .. } => "cut",
            EditOp::Copy { .. } => "copy",
            EditOp::Delete { .. } => "delete",
            EditOp::Paste { .. } => "paste",
            EditOp::Silence { .. } => "silence",
            EditOp::Trim { .. } => "trim",
            EditOp::InsertSilence { .. } => "insert_silence",
            EditOp::Gain { .. } => "gain",
            EditOp::Normalize { .. } => "normalize",
            EditOp::FadeIn { .. } => "fade_in",
            EditOp::FadeOut { .. } => "fade_out",
            EditOp::Reverse { .. } => "reverse",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct EditVersionInfo {
    /// Absolute step number (stable while the budget drops old steps).
    pub step: usize,
    pub op: String,
    pub seconds: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct EditStatus {
    pub take_id: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub seconds: f64,
    pub peak_dbfs: f32,
    /// Position in `versions` (0-based; the step shown is `versions[index].step`).
    pub index: usize,
    pub versions: Vec<EditVersionInfo>,
    pub clipboard_seconds: Option<f64>,
    pub can_undo: bool,
    pub can_redo: bool,
    /// True when the current version differs from the opened one and was not saved.
    pub dirty: bool,
}

fn status_of(e: &EditSession) -> EditStatus {
    let cur = e.current();
    EditStatus {
        take_id: e.take_id.clone(),
        sample_rate: cur.sample_rate,
        channels: cur.channels,
        seconds: cur.duration_seconds(),
        peak_dbfs: linear_to_dbfs(cur.peak()),
        index: e.index,
        versions: e
            .versions
            .iter()
            .enumerate()
            .map(|(i, v)| EditVersionInfo {
                step: e.dropped + i,
                op: v.op.clone(),
                seconds: v.buffer.duration_seconds(),
            })
            .collect(),
        clipboard_seconds: e.clipboard.as_ref().map(|c| c.duration_seconds()),
        can_undo: e.index > 0,
        can_redo: e.index + 1 < e.versions.len(),
        dirty: e.index + e.dropped > 0 && e.saved_index != Some(e.index),
    }
}

fn session(s: &mut Inner) -> CmdResult<&mut EditSession> {
    s.edit
        .as_mut()
        .ok_or_else(|| err("no_edit", "No take is open in the editor"))
}

/// Frames of [a, b) clamped to the buffer; errors on an empty region.
fn region(buf: &AudioBuffer, a: f64, b: f64) -> CmdResult<(usize, usize)> {
    let sr = buf.sample_rate as f64;
    let n = buf.frames();
    let fa = ((a.max(0.0) * sr).round() as usize).min(n);
    let fb = ((b.max(0.0) * sr).round() as usize).min(n);
    if fb <= fa {
        return Err(err("invalid_argument", "Empty selection"));
    }
    Ok((fa, fb))
}

fn at_frame(buf: &AudioBuffer, at: f64) -> usize {
    ((at.max(0.0) * buf.sample_rate as f64).round() as usize).min(buf.frames())
}

/// Concatenate parts (same rate and channel count: all come from this session).
fn concat(sample_rate: u32, channels: u16, parts: &[&AudioBuffer]) -> AudioBuffer {
    let total: usize = parts.iter().map(|p| p.samples.len()).sum();
    let mut samples = Vec::with_capacity(total);
    for p in parts {
        samples.extend_from_slice(&p.samples);
    }
    AudioBuffer::new(sample_rate, channels, samples)
}

/// Replace [fa, fb) of `buf` with `f(slice)`.
fn replace_region(
    buf: &AudioBuffer,
    fa: usize,
    fb: usize,
    f: impl FnOnce(&AudioBuffer) -> AudioBuffer,
) -> AudioBuffer {
    let head = buf.slice_frames(0, fa);
    let mid = f(&buf.slice_frames(fa, fb));
    let tail = buf.slice_frames(fb, buf.frames());
    concat(buf.sample_rate, buf.channels, &[&head, &mid, &tail])
}

/// The clipboard converted to the session's channel count (mono ⇄ stereo), same rate.
fn fit_channels(src: &AudioBuffer, channels: u16) -> AudioBuffer {
    if src.channels == channels {
        return src.clone();
    }
    let mono = src.to_mono();
    let mut samples = Vec::with_capacity(mono.samples.len() * channels as usize);
    for v in &mono.samples {
        for _ in 0..channels {
            samples.push(*v);
        }
    }
    AudioBuffer::new(src.sample_rate, channels, samples)
}

fn load_take_full(s: &Inner, take_id: &str) -> CmdResult<AudioBuffer> {
    let p = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    let take = p.take(take_id)?;
    if take.state == TakeState::Recording {
        return Err(err("invalid_state", "This take is still being recorded"));
    }
    let path = p.take_path(&take);
    if !path.exists() {
        return Err(err(
            "missing",
            format!("Original file is missing: {}", path.display()),
        ));
    }
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if ext == "mid" {
        return Err(err(
            "invalid_argument",
            "MIDI takes are edited in the piano roll, not in the audio editor",
        ));
    }
    let full = if ext == "wav" {
        read_wav(&path)?
    } else {
        decode_file(&path)?
    };
    // The take is edited as it is heard: trim and recipe applied.
    let end = take
        .trim_out_frames
        .map(|f| f as usize)
        .unwrap_or(full.frames())
        .min(full.frames());
    let start = (take.trim_in_frames as usize).min(end);
    let trimmed = if start > 0 || end < full.frames() {
        full.slice_frames(start, end)
    } else {
        full
    };
    Ok(apply_recipe(&trimmed, &take.recipe))
}

#[tauri::command]
pub async fn edit_open(state: State<'_, AppState>, take_id: String) -> CmdResult<EditStatus> {
    let mut s = state.lock();
    let buf = load_take_full(&s, &take_id)?;
    if buf.is_empty() {
        return Err(err("invalid_argument", "The take is empty"));
    }
    let e = EditSession {
        take_id,
        versions: vec![EditVersion {
            op: "open".into(),
            buffer: Arc::new(buf),
        }],
        index: 0,
        clipboard: None,
        dropped: 0,
        saved_index: None,
    };
    let st = status_of(&e);
    s.edit = Some(e);
    Ok(st)
}

#[tauri::command]
pub async fn edit_status(state: State<'_, AppState>) -> CmdResult<Option<EditStatus>> {
    let s = state.lock();
    Ok(s.edit.as_ref().map(status_of))
}

#[tauri::command]
pub async fn edit_close(state: State<'_, AppState>) -> CmdResult<()> {
    state.lock().edit = None;
    Ok(())
}

/// Peaks of the current version, of [from_s, to_s) when given (zoom).
#[tauri::command]
pub async fn edit_peaks(
    state: State<'_, AppState>,
    bins: usize,
    from_s: Option<f64>,
    to_s: Option<f64>,
) -> CmdResult<WaveformPeaks> {
    let mut s = state.lock();
    let e = session(&mut s)?;
    let cur = Arc::clone(e.current());
    let bins = bins.clamp(16, 20_000);
    match (from_s, to_s) {
        (Some(a), Some(b)) => {
            let (fa, fb) = region(&cur, a, b)?;
            Ok(compute_peaks(&cur.slice_frames(fa, fb), bins))
        }
        _ => Ok(compute_peaks(&cur, bins)),
    }
}

#[tauri::command]
pub async fn edit_apply(state: State<'_, AppState>, op: EditOp) -> CmdResult<EditStatus> {
    let mut s = state.lock();
    let e = session(&mut s)?;
    let cur = Arc::clone(e.current());
    let (sr, ch) = (cur.sample_rate, cur.channels);
    let next: Option<AudioBuffer> = match &op {
        EditOp::Cut { a, b } => {
            let (fa, fb) = region(&cur, *a, *b)?;
            e.clipboard = Some(Arc::new(cur.slice_frames(fa, fb)));
            Some(replace_region(&cur, fa, fb, |_| AudioBuffer::empty(sr, ch)))
        }
        EditOp::Copy { a, b } => {
            let (fa, fb) = region(&cur, *a, *b)?;
            e.clipboard = Some(Arc::new(cur.slice_frames(fa, fb)));
            None
        }
        EditOp::Delete { a, b } => {
            let (fa, fb) = region(&cur, *a, *b)?;
            Some(replace_region(&cur, fa, fb, |_| AudioBuffer::empty(sr, ch)))
        }
        EditOp::Paste { at } => {
            let clip = e
                .clipboard
                .as_ref()
                .ok_or_else(|| err("invalid_state", "The clipboard is empty"))?;
            let piece = fit_channels(clip, ch);
            let f = at_frame(&cur, *at);
            Some(replace_region(&cur, f, f, |_| piece))
        }
        EditOp::Silence { a, b } => {
            let (fa, fb) = region(&cur, *a, *b)?;
            Some(replace_region(&cur, fa, fb, |m| {
                AudioBuffer::new(sr, ch, vec![0.0; m.samples.len()])
            }))
        }
        EditOp::Trim { a, b } => {
            let (fa, fb) = region(&cur, *a, *b)?;
            Some(cur.slice_frames(fa, fb))
        }
        EditOp::InsertSilence { at, seconds } => {
            if !(*seconds > 0.0 && *seconds <= 600.0) {
                return Err(err("invalid_argument", "silence length 0..600 s"));
            }
            let f = at_frame(&cur, *at);
            let n = (seconds * sr as f64).round() as usize;
            Some(replace_region(&cur, f, f, |_| {
                AudioBuffer::new(sr, ch, vec![0.0; n * ch as usize])
            }))
        }
        EditOp::Gain { a, b, db } => {
            if !(-60.0..=24.0).contains(db) {
                return Err(err("invalid_argument", "gain −60..+24 dB"));
            }
            let (fa, fb) = region(&cur, *a, *b)?;
            Some(replace_region(&cur, fa, fb, |m| gain(m, *db)))
        }
        EditOp::Normalize { a, b, target_dbfs } => {
            if !(-40.0..=0.0).contains(target_dbfs) {
                return Err(err("invalid_argument", "target −40..0 dBFS"));
            }
            let (fa, fb) = region(&cur, *a, *b)?;
            Some(replace_region(&cur, fa, fb, |m| normalize(m, *target_dbfs)))
        }
        EditOp::FadeIn { a, b } => {
            let (fa, fb) = region(&cur, *a, *b)?;
            Some(replace_region(&cur, fa, fb, |m| {
                fade(m, m.duration_seconds(), 0.0)
            }))
        }
        EditOp::FadeOut { a, b } => {
            let (fa, fb) = region(&cur, *a, *b)?;
            Some(replace_region(&cur, fa, fb, |m| {
                fade(m, 0.0, m.duration_seconds())
            }))
        }
        EditOp::Reverse { a, b } => {
            let (fa, fb) = region(&cur, *a, *b)?;
            Some(replace_region(&cur, fa, fb, reverse))
        }
    };
    if let Some(buf) = next {
        if buf.is_empty() {
            return Err(err("invalid_argument", "That would leave no audio"));
        }
        e.push(op.key(), buf);
    }
    Ok(status_of(e))
}

#[tauri::command]
pub async fn edit_undo(state: State<'_, AppState>) -> CmdResult<EditStatus> {
    let mut s = state.lock();
    let e = session(&mut s)?;
    if e.index > 0 {
        e.index -= 1;
    }
    Ok(status_of(e))
}

#[tauri::command]
pub async fn edit_redo(state: State<'_, AppState>) -> CmdResult<EditStatus> {
    let mut s = state.lock();
    let e = session(&mut s)?;
    if e.index + 1 < e.versions.len() {
        e.index += 1;
    }
    Ok(status_of(e))
}

/// Jump to an absolute step number (as listed in the status).
#[tauri::command]
pub async fn edit_jump(state: State<'_, AppState>, step: usize) -> CmdResult<EditStatus> {
    let mut s = state.lock();
    let e = session(&mut s)?;
    let i = step
        .checked_sub(e.dropped)
        .filter(|i| *i < e.versions.len())
        .ok_or_else(|| err("invalid_argument", "No such step"))?;
    e.index = i;
    Ok(status_of(e))
}

/// The nearest zero crossing to `seconds` (within 10 ms), for clean cuts.
#[tauri::command]
pub async fn edit_snap(state: State<'_, AppState>, seconds: f64) -> CmdResult<f64> {
    let mut s = state.lock();
    let e = session(&mut s)?;
    let cur = e.current();
    let sr = cur.sample_rate as f64;
    let f = at_frame(cur, seconds);
    let z = nearest_zero_crossing(cur, f, (sr * 0.01) as usize);
    Ok(z as f64 / sr)
}

/// Play the current version from `from_s` (to `to_s` when given) on the chosen output.
/// Uses the main player, so "Stop" everywhere stops it too.
#[tauri::command]
pub async fn edit_play(
    state: State<'_, AppState>,
    from_s: f64,
    to_s: Option<f64>,
    device: Option<String>,
) -> CmdResult<PlayerStatus> {
    let mut s = state.lock();
    s.stop_playback();
    let e = session(&mut s)?;
    let cur = Arc::clone(e.current());
    let sr = cur.sample_rate as f64;
    let start = at_frame(&cur, from_s);
    let buf = match to_s {
        Some(b) => {
            let (fa, fb) = region(&cur, from_s, b)?;
            cur.slice_frames(fa, fb)
        }
        None => cur.slice_frames(start, cur.frames()),
    };
    if buf.is_empty() {
        return Err(err("invalid_argument", "Nothing to play"));
    }
    let player = Player::start_with(&buf, device.as_deref(), 0, &PlayOptions::default())?;
    let st = player.status();
    s.player = Some(player);
    let _ = sr;
    Ok(st)
}

#[derive(Debug, Clone, Serialize)]
pub struct EditSaved {
    pub project: ProjectView,
    pub take_id: String,
}

/// Write the current version as a new take derived from the opened one.
#[tauri::command]
pub async fn edit_save(state: State<'_, AppState>, label: Option<String>) -> CmdResult<EditSaved> {
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording first"));
    }
    let (parent, buf, index) = {
        let e = session(&mut s)?;
        (e.take_id.clone(), Arc::clone(e.current()), e.index)
    };
    let take = s
        .project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?
        .create_derived_take(&parent, label.as_deref(), |path| {
            write_wav(path, &buf, WavFormat::Float32)
        })?;
    s.health.insert(take.id.clone(), TakeHealth::Ok);
    if let Some(e) = s.edit.as_mut() {
        e.saved_index = Some(index);
    }
    let project = view(&s)?;
    Ok(EditSaved {
        project,
        take_id: take.id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_with(buf: AudioBuffer) -> EditSession {
        EditSession {
            take_id: "t".into(),
            versions: vec![EditVersion {
                op: "open".into(),
                buffer: Arc::new(buf),
            }],
            index: 0,
            clipboard: None,
            dropped: 0,
            saved_index: None,
        }
    }

    #[test]
    fn cut_paste_and_silence_move_and_keep_the_audio() {
        let sr = 1000u32;
        let buf = AudioBuffer::new(sr, 1, (0..3000).map(|i| i as f32).collect());
        let mut e = session_with(buf);
        let cur = Arc::clone(e.current());
        let (fa, fb) = region(&cur, 1.0, 2.0).unwrap();
        e.clipboard = Some(Arc::new(cur.slice_frames(fa, fb)));
        e.push(
            "cut",
            replace_region(&cur, fa, fb, |_| AudioBuffer::empty(sr, 1)),
        );
        assert_eq!(e.current().frames(), 2000);
        assert_eq!(e.current().samples[1000], 2000.0, "the tail moved left");
        let cur = Arc::clone(e.current());
        let piece = fit_channels(e.clipboard.as_ref().unwrap(), 1);
        e.push("paste", replace_region(&cur, 0, 0, |_| piece));
        assert_eq!(e.current().frames(), 3000);
        assert_eq!(
            e.current().samples[0],
            1000.0,
            "the clipboard is at the front"
        );
        let st = status_of(&e);
        assert_eq!(st.versions.len(), 3);
        assert!(st.can_undo && !st.can_redo && st.dirty);
        e.index = 0;
        assert_eq!(e.current().samples[0], 0.0, "undo to the opened version");
        let st = status_of(&e);
        assert!(!st.dirty && st.can_redo);
    }

    #[test]
    fn the_budget_drops_the_oldest_steps_but_keeps_step_numbers() {
        let buf = AudioBuffer::new(1000, 1, vec![0.5; 1000]);
        let mut e = session_with(buf);
        for _ in 0..(MAX_VERSIONS + 10) {
            let cur = Arc::clone(e.current());
            e.push("gain", gain(&cur, -0.1));
        }
        assert_eq!(e.versions.len(), MAX_VERSIONS);
        assert_eq!(e.dropped, 11);
        let st = status_of(&e);
        assert_eq!(st.versions[0].step, 11);
        assert_eq!(st.versions.last().unwrap().step, MAX_VERSIONS + 10);
    }

    #[test]
    fn stereo_clipboard_fits_a_mono_session_and_back() {
        let st = AudioBuffer::new(100, 2, vec![0.2, 0.4, 0.6, 0.8]);
        let mono = fit_channels(&st, 1);
        assert_eq!(mono.channels, 1);
        assert!((mono.samples[0] - 0.3).abs() < 1e-6);
        let back = fit_channels(&mono, 2);
        for (x, want) in back.samples.iter().zip([0.3f32, 0.3, 0.7, 0.7]) {
            assert!((x - want).abs() < 1e-6, "{x} vs {want}");
        }
    }
}
