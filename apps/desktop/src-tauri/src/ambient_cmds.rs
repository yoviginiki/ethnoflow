//! Ambient Playground commands (F1A.1/F1A.2): the project's rack (`ambient.json`) on
//! graph slots 4..12, next to the drone (0), the pad (1) and the arrangement's clips
//! (2). Layer edits go through atomics while playing; only a change of kind, seed
//! source or loop audio rebuilds that one slot. Tonal layers take their pitch set
//! from the project's scale and instrument intonation; without a scale, or when the
//! layer is set to "no key", they use an inharmonic set derived from the seed.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use audio_engine::{
    write_wav, AudioBuffer, BeatClock, BellParams, BellSource, BloomParams, BloomSource,
    ChoirParams, ChoirSource, GrainParams, GrainSource, Graph, GraphCmd, GraphParams, LoopParams,
    LoopSource, NoiseParams, NoiseSource, PluckParams, PluckSource, PulseParams, PulseSource,
    ResonatorParams, ResonatorSource, SamplerParams, SamplerSource, SamplerZone, TextureParams,
    TextureSource, WavFormat, WindParams, WindSource, MAX_PITCHES,
};
use project::{
    effective_rack, is_animated, Ambient, AmbientLayer, LayerSource, Macros, MAX_AMBIENT_LAYERS,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::graph_cmds::{ensure_graph, note_hz, Intonation};
use crate::state::{AppState, Inner};

/// First graph slot of the atmosphere rack; layer `i` sits in `AMBIENT_SLOT0 + i`.
pub const AMBIENT_SLOT0: usize = 4;
/// First graph slot of the Soundscapes rack (its own eight slots after the rhythm extras).
pub const SCAPES_SLOT0: usize = 24;
/// The Soundscapes rack's file in the project folder (same format as `ambient.json`).
pub const SCAPES_FILE: &str = "scapes.json";

/// Which of the two racks a command or helper works on. The atmosphere and the
/// Soundscapes journey are the same engine on different slots, buses and files, so
/// every rack function takes a `RackId` instead of assuming "the" rack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RackId {
    Ambient,
    Scapes,
}

impl RackId {
    pub const ALL: [RackId; 2] = [RackId::Ambient, RackId::Scapes];
    pub fn idx(self) -> usize {
        match self {
            RackId::Ambient => 0,
            RackId::Scapes => 1,
        }
    }
    pub fn slot0(self) -> usize {
        match self {
            RackId::Ambient => AMBIENT_SLOT0,
            RackId::Scapes => SCAPES_SLOT0,
        }
    }
    pub fn bus(self) -> usize {
        match self {
            RackId::Ambient => crate::bus_cmds::BUS_AMBIENT,
            RackId::Scapes => crate::bus_cmds::BUS_SCAPES,
        }
    }
    pub fn file(self) -> &'static str {
        match self {
            RackId::Ambient => project::AMBIENT_FILE,
            RackId::Scapes => SCAPES_FILE,
        }
    }
    /// Stem / group name in exports.
    pub fn stem(self) -> &'static str {
        match self {
            RackId::Ambient => "atmosphere",
            RackId::Scapes => "soundscapes",
        }
    }
    fn other(self) -> RackId {
        match self {
            RackId::Ambient => RackId::Scapes,
            RackId::Scapes => RackId::Ambient,
        }
    }
}

/// Everything the app keeps per rack while it is loaded / playing.
#[derive(Default)]
pub struct RackState {
    /// The rack (loaded from the project folder on first use).
    pub rack: Option<Ambient>,
    /// Whether the rack is on the graph.
    pub on: bool,
    /// Live handles per layer while it plays (None = slot not built).
    pub live: Vec<Option<LiveLayer>>,
    /// Automation / program clock while the rack plays.
    pub clock: Option<Clock>,
}

/// The master reverb / shimmer are shared by both racks: the atmosphere owns them while
/// it plays, otherwise the Soundscapes rack does.
fn owns_master(s: &Inner, id: RackId) -> bool {
    id == RackId::Ambient || !s.racks[RackId::Ambient.idx()].on
}

/// Live parameter handles of one layer while the rack plays.
pub enum LiveParams {
    Noise(Arc<NoiseParams>),
    Wind(Arc<WindParams>),
    Texture(Arc<TextureParams>),
    Bells(Arc<BellParams>),
    Pluck(Arc<PluckParams>),
    Loop(Arc<LoopParams>),
    Cloud(Arc<GrainParams>),
    Resonator(Arc<ResonatorParams>),
    Choir(Arc<ChoirParams>),
    Bloom(Arc<BloomParams>),
    Pulse(Arc<PulseParams>),
    Sampler(Arc<SamplerParams>),
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
    /// Program timeline (F1A.5): position in seconds, whether it runs, its end.
    pub program_position: f64,
    pub program_playing: bool,
    pub program_end: f64,
    /// Seconds of automation time since the rack started.
    pub lane_time: f64,
}

/// Shared clock state: written by the transport commands, read by the worker thread.
pub struct Clock {
    running: Arc<AtomicBool>,
    /// Program position in microseconds (so the UI can seek while it runs).
    program_us: Arc<AtomicU64>,
    program_playing: Arc<AtomicBool>,
    lane_us: Arc<AtomicU64>,
}

impl Clock {
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
    pub fn program_position(&self) -> f64 {
        self.program_us.load(Ordering::Relaxed) as f64 / 1e6
    }
    pub fn lane_time(&self) -> f64 {
        self.lane_us.load(Ordering::Relaxed) as f64 / 1e6
    }
    pub fn program_playing(&self) -> bool {
        self.program_playing.load(Ordering::Relaxed)
    }
}

const CLOCK_TICK_MS: u64 = 25;

