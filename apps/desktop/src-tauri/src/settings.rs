//! App-level settings stored in the OS config dir (not in any project):
//! the recent-projects list.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

const FILE: &str = "settings.json";
const MAX_RECENT: usize = 10;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default)]
    pub recent_projects: Vec<String>,
}

fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join(FILE))
}

pub fn load(app: &AppHandle) -> AppSettings {
    path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(app: &AppHandle, s: &AppSettings) {
    let Some(p) = path(app) else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    match serde_json::to_string_pretty(s) {
        Ok(text) => {
            if let Err(e) = std::fs::write(&p, text) {
                log::warn!("cannot save settings: {e}");
            }
        }
        Err(e) => log::warn!("cannot serialise settings: {e}"),
    }
}

/// Move `root` to the front of the recent list.
pub fn remember_project(app: &AppHandle, root: &Path) {
    let mut s = load(app);
    let r = root.to_string_lossy().into_owned();
    s.recent_projects.retain(|p| p != &r);
    s.recent_projects.insert(0, r);
    s.recent_projects.truncate(MAX_RECENT);
    save(app, &s);
}
