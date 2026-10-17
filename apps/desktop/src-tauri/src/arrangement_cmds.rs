//! Arrangement commands (F5.1): tracks of clips over the project's takes, saved as
//! `arrangement.json`, played on the audio graph (slot 2) by a `ClipSource` with a
//! shared transport — so the drone and the pad (slots 0 and 1) sound together with
//! the clips, through one master level and one limiter. Takes stay immutable: a clip
//! is a window into one. Offline bounce uses the same graph code (`render_offline`).

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::graph::params::db_to_gain;
use audio_engine::resample::resample_linear;
use audio_engine::{
    compute_peaks, decode_file, read_wav, write_wav, AudioBuffer, ClipData, ClipSource,
    ClipsParams, Graph, GraphCmd, GraphParams, WavFormat, WaveformPeaks, MAX_CLIP_TRACKS,
};
use project::{Arrangement, TakeState};
use serde::Serialize;
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::graph_cmds::{ensure_graph, release_slot};
use crate::project_cmds::ExportResult;
use crate::state::{AppState, Inner};

pub const ARRANGE_SLOT: usize = 2;
/// Rate used for peaks and bounces while no output stream is open.
const OFFLINE_RATE: u32 = 48_000;

#[derive(Debug, Clone, Serialize)]
pub struct ArrangementView {
    pub arrangement: Arrangement,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArrangeStatus {
    pub playing: bool,
    pub position_seconds: f64,
    pub duration_seconds: f64,
    pub finished: bool,
    pub sample_rate: u32,
    pub layer_peak: f32,
    pub master_peak: f32,
    pub limiter_gain: f32,
    pub stream_errors: u64,
}

fn project_ref(s: &Inner) -> CmdResult<&project::Project> {
    s.project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))
}

/// The arrangement of the open project, loaded from disk on first use.
pub fn arrangement_mut(s: &mut Inner) -> CmdResult<&mut Arrangement> {
    if s.arrangement.is_none() {
        let root = project_ref(s)?.root().to_path_buf();
        s.arrangement = Some(Arrangement::load(&root)?);
    }
    Ok(s.arrangement.as_mut().expect("just loaded"))
}

fn view(s: &mut Inner) -> CmdResult<ArrangementView> {
    let a = arrangement_mut(s)?;
    Ok(ArrangementView {
        duration_seconds: a.duration(),
        arrangement: a.clone(),
    })
}

