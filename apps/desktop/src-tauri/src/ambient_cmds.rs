//! Ambient Playground commands (F1A.1/F1A.2): the project's rack (`ambient.json`) on
//! graph slots 4..12, next to the drone (0), the pad (1) and the arrangement's clips
//! (2). Layer edits go through atomics while playing; only a change of kind, seed
//! source or loop audio rebuilds that one slot. Tonal layers take their pitch set
//! from the project's scale and instrument intonation; without a scale, or when the
//! layer is set to "no key", they use an inharmonic set derived from the seed.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::{
    AudioBuffer, BellParams, BellSource, GraphCmd, LoopParams, LoopSource, NoiseParams,
    NoiseSource, PluckParams, PluckSource, TextureParams, TextureSource, WindParams, WindSource,
    MAX_PITCHES,
};
use project::{Ambient, AmbientLayer, LayerSource, Macros, MAX_AMBIENT_LAYERS};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::graph_cmds::{ensure_graph, note_hz, Intonation};
use crate::state::{AppState, Inner};

/// First graph slot of the rack; layer `i` sits in `AMBIENT_SLOT0 + i`.
pub const AMBIENT_SLOT0: usize = 4;

/// Live parameter handles of one layer while the rack plays.
pub enum LiveParams {
    Noise(Arc<NoiseParams>),
    Wind(Arc<WindParams>),
    Texture(Arc<TextureParams>),
    Bells(Arc<BellParams>),
    Pluck(Arc<PluckParams>),
    Loop(Arc<LoopParams>),
}

