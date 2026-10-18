//! References (Program P2, Phase B): whole songs the user brings in to learn from, kept
//! in their own library **outside every project** so analysing a song never changes the
//! user's own material. A reference is an immutable copy of the file plus, on request,
//! its analysis (tempo candidates, key hypotheses, structure). Nothing flows into a
//! project unless the user presses "take": tempo, key or structure are then applied
//! through the ordinary project commands from the UI.
//!
//! Layout: `<config>/references/<id>/original.<ext>`, `meta.json`, `peaks.json`,
//! `analysis.json` (the full `analysis::ReferenceAnalysis`), `report.json` (UI size).

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use audio_engine::{
    compute_peaks, decode_file, resample_linear, AudioBuffer, GraphCmd, LoopParams, LoopSource,
    WaveformPeaks,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::forge_cmds::AUDITION_SLOT;
use crate::graph_cmds::ensure_graph;
use crate::state::{AppState, Inner};

const REFERENCE_EXTENSIONS: [&str; 4] = ["wav", "mp3", "flac", "ogg"];
const ANALYSIS_RATE: u32 = 22_050;
const CURVE_POINTS: usize = 800;
const MAX_NAME: usize = 120;
const MAX_NOTES: usize = 4000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceMeta {
    pub name: String,
    pub source_path: String,
    pub ext: String,
    pub bytes: u64,
    pub added_ms: i64,
    pub duration_s: f64,
    pub sample_rate: u32,
    pub channels: u16,
    #[serde(default)]
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceInfo {
    pub id: String,
    #[serde(flatten)]
    pub meta: ReferenceMeta,
    pub analyzed: bool,
}

/// A curve reduced to at most `CURVE_POINTS` values for drawing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Curve {
    pub step_s: f64,
    pub values: Vec<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceReport {
    pub id: String,
    pub analyzer_version: u32,
    pub analyzed_ms: i64,
    pub duration_s: f64,
    pub tempo: Vec<analysis::TempoCandidate>,
    pub beats: usize,
    pub key: Vec<music_core::key::Hypothesis>,
    pub chroma_mean: [f32; 12],
    pub boundaries: Vec<analysis::Boundary>,
    pub segments: Vec<analysis::SegmentHypothesis>,
    pub energy: Curve,
    pub density: Curve,
    pub low: Curve,
}

fn references_dir(s: &Inner) -> CmdResult<PathBuf> {
    s.presets_dir
        .as_ref()
        .and_then(|p| p.parent().map(|d| d.join("references")))
        .ok_or_else(|| err("no_library_dir", "Library folder is not available"))
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn clean(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

fn valid_id(id: &str) -> CmdResult<()> {
    if id.is_empty()
        || id.len() > 40
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(err("invalid_argument", "bad reference id"));
    }
    Ok(())
}

pub(crate) fn folder(s: &Inner, id: &str) -> CmdResult<PathBuf> {
    valid_id(id)?;
    let dir = references_dir(s)?.join(id);
    if !dir.is_dir() {
        return Err(err("not_found", "No such reference"));
    }
    Ok(dir)
}

pub(crate) fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> CmdResult<Option<T>> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path).map_err(|e| err("io", e.to_string()))?;
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|e| err("corrupt", format!("{}: {e}", path.display())))
}

pub(crate) fn write_json<T: Serialize>(path: &Path, value: &T) -> CmdResult<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| err("json", e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| err("io", e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| err("io", e.to_string()))
}

pub(crate) fn original_path(dir: &Path, meta: &ReferenceMeta) -> PathBuf {
    dir.join(format!("original.{}", meta.ext))
}

fn info_of(dir: &Path) -> CmdResult<Option<ReferenceInfo>> {
    let Some(meta) = read_json::<ReferenceMeta>(&dir.join("meta.json"))? else {
        return Ok(None);
    };
    let id = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let analyzed = dir.join("report.json").is_file();
    Ok(Some(ReferenceInfo { id, meta, analyzed }))
}

/// The decoded original of the last reference touched (decoding a 5-minute MP3 takes a
/// moment; playing section after section should not repeat it).
static DECODED: Mutex<Option<(String, Arc<AudioBuffer>)>> = Mutex::new(None);

pub(crate) fn decoded(id: &str, path: &Path) -> CmdResult<Arc<AudioBuffer>> {
    if let Some((cached_id, buf)) = DECODED.lock().unwrap().as_ref() {
        if cached_id == id {
            return Ok(Arc::clone(buf));
        }
    }
    let buf = Arc::new(decode_file(path)?);
    *DECODED.lock().unwrap() = Some((id.to_string(), Arc::clone(&buf)));
    Ok(buf)
}

