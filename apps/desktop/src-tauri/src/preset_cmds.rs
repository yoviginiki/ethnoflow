//! Ambient presets (F1A.3): the factory racks from `project::ambient_presets` plus
//! the user's own, saved as JSON files in the app config folder. Loading a preset
//! replaces the project's rack through `set_ambient` (the UI keeps the undo).

use std::path::PathBuf;

use project::{factory_presets, sanitize_name, Ambient, Preset};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::state::{AppState, Inner};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetSummary {
    pub id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub factory: bool,
    pub layer_count: usize,
}

const USER_PREFIX: &str = "user:";

fn presets_dir(s: &Inner) -> CmdResult<PathBuf> {
    s.presets_dir
        .clone()
        .ok_or_else(|| err("no_presets_dir", "Preset folder is not available"))
}

fn user_path(s: &Inner, id: &str) -> CmdResult<PathBuf> {
    let stem = id
        .strip_prefix(USER_PREFIX)
        .ok_or_else(|| err("invalid_argument", "not a user preset"))?;
    // Ids come from file stems we produced; re-sanitise so a crafted id cannot escape.
    let safe = sanitize_name(stem).map_err(|e| err("invalid_argument", e.to_string()))?;
    if safe != stem {
        return Err(err("invalid_argument", "invalid preset id"));
    }
    Ok(presets_dir(s)?.join(format!("{stem}.json")))
}

fn summary(p: &Preset) -> PresetSummary {
    PresetSummary {
        id: p.id.clone(),
        name: p.name.clone(),
        tags: p.tags.clone(),
        factory: p.factory,
        layer_count: p.rack.layers.len(),
    }
}

fn user_presets(s: &Inner) -> Vec<Preset> {
    let Ok(dir) = presets_dir(s) else {
        return Vec::new();
    };
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<Preset> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .filter_map(|path| {
            let stem = path.file_stem()?.to_string_lossy().to_string();
            let rack = Ambient::load_file(&path).ok()?;
            Some(Preset {
                id: format!("{USER_PREFIX}{stem}"),
                name: stem,
                tags: rack.tags.clone(),
                factory: false,
                rack,
            })
        })
        .collect();
    out.sort_by_key(|p| p.name.to_lowercase());
    out
}

#[tauri::command]
pub async fn ambient_presets(state: State<'_, AppState>) -> CmdResult<Vec<PresetSummary>> {
    let s = state.lock();
    let mut v: Vec<PresetSummary> = factory_presets().iter().map(summary).collect();
    v.extend(user_presets(&s).iter().map(summary));
    Ok(v)
}

/// The rack of a preset (factory or user), with fresh layer ids so two loads never
/// share ids inside one project.
#[tauri::command]
pub async fn ambient_preset_load(state: State<'_, AppState>, id: String) -> CmdResult<Ambient> {
    let s = state.lock();
    let found = factory_presets()
        .into_iter()
        .chain(user_presets(&s))
        .find(|p| p.id == id)
        .ok_or_else(|| err("missing", format!("Unknown preset {id}")))?;
    let mut rack = found.rack;
    rack.preset = Some(found.id.clone());
    let stamp = project::now_ms_pub();
    for (i, l) in rack.layers.iter_mut().enumerate() {
        l.id = format!("p{stamp:x}-{i}");
    }
    Ok(rack)
}

/// Save a rack as a user preset named `name` (overwrites a preset of the same name).
#[tauri::command]
pub async fn ambient_preset_save(
    state: State<'_, AppState>,
    name: String,
    tags: Vec<String>,
    rack: Ambient,
) -> CmdResult<Vec<PresetSummary>> {
    let safe = sanitize_name(&name).map_err(|e| err("invalid_argument", e.to_string()))?;
    if safe.is_empty() {
        return Err(err("invalid_argument", "Preset name is empty"));
    }
    rack.validate()
        .map_err(|e| err("invalid_argument", e.to_string()))?;
    let s = state.lock();
    let dir = presets_dir(&s)?;
    std::fs::create_dir_all(&dir).map_err(|e| err("io", e.to_string()))?;
    let mut saved = rack;
    saved.preset = Some(format!("{USER_PREFIX}{safe}"));
    saved.tags = tags
        .into_iter()
        .filter(|t| project::AMBIENT_TAGS.contains(&t.as_str()))
        .collect();
    saved
        .save_file(&dir.join(format!("{safe}.json")))
        .map_err(|e| err("io", e.to_string()))?;
    let mut v: Vec<PresetSummary> = factory_presets().iter().map(summary).collect();
    v.extend(user_presets(&s).iter().map(summary));
    Ok(v)
}

#[tauri::command]
pub async fn ambient_preset_delete(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<Vec<PresetSummary>> {
    let s = state.lock();
    let path = user_path(&s, &id)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| err("io", e.to_string()))?;
    }
    let mut v: Vec<PresetSummary> = factory_presets().iter().map(summary).collect();
    v.extend(user_presets(&s).iter().map(summary));
    Ok(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_preset_ids_cannot_escape_the_folder() {
        let s = Inner {
            presets_dir: Some(std::env::temp_dir().join("ethnoflow-presets-test")),
            ..Default::default()
        };
        assert!(user_path(&s, "user:..\\..\\evil").is_err());
        assert!(user_path(&s, "user:../evil").is_err());
        assert!(user_path(&s, "mountain_mist").is_err());
        let ok = user_path(&s, "user:my scene").unwrap();
        assert!(ok.ends_with("my scene.json"));
    }
}
