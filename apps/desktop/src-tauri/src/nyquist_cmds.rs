//! A9: Audacity Nyquist plug-ins (`.ny`) — the user's folder, the bundled examples, the
//! headers for the UI. Running one on the editor's selection lives in `edit_cmds`
//! (`EditOp::Nyquist` and `ny_analyze`).

use std::path::{Path, PathBuf};

use nyquist::{parse_header, Control};
use serde::Serialize;

use audio_engine::{write_wav, AudioBuffer, WavFormat};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::edit_cmds::{generated_samples_dir, interleave, ny_values, NyValue};
use crate::state::AppState;

/// Plug-ins shipped with the app (real Nyquist code, also the templates to copy from).
const BUNDLED: &[(&str, &str, &str)] = &[
    ("gain", "ef-gain.ny", include_str!("../plugins/ef-gain.ny")),
    (
        "tremolo",
        "ef-tremolo.ny",
        include_str!("../plugins/ef-tremolo.ny"),
    ),
    (
        "fade-curve",
        "ef-fade-curve.ny",
        include_str!("../plugins/ef-fade-curve.ny"),
    ),
    (
        "measure",
        "ef-measure.ny",
        include_str!("../plugins/ef-measure.ny"),
    ),
    ("tone", "ef-tone.ny", include_str!("../plugins/ef-tone.ny")),
];

#[derive(Debug, Clone, Serialize)]
pub struct NyPluginInfo {
    /// `bundled:<id>` or the absolute path of the `.ny` file.
    pub id: String,
    pub file: String,
    pub bundled: bool,
    pub name: String,
    /// process | generate | analyze | tool
    pub kind: String,
    pub author: String,
    pub info: String,
    pub preview: String,
    pub codetype: String,
    pub controls: Vec<Control>,
}

/// `Documents\EthnoFlow\Plugins` (created on first use).
pub fn plugins_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_default();
    let dir = Path::new(&home)
        .join("Documents")
        .join("EthnoFlow")
        .join("Plugins");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// The source of a plug-in by id (`bundled:<id>` or a `.ny` path).
pub fn load_source(id: &str) -> CmdResult<String> {
    if let Some(key) = id.strip_prefix("bundled:") {
        return BUNDLED
            .iter()
            .find(|(k, _, _)| *k == key)
            .map(|(_, _, src)| src.to_string())
            .ok_or_else(|| err("not_found", format!("no bundled plug-in {key}")));
    }
    let p = Path::new(id);
    if p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("ny"))
        != Some(true)
    {
        return Err(err("invalid_argument", "a Nyquist plug-in is a .ny file"));
    }
    std::fs::read_to_string(p).map_err(|e| err("io", format!("{}: {e}", p.display())))
}

fn info(id: String, file: String, bundled: bool, src: &str) -> NyPluginInfo {
    let h = parse_header(src);
    NyPluginInfo {
        id,
        bundled,
        name: if h.name.is_empty() {
            file.trim_end_matches(".ny").to_string()
        } else {
            h.name
        },
        file,
        kind: h.kind,
        author: h.author,
        info: h.info,
        preview: h.preview,
        codetype: h.codetype,
        controls: h.controls,
    }
}

/// Bundled examples first, then the user's folder (by file name).
#[tauri::command]
pub async fn ny_list() -> CmdResult<Vec<NyPluginInfo>> {
    let mut out: Vec<NyPluginInfo> = BUNDLED
        .iter()
        .map(|(k, f, src)| info(format!("bundled:{k}"), f.to_string(), true, src))
        .collect();
    let dir = plugins_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("ny"))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    for p in files {
        let Ok(src) = std::fs::read_to_string(&p) else {
            continue;
        };
        let file = p
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("plugin.ny")
            .to_string();
        out.push(info(p.to_string_lossy().to_string(), file, false, &src));
    }
    Ok(out)
}

#[tauri::command]
pub async fn ny_folder() -> CmdResult<String> {
    Ok(plugins_dir().to_string_lossy().to_string())
}

/// Shows the plug-in folder in the file manager (Explorer on Windows).
#[tauri::command]
pub async fn ny_open_folder() -> CmdResult<String> {
    let dir = plugins_dir();
    #[cfg(target_os = "windows")]
    let r = std::process::Command::new("explorer").arg(&dir).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(&dir).spawn();
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(&dir).spawn();
    r.map_err(|e| err("io", format!("cannot open {}: {e}", dir.display())))?;
    Ok(dir.to_string_lossy().to_string())
}