fn downsample(values: &[f32], hop_s: f64, points: usize) -> Curve {
    if values.is_empty() {
        return Curve {
            step_s: hop_s,
            values: Vec::new(),
        };
    }
    let per = values.len().div_ceil(points).max(1);
    let out: Vec<f32> = values
        .chunks(per)
        .map(|c| c.iter().sum::<f32>() / c.len() as f32)
        .collect();
    Curve {
        step_s: hop_s * per as f64,
        values: out,
    }
}

/// Key hypotheses from the energy-weighted mean chroma: the same ranking the take
/// analysis uses, so a reference and the user's melody speak the same language.
fn key_from_chroma(
    a: &analysis::ReferenceAnalysis,
) -> ([f32; 12], Vec<music_core::key::Hypothesis>) {
    let mut mean = [0.0f32; 12];
    let mut wsum = 0.0f32;
    for (i, w) in a.curves.rms.iter().enumerate() {
        for (k, m) in mean.iter_mut().enumerate() {
            *m += a.curves.chroma[i * 12 + k] * w;
        }
        wsum += w;
    }
    if wsum > 0.0 {
        for m in mean.iter_mut() {
            *m /= wsum;
        }
    }
    let hist = music_core::key::PitchClassHistogram::from_midi(
        mean.iter()
            .enumerate()
            .map(|(pc, w)| (60.0 + pc as f64, *w as f64)),
    );
    let ranked = music_core::key::rank_hypotheses(&hist, &music_core::scale::ScaleKind::ALL);
    (mean, ranked.into_iter().take(6).collect())
}

fn build_report(id: &str, a: &analysis::ReferenceAnalysis) -> ReferenceReport {
    let (chroma_mean, key) = key_from_chroma(a);
    let c = &a.curves;
    ReferenceReport {
        id: id.to_string(),
        analyzer_version: a.analyzer_version,
        analyzed_ms: now_ms(),
        duration_s: a.duration_s,
        tempo: a.tempo.clone(),
        beats: a.beats_s.len(),
        key,
        chroma_mean,
        boundaries: a.boundaries.clone(),
        segments: a.segments.clone(),
        energy: downsample(&c.rms, c.hop_s, CURVE_POINTS),
        density: downsample(&c.flux, c.hop_s, CURVE_POINTS),
        low: downsample(&c.low_band, c.hop_s, CURVE_POINTS),
    }
}

// ---------- commands ----------

/// Copy a song into the references library (never into a project) and read its facts.
#[tauri::command]
pub async fn reference_add(state: State<'_, AppState>, path: String) -> CmdResult<ReferenceInfo> {
    let src = PathBuf::from(&path);
    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if !REFERENCE_EXTENSIONS.contains(&ext.as_str()) {
        return Err(err("unsupported", "Supported: WAV, MP3, FLAC, OGG"));
    }
    let bytes = std::fs::metadata(&src)
        .map_err(|e| err("io", e.to_string()))?
        .len();
    let buf = decode_file(&src)?;
    if buf.is_empty() {
        return Err(err("empty_file", "The file contains no audio"));
    }
    let root = {
        let s = state.lock();
        references_dir(&s)?
    };
    std::fs::create_dir_all(&root).map_err(|e| err("io", e.to_string()))?;
    let id = format!("r{}", now_ms());
    let dir = root.join(&id);
    std::fs::create_dir_all(&dir).map_err(|e| err("io", e.to_string()))?;
    let name = src
        .file_stem()
        .map(|n| clean(&n.to_string_lossy(), MAX_NAME))
        .unwrap_or_else(|| id.clone());
    let meta = ReferenceMeta {
        name,
        source_path: path.clone(),
        ext,
        bytes,
        added_ms: now_ms(),
        duration_s: buf.duration_seconds(),
        sample_rate: buf.sample_rate,
        channels: buf.channels,
        notes: String::new(),
    };
    std::fs::copy(&src, original_path(&dir, &meta)).map_err(|e| err("io", e.to_string()))?;
    write_json(&dir.join("meta.json"), &meta)?;
    write_json(&dir.join("peaks.json"), &compute_peaks(&buf, 1200))?;
    *DECODED.lock().unwrap() = Some((id.clone(), Arc::new(buf)));
    Ok(ReferenceInfo {
        id,
        meta,
        analyzed: false,
    })
}

