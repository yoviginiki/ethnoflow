//! Preset library: one JSON file per entry in the app config folder, for every kind
//! of setting the UI can save (drone, pad, rhythm, harmony, atmosphere rack, the
//! whole background). Entries carry a name, tags, free-text notes, the project they
//! were saved from and timestamps, so the browser can search notes and sort by date.
//! The payload is the UI's own JSON; the shell only stores and lists it.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::state::{AppState, Inner};

pub const KINDS: [&str; 7] = [
    "drone",
    "pad",
    "rhythm",
    "harmony",
    "intent",
    "ambient",
    "background",
];
const MAX_PAYLOAD_BYTES: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibraryEntry {
    pub id: String,
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    /// Project the entry was saved from (free text, for grouping).
    #[serde(default)]
    pub project: String,
    pub created_ms: i64,
    pub updated_ms: i64,
    /// The UI's own JSON for the kind.
    pub payload: serde_json::Value,
}

/// What the list returns (no payload: a library can hold many racks).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LibrarySummary {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub project: String,
    pub created_ms: i64,
    pub updated_ms: i64,
}

impl From<&LibraryEntry> for LibrarySummary {
    fn from(e: &LibraryEntry) -> Self {
        Self {
            id: e.id.clone(),
            kind: e.kind.clone(),
            name: e.name.clone(),
            tags: e.tags.clone(),
            notes: e.notes.clone(),
            project: e.project.clone(),
            created_ms: e.created_ms,
            updated_ms: e.updated_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewEntry {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    DateDesc,
    DateAsc,
    Name,
    Kind,
}

fn library_dir(s: &Inner) -> CmdResult<PathBuf> {
    s.presets_dir
        .as_ref()
        .and_then(|p| p.parent().map(|d| d.join("library")))
        .ok_or_else(|| err("no_library_dir", "Library folder is not available"))
}

fn clean(text: &str, max: usize) -> String {
    text.trim().chars().take(max).collect()
}

fn clean_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = tags
        .iter()
        .map(|t| t.trim().to_lowercase())
        .filter(|t| !t.is_empty())
        .map(|t| t.chars().take(40).collect())
        .collect();
    out.sort();
    out.dedup();
    out
}

fn entry_path(dir: &Path, id: &str) -> CmdResult<PathBuf> {
    if id.is_empty()
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(err("invalid_argument", "invalid library id"));
    }
    Ok(dir.join(format!("{id}.json")))
}

fn write_entry(dir: &Path, e: &LibraryEntry) -> CmdResult<()> {
    std::fs::create_dir_all(dir).map_err(|x| err("io", x.to_string()))?;
    let path = entry_path(dir, &e.id)?;
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(e).map_err(|x| err("io", x.to_string()))?;
    std::fs::write(&tmp, text).map_err(|x| err("io", x.to_string()))?;
    std::fs::rename(&tmp, &path).map_err(|x| err("io", x.to_string()))?;
    Ok(())
}

fn read_all(dir: &Path) -> Vec<LibraryEntry> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    rd.filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .filter_map(|text| serde_json::from_str::<LibraryEntry>(&text).ok())
        .collect()
}

/// Case-insensitive match over name, tags, notes, project and kind.
fn matches(e: &LibraryEntry, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let hay = format!(
        "{} {} {} {} {}",
        e.name,
        e.tags.join(" "),
        e.notes,
        e.project,
        e.kind
    )
    .to_lowercase();
    // Every word must appear somewhere ("river night" finds a night river).
    q.split_whitespace().all(|w| hay.contains(w))
}

fn sort(list: &mut [LibraryEntry], by: SortBy) {
    match by {
        SortBy::DateDesc => list.sort_by_key(|e| std::cmp::Reverse(e.updated_ms)),
        SortBy::DateAsc => list.sort_by_key(|e| e.updated_ms),
        SortBy::Name => list.sort_by_key(|e| e.name.to_lowercase()),
        SortBy::Kind => list.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then_with(|| b.updated_ms.cmp(&a.updated_ms))
        }),
    }
}