/// Push one effective state (layers + macros) into the live graph params.
fn push_effective(s: &Inner, id: RackId, layers: &[AmbientLayer], macros: &Macros) {
    let Some(g) = s.graph.as_ref() else { return };
    let sync = s.racks[id.idx()].rack.as_ref().map(|a| a.sync).unwrap_or(0);
    let chord = chord_for(s, id);
    let any_solo = layers.iter().any(|l| l.solo);
    for (i, l) in layers.iter().enumerate() {
        if let Some(Some(live)) = s.racks[id.idx()].live.get(i) {
            apply_source(s, &live.params, l, macros, chord.as_deref());
            apply_sync(&live.params, sync);
        }
        if id.slot0() + i < audio_engine::MAX_LAYERS {
            apply_layer_strip(
                &g.params().layers[id.slot0() + i],
                l,
                macros,
                any_solo,
                s,
                id.bus(),
            );
        }
    }
    if owns_master(s, id) {
        g.params().master.space.set(macros.space as f32);
        g.params().master.space_size.set(macros.space_size as f32);
        g.params().master.shimmer.set(macros.shimmer as f32);
        g.params().master.shimmer_st.set(macros.shimmer_st as f32);
    }
}

/// Start the worker that advances the automation/program time and pushes the
/// effective state every `CLOCK_TICK_MS`. Cheap when nothing is animated.
fn start_clock(app: tauri::AppHandle, s: &mut Inner, id: RackId) {
    if s.racks[id.idx()].clock.is_some() {
        return;
    }
    let clock = Clock {
        running: Arc::new(AtomicBool::new(true)),
        program_us: Arc::new(AtomicU64::new(0)),
        program_playing: Arc::new(AtomicBool::new(false)),
        lane_us: Arc::new(AtomicU64::new(0)),
    };
    let running = Arc::clone(&clock.running);
    let program_us = Arc::clone(&clock.program_us);
    let program_playing = Arc::clone(&clock.program_playing);
    let lane_us = Arc::clone(&clock.lane_us);
    s.racks[id.idx()].clock = Some(clock);
    std::thread::spawn(move || {
        use tauri::Manager;
        let mut last = std::time::Instant::now();
        while running.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_millis(CLOCK_TICK_MS));
            let now = std::time::Instant::now();
            let dt_us = (now - last).as_micros() as u64;
            last = now;
            lane_us.fetch_add(dt_us, Ordering::Relaxed);
            let st = app.state::<AppState>();
            let s = st.lock();
            if !s.racks[id.idx()].on || s.graph.is_none() {
                continue;
            }
            let Some(a) = s.racks[id.idx()].rack.as_ref() else {
                continue;
            };
            let end = a.program.end();
            let mut pos = program_us.load(Ordering::Relaxed) as f64 / 1e6;
            if program_playing.load(Ordering::Relaxed) && end > 0.0 {
                pos += dt_us as f64 / 1e6;
                pos = a.program.wrap(pos);
                if pos >= end && a.program.loop_region.is_none() {
                    // Finale: hold the last state and stop the program clock.
                    pos = end;
                    program_playing.store(false, Ordering::Relaxed);
                }
                program_us.store((pos * 1e6) as u64, Ordering::Relaxed);
            }
            if !is_animated(a) {
                continue;
            }
            let lane_t = lane_us.load(Ordering::Relaxed) as f64 / 1e6;
            let program_t = if end > 0.0 { Some(pos) } else { None };
            let (layers, macros) = effective_rack(a, program_t, lane_t);
            push_effective(&s, id, &layers, &macros);
        }
    });
}

fn project_root(s: &Inner) -> CmdResult<std::path::PathBuf> {
    s.project
        .as_ref()
        .map(|p| p.root().to_path_buf())
        .ok_or_else(|| err("no_project", "No project is open"))
}

/// The rack of the open project, loaded from disk on first use.
pub fn rack_mut(s: &mut Inner, id: RackId) -> CmdResult<&mut Ambient> {
    if s.racks[id.idx()].rack.is_none() {
        let root = project_root(s)?;
        let a = Ambient::load_file(&root.join(id.file())).map_err(|e| err("io", e.to_string()))?;
        s.racks[id.idx()].rack = Some(a);
    }
    Ok(s.racks[id.idx()].rack.as_mut().expect("just loaded"))
}

/// Pitch set of a tonal layer: every degree of the project's scale inside the
/// register (profile intonation when available), thinned to `MAX_PITCHES` spread
/// across the register. Without a scale or with `tonal = false`: an inharmonic set
/// from the seed (no key is ever assumed).
/// The chord a rack follows now: its pitch classes, when the rack says so and a chord is set.
pub fn chord_for(s: &Inner, id: RackId) -> Option<Vec<u8>> {
    let follows = s.racks[id.idx()]
        .rack
        .as_ref()
        .map(|a| a.follow_chord)
        .unwrap_or(false);
    if !follows {
        return None;
    }
    s.chord_midi.clone().filter(|c| !c.is_empty())
}

/// Every note of the chord's pitch classes inside the register (thinned to the engine's
/// maximum, spread across the register, lowest first), through the project's A4.
fn chord_hz_in_register(s: &Inner, chord: &[u8], register: (u8, u8)) -> Vec<f32> {
    let (lo, hi) = (register.0.min(register.1), register.1.max(register.0));
    let pcs: Vec<u8> = chord.iter().map(|m| m % 12).collect();
    let notes: Vec<u8> = (lo..=hi).filter(|m| pcs.contains(&(m % 12))).collect();
    if notes.is_empty() {
        return Vec::new();
    }
    let picked: Vec<u8> = if notes.len() <= MAX_PITCHES {
        notes
    } else {
        (0..MAX_PITCHES)
            .map(|k| notes[k * (notes.len() - 1) / (MAX_PITCHES - 1)])
            .collect()
    };
    picked
        .into_iter()
        .map(|m| note_hz(s, m, 0.0, Intonation::Profile).0)
        .collect()
}

/// A tonal layer's pitches: the followed chord inside its register when the rack follows
/// one, otherwise the mode (or the no-key set).
pub fn layer_pitches(
    s: &Inner,
    chord: Option<&[u8]>,
    tonal: bool,
    register: (u8, u8),
    seed: u32,
) -> Vec<f32> {
    if tonal {
        if let Some(c) = chord {
            let hz = chord_hz_in_register(s, c, register);
            if !hz.is_empty() {
                return hz;
            }
        }
    }
    pitch_set(s, tonal, register, seed)
}

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
        LayerSource::Cloud { take_id, .. } => format!("cloud:{take_id}:{rate}:{}", l.seed),
        LayerSource::Sampler { take_id, zones, .. } => format!(
            "sampler:{take_id}:{rate}:{}:{}",
            l.seed,
            zones
                .iter()
                .map(|z| format!("{}@{}", z.take_id, z.root_midi))
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => format!("{}:{}", other.kind(), l.seed),
    }
}

