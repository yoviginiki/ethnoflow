//! A9: Audacity Nyquist plug-ins (`.ny`) — the user's folder, the bundled examples, the
//! headers for the UI. Running one on the editor's selection lives in `edit_cmds`
//! (`EditOp::Nyquist` and `ny_analyze`).

use std::path::{Path, PathBuf};

use nyquist::{parse_header, Control};
use serde::Serialize;

use crate::commands::{err, CmdResult};

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
                ("analyze", nyquist::PluginOutput::Message(m)) => {
                    assert!(m.contains("dBFS"), "{k}: {m}")
                }
                (kind, other) => panic!("{k} ({kind}) returned {other:?}"),
            }
        }
    }
}