#[tauri::command]
pub async fn reference_list(state: State<'_, AppState>) -> CmdResult<Vec<ReferenceInfo>> {
    let root = {
        let s = state.lock();
        references_dir(&s)?
    };
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&root) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                if let Ok(Some(info)) = info_of(&e.path()) {
                    out.push(info);
                }
            }
        }
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.meta.added_ms));
    Ok(out)
}

#[tauri::command]
pub async fn reference_set_meta(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    notes: Option<String>,
) -> CmdResult<ReferenceInfo> {
    let dir = {
        let s = state.lock();
        folder(&s, &id)?
    };
    let mut meta: ReferenceMeta =
        read_json(&dir.join("meta.json"))?.ok_or_else(|| err("corrupt", "meta.json is missing"))?;
    if let Some(n) = name {
        let n = clean(&n, MAX_NAME);
        if !n.is_empty() {
            meta.name = n;
        }
    }
    if let Some(n) = notes {
        meta.notes = clean(&n, MAX_NOTES);
    }
    write_json(&dir.join("meta.json"), &meta)?;
    Ok(ReferenceInfo {
        id,
        meta,
        analyzed: dir.join("report.json").is_file(),
    })
}

/// Deletes the library copy and its analysis (the user's source file is untouched).
#[tauri::command]
pub async fn reference_remove(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let dir = {
        let mut s = state.lock();
        if s.previewing {
            s.previewing = false;
            s.preview_token = s.preview_token.wrapping_add(1);
            crate::graph_cmds::release_slot(&mut s, AUDITION_SLOT);
        }
        folder(&s, &id)?
    };
    if DECODED
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|(c, _)| *c == id)
    {
        *DECODED.lock().unwrap() = None;
    }
    std::fs::remove_dir_all(&dir).map_err(|e| err("io", e.to_string()))
}

#[tauri::command]
pub async fn reference_peaks(state: State<'_, AppState>, id: String) -> CmdResult<WaveformPeaks> {
    let dir = {
        let s = state.lock();
        folder(&s, &id)?
    };
    if let Some(p) = read_json::<WaveformPeaks>(&dir.join("peaks.json"))? {
        return Ok(p);
    }
    let meta: ReferenceMeta =
        read_json(&dir.join("meta.json"))?.ok_or_else(|| err("corrupt", "meta.json is missing"))?;
    let buf = decoded(&id, &original_path(&dir, &meta))?;
    let peaks = compute_peaks(&buf, 1200);
    write_json(&dir.join("peaks.json"), &peaks)?;
    Ok(peaks)
}