/// Pure listing over a folder (tested without Tauri).
pub fn list_in(
    dir: &Path,
    kind: Option<&str>,
    project: Option<&str>,
    query: &str,
    by: SortBy,
) -> Vec<LibrarySummary> {
    let mut all: Vec<LibraryEntry> = read_all(dir)
        .into_iter()
        .filter(|e| kind.map(|k| e.kind == k).unwrap_or(true))
        .filter(|e| project.map(|p| e.project == p).unwrap_or(true))
        .filter(|e| matches(e, query))
        .collect();
    sort(&mut all, by);
    all.iter().map(LibrarySummary::from).collect()
}

fn new_id() -> String {
    let ms = project::now_ms_pub();
    let mut x = (ms as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xA5A5_5A5A;
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    format!("{ms:x}-{:04x}", (x >> 48) as u16)
}

pub fn save_in(dir: &Path, new: NewEntry, project_name: &str) -> CmdResult<LibraryEntry> {
    if !KINDS.contains(&new.kind.as_str()) {
        return Err(err(
            "invalid_argument",
            format!("unknown kind {}", new.kind),
        ));
    }
    let name = clean(&new.name, 120);
    if name.is_empty() {
        return Err(err("invalid_argument", "The name is empty"));
    }
    let size = serde_json::to_vec(&new.payload)
        .map(|v| v.len())
        .unwrap_or(usize::MAX);
    if size > MAX_PAYLOAD_BYTES {
        return Err(err("invalid_argument", "preset too large"));
    }
    let now = project::now_ms_pub();
    let e = LibraryEntry {
        id: new_id(),
        kind: new.kind,
        name,
        tags: clean_tags(&new.tags),
        notes: clean(&new.notes, 4000),
        project: clean(project_name, 120),
        created_ms: now,
        updated_ms: now,
        payload: new.payload,
    };
    write_entry(dir, &e)?;
    Ok(e)
}

pub fn get_in(dir: &Path, id: &str) -> CmdResult<LibraryEntry> {
    let path = entry_path(dir, id)?;
    let text = std::fs::read_to_string(&path)
        .map_err(|_| err("missing", format!("No library entry {id}")))?;
    serde_json::from_str(&text).map_err(|x| err("io", x.to_string()))
}

#[tauri::command]
pub async fn library_list(
    state: State<'_, AppState>,
    kind: Option<String>,
    project: Option<String>,
    query: Option<String>,
    sort_by: Option<SortBy>,
) -> CmdResult<Vec<LibrarySummary>> {
    let s = state.lock();
    let dir = library_dir(&s)?;
    Ok(list_in(
        &dir,
        kind.as_deref(),
        project.as_deref(),
        query.as_deref().unwrap_or(""),
        sort_by.unwrap_or(SortBy::DateDesc),
    ))
}

#[tauri::command]
pub async fn library_save(
    state: State<'_, AppState>,
    entry: NewEntry,
) -> CmdResult<LibrarySummary> {
    let s = state.lock();
    let dir = library_dir(&s)?;
    let project_name = s
        .project
        .as_ref()
        .map(|p| p.manifest().name.clone())
        .unwrap_or_default();
    let e = save_in(&dir, entry, &project_name)?;
    Ok(LibrarySummary::from(&e))
}

#[tauri::command]
pub async fn library_get(state: State<'_, AppState>, id: String) -> CmdResult<LibraryEntry> {
    let s = state.lock();
    get_in(&library_dir(&s)?, &id)
}

/// Rename / retag / annotate (the payload stays).
#[tauri::command]
pub async fn library_update(
    state: State<'_, AppState>,
    id: String,
    name: String,
    tags: Vec<String>,
    notes: String,
) -> CmdResult<LibrarySummary> {
    let s = state.lock();
    let dir = library_dir(&s)?;
    let mut e = get_in(&dir, &id)?;
    let name = clean(&name, 120);
    if name.is_empty() {
        return Err(err("invalid_argument", "The name is empty"));
    }
    e.name = name;
    e.tags = clean_tags(&tags);
    e.notes = clean(&notes, 4000);
    e.updated_ms = project::now_ms_pub();
    write_entry(&dir, &e)?;
    Ok(LibrarySummary::from(&e))
}

/// Replace the payload of an existing entry with the current settings ("overwrite").
#[tauri::command]
pub async fn library_replace_payload(
    state: State<'_, AppState>,
    id: String,
    payload: serde_json::Value,
) -> CmdResult<LibrarySummary> {
    let s = state.lock();
    let dir = library_dir(&s)?;
    let mut e = get_in(&dir, &id)?;
    e.payload = payload;
    e.updated_ms = project::now_ms_pub();
    write_entry(&dir, &e)?;
    Ok(LibrarySummary::from(&e))
}

#[tauri::command]
pub async fn library_delete(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let s = state.lock();
    let path = entry_path(&library_dir(&s)?, &id)?;
    if path.exists() {
        std::fs::remove_file(&path).map_err(|x| err("io", x.to_string()))?;
    }
    Ok(())
}

/// Distinct project names and tags in the library (for the filter chips).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryFacets {
    pub projects: Vec<String>,
    pub tags: Vec<String>,
    pub counts: Vec<(String, usize)>,
}

