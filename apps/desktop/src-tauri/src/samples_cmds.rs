//! Sample browser (owner backlog #6): scan folders such as Soundly's download folder for
//! audio files, list them with facts, preview one through the graph, and hand the path
//! to the existing import. Nothing is copied until the user imports a file.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::{decode_file, resample_linear, wav_info, GraphCmd, LoopParams, LoopSource};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::forge_cmds::AUDITION_SLOT;
use crate::graph_cmds::ensure_graph;
use crate::state::AppState;

const SAMPLE_EXTENSIONS: [&str; 4] = ["wav", "mp3", "flac", "ogg"];
const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SampleEntry {
    pub path: String,
    pub name: String,
    pub folder: String,
    pub ext: String,
    pub bytes: u64,
    pub modified_ms: i64,
    /// Known for WAV without decoding; None for compressed files until imported.
    pub duration_s: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SampleScan {
    pub entries: Vec<SampleEntry>,
    pub folders_scanned: usize,
    pub truncated: bool,
}

fn walk(root: &Path, out: &mut Vec<SampleEntry>, max: usize) -> bool {
    let mut stack = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            if p.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((p, depth + 1));
                }
                continue;
            }
            let ext = p
                .extension()
                .map(|x| x.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if !SAMPLE_EXTENSIONS.contains(&ext.as_str()) {
                continue;
            }
            let meta = e.metadata().ok();
            let (duration_s, sample_rate, channels) = if ext == "wav" {
                wav_info(&p)
                    .map(|i| {
                        (
                            Some(i.duration_seconds),
                            Some(i.sample_rate),
                            Some(i.channels),
                        )
                    })
                    .unwrap_or((None, None, None))
            } else {
                (None, None, None)
            };
            out.push(SampleEntry {
                path: p.to_string_lossy().into_owned(),
                name: p
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or(name),
                folder: root.to_string_lossy().into_owned(),
                ext,
                bytes: meta.as_ref().map(|m| m.len()).unwrap_or(0),
                modified_ms: meta
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0),
                duration_s,
                sample_rate,
                channels,
            });
            if out.len() >= max {
                return true;
            }
        }
    }
    false
}

/// Scan folders for audio files (recursive, hidden folders skipped, capped).
#[tauri::command]
pub async fn samples_scan(folders: Vec<String>, max: Option<usize>) -> CmdResult<SampleScan> {
    let max = max.unwrap_or(5000).clamp(1, 50_000);
    let mut entries = Vec::new();
    let mut truncated = false;
    let mut scanned = 0;
    for f in folders.iter().take(32) {
        let root = PathBuf::from(f);
        if !root.is_dir() {
            continue;
        }
        scanned += 1;
        if walk(&root, &mut entries, max) {
            truncated = true;
            break;
        }
    }
    entries.sort_by_key(|a| a.name.to_lowercase());
    Ok(SampleScan {
        entries,
        folders_scanned: scanned,
        truncated,
    })
}

/// Folders worth offering by default when they exist (Soundly's download folder, Music).
#[tauri::command]
pub async fn samples_default_folders() -> CmdResult<Vec<String>> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let candidates = [
        format!("{home}\\Soundly Audio"),
        format!("{home}\\Music"),
        format!("{home}\\Documents\\Samples"),
    ];
    Ok(candidates
        .into_iter()
        .filter(|p| Path::new(p).is_dir())
        .collect())
}

/// Play a file once through the graph's audition slot (decoded off the audio thread);
/// stops by itself when the file ends. Returns its length in seconds.
#[tauri::command]
pub async fn sample_preview(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    path: String,
    device: Option<String>,
) -> CmdResult<f64> {
    let src = PathBuf::from(&path);
    let buf = decode_file(&src)?;
    if buf.is_empty() {
        return Err(err("empty_file", "The file contains no audio"));
    }
    let mut s = state.lock();
    ensure_graph(&mut s, device.as_deref())?;
    let rate = s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000);
    let mono = buf.to_mono();
    let mono = if mono.sample_rate == rate {
        mono
    } else {
        resample_linear(&mono, rate)
    };
    let seconds = mono.duration_seconds();
    let params = Arc::new(LoopParams::default());
    params.crossfade_frames.store(0, Ordering::Relaxed);
    // Loop region = the whole file; the stop below ends it after one pass.
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
        std::thread::sleep(std::time::Duration::from_secs_f64(seconds + 0.15));
        let st = app.state::<AppState>();
        let mut s = st.lock();
        if s.preview_token == token && s.previewing {
            s.previewing = false;
            crate::graph_cmds::release_slot(&mut s, AUDITION_SLOT);
        }
    });
    Ok(seconds)
}

#[tauri::command]
pub async fn sample_preview_stop(state: State<'_, AppState>) -> CmdResult<()> {
    let mut s = state.lock();
    if s.previewing {
        s.previewing = false;
        s.preview_token = s.preview_token.wrapping_add(1);
        crate::graph_cmds::release_slot(&mut s, AUDITION_SLOT);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_finds_audio_files_recursively_with_wav_facts_and_skips_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("kit").join("deep");
        std::fs::create_dir_all(&sub).unwrap();
        let wav = audio_engine::AudioBuffer::sine(22_050, 440.0, 0.5, 0.2);
        audio_engine::write_wav(
            sub.join("tap.wav"),
            &wav,
            audio_engine::WavFormat::default(),
        )
        .unwrap();
        std::fs::write(sub.join("notes.txt"), "x").unwrap();
        std::fs::write(dir.path().join("loop.mp3"), [0u8; 10]).unwrap();
        std::fs::create_dir_all(dir.path().join(".hidden")).unwrap();
        std::fs::write(dir.path().join(".hidden").join("h.wav"), [0u8; 10]).unwrap();
        let mut out = Vec::new();
        let truncated = walk(dir.path(), &mut out, 100);
        assert!(!truncated);
        let mut names: Vec<&str> = out.iter().map(|e| e.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["loop", "tap"]);
        let tap = out.iter().find(|e| e.name == "tap").unwrap();
        assert_eq!(tap.sample_rate, Some(22_050));
        assert!((tap.duration_s.unwrap() - 0.5).abs() < 0.01);
        assert_eq!(tap.ext, "wav");
        let mut few = Vec::new();
        assert!(walk(dir.path(), &mut few, 1), "cap reported as truncated");
    }
}
