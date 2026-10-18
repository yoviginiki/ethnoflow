//! CLAP instruments in the app (ADR-0018 spike, F4.4): scan folders for `.clap`
//! libraries, list their instruments, and render a short phrase through one of them to
//! a WAV with the same graph the rest of the engine uses. No UI yet (bridge + docs);
//! the host core and its tests live in `crates/plugin-host`.

use std::path::PathBuf;
use std::sync::Arc;

use audio_engine::{
    write_wav, AudioBuffer, BeatClock, Graph, GraphParams, NotePatternData, PatternNote, WavFormat,
};
use plugin_host::{scan_folder, ClapLibrary, ClapSource, PluginDescriptor};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::forge_cmds::AUDITION_SLOT;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClapFile {
    pub path: String,
    pub plugins: Vec<PluginDescriptor>,
    /// Why the file could not be read, if it could not.
    pub error: Option<String>,
}

/// Default folders to look in (Windows: the CLAP standard location).
#[tauri::command]
pub async fn clap_default_folders() -> CmdResult<Vec<String>> {
    let mut out = Vec::new();
    if let Ok(pf) = std::env::var("COMMONPROGRAMFILES") {
        out.push(format!("{pf}\\CLAP"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        out.push(format!("{local}\\Programs\\Common\\CLAP"));
    }
    Ok(out
        .into_iter()
        .filter(|p| std::path::Path::new(p).is_dir())
        .collect())
}

/// `.clap` files in the folders with the instruments each one exports. Loading runs on
/// a blocking thread: a plugin's entry may do real work.
#[tauri::command]
pub async fn clap_scan(folders: Vec<String>) -> CmdResult<Vec<ClapFile>> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = Vec::new();
        for f in folders {
            for path in scan_folder(std::path::Path::new(&f)) {
                let entry = match ClapLibrary::load(&path) {
                    Ok(lib) => ClapFile {
                        path: path.display().to_string(),
                        plugins: lib.descriptors(),
                        error: None,
                    },
                    Err(e) => ClapFile {
                        path: path.display().to_string(),
                        plugins: Vec::new(),
                        error: Some(e.to_string()),
                    },
                };
                out.push(entry);
            }
        }
        out
    })
    .await
    .map_err(|e| err("clap", e.to_string()))
}

#[derive(Debug, Clone, Serialize)]
pub struct ClapRenderResult {
    pub path: String,
    pub seconds: f64,
    pub peak: f32,
    pub plugin: String,
}

/// Render a mode-neutral demo phrase (root, 2nd, 4th, 5th, 6th; two bars) through a CLAP
/// instrument into a WAV. Proof that the plugin plays inside EthnoFlow's graph.
#[tauri::command]
pub async fn clap_render(
    state: State<'_, AppState>,
    path: String,
    plugin_id: String,
    out_path: String,
    midi: Option<i32>,
) -> CmdResult<ClapRenderResult> {
    let bpm = {
        let s = state.lock();
        s.project
            .as_ref()
            .and_then(|p| p.manifest().tempo_map.first().map(|t| t.bpm))
            .unwrap_or(110.0)
    };
    let root = midi.unwrap_or(62).clamp(24, 96) as f64;
    let hz = |m: f64| (440.0 * 2f64.powf((m - 69.0) / 12.0)) as f32;
    let steps: [(f64, f64, f64); 9] = [
        (0.0, 0.45, 0.0),
        (0.5, 0.45, 2.0),
        (1.0, 0.45, 5.0),
        (1.5, 1.4, 7.0),
        (3.0, 0.5, 9.0),
        (3.5, 0.45, 7.0),
        (4.0, 0.95, 5.0),
        (5.0, 0.45, 2.0),
        (5.5, 2.4, 0.0),
    ];
    let events: Vec<PatternNote> = steps
        .iter()
        .enumerate()
        .map(|(i, (beat, len, st))| PatternNote {
            beat: *beat,
            length_beats: *len,
            hz: hz(root + st),
            velocity: if i % 3 == 0 { 1.0 } else { 0.75 },
        })
        .collect();
    let seconds = 8.0 * 60.0 / bpm + 1.0;
    let out_p = PathBuf::from(&out_path);
    let clap_p = PathBuf::from(&path);
    let rendered =
        tauri::async_runtime::spawn_blocking(move || -> CmdResult<(AudioBuffer, String)> {
            let lib = Arc::new(ClapLibrary::load(&clap_p).map_err(|e| err("clap", e.to_string()))?);
            let desc = lib
                .descriptors()
                .into_iter()
                .find(|d| d.id == plugin_id)
                .ok_or_else(|| {
                    err(
                        "clap",
                        format!("no plugin {plugin_id} in {}", clap_p.display()),
                    )
                })?;
            let inst = lib
                .instantiate(&desc.id)
                .map_err(|e| err("clap", e.to_string()))?;
            let clock = Arc::new(BeatClock::new(bpm as f32));
            let src = ClapSource::new(
                inst,
                Arc::clone(&clock),
                Arc::new(NotePatternData::new(events, 8.0)),
                48_000,
            )
            .map_err(|e| err("clap", e.to_string()))?;
            let gp = Arc::new(GraphParams::default());
            gp.master.gain_db.set(0.0);
            gp.layers[AUDITION_SLOT].gain_db.set(-6.0);
            let mut g = Graph::new(gp, 48_000);
            g.set_source(AUDITION_SLOT, Box::new(src));
            clock.start(0, 0.0);
            Ok((g.render_offline(seconds, 2), desc.name))
        })
        .await
        .map_err(|e| err("clap", e.to_string()))??;
    let (buf, plugin) = rendered;
    let mut p = out_p;
    if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("wav")) {
        p.set_extension("wav");
    }
    write_wav(&p, &buf, WavFormat::default())?;
    let peak = buf.samples.iter().fold(0.0f32, |m, x| m.max(x.abs()));
    Ok(ClapRenderResult {
        path: p.display().to_string(),
        seconds,
        peak,
        plugin,
    })
}