pub struct LiveLayer {
    /// What the source in the slot was built from; a change here rebuilds it.
    pub build_key: String,
    pub params: LiveParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbientView {
    pub ambient: Ambient,
    pub playing: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbientStatus {
    pub playing: bool,
    pub sample_rate: u32,
    /// Peak per layer, in rack order.
    pub layer_peaks: Vec<f32>,
    pub master_peak: f32,
    pub limiter_gain: f32,
    pub stream_errors: u64,
    /// Pitches each tonal layer actually plays (Hz), in rack order; empty for others.
    pub pitches_hz: Vec<Vec<f32>>,
}

fn project_root(s: &Inner) -> CmdResult<std::path::PathBuf> {
    s.project
        .as_ref()
        .map(|p| p.root().to_path_buf())
        .ok_or_else(|| err("no_project", "No project is open"))
}

/// The rack of the open project, loaded from disk on first use.
pub fn ambient_mut(s: &mut Inner) -> CmdResult<&mut Ambient> {
    if s.ambient.is_none() {
        let root = project_root(s)?;
        let a = Ambient::load(&root).map_err(|e| err("io", e.to_string()))?;
        s.ambient = Some(a);
    }
    Ok(s.ambient.as_mut().expect("just loaded"))
}

/// Pitch set of a tonal layer: every degree of the project's scale inside the
/// register (profile intonation when available), thinned to `MAX_PITCHES` spread
/// across the register. Without a scale or with `tonal = false`: an inharmonic set
/// from the seed (no key is ever assumed).
pub fn pitch_set(s: &Inner, tonal: bool, register: (u8, u8), seed: u32) -> Vec<f32> {
    let (lo, hi) = (register.0.min(register.1), register.1.max(register.0));
    let scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
    if tonal {
        if let Some(scale) = scale {
            let mut notes: Vec<(u8, f64)> = Vec::new();
            for midi in lo..=hi {
                let pc = (midi as i32 - scale.tonic_pc as i32).rem_euclid(12) as f64 * 100.0;
                // A degree within ±50 cents of this semitone belongs to it.
                if let Some(deg) = scale
                    .degrees_cents
                    .iter()
                    .find(|&&d| (d - pc).abs() < 50.0 || (d - pc - 1200.0).abs() < 50.0)
                {
                    let off = if (deg - pc).abs() < 50.0 {
                        deg - pc
                    } else {
                        deg - pc - 1200.0
                    };
                    notes.push((midi, off));
                }
            }
            if !notes.is_empty() {
                let picked: Vec<(u8, f64)> = if notes.len() <= MAX_PITCHES {
                    notes
                } else {
                    // Spread across the register, always keeping the lowest note.
                    (0..MAX_PITCHES)
                        .map(|k| notes[k * (notes.len() - 1) / (MAX_PITCHES - 1)])
                        .collect()
                };
                return picked
                    .into_iter()
                    .map(|(m, cents)| note_hz(s, m, cents, Intonation::Profile).0)
                    .collect();
            }
        }
    }
    // No key: a fixed inharmonic series over a base chosen by the seed inside the
    // lower half of the register — consistent for the seed, tonal for no one.
    let base_midi = lo as f64 + ((seed as f64 * 0.618_033_988_7).fract() * 6.0);
    let base = note_hz(s, lo, 0.0, Intonation::Tempered).0 as f64
        * 2f64.powf((base_midi - lo as f64) / 12.0);
    let top = note_hz(s, hi, 0.0, Intonation::Tempered).0 as f64;
    [1.0f64, 1.5, 2.0, 2.76, 3.47, 4.5, 5.4, 6.8]
        .iter()
        .map(|r| (base * r) as f32)
        .filter(|&f| (f as f64) <= top * 1.05)
        .collect()
}

/// Where the layer gets its sound from, as a string: a change rebuilds the slot.
fn build_key(l: &AmbientLayer, rate: u32) -> String {
    match &l.source {
        LayerSource::Loop {
            take_id,
            start,
            end,
            reverse,
            ..
        } => format!("loop:{take_id}:{start}:{end}:{reverse}:{rate}"),
        other => format!("{}:{}", other.kind(), l.seed),
    }
}

/// Apply macros + the layer's own level/pan/filters to its graph slot.
fn apply_layer_strip(
    layer: &audio_engine::LayerParams,
    l: &AmbientLayer,
    m: &Macros,
    any_solo: bool,
) {
    // Intensity: −40 dB at 0 … 0 dB at 1 (log-ish curve so the slider feels even).
    let intensity_db = if m.intensity <= 0.0 {
        -60.0
    } else {
        20.0 * m.intensity.log10().max(-2.0)
    };
    // Tonal gravity: ±12 dB between tonal and textural layers around neutral.
    let gravity = (m.tonal_gravity - 0.5) * 24.0;
    let gravity_db = if l.source.is_tonal() {
        gravity
    } else {
        -gravity
    };
    let gain = (l.gain_db + intensity_db + gravity_db).clamp(-120.0, 12.0);
    layer.gain_db.set(gain as f32);
    layer.pan.set((l.pan * m.width) as f32);
    // Brightness: shifts the low-pass by ±2 octaves around the layer's own setting.
    let lp = (l.lowpass_hz * 2f64.powf((m.brightness - 0.5) * 4.0)).clamp(40.0, 20_000.0);
    layer.lowpass_hz.set(lp as f32);
    layer.highpass_hz.set(l.highpass_hz as f32);
    layer.mute.store(l.mute, Ordering::Relaxed);
    // Solo is rack-wide, but the graph's solo is graph-wide; keep the drone/pad/clips
    // audible by expressing a rack solo as mutes of the other rack layers.
    layer.solo.store(false, Ordering::Relaxed);
    if any_solo && !l.solo {
        layer.mute.store(true, Ordering::Relaxed);
    }
}

/// Push the layer's source parameters (motion macro included) into the live handles.
fn apply_source(s: &Inner, live: &LiveParams, l: &AmbientLayer, m: &Macros) {
    let motion = 2f64.powf((m.motion - 0.5) * 2.0); // ×0.5 … ×2
    match (&l.source, live) {
        (LayerSource::Noise { color }, LiveParams::Noise(p)) => {
            p.color.set(*color as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Wind {
                gust,
                speed,
                center_hz,
                resonance,
            },
            LiveParams::Wind(p),
        ) => {
            p.gust.set((*gust * motion).min(1.0) as f32);
            p.speed.set((*speed * motion).min(1.0) as f32);
            p.center_hz.set(*center_hz as f32);
            p.resonance.set(*resonance as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Texture {
                brightness,
                motion: mo,
                resonance,
                tonal,
                register,
            },
            LiveParams::Texture(p),
        ) => {
            p.pitches.set(&pitch_set(s, *tonal, *register, l.seed));
            p.brightness.set(*brightness as f32);
            p.motion.set((*mo * motion).min(1.0) as f32);
            p.resonance.set(*resonance as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Bells {
                density,
                decay_s,
                brightness,
                velocity_spread,
                tonal,
                register,
            },
            LiveParams::Bells(p),
        ) => {
            p.pitches.set(&pitch_set(s, *tonal, *register, l.seed));
            p.density.set((*density * motion) as f32);
            p.decay_s.set(*decay_s as f32);
            p.brightness.set(*brightness as f32);
            p.velocity_spread.set(*velocity_spread as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Pluck {
                density,
                damping,
                brightness,
                velocity_spread,
                tonal,
                register,
            },
            LiveParams::Pluck(p),
        ) => {
            p.pitches.set(&pitch_set(s, *tonal, *register, l.seed));
            p.density.set((*density * motion) as f32);
            p.damping.set(*damping as f32);
            p.brightness.set(*brightness as f32);
            p.velocity_spread.set(*velocity_spread as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Loop {
                start,
                end,
                crossfade,
                rate,
                ..
            },
            LiveParams::Loop(p),
        ) => {
            let sr = s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000) as f64;
            p.start_frames
                .store((start * sr).round() as u64, Ordering::Relaxed);
            p.end_frames
                .store((end * sr).round() as u64, Ordering::Relaxed);
            p.crossfade_frames
                .store((crossfade * sr).round() as u32, Ordering::Relaxed);
            p.rate.set(*rate as f32);
        }
        _ => {}
    }
}

/// The audio a loop layer plays: the take (trim applied, mono, at the stream rate),
/// reversed offline when asked. Cached with the clips' cache.
fn loop_buffer(
    s: &mut Inner,
    take_id: &str,
    reverse: bool,
    rate: u32,
) -> CmdResult<Arc<AudioBuffer>> {
    let fwd = crate::arrangement_cmds::take_buffer(s, take_id, rate)?;
    if !reverse {
        return Ok(fwd);
    }
    let key = format!("{take_id}:rev:{rate}:{}", fwd.frames());
    if let Some(b) = s.clip_cache.get(&key) {
        return Ok(Arc::clone(b));
    }
    let mut samples = fwd.samples.clone();
    samples.reverse();
    let b = Arc::new(AudioBuffer::new(fwd.sample_rate, 1, samples));
    s.clip_cache.insert(key, Arc::clone(&b));
    Ok(b)
}

/// Build (or rebuild) the source for layer `i` in its slot; starts silent and ramps.
fn build_layer(s: &mut Inner, i: usize) -> CmdResult<()> {
    let l = s
        .ambient
        .as_ref()
        .and_then(|a| a.layers.get(i))
        .cloned()
        .ok_or_else(|| err("invalid_argument", "no such layer"))?;
    let rate = s
        .graph
        .as_ref()
        .map(|g| g.sample_rate())
        .ok_or_else(|| err("no_graph", "The audio graph is not running"))?;
    let key = build_key(&l, rate);
    let (params, source): (LiveParams, Box<dyn audio_engine::Source>) = match &l.source {
        LayerSource::Noise { .. } => {
            let p = Arc::new(NoiseParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Noise(Arc::clone(&p)),
                Box::new(NoiseSource::new(p)),
            )
        }
        LayerSource::Wind { .. } => {
            let p = Arc::new(WindParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Wind(Arc::clone(&p)),
                Box::new(WindSource::new(p)),
            )
        }
        LayerSource::Texture { .. } => {
            let p = Arc::new(TextureParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Texture(Arc::clone(&p)),
                Box::new(TextureSource::new(p)),
            )
        }
        LayerSource::Bells { .. } => {
            let p = Arc::new(BellParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Bells(Arc::clone(&p)),
                Box::new(BellSource::new(p)),
            )
        }
        LayerSource::Pluck { .. } => {
            let p = Arc::new(PluckParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Pluck(Arc::clone(&p)),
                Box::new(PluckSource::new(p, rate)),
            )
        }
        LayerSource::Loop {
            take_id, reverse, ..
        } => {
            let buf = loop_buffer(s, take_id, *reverse, rate)?;
            let p = Arc::new(LoopParams::default());
            (
                LiveParams::Loop(Arc::clone(&p)),
                Box::new(LoopSource::new(p, buf)),
            )
        }
    };
    let macros = s
        .ambient
        .as_ref()
        .map(|a| a.macros.clone())
        .unwrap_or_default();
    apply_source(s, &params, &l, &macros);
    let slot = AMBIENT_SLOT0 + i;
    let graph = s.graph.as_mut().expect("checked above");
    graph.params().layers[slot].gain_db.set(-60.0);
    if !graph.send(GraphCmd::SetSource { slot, source }) {
        return Err(err("graph_busy", "Graph command queue is full"));
    }
    while s.ambient_live.len() <= i {
        s.ambient_live.push(None);
    }
    s.ambient_live[i] = Some(LiveLayer {
        build_key: key,
        params,
    });
    Ok(())
}

/// Bring the graph in line with the rack: rebuild changed slots, update the rest,
/// clear slots of removed layers, set the master space.
fn sync_live(s: &mut Inner) -> CmdResult<()> {
    if !s.ambient_on || s.graph.is_none() {
        return Ok(());
    }
    let rate = s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000);
    let a = ambient_mut(s)?.clone();
    let any_solo = a.layers.iter().any(|l| l.solo);
    for (i, l) in a.layers.iter().enumerate() {
        let key = build_key(l, rate);
        let same = s
            .ambient_live
            .get(i)
            .and_then(|x| x.as_ref())
            .map(|x| x.build_key == key)
            .unwrap_or(false);
        if !same {
            build_layer(s, i)?;
        } else if let Some(Some(live)) = s.ambient_live.get(i) {
            apply_source(s, &live.params, l, &a.macros);
        }
        let graph = s.graph.as_ref().expect("graph");
        apply_layer_strip(
            &graph.params().layers[AMBIENT_SLOT0 + i],
            l,
            &a.macros,
            any_solo,
        );
    }
    // Layers that no longer exist: fade and clear.
    let live_len = s.ambient_live.len();
    for i in a.layers.len()..live_len {
        if s.ambient_live[i].is_some() {
            if let Some(g) = s.graph.as_mut() {
                g.params().layers[AMBIENT_SLOT0 + i].gain_db.set(-60.0);
                g.send(GraphCmd::Clear {
                    slot: AMBIENT_SLOT0 + i,
                });
            }
            s.ambient_live[i] = None;
        }
    }
    s.ambient_live.truncate(a.layers.len());
    if let Some(g) = s.graph.as_ref() {
        g.params().master.space.set(a.macros.space as f32);
        g.params().master.space_size.set(a.macros.space_size as f32);
    }
    Ok(())
}

/// Stop every rack layer (fade, clear) and stop the stream if nothing else uses it.
pub fn release_all(s: &mut Inner) {
    if let Some(g) = s.graph.as_mut() {
        for i in 0..MAX_AMBIENT_LAYERS {
            g.params().layers[AMBIENT_SLOT0 + i].gain_db.set(-60.0);
        }
        g.params().master.space.set(0.0);
        std::thread::sleep(std::time::Duration::from_millis(30));
        for i in 0..s.ambient_live.len() {
            g.send(GraphCmd::Clear {
                slot: AMBIENT_SLOT0 + i,
            });
        }
    }
    s.ambient_live.clear();
    s.ambient_on = false;
    if s.graph_idle() {
        s.stop_graph();
    }
}

fn status(s: &mut Inner) -> CmdResult<AmbientStatus> {
    let playing = s.ambient_on && s.graph.is_some();
    let a = ambient_mut(s)?.clone();
    let pitches_hz: Vec<Vec<f32>> = a
        .layers
        .iter()
        .map(|l| match &l.source {
            LayerSource::Texture {
                tonal, register, ..
            }
            | LayerSource::Bells {
                tonal, register, ..
            }
            | LayerSource::Pluck {
                tonal, register, ..
            } => pitch_set(s, *tonal, *register, l.seed),
            _ => Vec::new(),
        })
        .collect();
    let n = a.layers.len();
    let Some(graph) = s.graph.as_mut() else {
        return Ok(AmbientStatus {
            playing: false,
            sample_rate: 0,
            layer_peaks: vec![0.0; n],
            master_peak: 0.0,
            limiter_gain: 1.0,
            stream_errors: 0,
            pitches_hz,
        });
    };
    let st = graph.status();
    let m = graph.params().meters(AMBIENT_SLOT0 + n);
    Ok(AmbientStatus {
        playing,
        sample_rate: st.sample_rate,
        layer_peaks: (0..n)
            .map(|i| m.layers.get(AMBIENT_SLOT0 + i).copied().unwrap_or(0.0))
            .collect(),
        master_peak: m.master_peak,
        limiter_gain: m.limiter_gain,
        stream_errors: st.stream_errors,
        pitches_hz,
    })
}

/// A take was deleted: loop layers that used it are removed from the rack.
pub fn forget_take(s: &mut Inner, take_id: &str) {
    let Ok(a) = ambient_mut(s) else { return };
    let before = a.layers.len();
    a.layers
        .retain(|l| !matches!(&l.source, LayerSource::Loop { take_id: t, .. } if t == take_id));
    if a.layers.len() != before {
        if let Ok(root) = project_root(s) {
            if let Some(a) = s.ambient.as_ref() {
                let _ = a.save(&root);
            }
        }
        let _ = sync_live(s);
    }
}

#[tauri::command]
pub async fn ambient_get(state: State<'_, AppState>) -> CmdResult<AmbientView> {
    let mut s = state.lock();
    let playing = s.ambient_on && s.graph.is_some();
    let a = ambient_mut(&mut s)?.clone();
    Ok(AmbientView {
        ambient: a,
        playing,
    })
}

/// Replace the rack: validate, check loop takes exist, save, apply live.
#[tauri::command]
pub async fn set_ambient(state: State<'_, AppState>, ambient: Ambient) -> CmdResult<AmbientView> {
    ambient
        .validate()
        .map_err(|e| err("invalid_argument", e.to_string()))?;
    let mut s = state.lock();
    let root = project_root(&s)?;
    for l in &ambient.layers {
        if let LayerSource::Loop { take_id, .. } = &l.source {
            let p = s.project.as_ref().expect("project checked");
            p.take(take_id)
                .map_err(|_| err("missing", format!("Unknown take {take_id}")))?;
        }
    }
    let mut a = ambient;
    a.schema_version = project::ambient::AMBIENT_SCHEMA_VERSION;
    a.save(&root).map_err(|e| err("io", e.to_string()))?;
    s.ambient = Some(a.clone());
    sync_live(&mut s)?;
    let playing = s.ambient_on && s.graph.is_some();
    Ok(AmbientView {
        ambient: a,
        playing,
    })
}

/// Start the rack on the output (never called automatically: the user presses Play).
#[tauri::command]
pub async fn ambient_play(
    state: State<'_, AppState>,
    device: Option<String>,
) -> CmdResult<AmbientStatus> {
    let mut s = state.lock();
    let _ = ambient_mut(&mut s)?;
    ensure_graph(&mut s, device.as_deref())?;
    s.ambient_on = true;
    s.ambient_live.clear();
    if let Err(e) = sync_live(&mut s) {
        release_all(&mut s);
        return Err(e);
    }
    status(&mut s)
}

#[tauri::command]
pub async fn ambient_stop(state: State<'_, AppState>) -> CmdResult<()> {
    release_all(&mut state.lock());
    Ok(())
}

#[tauri::command]
pub async fn ambient_status(state: State<'_, AppState>) -> CmdResult<AmbientStatus> {
    status(&mut state.lock())
}

/// Fade the whole background (rack, drone, pad) out over `seconds`, then stop it.
/// Runs on a worker thread; the UI polls the statuses and sees everything off.
#[tauri::command]
pub async fn background_fade_out(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    seconds: f64,
) -> CmdResult<()> {
    let seconds = seconds.clamp(0.1, 600.0);
    let from_db = {
        let s = state.lock();
        if s.graph.is_none() {
            return Ok(());
        }
        s.master_db.unwrap_or(0.0)
    };
    std::thread::spawn(move || {
        use tauri::Manager;
        let steps = ((seconds * 20.0) as u32).max(1);
        for k in 1..=steps {
            std::thread::sleep(std::time::Duration::from_secs_f64(seconds / steps as f64));
            let st = app.state::<AppState>();
            let s = st.lock();
            let Some(g) = s.graph.as_ref() else { return };
            let db = from_db + (-60.0 - from_db) * (k as f32 / steps as f32);
            g.params().master.gain_db.set(db);
        }
        let st = app.state::<AppState>();
        let mut s = st.lock();
        release_all(&mut s);
        crate::graph_cmds::release_slot(&mut s, crate::graph_cmds::DRONE_SLOT);
        crate::graph_cmds::release_slot(&mut s, crate::graph_cmds::PAD_SLOT);
        // Next start comes back at the user's level, not at −60.
        if let Some(g) = s.graph.as_ref() {
            g.params().master.gain_db.set(from_db);
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_key_pitch_set_is_inharmonic_consistent_and_inside_the_register() {
        let s = Inner::default();
        let a = pitch_set(&s, false, (48, 72), 5);
        let b = pitch_set(&s, false, (48, 72), 5);
        let c = pitch_set(&s, false, (48, 72), 6);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(!a.is_empty() && a.len() <= MAX_PITCHES);
        let top = 440.0 * 2f32.powf((72.0 - 69.0) / 12.0) * 1.05;
        assert!(a.iter().all(|&f| f > 100.0 && f <= top), "{a:?}");
        // Asking for tonal without a project falls back to the same no-key set.
        assert_eq!(pitch_set(&s, true, (48, 72), 5), a);
    }
}