#[tauri::command]
pub async fn library_facets(state: State<'_, AppState>) -> CmdResult<LibraryFacets> {
    let s = state.lock();
    let all = read_all(&library_dir(&s)?);
    let mut projects: Vec<String> = all
        .iter()
        .map(|e| e.project.clone())
        .filter(|p| !p.is_empty())
        .collect();
    projects.sort();
    projects.dedup();
    let mut tags: Vec<String> = all.iter().flat_map(|e| e.tags.clone()).collect();
    tags.sort();
    tags.dedup();
    let counts = KINDS
        .iter()
        .map(|k| (k.to_string(), all.iter().filter(|e| e.kind == *k).count()))
        .collect();
    Ok(LibraryFacets {
        projects,
        tags,
        counts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: &str, name: &str, tags: &[&str], notes: &str) -> NewEntry {
        NewEntry {
            kind: kind.into(),
            name: name.into(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            notes: notes.into(),
            payload: serde_json::json!({ "x": name }),
        }
    }

    #[test]
    fn save_search_sort_update_delete() {
        let dir = tempfile::tempdir().unwrap();
        let d = dir.path();
        let a = save_in(
            d,
            entry(
                "drone",
                "Планински дрон",
                &["планина", "Етно"],
                "за кавал в ре",
            ),
            "Проект А",
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let b = save_in(
            d,
            entry("rhythm", "Liquid 172", &["dnb"], "бърз, с reese бас"),
            "Проект Б",
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        let c = save_in(
            d,
            entry(
                "ambient",
                "Река нощем",
                &["река", "нощ"],
                "мъгла над водата, кавал в ре",
            ),
            "Проект А",
        )
        .unwrap();
        assert!(
            save_in(d, entry("piano", "x", &[], ""), "p").is_err(),
            "unknown kind"
        );
        assert!(
            save_in(d, entry("drone", "   ", &[], ""), "p").is_err(),
            "empty name"
        );
        assert_eq!(a.tags, vec!["етно", "планина"], "tags cleaned and sorted");
        // Newest first by default.
        let all = list_in(d, None, None, "", SortBy::DateDesc);
        assert_eq!(
            all.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec![c.id.as_str(), b.id.as_str(), a.id.as_str()]
        );
        // Search in notes, case-insensitive, all words.
        let hits = list_in(d, None, None, "кавал РЕ", SortBy::DateDesc);
        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|e| e.id != b.id));
        assert_eq!(list_in(d, None, None, "reese", SortBy::Name).len(), 1);
        assert_eq!(list_in(d, Some("drone"), None, "", SortBy::Name).len(), 1);
        assert_eq!(
            list_in(d, None, Some("Проект А"), "", SortBy::Name).len(),
            2
        );
        assert_eq!(list_in(d, None, None, "нищо такова", SortBy::Name).len(), 0);
        let by_name = list_in(d, None, None, "", SortBy::Name);
        assert_eq!(by_name[0].name, "Liquid 172");
        // Get returns the payload; update keeps it and bumps the date.
        let got = get_in(d, &a.id).unwrap();
        assert_eq!(got.payload["x"], "Планински дрон");
        assert!(get_in(d, "../evil").is_err());
        let path = entry_path(d, &a.id).unwrap();
        let mut e = get_in(d, &a.id).unwrap();
        e.name = "Нов".into();
        e.notes = "търси ме тук".into();
        e.updated_ms += 1000;
        write_entry(d, &e).unwrap();
        assert!(path.exists());
        assert_eq!(
            list_in(d, None, None, "търси ме", SortBy::DateDesc)[0].name,
            "Нов"
        );
        std::fs::remove_file(&path).unwrap();
        assert_eq!(list_in(d, None, None, "", SortBy::DateDesc).len(), 2);
    }
}