/// Copies a `.ny` file into the plug-in folder (same name; an existing copy is replaced).
#[tauri::command]
pub async fn ny_import(path: String) -> CmdResult<NyPluginInfo> {
    let src_path = Path::new(&path);
    let src = load_source(&path)?;
    let file = src_path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| err("invalid_argument", "bad file name"))?
        .to_string();
    let dest = plugins_dir().join(&file);
    std::fs::write(&dest, &src).map_err(|e| err("io", format!("{}: {e}", dest.display())))?;
    Ok(info(dest.to_string_lossy().to_string(), file, false, &src))
}

/// The source text of a plug-in (for the editor's "show code" view).
#[tauri::command]
pub async fn ny_source(id: String) -> CmdResult<String> {
    load_source(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_plugins_parse_and_run() {
        for (k, _, src) in BUNDLED {
            let h = parse_header(src);
            assert!(!h.name.is_empty(), "{k} has a name");
            let input: Vec<Vec<f32>> = if h.kind == "generate" {
                Vec::new()
            } else {
                let sine: Vec<f32> = (0..8000)
                    .map(|i| (i as f64 * 440.0 / 8000.0 * std::f64::consts::TAU).sin() as f32 * 0.5)
                    .collect();
                vec![sine.clone(), sine]
            };
            let r = nyquist::run_plugin(&h, &[], &input, 8000.0, 0.0)
                .unwrap_or_else(|e| panic!("{k}: {e}"));
            match (h.kind.as_str(), r.output) {
                ("process", nyquist::PluginOutput::Audio(ch)) => {
                    assert_eq!(ch.len(), 2, "{k} keeps stereo");
                    assert_eq!(ch[0].len(), 8000, "{k} keeps the length");
                }
                ("generate", nyquist::PluginOutput::Audio(ch)) => {
                    assert_eq!(ch[0].len(), 16000, "{k} makes 2 s");
                    let peak = ch[0].iter().fold(0.0f32, |m, v| m.max(v.abs()));
                    assert!(peak > 0.1 && peak <= 1.0, "{k} peak {peak}");
                }
                ("analyze", nyquist::PluginOutput::Message(m)) => {
                    assert!(m.contains("dBFS"), "{k}: {m}")
                }
                (kind, other) => panic!("{k} ({kind}) returned {other:?}"),
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct NyGenerated {
    pub path: String,
    pub folder: String,
    pub seconds: f64,
    pub sample_rate: u32,
    pub channels: u16,
}

/// A free `<name>.wav` (then `<name> 2.wav`, …) in `dir`; the name is made file-safe.
pub(crate) fn unique_wav_path(dir: &Path, name: &str) -> PathBuf {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let safe = safe.trim().trim_end_matches('.').to_string();
    let safe = if safe.is_empty() {
        "sample".to_string()
    } else {
        safe
    };
    let mut path = dir.join(format!("{safe}.wav"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{safe} {n}.wav"));
        n += 1;
    }
    path
}

/// A9: render a *generate* plug-in with no editor open — at the output's sample rate, over
/// `seconds` of silence when given (so `(get-duration 1)` has a length) — and write it as a
/// 24-bit WAV named `name` into `Documents\EthnoFlow\Samples\Generated`.
#[tauri::command]
pub async fn ny_generate_wav(
    state: State<'_, AppState>,
    plugin: String,
    values: Vec<NyValue>,
    seconds: Option<f64>,
    name: String,
) -> CmdResult<NyGenerated> {
    let src = load_source(&plugin)?;
    let header = parse_header(&src);
    let sr = {
        let s = state.lock();
        s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000)
    };
    let input: Vec<Vec<f32>> = match seconds {
        Some(secs) if secs > 0.0 => vec![vec![0.0; (secs.min(600.0) * sr as f64) as usize]],
        _ => Vec::new(),
    };
    let res = nyquist::run_plugin(&header, &ny_values(&values), &input, sr as f64, 0.0)
        .map_err(|e| err("plugin_error", e))?;
    let chans = match res.output {
        nyquist::PluginOutput::Audio(c) => c,
        nyquist::PluginOutput::Message(m) => return Err(err("plugin_message", m)),
        _ => {
            return Err(err(
                "plugin_message",
                "the plug-in returned no audio (is it a generate plug-in?)",
            ))
        }
    };
    let n = chans.iter().map(|c| c.len()).max().unwrap_or(0);
    if n == 0 {
        return Err(err("plugin_message", "the plug-in returned an empty sound"));
    }
    if n > (sr as usize) * 600 {
        return Err(err(
            "plugin_error",
            "the plug-in returned more than 10 minutes",
        ));
    }
    let channels = chans.len().clamp(1, 2) as u16;
    let buf = AudioBuffer::new(sr, channels, interleave(&chans, channels as usize));
    let dir = generated_samples_dir();
    let path = unique_wav_path(&dir, &name);
    write_wav(&path, &buf, WavFormat::Pcm24)?;
    Ok(NyGenerated {
        path: path.to_string_lossy().to_string(),
        folder: dir.to_string_lossy().to_string(),
        seconds: n as f64 / sr as f64,
        sample_rate: sr,
        channels,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct NyCheck {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub ok: bool,
    pub error: Option<String>,
    pub ms: u64,
    /// Seconds of audio the plug-in produced (0 for messages).
    pub seconds: f64,
}

/// A9: run every plug-in (or the one given) with its header defaults on a one-second
/// stereo test tone, headless, and report which ones fail — so a broken `.ny` in the folder
/// is found before a button in the editor is pressed.
#[tauri::command]
pub async fn ny_check(plugin: Option<String>) -> CmdResult<Vec<NyCheck>> {
    let list = ny_list().await?;
    let sr = 48_000.0;
    let n = sr as usize;
    let l: Vec<f32> = (0..n)
        .map(|i| (i as f64 * 440.0 / sr * std::f64::consts::TAU).sin() as f32 * 0.5)
        .collect();
    let r: Vec<f32> = (0..n)
        .map(|i| (i as f64 * 220.0 / sr * std::f64::consts::TAU).sin() as f32 * 0.4)
        .collect();
    let mut out = Vec::new();
    for p in list {
        if let Some(only) = &plugin {
            if &p.id != only {
                continue;
            }
        }
        let t = std::time::Instant::now();
        let res = load_source(&p.id).and_then(|src| {
            let header = parse_header(&src);
            let input: Vec<Vec<f32>> = if header.kind == "generate" {
                Vec::new()
            } else {
                vec![l.clone(), r.clone()]
            };
            nyquist::run_plugin(&header, &[], &input, sr, 0.0).map_err(|e| err("plugin_error", e))
        });
        let ms = t.elapsed().as_millis() as u64;
        out.push(match res {
            Ok(rr) => {
                let seconds = match &rr.output {
                    nyquist::PluginOutput::Audio(ch) => {
                        ch.first().map(|c| c.len()).unwrap_or(0) as f64 / sr
                    }
                    _ => 0.0,
                };
                // A prompt-style plug-in (only text controls, all empty by default) waits for
                // the user's input; NIL with empty inputs is its correct answer, as in Audacity.
                let waits_for_input = !p.controls.is_empty()
                    && p.controls.iter().all(|c| {
                        matches!(
                            c.kind,
                            nyquist::ControlKind::String | nyquist::ControlKind::Text
                        ) && c.default_text.trim().is_empty()
                    })
                    && matches!(rr.output, nyquist::PluginOutput::Nothing);
                let ok = waits_for_input
                    || match (&p.kind[..], &rr.output) {
                        ("generate", nyquist::PluginOutput::Audio(ch)) => {
                            ch.first().map(|c| !c.is_empty()).unwrap_or(false)
                        }
                        ("analyze", _) => true,
                        (_, nyquist::PluginOutput::Audio(_)) => true,
                        // A process plug-in that answers with text is reporting a condition
                        // (e.g. "needs a stereo track"), not a failure of the interpreter.
                        (_, nyquist::PluginOutput::Message(_)) => true,
                        _ => false,
                    };
                NyCheck {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    kind: p.kind.clone(),
                    ok,
                    error: if ok {
                        None
                    } else {
                        Some("returned nothing".into())
                    },
                    ms,
                    seconds,
                }
            }
            Err(e) => NyCheck {
                id: p.id.clone(),
                name: p.name.clone(),
                kind: p.kind.clone(),
                ok: false,
                error: Some(e.message),
                ms,
                seconds: 0.0,
            },
        });
    }
    Ok(out)
}