/// Apply macros + the layer's own level/pan/filters to its graph slot.
/// Re-push every layer's strip (level/pan/filters) through the atmosphere bus.
pub fn reapply_strips(s: &Inner, id: RackId) {
    let (Some(a), Some(g)) = (s.racks[id.idx()].rack.as_ref(), s.graph.as_ref()) else {
        return;
    };
    let any_solo = a.layers.iter().any(|l| l.solo);
    for (i, l) in a.layers.iter().enumerate() {
        if id.slot0() + i < audio_engine::MAX_LAYERS {
            apply_layer_strip(
                &g.params().layers[id.slot0() + i],
                l,
                &a.macros,
                any_solo,
                s,
                id.bus(),
            );
        }
    }
}

pub(crate) fn apply_layer_strip(
    layer: &audio_engine::LayerParams,
    l: &AmbientLayer,
    m: &Macros,
    any_solo: bool,
    s: &Inner,
    bus: usize,
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
    // The slider's floor (−60) and intensity 0 mean "off": the graph then skips the
    // layer entirely instead of leaving it faintly audible.
    let gain = if l.gain_db <= -59.5 || m.intensity <= 0.001 {
        -120.0
    } else {
        (l.gain_db + intensity_db + gravity_db).clamp(-120.0, 12.0)
    };
    layer
        .gain_db
        .set(crate::bus_cmds::bus_gain(s, bus, gain as f32));
    // Brightness: shifts the low-pass by ±2 octaves around the layer's own setting.
    let lp = (l.lowpass_hz * 2f64.powf((m.brightness - 0.5) * 4.0)).clamp(40.0, 20_000.0);
    crate::bus_cmds::apply_bus_strip(
        s,
        bus,
        layer,
        (l.pan * m.width) as f32,
        lp as f32,
        l.highpass_hz as f32,
    );
    layer.mute.store(l.mute, Ordering::Relaxed);
    // Solo is rack-wide, but the graph's solo is graph-wide; keep the drone/pad/clips
    // audible by expressing a rack solo as mutes of the other rack layers.
    layer.solo.store(false, Ordering::Relaxed);
    if any_solo && !l.solo {
        layer.mute.store(true, Ordering::Relaxed);
    }
}

/// Push the layer's source parameters (motion macro included) into the live handles.
/// The rack's beat grid into its event sources (item 7): 0 free, else lines per beat.
pub(crate) fn apply_sync(live: &LiveParams, sync: u8) {
    match live {
        LiveParams::Bells(p) => p.sync.store(sync, Ordering::Relaxed),
        LiveParams::Pluck(p) => p.sync.store(sync, Ordering::Relaxed),
        LiveParams::Resonator(p) => p.sync.store(sync, Ordering::Relaxed),
        LiveParams::Bloom(p) => p.sync.store(sync, Ordering::Relaxed),
        LiveParams::Sampler(p) => p.sync.store(sync, Ordering::Relaxed),
        _ => {}
    }
}