#[tauri::command]
pub async fn reference_report(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<Option<ReferenceReport>> {
    let dir = {
        let s = state.lock();
        folder(&s, &id)?
    };
    read_json(&dir.join("report.json"))
}

/// Analyse on request (never automatically): decode → mono → 22.05 kHz → analyzer →
/// key hypotheses; the full analysis and the UI report are written next to the copy.
/// Runs on a blocking thread so the UI and the audio graph keep going.
#[tauri::command]
pub async fn reference_analyze(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<ReferenceReport> {
    let (dir, meta) = {
        let s = state.lock();
        let dir = folder(&s, &id)?;
        let meta: ReferenceMeta = read_json(&dir.join("meta.json"))?
            .ok_or_else(|| err("corrupt", "meta.json is missing"))?;
        (dir, meta)
    };
    let id2 = id.clone();
    tauri::async_runtime::spawn_blocking(move || -> CmdResult<ReferenceReport> {
        let buf = decoded(&id2, &original_path(&dir, &meta))?;
        let mono = buf.to_mono();
        let mono = if mono.sample_rate == ANALYSIS_RATE {
            mono
        } else {
            resample_linear(&mono, ANALYSIS_RATE)
        };
        let opts = analysis::Options {
            sample_rate: ANALYSIS_RATE,
            ..analysis::Options::default()
        };
        let a = analysis::analyze(&mono.samples, &opts);
        let report = build_report(&id2, &a);
        write_json(&dir.join("analysis.json"), &a)?;
        write_json(&dir.join("report.json"), &report)?;
        Ok(report)
    })
    .await
    .map_err(|e| err("analysis", e.to_string()))?
}

/// Play the reference from `from_s` through the audition slot (to hear a section before
/// taking anything from it); stops by itself at the end. Returns the seconds that play.
#[tauri::command]
pub async fn reference_play(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    from_s: f64,
    seconds: Option<f64>,
    device: Option<String>,
) -> CmdResult<f64> {
    let (dir, meta) = {
        let s = state.lock();
        let dir = folder(&s, &id)?;
        let meta: ReferenceMeta = read_json(&dir.join("meta.json"))?
            .ok_or_else(|| err("corrupt", "meta.json is missing"))?;
        (dir, meta)
    };
    let buf = decoded(&id, &original_path(&dir, &meta))?;
    let start = ((from_s.max(0.0)) * buf.sample_rate as f64) as usize;
    let end = seconds
        .map(|s| start + (s.max(0.0) * buf.sample_rate as f64) as usize)
        .unwrap_or(buf.frames())
        .min(buf.frames());
    if start >= end {
        return Err(err("invalid_argument", "start is past the end"));
    }
    let mut s = state.lock();
    ensure_graph(&mut s, device.as_deref())?;
    let rate = s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000);
    let mono = buf.slice_frames(start, end).to_mono();
    let mono = if mono.sample_rate == rate {
        mono
    } else {
        resample_linear(&mono, rate)
    };
    let play_s = mono.duration_seconds();
    let params = Arc::new(LoopParams::default());
    params.crossfade_frames.store(0, Ordering::Relaxed);
    params
        .end_frames
        .store(mono.frames() as u64, Ordering::Relaxed);
    let token = s.preview_token.wrapping_add(1);
    s.preview_token = token;
    s.audition = None;
    let g = s.graph.as_mut().expect("graph just started");
    g.params().layers[AUDITION_SLOT].gain_db.set(-60.0);
    g.params().layers[AUDITION_SLOT].pan.set(0.0);
    if !g.send(GraphCmd::SetSource {
        slot: AUDITION_SLOT,
        source: Box::new(LoopSource::new(params, Arc::new(mono))),
    }) {
        return Err(err("graph_busy", "Graph command queue is full"));
    }
    g.params().layers[AUDITION_SLOT].gain_db.set(-6.0);
    s.previewing = true;
    drop(s);
    std::thread::spawn(move || {
        use tauri::Manager;
        std::thread::sleep(std::time::Duration::from_secs_f64(play_s + 0.15));
        let st = app.state::<AppState>();
        let mut s = st.lock();
        if s.preview_token == token && s.previewing {
            s.previewing = false;
            crate::graph_cmds::release_slot(&mut s, AUDITION_SLOT);
        }
    });
    Ok(play_s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_is_small_and_keys_come_from_the_chroma() {
        // A D-major-ish spectrum: a synthetic analysis with chroma mass on D, F#, A.
        let n = 40usize;
        let mut curves = analysis::FeatureCurves {
            hop_s: 0.0232,
            ..Default::default()
        };
        for _ in 0..n {
            curves.rms.push(0.5);
            curves.flux.push(1.0);
            curves.low_band.push(0.2);
            curves.centroid_hz.push(1000.0);
            let mut ch = [0.02f32; 12];
            ch[2] = 0.4;
            ch[6] = 0.25;
            ch[9] = 0.3;
            curves.chroma.extend(ch);
            curves.timbre.extend([0.0f32; 12]);
        }
        let a = analysis::ReferenceAnalysis {
            analyzer_version: analysis::ANALYZER_VERSION,
            sample_rate: 22_050,
            duration_s: 1.0,
            curves,
            tempo: Vec::new(),
            beats_s: Vec::new(),
            boundaries: Vec::new(),
            segments: Vec::new(),
        };
        let r = build_report("r1", &a);
        assert_eq!(r.key[0].tonic_pc, 2, "{:?}", r.key);
        assert!(r.energy.values.len() <= CURVE_POINTS && !r.energy.values.is_empty());
        let big: Vec<f32> = (0..10_000).map(|i| i as f32).collect();
        let c = downsample(&big, 0.01, CURVE_POINTS);
        assert!(c.values.len() <= CURVE_POINTS);
        assert!((c.step_s - 0.01 * (10_000f64 / c.values.len() as f64).ceil()).abs() < 1e-3);
    }

    #[test]
    fn ids_are_validated_before_touching_the_disk() {
        assert!(valid_id("r1729000000000").is_ok());
        assert!(valid_id("../etc").is_err());
        assert!(valid_id("").is_err());
        assert!(valid_id("a b").is_err());
    }
}
