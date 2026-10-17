//! Assistant bridge (owner request, 2026-10-17): a folder the running app watches so a
//! coding assistant in the terminal (Claude Code) can read the app's state and drive
//! every musical setting. Files only, no network:
//!
//! - `<config>/assistant/inbox/<name>.json`  — one command per file, consumed by the UI
//! - `<config>/assistant/outbox/<name>.json` — the reply to that command
//! - `<config>/assistant/state.json`         — a snapshot the UI rewrites on change
//!
//! The shell only moves JSON around; the UI interprets the commands (see
//! `apps/desktop/src/assistant.ts` and `docs/ASSISTANT_BRIDGE.md`).

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::state::{AppState, Inner};

fn dir(s: &Inner) -> CmdResult<PathBuf> {
    let d = s
        .assistant_dir
        .clone()
        .ok_or_else(|| err("no_config", "assistant folder not set"))?;
    std::fs::create_dir_all(d.join("inbox")).map_err(|e| err("io", e.to_string()))?;
    std::fs::create_dir_all(d.join("outbox")).map_err(|e| err("io", e.to_string()))?;
    Ok(d)
}

fn write_json(path: &Path, v: &serde_json::Value) -> CmdResult<()> {
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(v).map_err(|e| err("io", e.to_string()))?;
    std::fs::write(&tmp, text).map_err(|e| err("io", e.to_string()))?;
    std::fs::rename(&tmp, path).map_err(|e| err("io", e.to_string()))?;
    Ok(())
}

fn safe_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantCommand {
    pub name: String,
    pub body: serde_json::Value,
}

/// Where the bridge lives (shown in the Project menu so the user can find it).
#[tauri::command]
pub async fn assistant_dir(state: State<'_, AppState>) -> CmdResult<String> {
    let s = state.lock();
    Ok(dir(&s)?.display().to_string())
}

/// Take every command file from the inbox, oldest name first. A file that is not yet
/// valid JSON is left for the next poll (it may still be being written) unless it
/// already ends like a document, in which case it gets an error reply.
#[tauri::command]
pub async fn assistant_poll(state: State<'_, AppState>) -> CmdResult<Vec<AssistantCommand>> {
    let d = {
        let s = state.lock();
        dir(&s)?
    };
    let inbox = d.join("inbox");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&inbox)
        .map_err(|e| err("io", e.to_string()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    let mut out = Vec::new();
    for p in files {
        let name = p
            .file_stem()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(body) => {
                let _ = std::fs::remove_file(&p);
                out.push(AssistantCommand { name, body });
            }
            Err(e) => {
                let t = text.trim_end();
                if t.ends_with('}') || t.ends_with(']') {
                    let _ = std::fs::remove_file(&p);
                    write_json(
                        &d.join("outbox").join(format!("{}.json", safe_name(&name))),
                        &serde_json::json!({ "ok": false, "error": format!("invalid JSON: {e}") }),
                    )?;
                }
            }
        }
    }
    Ok(out)
}

/// Write the reply to a command.
#[tauri::command]
pub async fn assistant_reply(
    state: State<'_, AppState>,
    name: String,
    reply: serde_json::Value,
) -> CmdResult<()> {
    let d = {
        let s = state.lock();
        dir(&s)?
    };
    write_json(
        &d.join("outbox").join(format!("{}.json", safe_name(&name))),
        &reply,
    )
}

/// Replace the state snapshot.
#[tauri::command]
pub async fn assistant_write_state(
    state: State<'_, AppState>,
    snapshot: serde_json::Value,
) -> CmdResult<()> {
    let d = {
        let s = state.lock();
        dir(&s)?
    };
    write_json(&d.join("state.json"), &snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_names_and_atomic_json() {
        assert_eq!(safe_name("17-ab_c.d/..\\x"), "17-ab_c.d..x");
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("r.json");
        write_json(&p, &serde_json::json!({ "ok": true })).unwrap();
        assert!(!dir.path().join("r.json.tmp").exists());
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["ok"], true);
    }
}