pub(crate) fn apply_source(
    s: &Inner,
    live: &LiveParams,
    l: &AmbientLayer,
    m: &Macros,
    chord: Option<&[u8]>,
) {
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
            p.pitches
                .set(&layer_pitches(s, chord, *tonal, *register, l.seed));
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
                tension,
                tonal,
                register,
            },
            LiveParams::Bells(p),
        ) => {
            p.tension.set(*tension as f32);
            p.pitches
                .set(&layer_pitches(s, chord, *tonal, *register, l.seed));
            p.density.set((*density * motion) as f32);
            p.decay_s.set(*decay_s as f32);
            p.brightness.set(*brightness as f32);
            p.velocity_spread.set(*velocity_spread as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Resonator {
                density,
                decay_s,
                structure,
                brightness,
                strike,
                tonal,
                register,
            },
            LiveParams::Resonator(p),
        ) => {
            p.pitches
                .set(&layer_pitches(s, chord, *tonal, *register, l.seed));
            p.density.set((*density * motion) as f32);
            p.decay_s.set(*decay_s as f32);
            p.structure.set(*structure as f32);
            p.brightness.set(*brightness as f32);
            p.strike.set(*strike as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Choir {
                vowel,
                breath,
                spread,
                brightness,
                morph,
                voices,
                tonal,
                register,
            },
            LiveParams::Choir(p),
        ) => {
            p.pitches
                .set(&layer_pitches(s, chord, *tonal, *register, l.seed));
            p.vowel.set(*vowel as f32);
            p.breath.set(*breath as f32);
            p.width.set(*spread as f32);
            p.brightness.set(*brightness as f32);
            p.morph.set((*morph * motion.max(0.1)) as f32);
            p.voices.store(*voices as u32, Ordering::Relaxed);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Pulse {
                period_s,
                presence,
                color,
                softness,
                tonal,
                register,
            },
            LiveParams::Pulse(p),
        ) => {
            p.pitches
                .set(&layer_pitches(s, chord, *tonal, *register, l.seed));
            p.period_s.set(*period_s as f32);
            p.presence.set(*presence as f32);
            p.color.set(*color as f32);
            p.softness.set(*softness as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Bloom {
                density,
                note_s,
                brightness,
                attack_s,
                phrase,
                tonal,
                register,
            },
            LiveParams::Bloom(p),
        ) => {
            p.pitches
                .set(&layer_pitches(s, chord, *tonal, *register, l.seed));
            p.density.set((*density * motion) as f32);
            p.note_s.set(*note_s as f32);
            p.brightness.set(*brightness as f32);
            p.attack_s.set(*attack_s as f32);
            p.phrase.set(*phrase as f32);
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
            p.pitches
                .set(&layer_pitches(s, chord, *tonal, *register, l.seed));
            p.density.set((*density * motion) as f32);
            p.damping.set(*damping as f32);
            p.brightness.set(*brightness as f32);
            p.velocity_spread.set(*velocity_spread as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Sampler {
                root_midi,
                density,
                note_s,
                attack_s,
                release_s,
                cutoff_hz,
                resonance,
                velocity_spread,
                start,
                tonal,
                register,
                ..
            },
            LiveParams::Sampler(p),
        ) => {
            p.pitches
                .set(&layer_pitches(s, chord, *tonal, *register, l.seed));
            let (root_hz, _) = crate::graph_cmds::note_hz(
                s,
                root_midi.round().clamp(0.0, 127.0) as u8,
                0.0,
                crate::graph_cmds::Intonation::Tempered,
            );
            p.root_hz.set(root_hz);
            p.density.set((*density * motion) as f32);
            p.note_s.set(*note_s as f32);
            p.attack_s.set(*attack_s as f32);
            p.release_s.set(*release_s as f32);
            // Brightness macro shifts the filter by ±2 octaves like the strip's low-pass.
            p.cutoff_hz.set(
                (*cutoff_hz * 2f64.powf((m.brightness - 0.5) * 4.0)).clamp(40.0, 20_000.0) as f32,
            );
            p.resonance.set(*resonance as f32);
            p.velocity_spread.set(*velocity_spread as f32);
            p.start.set(*start as f32);
            p.seed.store(l.seed, Ordering::Relaxed);
        }
        (
            LayerSource::Cloud {
                grain_ms,
                density,
                position,
                spray,
                scan,
                pitch,
                jitter,
                reverse_p,
                octave_p,
                ..
            },
            LiveParams::Cloud(p),
        ) => {
            p.grain_ms.set(*grain_ms as f32);
            p.density.set((*density * motion) as f32);
            p.position.set(*position as f32);
            p.spray.set(*spray as f32);
            p.scan.set(*scan as f32);
            p.pitch.set(*pitch as f32);
            p.jitter.set(*jitter as f32);
            p.reverse.set(*reverse_p as f32);
            p.octave.set(*octave_p as f32);
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
fn build_layer(s: &mut Inner, id: RackId, i: usize) -> CmdResult<()> {
    let l = s.racks[id.idx()]
        .rack
        .as_ref()
        .and_then(|a| a.layers.get(i))
        .cloned()
        .ok_or_else(|| err("invalid_argument", "no such layer"))?;
    let rate = s
        .graph
        .as_ref()
        .map(|g| g.sample_rate())
        .ok_or_else(|| err("no_graph", "The audio graph is not running"))?;
    let clock = Some(Arc::clone(&s.master_clock));
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
                Box::new(BellSource::new(p).with_clock(clock.clone())),
            )
        }
        LayerSource::Resonator { .. } => {
            let p = Arc::new(ResonatorParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Resonator(Arc::clone(&p)),
                Box::new(ResonatorSource::new(p).with_clock(clock.clone())),
            )
        }
        LayerSource::Choir { .. } => {
            let p = Arc::new(ChoirParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Choir(Arc::clone(&p)),
                Box::new(ChoirSource::new(p)),
            )
        }
        LayerSource::Pulse { .. } => {
            let p = Arc::new(PulseParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Pulse(Arc::clone(&p)),
                Box::new(PulseSource::new(p)),
            )
        }
        LayerSource::Bloom { .. } => {
            let p = Arc::new(BloomParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Bloom(Arc::clone(&p)),
                Box::new(BloomSource::new(p).with_clock(clock.clone())),
            )
        }
        LayerSource::Pluck { .. } => {
            let p = Arc::new(PluckParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Pluck(Arc::clone(&p)),
                Box::new(PluckSource::new(p, rate).with_clock(clock.clone())),
            )
        }
        LayerSource::Sampler { take_id, zones, .. } => {
            let p = Arc::new(SamplerParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            let src: Box<dyn audio_engine::Source> = if zones.is_empty() {
                let buf = loop_buffer(s, take_id, false, rate)?;
                Box::new(SamplerSource::new(Arc::clone(&p), buf).with_clock(clock.clone()))
            } else {
                let mut zs = Vec::with_capacity(zones.len());
                for z in zones {
                    let buf = loop_buffer(s, &z.take_id, false, rate)?;
                    let (root_hz, _) = crate::graph_cmds::note_hz(
                        s,
                        z.root_midi.round().clamp(0.0, 127.0) as u8,
                        0.0,
                        crate::graph_cmds::Intonation::Tempered,
                    );
                    zs.push(SamplerZone { root_hz, buf });
                }
                Box::new(SamplerSource::new_multi(Arc::clone(&p), zs).with_clock(clock.clone()))
            };
            (LiveParams::Sampler(p), src)
        }
        LayerSource::Cloud { take_id, .. } => {
            let buf = loop_buffer(s, take_id, false, rate)?;
            let p = Arc::new(GrainParams::default());
            p.seed.store(l.seed, Ordering::Relaxed);
            (
                LiveParams::Cloud(Arc::clone(&p)),
                Box::new(GrainSource::new(p, buf)),
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
    let macros = s.racks[id.idx()]
        .rack
        .as_ref()
        .map(|a| a.macros.clone())
        .unwrap_or_default();
    let chord = chord_for(s, id);
    apply_source(s, &params, &l, &macros, chord.as_deref());
    apply_sync(
        &params,
        s.racks[id.idx()].rack.as_ref().map(|a| a.sync).unwrap_or(0),
    );
    let slot = id.slot0() + i;
    let graph = s.graph.as_mut().expect("checked above");
    graph.params().layers[slot].gain_db.set(-60.0);
    if !graph.send(GraphCmd::SetSource { slot, source }) {
        return Err(err("graph_busy", "Graph command queue is full"));
    }
    while s.racks[id.idx()].live.len() <= i {
        s.racks[id.idx()].live.push(None);
    }
    s.racks[id.idx()].live[i] = Some(LiveLayer {
        build_key: key,
        params,
    });
    Ok(())
}

/// Bring the graph in line with the rack: rebuild changed slots, update the rest,
/// clear slots of removed layers, set the master space.
fn sync_live(s: &mut Inner, id: RackId) -> CmdResult<()> {
    if !s.racks[id.idx()].on || s.graph.is_none() {
        return Ok(());
    }
    let rate = s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000);
    let a = rack_mut(s, id)?.clone();
    let any_solo = a.layers.iter().any(|l| l.solo);
    for (i, l) in a.layers.iter().enumerate() {
        let key = build_key(l, rate);
        let same = s.racks[id.idx()]
            .live
            .get(i)
            .and_then(|x| x.as_ref())
            .map(|x| x.build_key == key)
            .unwrap_or(false);
        if !same {
            build_layer(s, id, i)?;
        } else if let Some(Some(live)) = s.racks[id.idx()].live.get(i) {
            let chord = chord_for(s, id);
            apply_source(s, &live.params, l, &a.macros, chord.as_deref());
            apply_sync(&live.params, a.sync);
        }
        let graph = s.graph.as_ref().expect("graph");
        apply_layer_strip(
            &graph.params().layers[id.slot0() + i],
            l,
            &a.macros,
            any_solo,
            &*s,
            id.bus(),
        );
    }
    // Layers that no longer exist: fade and clear.
    let live_len = s.racks[id.idx()].live.len();
    for i in a.layers.len()..live_len {
        if s.racks[id.idx()].live[i].is_some() {
            if let Some(g) = s.graph.as_mut() {
                g.params().layers[id.slot0() + i].gain_db.set(-60.0);
                g.send(GraphCmd::Clear {
                    slot: id.slot0() + i,
                });
            }
            s.racks[id.idx()].live[i] = None;
        }
    }
    s.racks[id.idx()].live.truncate(a.layers.len());
    if let (Some(g), true) = (s.graph.as_ref(), owns_master(s, id)) {
        g.params().master.space.set(a.macros.space as f32);
        g.params().master.space_size.set(a.macros.space_size as f32);
        g.params().master.shimmer.set(a.macros.shimmer as f32);
        g.params().master.shimmer_st.set(a.macros.shimmer_st as f32);
    }
    Ok(())
}

/// Stop every rack layer (fade, clear) and stop the stream if nothing else uses it.
pub fn release_all(s: &mut Inner, id: RackId) {
    let other_on = s.racks[id.other().idx()].on;
    let live_len = s.racks[id.idx()].live.len();
    if let Some(g) = s.graph.as_mut() {
        for i in 0..MAX_AMBIENT_LAYERS {
            g.params().layers[id.slot0() + i].gain_db.set(-60.0);
        }
        if !other_on {
            g.params().master.space.set(0.0);
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
        for i in 0..live_len {
            g.send(GraphCmd::Clear {
                slot: id.slot0() + i,
            });
        }
    }
    s.racks[id.idx()].live.clear();
    s.racks[id.idx()].on = false;
    if let Some(c) = s.racks[id.idx()].clock.take() {
        c.stop();
    }
    if s.graph_idle() {
        s.stop_graph();
    }
}

fn status(s: &mut Inner, id: RackId) -> CmdResult<AmbientStatus> {
    let playing = s.racks[id.idx()].on && s.graph.is_some();
    let a = rack_mut(s, id)?.clone();
    let chord = chord_for(s, id);
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
            }
            | LayerSource::Resonator {
                tonal, register, ..
            }
            | LayerSource::Choir {
                tonal, register, ..
            }
            | LayerSource::Pulse {
                tonal, register, ..
            }
            | LayerSource::Bloom {
                tonal, register, ..
            }
            | LayerSource::Sampler {
                tonal, register, ..
            } => layer_pitches(s, chord.as_deref(), *tonal, *register, l.seed),
            _ => Vec::new(),
        })
        .collect();
    let n = a.layers.len();
    let program_end = a.program.end();
    let (program_position, program_playing, lane_time) = match s.racks[id.idx()].clock.as_ref() {
        Some(c) => (c.program_position(), c.program_playing(), c.lane_time()),
        None => (0.0, false, 0.0),
    };
    let Some(graph) = s.graph.as_mut() else {
        return Ok(AmbientStatus {
            playing: false,
            sample_rate: 0,
            layer_peaks: vec![0.0; n],
            master_peak: 0.0,
            limiter_gain: 1.0,
            stream_errors: 0,
            pitches_hz,
            program_position,
            program_playing: false,
            program_end,
            lane_time,
        });
    };
    let st = graph.status();
    let m = graph.params().meters(id.slot0() + n);
    Ok(AmbientStatus {
        playing,
        sample_rate: st.sample_rate,
        layer_peaks: (0..n)
            .map(|i| m.layers.get(id.slot0() + i).copied().unwrap_or(0.0))
            .collect(),
        master_peak: m.master_peak,
        limiter_gain: m.limiter_gain,
        stream_errors: st.stream_errors,
        pitches_hz,
        program_position,
        program_playing,
        program_end,
        lane_time,
    })
}

/// A take was deleted: loop layers that used it are removed from the rack.
pub fn forget_take(s: &mut Inner, take_id: &str) {
    for id in RackId::ALL {
        let Ok(a) = rack_mut(s, id) else { continue };
        let before = a.layers.len();
        for l in a.layers.iter_mut() {
            if let LayerSource::Sampler { zones, .. } = &mut l.source {
                zones.retain(|z| z.take_id != take_id);
            }
        }
        a.layers.retain(|l| {
            !l.source.take_ids().contains(&take_id) && !l.source.take_ids().is_empty()
                || l.source.take_id().is_none()
        });
        if a.layers.len() != before {
            if let Ok(root) = project_root(s) {
                if let Some(a) = s.racks[id.idx()].rack.as_ref() {
                    let _ = a.save_file(&root.join(id.file()));
                }
            }
            let _ = sync_live(s, id);
        }
    }
}

fn rack_get(state: &AppState, id: RackId) -> CmdResult<AmbientView> {
    let mut s = state.lock();
    let playing = s.racks[id.idx()].on && s.graph.is_some();
    let a = rack_mut(&mut s, id)?.clone();
    Ok(AmbientView {
        ambient: a,
        playing,
    })
}

/// Replace the rack: validate, check loop takes exist, save, apply live.
fn rack_set(state: &AppState, id: RackId, ambient: Ambient) -> CmdResult<AmbientView> {
    ambient
        .validate()
        .map_err(|e| err("invalid_argument", e.to_string()))?;
    let mut s = state.lock();
    let root = project_root(&s)?;
    for l in &ambient.layers {
        for take_id in l.source.take_ids() {
            let p = s.project.as_ref().expect("project checked");
            p.take(take_id)
                .map_err(|_| err("missing", format!("Unknown take {take_id}")))?;
        }
    }
    let mut a = ambient;
    a.schema_version = project::ambient::AMBIENT_SCHEMA_VERSION;
    a.save_file(&root.join(id.file()))
        .map_err(|e| err("io", e.to_string()))?;
    s.racks[id.idx()].rack = Some(a.clone());
    sync_live(&mut s, id)?;
    let playing = s.racks[id.idx()].on && s.graph.is_some();
    Ok(AmbientView {
        ambient: a,
        playing,
    })
}

/// Start the rack on the output (never called automatically: the user presses Play).
fn rack_play(
    app: tauri::AppHandle,
    state: &AppState,
    id: RackId,
    device: Option<String>,
) -> CmdResult<AmbientStatus> {
    let mut s = state.lock();
    let _ = rack_mut(&mut s, id)?;
    ensure_graph(&mut s, device.as_deref())?;
    s.racks[id.idx()].on = true;
    s.racks[id.idx()].live.clear();
    if let Err(e) = sync_live(&mut s, id) {
        release_all(&mut s, id);
        return Err(e);
    }
    start_clock(app, &mut s, id);
    status(&mut s, id)
}

/// Program transport (F1A.5): run/pause the scene timeline and/or jump to a position.
/// Works while the rack is stopped too (the position is applied when it plays).
fn rack_program_set(
    state: &AppState,
    id: RackId,
    playing: Option<bool>,
    position: Option<f64>,
) -> CmdResult<AmbientStatus> {
    let mut s = state.lock();
    let end = rack_mut(&mut s, id)?.program.end();
    if let Some(c) = s.racks[id.idx()].clock.as_ref() {
        if let Some(p) = position {
            let p = p.clamp(0.0, end.max(0.0));
            c.program_us.store((p * 1e6) as u64, Ordering::Relaxed);
        }
        if let Some(pl) = playing {
            c.program_playing.store(pl && end > 0.0, Ordering::Relaxed);
        }
        // Apply the new position at once (the thread would do it within a tick).
        if let Some(a) = s.racks[id.idx()].rack.as_ref() {
            let pos = c.program_position();
            let lane_t = c.lane_time();
            let program_t = if end > 0.0 { Some(pos) } else { None };
            let (layers, macros) = effective_rack(a, program_t, lane_t);
            push_effective(&s, id, &layers, &macros);
        }
    }
    status(&mut s, id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmbientExportResult {
    pub path: String,
    pub bytes: u64,
    pub format: WavFormat,
    pub seconds: f64,
    /// Stem files written next to the mix (empty unless asked for).
    pub stems: Vec<String>,
    pub render_ms: u64,
}

/// Build offline sources for every layer at `rate` (loop audio read under the lock).
pub(crate) fn offline_sources(
    s: &mut Inner,
    a: &Ambient,
    rate: u32,
    clock: Option<Arc<BeatClock>>,
) -> CmdResult<Vec<(LiveParams, Box<dyn audio_engine::Source>)>> {
    let mut out = Vec::new();
    for l in &a.layers {
        let item: (LiveParams, Box<dyn audio_engine::Source>) = match &l.source {
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
                    Box::new(BellSource::new(p).with_clock(clock.clone())),
                )
            }
            LayerSource::Resonator { .. } => {
                let p = Arc::new(ResonatorParams::default());
                p.seed.store(l.seed, Ordering::Relaxed);
                (
                    LiveParams::Resonator(Arc::clone(&p)),
                    Box::new(ResonatorSource::new(p).with_clock(clock.clone())),
                )
            }
            LayerSource::Choir { .. } => {
                let p = Arc::new(ChoirParams::default());
                p.seed.store(l.seed, Ordering::Relaxed);
                (
                    LiveParams::Choir(Arc::clone(&p)),
                    Box::new(ChoirSource::new(p)),
                )
            }
            LayerSource::Pulse { .. } => {
                let p = Arc::new(PulseParams::default());
                p.seed.store(l.seed, Ordering::Relaxed);
                (
                    LiveParams::Pulse(Arc::clone(&p)),
                    Box::new(PulseSource::new(p)),
                )
            }
            LayerSource::Bloom { .. } => {
                let p = Arc::new(BloomParams::default());
                p.seed.store(l.seed, Ordering::Relaxed);
                (
                    LiveParams::Bloom(Arc::clone(&p)),
                    Box::new(BloomSource::new(p).with_clock(clock.clone())),
                )
            }
            LayerSource::Pluck { .. } => {
                let p = Arc::new(PluckParams::default());
                p.seed.store(l.seed, Ordering::Relaxed);
                (
                    LiveParams::Pluck(Arc::clone(&p)),
                    Box::new(PluckSource::new(p, rate).with_clock(clock.clone())),
                )
            }
            LayerSource::Sampler { take_id, zones, .. } => {
                let p = Arc::new(SamplerParams::default());
                p.seed.store(l.seed, Ordering::Relaxed);
                let src: Box<dyn audio_engine::Source> = if zones.is_empty() {
                    let buf = loop_buffer(s, take_id, false, rate)?;
                    Box::new(SamplerSource::new(Arc::clone(&p), buf).with_clock(clock.clone()))
                } else {
                    let mut zs = Vec::with_capacity(zones.len());
                    for z in zones {
                        let buf = loop_buffer(s, &z.take_id, false, rate)?;
                        let (root_hz, _) = crate::graph_cmds::note_hz(
                            s,
                            z.root_midi.round().clamp(0.0, 127.0) as u8,
                            0.0,
                            crate::graph_cmds::Intonation::Tempered,
                        );
                        zs.push(SamplerZone { root_hz, buf });
                    }
                    Box::new(SamplerSource::new_multi(Arc::clone(&p), zs).with_clock(clock.clone()))
                };
                (LiveParams::Sampler(p), src)
            }
            LayerSource::Cloud { take_id, .. } => {
                let buf = loop_buffer(s, take_id, false, rate)?;
                let p = Arc::new(GrainParams::default());
                p.seed.store(l.seed, Ordering::Relaxed);
                (
                    LiveParams::Cloud(Arc::clone(&p)),
                    Box::new(GrainSource::new(p, buf)),
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
        out.push(item);
    }
    Ok(out)
}

/// Offline bounce of the rack (F1A.5): `seconds` or the program's length, through
/// the same graph code as playback, with the program and automation evaluated per
/// block — deterministic and much faster than real time. Optional stems per layer.
#[tauri::command]
pub async fn export_ambient(
    state: State<'_, AppState>,
    path: String,
    format: Option<WavFormat>,
    seconds: Option<f64>,
    stems: bool,
    sample_rate: Option<u32>,
) -> CmdResult<AmbientExportResult> {
    let rate = sample_rate.unwrap_or(48_000);
    if !(8_000..=192_000).contains(&rate) {
        return Err(err("invalid_argument", "sample rate outside 8..192 kHz"));
    }
    let id = RackId::Ambient;
    let started = std::time::Instant::now();
    // Everything that needs the app state: the rack, the pitch sets, loop audio.
    let (a, sources, pitch_sets, master_db) = {
        let mut s = state.lock();
        let a = rack_mut(&mut s, id)?.clone();
        if a.layers.is_empty() {
            return Err(err("empty", "The atmosphere has no layers"));
        }
        let sources = offline_sources(&mut s, &a, rate, None)?;
        let pitch_sets: Vec<Vec<f32>> = a
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
                }
                | LayerSource::Resonator {
                    tonal, register, ..
                }
                | LayerSource::Choir {
                    tonal, register, ..
                }
                | LayerSource::Pulse {
                    tonal, register, ..
                }
                | LayerSource::Bloom {
                    tonal, register, ..
                }
                | LayerSource::Sampler {
                    tonal, register, ..
                } => pitch_set(&s, *tonal, *register, l.seed),
                _ => Vec::new(),
            })
            .collect();
        (a, sources, pitch_sets, s.master_db.unwrap_or(0.0))
    };
    let program_end = a.program.end();
    let seconds = seconds
        .filter(|s| *s > 0.0)
        .unwrap_or(if program_end > 0.0 { program_end } else { 60.0 })
        .min(4.0 * 3600.0);
    // An offline Inner only for `apply_source` (pitch sets are pre-resolved below).
    let gp = Arc::new(GraphParams::default());
    gp.master.gain_db.set(master_db.min(0.0));
    let mut g = Graph::new(Arc::clone(&gp), rate);
    let mut live: Vec<LiveParams> = Vec::new();
    for (i, (params, source)) in sources.into_iter().enumerate() {
        g.set_source(id.slot0() + i, source);
        live.push(params);
    }
    let frames = (seconds * rate as f64).round() as usize;
    let ch = 2usize;
    let mut out = vec![0.0f32; frames * ch];
    let mut stem_bufs: Vec<Vec<f32>> = if stems {
        (0..audio_engine::MAX_LAYERS)
            .map(|_| vec![0.0f32; frames * ch])
            .collect()
    } else {
        Vec::new()
    };
    let block = 512usize;
    let mut done = 0usize;
    let offline = Inner::default();
    while done < frames {
        let n = (frames - done).min(block);
        let t = done as f64 / rate as f64;
        let program_t = if program_end > 0.0 {
            Some(a.program.wrap(t).min(program_end))
        } else {
            None
        };
        let (layers, macros) = effective_rack(&a, program_t, t);
        let any_solo = layers.iter().any(|l| l.solo);
        for (i, l) in layers.iter().enumerate() {
            if let Some(p) = live.get(i) {
                apply_source(&offline, p, l, &macros, None);
                apply_sync(p, a.sync);
                // The offline Inner has no project: put the real pitch sets back.
                match p {
                    LiveParams::Texture(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Bells(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Pluck(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Resonator(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Choir(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Pulse(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Bloom(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Sampler(x) => x.pitches.set(&pitch_sets[i]),
                    LiveParams::Loop(x) => {
                        // apply_source used 48 kHz for the frame math; redo at `rate`.
                        if let LayerSource::Loop {
                            start,
                            end,
                            crossfade,
                            ..
                        } = &l.source
                        {
                            x.start_frames
                                .store((start * rate as f64).round() as u64, Ordering::Relaxed);
                            x.end_frames
                                .store((end * rate as f64).round() as u64, Ordering::Relaxed);
                            x.crossfade_frames
                                .store((crossfade * rate as f64).round() as u32, Ordering::Relaxed);
                        }
                    }
                    _ => {}
                }
            }
            apply_layer_strip(
                &gp.layers[id.slot0() + i],
                l,
                &macros,
                any_solo,
                &offline,
                id.bus(),
            );
        }
        gp.master.space.set(macros.space as f32);
        gp.master.space_size.set(macros.space_size as f32);
        gp.master.shimmer.set(macros.shimmer as f32);
        gp.master.shimmer_st.set(macros.shimmer_st as f32);
        if done == 0 {
            // Start exactly at the first state: no fade-in ramp from −60 dB.
            let mut warm = vec![0.0f32; 2048 * ch];
            g.render(&mut warm, ch);
            g.reset();
        }
        let slice = &mut out[done * ch..(done + n) * ch];
        if stems {
            let mut parts: Vec<Vec<f32>> = (0..audio_engine::MAX_LAYERS)
                .map(|_| vec![0.0f32; n * ch])
                .collect();
            g.render_with_stems(slice, ch, &mut parts);
            for (sb, part) in stem_bufs.iter_mut().zip(parts.iter()) {
                sb[done * ch..(done + n) * ch].copy_from_slice(part);
            }
        } else {
            g.render(slice, ch);
        }
        done += n;
    }
    let mix = AudioBuffer::new(rate, ch as u16, out);
    let mut p = std::path::PathBuf::from(&path);
    if p.extension().is_none_or(|e| !e.eq_ignore_ascii_case("wav")) {
        p.set_extension("wav");
    }
    let format = format.unwrap_or_default();
    write_wav(&p, &mix, format)?;
    let mut stem_paths = Vec::new();
    if stems {
        let stem_dir = p.with_extension("");
        for (i, l) in a.layers.iter().enumerate() {
            let buf = AudioBuffer::new(
                rate,
                ch as u16,
                std::mem::take(&mut stem_bufs[id.slot0() + i]),
            );
            let name =
                project::sanitize_name(&l.name).unwrap_or_else(|_| format!("layer{}", i + 1));
            let sp = std::path::PathBuf::from(format!(
                "{}-{:02}-{}.wav",
                stem_dir.to_string_lossy(),
                i + 1,
                name
            ));
            write_wav(&sp, &buf, format)?;
            stem_paths.push(sp.to_string_lossy().into_owned());
        }
    }
    let bytes = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
    Ok(AmbientExportResult {
        path: p.to_string_lossy().into_owned(),
        bytes,
        format,
        seconds,
        stems: stem_paths,
        render_ms: started.elapsed().as_millis() as u64,
    })
}

// ---- the atmosphere rack ----
#[tauri::command]
pub async fn ambient_get(state: State<'_, AppState>) -> CmdResult<AmbientView> {
    rack_get(&state, RackId::Ambient)
}
#[tauri::command]
pub async fn set_ambient(state: State<'_, AppState>, ambient: Ambient) -> CmdResult<AmbientView> {
    rack_set(&state, RackId::Ambient, ambient)
}
#[tauri::command]
pub async fn ambient_play(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    device: Option<String>,
) -> CmdResult<AmbientStatus> {
    rack_play(app, &state, RackId::Ambient, device)
}
#[tauri::command]
pub async fn ambient_program_set(
    state: State<'_, AppState>,
    playing: Option<bool>,
    position: Option<f64>,
) -> CmdResult<AmbientStatus> {
    rack_program_set(&state, RackId::Ambient, playing, position)
}
#[tauri::command]
pub async fn ambient_stop(state: State<'_, AppState>) -> CmdResult<()> {
    release_all(&mut state.lock(), RackId::Ambient);
    Ok(())
}
#[tauri::command]
pub async fn ambient_status(state: State<'_, AppState>) -> CmdResult<AmbientStatus> {
    status(&mut state.lock(), RackId::Ambient)
}

/// The chord the pad / progression follows now (MIDI notes, or none): racks with
/// `follow_chord` re-pitch their tonal layers at once. Called by the UI's follower.
#[tauri::command]
pub async fn set_chord_pitches(state: State<'_, AppState>, midi: Option<Vec<u8>>) -> CmdResult<()> {
    let mut s = state.lock();
    s.chord_midi = midi.filter(|m| !m.is_empty());
    for id in RackId::ALL {
        if s.racks[id.idx()].on {
            let _ = sync_live(&mut s, id);
        }
    }
    Ok(())
}

// ---- the Soundscapes rack: the same engine on its own slots, bus and file ----
#[tauri::command]
pub async fn scapes_get(state: State<'_, AppState>) -> CmdResult<AmbientView> {
    rack_get(&state, RackId::Scapes)
}
#[tauri::command]
pub async fn set_scapes(state: State<'_, AppState>, ambient: Ambient) -> CmdResult<AmbientView> {
    rack_set(&state, RackId::Scapes, ambient)
}
#[tauri::command]
pub async fn scapes_play(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    device: Option<String>,
) -> CmdResult<AmbientStatus> {
    rack_play(app, &state, RackId::Scapes, device)
}
#[tauri::command]
pub async fn scapes_program_set(
    state: State<'_, AppState>,
    playing: Option<bool>,
    position: Option<f64>,
) -> CmdResult<AmbientStatus> {
    rack_program_set(&state, RackId::Scapes, playing, position)
}
#[tauri::command]
pub async fn scapes_stop(state: State<'_, AppState>) -> CmdResult<()> {
    release_all(&mut state.lock(), RackId::Scapes);
    Ok(())
}
/// Keyboard K3: one strike of an event layer (bells, pluck, resonator, sampler) of a
/// rack on the key's pitch. `rack` is "ambient" or "scapes"; `layer` the index in the
/// rack. Returns false when that layer is not live (rack off) or not an event source.
#[tauri::command]
pub async fn rack_note(
    state: State<'_, AppState>,
    rack: String,
    layer: usize,
    midi: u8,
    on: bool,
    velocity: Option<f32>,
) -> CmdResult<bool> {
    let id = match rack.as_str() {
        "ambient" => RackId::Ambient,
        "scapes" => RackId::Scapes,
        _ => return Err(err("invalid_argument", "rack is ambient or scapes")),
    };
    if midi > 127 {
        return Err(err("invalid_argument", "midi outside 0..127"));
    }
    let s = state.lock();
    let Some(Some(live)) = s.racks[id.idx()].live.get(layer) else {
        return Ok(false);
    };
    let vel = velocity.unwrap_or(0.8).clamp(0.0, 1.0);
    let ok = match &live.params {
        LiveParams::Bells(p) => p.live.note(midi, on, vel),
        LiveParams::Pluck(p) => p.live.note(midi, on, vel),
        LiveParams::Resonator(p) => p.live.note(midi, on, vel),
        LiveParams::Sampler(p) => p.live.note(midi, on, vel),
        _ => return Ok(false),
    };
    Ok(ok)
}

#[tauri::command]
pub async fn scapes_status(state: State<'_, AppState>) -> CmdResult<AmbientStatus> {
    status(&mut state.lock(), RackId::Scapes)
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
        for id in RackId::ALL {
            release_all(&mut s, id);
        }
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