/// A take's playable audio (trim applied, mono, at `rate`), cached per take/trim/rate.
pub(crate) fn take_buffer(s: &mut Inner, take_id: &str, rate: u32) -> CmdResult<Arc<AudioBuffer>> {
    let p = project_ref(s)?;
    let take = p.take(take_id)?;
    if take.state == TakeState::Recording {
        return Err(err("invalid_state", "This take is still being recorded"));
    }
    let key = format!(
        "{}:{}:{:?}:{}:{}",
        take.id,
        take.trim_in_frames,
        take.trim_out_frames,
        rate,
        serde_json::to_string(&take.recipe).unwrap_or_default()
    );
    if let Some(b) = s.clip_cache.get(&key) {
        return Ok(Arc::clone(b));
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
    let trimmed = if ext == "mid" {
        let a4 = p
            .manifest()
            .a4_hz
            .unwrap_or(crate::analysis_cmds::DEFAULT_A4_HZ);
        let loaded = crate::midi_cmds::load_midi_take(&take.id, &path, a4)?;
        Arc::try_unwrap(loaded.buffer).unwrap_or_else(|a| (*a).clone())
    } else {
        let full = if ext == "wav" {
            read_wav(&path)?
        } else {
            decode_file(&path)?
        };
        let end = take
            .trim_out_frames
            .map(|f| f as usize)
            .unwrap_or(full.frames())
            .min(full.frames());
        let start = (take.trim_in_frames as usize).min(end);
        if start > 0 || end < full.frames() {
            full.slice_frames(start, end)
        } else {
            full
        }
    };
    let processed = audio_engine::apply_recipe(&trimmed, &take.recipe);
    let mono = processed.to_mono();
    let at_rate = if mono.sample_rate == rate {
        mono
    } else {
        resample_linear(&mono, rate)
    };
    let buf = Arc::new(at_rate);
    s.clip_cache.insert(key, Arc::clone(&buf));
    Ok(buf)
}

/// Drop cached audio no clip refers to any more.
fn prune_cache(s: &mut Inner) {
    let used: Vec<String> = s
        .arrangement
        .as_ref()
        .map(|a| {
            a.tracks
                .iter()
                .flat_map(|t| t.clips.iter().map(|c| format!("{}:", c.take_id)))
                .collect()
        })
        .unwrap_or_default();
    // Audio loops of the ambient rack share this cache.
    let mut used = used;
    if let Some(a) = s.ambient.as_ref() {
        for l in &a.layers {
            if let project::LayerSource::Loop { take_id, .. } = &l.source {
                used.push(format!("{take_id}:"));
            }
        }
    }
    s.clip_cache
        .retain(|k, _| used.iter().any(|prefix| k.starts_with(prefix)));
}

fn frames(seconds: f64, rate: u32) -> u64 {
    (seconds.max(0.0) * rate as f64).round() as u64
}

fn build_clips(s: &mut Inner, a: &Arrangement, rate: u32) -> CmdResult<Vec<ClipData>> {
    let mut out = Vec::new();
    for (ti, t) in a.tracks.iter().enumerate().take(MAX_CLIP_TRACKS) {
        for c in &t.clips {
            let buffer = take_buffer(s, &c.take_id, rate)?;
            let offset = frames(c.offset, rate);
            let avail = (buffer.frames() as u64).saturating_sub(offset);
            let length = frames(c.length, rate).min(avail);
            if length == 0 {
                continue;
            }
            out.push(ClipData {
                track: ti,
                buffer,
                start: frames(c.start, rate),
                offset,
                length,
                gain: db_to_gain(c.gain_db as f32),
                fade_in: frames(c.fade_in, rate).min(length),
                fade_out: frames(c.fade_out, rate).min(length),
                mute: c.mute,
            });
        }
    }
    Ok(out)
}

/// Push track gain/mute/solo, the end and the loop region to the transport.
fn apply_tracks(params: &ClipsParams, a: &Arrangement, rate: u32) {
    for (i, tp) in params.tracks.iter().enumerate() {
        match a.tracks.get(i) {
            Some(t) => {
                tp.gain_db.set(t.gain_db as f32);
                tp.mute.store(t.mute, Ordering::Relaxed);
                tp.solo.store(t.solo, Ordering::Relaxed);
            }
            None => {
                tp.gain_db.set(0.0);
                tp.mute.store(false, Ordering::Relaxed);
                tp.solo.store(false, Ordering::Relaxed);
            }
        }
    }
    let tr = &params.transport;
    tr.end_frames
        .store(frames(a.duration(), rate), Ordering::Relaxed);
    match a.loop_region {
        Some((x, y)) => {
            tr.loop_start.store(frames(x, rate), Ordering::Relaxed);
            tr.loop_end.store(frames(y, rate), Ordering::Relaxed);
            tr.loop_on.store(true, Ordering::Relaxed);
        }
        None => tr.loop_on.store(false, Ordering::Relaxed),
    }
}

/// (Re)build the clip source on the running graph from the current arrangement.
fn rebuild_source(s: &mut Inner) -> CmdResult<()> {
    let Some(params) = s.clips.clone() else {
        return Ok(());
    };
    let rate = s
        .graph
        .as_ref()
        .map(|g| g.sample_rate())
        .ok_or_else(|| err("no_graph", "The output stream is not running"))?;
    let a = arrangement_mut(s)?.clone();
    let clips = build_clips(s, &a, rate)?;
    apply_tracks(&params, &a, rate);
    let graph = s.graph.as_mut().expect("checked above");
    let ok = graph.send(GraphCmd::SetSource {
        slot: ARRANGE_SLOT,
        source: Box::new(ClipSource::new(params, clips)),
    });
    if !ok {
        return Err(err("graph_busy", "Graph command queue is full"));
    }
    Ok(())
}

fn status(s: &mut Inner) -> Option<ArrangeStatus> {
    let params = s.clips.clone()?;
    let graph = s.graph.as_mut()?;
    let st = graph.status();
    let m = graph.params().meters(ARRANGE_SLOT + 1);
    let tr = &params.transport;
    let rate = st.sample_rate.max(1) as f64;
    Some(ArrangeStatus {
        playing: tr.playing.load(Ordering::Relaxed),
        position_seconds: tr.position() as f64 / rate,
        duration_seconds: tr.end_frames.load(Ordering::Relaxed) as f64 / rate,
        finished: tr.finished.load(Ordering::Relaxed),
        sample_rate: st.sample_rate,
        layer_peak: m.layers.get(ARRANGE_SLOT).copied().unwrap_or(0.0),
        master_peak: m.master_peak,
        limiter_gain: m.limiter_gain,
        stream_errors: st.stream_errors,
    })
}

/// A take was deleted: its clips go too (saved), the cache forgets it.
pub fn forget_take(s: &mut Inner, take_id: &str) {
    let Ok(root) = project_ref(s).map(|p| p.root().to_path_buf()) else {
        return;
    };
    if let Ok(a) = arrangement_mut(s) {
        let before: usize = a.tracks.iter().map(|t| t.clips.len()).sum();
        for t in a.tracks.iter_mut() {
            t.clips.retain(|c| c.take_id != take_id);
        }
        let after: usize = a.tracks.iter().map(|t| t.clips.len()).sum();
        if before != after {
            if let Err(e) = a.save(&root) {
                log::error!("could not save arrangement: {e}");
            }
            if let Err(e) = rebuild_source(s) {
                log::error!("could not rebuild clips: {}", e.message);
            }
        }
    }
    prune_cache(s);
}

#[tauri::command]
pub async fn arrangement_get(state: State<'_, AppState>) -> CmdResult<ArrangementView> {
    view(&mut state.lock())
}

/// Replace the arrangement (the UI edits a copy and sends it back; undo/redo live
/// there). Saved atomically; a playing source is rebuilt without stopping.
#[tauri::command]
pub async fn set_arrangement(
    state: State<'_, AppState>,
    arrangement: Arrangement,
) -> CmdResult<ArrangementView> {
    arrangement.validate()?;
    let mut s = state.lock();
    {
        let p = project_ref(&s)?;
        for c in arrangement.tracks.iter().flat_map(|t| t.clips.iter()) {
            p.take(&c.take_id)?;
        }
        arrangement.save(p.root())?;
    }
    s.arrangement = Some(arrangement);
    rebuild_source(&mut s)?;
    prune_cache(&mut s);
    view(&mut s)
}

/// Start (or resume) the transport; `from_seconds` seeks first.
#[tauri::command]
pub async fn arrangement_play(
    state: State<'_, AppState>,
    device: Option<String>,
    from_seconds: Option<f64>,
) -> CmdResult<ArrangeStatus> {
    let mut s = state.lock();
    ensure_graph(&mut s, device.as_deref())?;
    let rate = s.graph.as_ref().expect("graph just started").sample_rate();
    let params = match s.clips.clone() {
        Some(p) => p,
        None => {
            let p = Arc::new(ClipsParams::default());
            s.clips = Some(Arc::clone(&p));
            p
        }
    };
    if let Some(from) = from_seconds {
        params.transport.seek(frames(from, rate));
    }
    rebuild_source(&mut s)?;
    let layer = &s.graph.as_ref().expect("graph").params().layers[ARRANGE_SLOT];
    layer.pan.set(0.0);
    layer.gain_db.set(0.0);
    params.transport.finished.store(false, Ordering::Relaxed);
    params.transport.playing.store(true, Ordering::Relaxed);
    status(&mut s).ok_or_else(|| err("no_graph", "The output stream is not running"))
}

/// Pause: the position stays, the source stays on the graph.
#[tauri::command]
pub async fn arrangement_pause(state: State<'_, AppState>) -> CmdResult<Option<ArrangeStatus>> {
    let mut s = state.lock();
    if let Some(p) = s.clips.as_ref() {
        p.transport.playing.store(false, Ordering::Relaxed);
    }
    Ok(status(&mut s))
}

#[tauri::command]
pub async fn arrangement_seek(
    state: State<'_, AppState>,
    seconds: f64,
) -> CmdResult<Option<ArrangeStatus>> {
    let mut s = state.lock();
    if let (Some(p), Some(g)) = (s.clips.as_ref(), s.graph.as_ref()) {
        p.transport.seek(frames(seconds, g.sample_rate()));
    }
    Ok(status(&mut s))
}

/// Take the clips off the graph (the stream stops when the drone and pad are off too).
#[tauri::command]
pub async fn arrangement_release(state: State<'_, AppState>) -> CmdResult<()> {
    let mut s = state.lock();
    if let Some(p) = s.clips.as_ref() {
        p.transport.playing.store(false, Ordering::Relaxed);
    }
    release_slot(&mut s, ARRANGE_SLOT);
    Ok(())
}

#[tauri::command]
pub async fn arrangement_status(state: State<'_, AppState>) -> CmdResult<Option<ArrangeStatus>> {
    Ok(status(&mut state.lock()))
}

/// Peaks of a take's playable audio (for drawing clips); cached like playback.
#[tauri::command]
pub async fn take_peaks(
    state: State<'_, AppState>,
    take_id: String,
    bins: Option<usize>,
) -> CmdResult<WaveformPeaks> {
    let mut s = state.lock();
    let rate = s
        .graph
        .as_ref()
        .map(|g| g.sample_rate())
        .unwrap_or(OFFLINE_RATE);
    let buf = take_buffer(&mut s, &take_id, rate)?;
    Ok(compute_peaks(&buf, bins.unwrap_or(2000).clamp(16, 20_000)))
}

/// Bounce the arrangement (clips only, same graph code and limiter as playback) to a
/// stereo WAV outside the project.
#[tauri::command]
pub async fn export_arrangement_mix(
    state: State<'_, AppState>,
    path: String,
    format: Option<WavFormat>,
    sample_rate: Option<u32>,
) -> CmdResult<ExportResult> {
    let rate = sample_rate.unwrap_or(OFFLINE_RATE);
    if !(8_000..=192_000).contains(&rate) {
        return Err(err("invalid_argument", "sample rate outside 8..192 kHz"));
    }
    let buf = {
        let mut s = state.lock();
        let a = arrangement_mut(&mut s)?.clone();
        let seconds = a.duration();
        if seconds <= 0.0 {
            return Err(err("empty", "The arrangement has no clips"));
        }
        let clips = build_clips(&mut s, &a, rate)?;
        drop(s);
        let params = Arc::new(ClipsParams::default());
        apply_tracks(&params, &a, rate);
        params.transport.loop_on.store(false, Ordering::Relaxed);
        params.transport.playing.store(true, Ordering::Relaxed);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[ARRANGE_SLOT].gain_db.set(0.0);
        let mut g = Graph::new(gp, rate);
        g.set_source(ARRANGE_SLOT, Box::new(ClipSource::new(params, clips)));
        g.render_offline(seconds, 2)
    };
    let mut p = std::path::PathBuf::from(&path);
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
