//! Automation and scenes for the ambient rack (F1A.4, F1A.5): pure functions of
//! time. A lane turns `t` into a parameter value from its seed and settings alone
//! (no running state), so the same seed and time always give the same value, live
//! and in an offline bounce. A program is a sequence of scenes (snapshots of the rack)
//! with crossfaded transitions; `effective()` combines rack, program and lanes.

use serde::{Deserialize, Serialize};

use crate::ambient::{Ambient, AmbientLayer, LayerSource, Macros};
use crate::error::{ProjectError, Result};

/// What a lane drives: a layer parameter (`layer` = its id) or a macro (`layer` None).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParamTarget {
    #[serde(default)]
    pub layer: Option<String>,
    pub param: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LfoShape {
    Sine,
    Triangle,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum AutomationMode {
    /// Holds the base value (a lane that is parked).
    Static,
    /// Periodic around the base value.
    Lfo {
        period_s: f64,
        /// 0..1 turns.
        phase: f64,
        shape: LfoShape,
    },
    /// Slow bounded random walk around the base value (seeded value noise).
    Walk { period_s: f64 },
    /// Absolute values at times (seconds), linearly interpolated; a drawn curve is a
    /// dense list of keyframes. Held before the first and after the last point.
    Keyframes { points: Vec<(f64, f64)> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AutomationLane {
    pub id: String,
    pub target: ParamTarget,
    pub mode: AutomationMode,
    /// 0..1 share of the parameter's range the modulation may cover (LFO/Walk).
    #[serde(default = "default_depth")]
    pub depth: f64,
    /// 0..1 small seeded wobble added on top (also to keyframes).
    #[serde(default)]
    pub humanize: f64,
    #[serde(default = "default_seed")]
    pub seed: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_depth() -> f64 {
    0.5
}
fn default_seed() -> u32 {
    1
}
fn default_true() -> bool {
    true
}

/// Numeric range of an automatable parameter, by name (None = not automatable).
pub fn param_range(param: &str) -> Option<(f64, f64)> {
    Some(match param {
        "gain_db" => (-60.0, 12.0),
        "pan" => (-1.0, 1.0),
        "lowpass_hz" => (40.0, 20_000.0),
        "highpass_hz" => (10.0, 8_000.0),
        "color" | "gust" | "speed" | "resonance" | "brightness" | "motion" | "damping"
        | "velocity_spread" | "structure" | "strike" => (0.0, 1.0),
        "center_hz" => (80.0, 6_000.0),
        "density" => (0.0, 120.0),
        "decay_s" => (0.2, 30.0),
        "rate" => (0.25, 4.0),
        "crossfade" => (0.0, 5.0),
        "grain_ms" => (10.0, 500.0),
        "position" | "spray" | "reverse_p" | "octave_p" => (0.0, 1.0),
        "scan" => (0.0, 2.0),
        "pitch" => (-24.0, 24.0),
        "jitter" => (0.0, 100.0),
        // macros
        "intensity" | "space" | "space_size" | "tonal_gravity" | "width" | "shimmer" => (0.0, 1.0),
        _ => return None,
    })
}

/// Parameters a layer of this kind exposes to automation (besides the strip ones).
pub fn source_params(src: &LayerSource) -> &'static [&'static str] {
    match src {
        LayerSource::Noise { .. } => &["color"],
        LayerSource::Wind { .. } => &["gust", "speed", "center_hz", "resonance"],
        LayerSource::Texture { .. } => &["brightness", "motion", "resonance"],
        LayerSource::Bells { .. } => &["density", "decay_s", "brightness", "velocity_spread"],
        LayerSource::Pluck { .. } => &["density", "damping", "brightness", "velocity_spread"],
        LayerSource::Loop { .. } => &["rate", "crossfade"],
        LayerSource::Resonator { .. } => {
            &["density", "decay_s", "structure", "brightness", "strike"]
        }
        LayerSource::Cloud { .. } => &[
            "grain_ms",
            "density",
            "position",
            "spray",
            "scan",
            "pitch",
            "jitter",
            "reverse_p",
            "octave_p",
        ],
    }
}
pub const STRIP_PARAMS: [&str; 4] = ["gain_db", "pan", "lowpass_hz", "highpass_hz"];
pub const MACRO_PARAMS: [&str; 8] = [
    "intensity",
    "brightness",
    "motion",
    "space",
    "space_size",
    "tonal_gravity",
    "width",
    "shimmer",
];

/// Read a numeric parameter of a layer by name.
pub fn layer_param(l: &AmbientLayer, param: &str) -> Option<f64> {
    Some(match param {
        "gain_db" => l.gain_db,
        "pan" => l.pan,
        "lowpass_hz" => l.lowpass_hz,
        "highpass_hz" => l.highpass_hz,
        _ => return source_param(&l.source, param),
    })
}

fn source_param(s: &LayerSource, param: &str) -> Option<f64> {
    Some(match (s, param) {
        (LayerSource::Noise { color }, "color") => *color,
        (LayerSource::Wind { gust, .. }, "gust") => *gust,
        (LayerSource::Wind { speed, .. }, "speed") => *speed,
        (LayerSource::Wind { center_hz, .. }, "center_hz") => *center_hz,
        (LayerSource::Wind { resonance, .. }, "resonance") => *resonance,
        (LayerSource::Texture { brightness, .. }, "brightness") => *brightness,
        (LayerSource::Texture { motion, .. }, "motion") => *motion,
        (LayerSource::Texture { resonance, .. }, "resonance") => *resonance,
        (LayerSource::Bells { density, .. }, "density") => *density,
        (LayerSource::Bells { decay_s, .. }, "decay_s") => *decay_s,
        (LayerSource::Bells { brightness, .. }, "brightness") => *brightness,
        (
            LayerSource::Bells {
                velocity_spread, ..
            },
            "velocity_spread",
        ) => *velocity_spread,
        (LayerSource::Pluck { density, .. }, "density") => *density,
        (LayerSource::Pluck { damping, .. }, "damping") => *damping,
        (LayerSource::Pluck { brightness, .. }, "brightness") => *brightness,
        (
            LayerSource::Pluck {
                velocity_spread, ..
            },
            "velocity_spread",
        ) => *velocity_spread,
        (LayerSource::Loop { rate, .. }, "rate") => *rate,
        (LayerSource::Loop { crossfade, .. }, "crossfade") => *crossfade,
        (LayerSource::Resonator { density, .. }, "density") => *density,
        (LayerSource::Resonator { decay_s, .. }, "decay_s") => *decay_s,
        (LayerSource::Resonator { structure, .. }, "structure") => *structure,
        (LayerSource::Resonator { brightness, .. }, "brightness") => *brightness,
        (LayerSource::Resonator { strike, .. }, "strike") => *strike,
        (LayerSource::Cloud { grain_ms, .. }, "grain_ms") => *grain_ms,
        (LayerSource::Cloud { density, .. }, "density") => *density,
        (LayerSource::Cloud { position, .. }, "position") => *position,
        (LayerSource::Cloud { spray, .. }, "spray") => *spray,
        (LayerSource::Cloud { scan, .. }, "scan") => *scan,
        (LayerSource::Cloud { pitch, .. }, "pitch") => *pitch,
        (LayerSource::Cloud { jitter, .. }, "jitter") => *jitter,
        (LayerSource::Cloud { reverse_p, .. }, "reverse_p") => *reverse_p,
        (LayerSource::Cloud { octave_p, .. }, "octave_p") => *octave_p,
        _ => return None,
    })
}

/// Write a numeric parameter of a layer by name (clamped to its range).
pub fn set_layer_param(l: &mut AmbientLayer, param: &str, v: f64) -> bool {
    let Some((lo, hi)) = param_range(param) else {
        return false;
    };
    let v = v.clamp(lo, hi);
    match param {
        "gain_db" => l.gain_db = v,
        "pan" => l.pan = v,
        "lowpass_hz" => l.lowpass_hz = v,
        "highpass_hz" => l.highpass_hz = v,
        _ => return set_source_param(&mut l.source, param, v),
    }
    true
}

fn set_source_param(s: &mut LayerSource, param: &str, v: f64) -> bool {
    match (s, param) {
        (LayerSource::Noise { color }, "color") => *color = v,
        (LayerSource::Wind { gust, .. }, "gust") => *gust = v,
        (LayerSource::Wind { speed, .. }, "speed") => *speed = v,
        (LayerSource::Wind { center_hz, .. }, "center_hz") => *center_hz = v,
        (LayerSource::Wind { resonance, .. }, "resonance") => *resonance = v,
        (LayerSource::Texture { brightness, .. }, "brightness") => *brightness = v,
        (LayerSource::Texture { motion, .. }, "motion") => *motion = v,
        (LayerSource::Texture { resonance, .. }, "resonance") => *resonance = v,
        (LayerSource::Bells { density, .. }, "density") => *density = v,
        (LayerSource::Bells { decay_s, .. }, "decay_s") => *decay_s = v,
        (LayerSource::Bells { brightness, .. }, "brightness") => *brightness = v,
        (
            LayerSource::Bells {
                velocity_spread, ..
            },
            "velocity_spread",
        ) => *velocity_spread = v,
        (LayerSource::Pluck { density, .. }, "density") => *density = v,
        (LayerSource::Pluck { damping, .. }, "damping") => *damping = v,
        (LayerSource::Pluck { brightness, .. }, "brightness") => *brightness = v,
        (
            LayerSource::Pluck {
                velocity_spread, ..
            },
            "velocity_spread",
        ) => *velocity_spread = v,
        (LayerSource::Loop { rate, .. }, "rate") => *rate = v,
        (LayerSource::Loop { crossfade, .. }, "crossfade") => *crossfade = v,
        (LayerSource::Resonator { density, .. }, "density") => *density = v,
        (LayerSource::Resonator { decay_s, .. }, "decay_s") => *decay_s = v,
        (LayerSource::Resonator { structure, .. }, "structure") => *structure = v,
        (LayerSource::Resonator { brightness, .. }, "brightness") => *brightness = v,
        (LayerSource::Resonator { strike, .. }, "strike") => *strike = v,
        (LayerSource::Cloud { grain_ms, .. }, "grain_ms") => *grain_ms = v,
        (LayerSource::Cloud { density, .. }, "density") => *density = v,
        (LayerSource::Cloud { position, .. }, "position") => *position = v,
        (LayerSource::Cloud { spray, .. }, "spray") => *spray = v,
        (LayerSource::Cloud { scan, .. }, "scan") => *scan = v,
        (LayerSource::Cloud { pitch, .. }, "pitch") => *pitch = v,
        (LayerSource::Cloud { jitter, .. }, "jitter") => *jitter = v,
        (LayerSource::Cloud { reverse_p, .. }, "reverse_p") => *reverse_p = v,
        (LayerSource::Cloud { octave_p, .. }, "octave_p") => *octave_p = v,
        _ => return false,
    }
    true
}

pub fn macro_param(m: &Macros, param: &str) -> Option<f64> {
    Some(match param {
        "intensity" => m.intensity,
        "brightness" => m.brightness,
        "motion" => m.motion,
        "space" => m.space,
        "space_size" => m.space_size,
        "tonal_gravity" => m.tonal_gravity,
        "width" => m.width,
        "shimmer" => m.shimmer,
        _ => return None,
    })
}

pub fn set_macro_param(m: &mut Macros, param: &str, v: f64) -> bool {
    let v = v.clamp(0.0, 1.0);
    match param {
        "shimmer" => m.shimmer = v,
        "intensity" => m.intensity = v,
        "brightness" => m.brightness = v,
        "motion" => m.motion = v,
        "space" => m.space = v,
        "space_size" => m.space_size = v,
        "tonal_gravity" => m.tonal_gravity = v,
        "width" => m.width = v,
        _ => return false,
    }
    true
}

/// Deterministic hash → [−1, 1].
fn hash_unit(seed: u32, i: i64) -> f64 {
    let mut x =
        (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ ((seed as u64) << 32 | seed as u64);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    ((x >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
}

/// Smooth value noise in [−1, 1] at position `x` (one random value per unit, cosine
/// interpolated). Stateless: any `x` gives the same value whenever it is asked.
pub fn value_noise(seed: u32, x: f64) -> f64 {
    let i = x.floor();
    let f = x - i;
    let a = hash_unit(seed, i as i64);
    let b = hash_unit(seed, i as i64 + 1);
    let s = 0.5 - 0.5 * (f * std::f64::consts::PI).cos();
    a + (b - a) * s
}

fn interp_keyframes(points: &[(f64, f64)], t: f64) -> Option<f64> {
    let first = points.first()?;
    if t <= first.0 {
        return Some(first.1);
    }
    for w in points.windows(2) {
        let (t0, v0) = w[0];
        let (t1, v1) = w[1];
        if t >= t0 && t <= t1 {
            if t1 <= t0 {
                return Some(v1);
            }
            return Some(v0 + (v1 - v0) * ((t - t0) / (t1 - t0)));
        }
    }
    points.last().map(|p| p.1)
}

impl AutomationLane {
    /// The lane's value at `t` seconds for a parameter whose current (base) value
    /// is `base`; always inside the parameter's range.
    pub fn value(&self, t: f64, base: f64) -> f64 {
        let (lo, hi) = param_range(&self.target.param).unwrap_or((0.0, 1.0));
        let span = hi - lo;
        let depth = self.depth.clamp(0.0, 1.0);
        let mut v = match &self.mode {
            AutomationMode::Static => base,
            AutomationMode::Lfo {
                period_s,
                phase,
                shape,
            } => {
                let p = period_s.max(0.01);
                let x = (t / p + phase).rem_euclid(1.0);
                let s = match shape {
                    LfoShape::Sine => (x * std::f64::consts::TAU).sin(),
                    LfoShape::Triangle => 1.0 - 4.0 * (x - 0.5).abs(),
                };
                base + s * depth * span * 0.5
            }
            AutomationMode::Walk { period_s } => {
                let p = period_s.max(0.01);
                base + value_noise(self.seed, t / p) * depth * span * 0.5
            }
            AutomationMode::Keyframes { points } => interp_keyframes(points, t).unwrap_or(base),
        };
        if self.humanize > 0.0 {
            v += value_noise(self.seed ^ 0xA5A5, t * 0.7)
                * self.humanize.clamp(0.0, 1.0)
                * span
                * 0.05;
        }
        v.clamp(lo, hi)
    }

    pub fn validate(&self, layers: &[AmbientLayer]) -> Result<()> {
        if param_range(&self.target.param).is_none() {
            return Err(ProjectError::InvalidState(format!(
                "lane {}: unknown parameter {}",
                self.id, self.target.param
            )));
        }
        match &self.target.layer {
            Some(id) => {
                let l = layers.iter().find(|l| &l.id == id).ok_or_else(|| {
                    ProjectError::InvalidState(format!("lane {}: no layer {id}", self.id))
                })?;
                if layer_param(l, &self.target.param).is_none() {
                    return Err(ProjectError::InvalidState(format!(
                        "lane {}: layer has no parameter {}",
                        self.id, self.target.param
                    )));
                }
            }
            None => {
                if !MACRO_PARAMS.contains(&self.target.param.as_str()) {
                    return Err(ProjectError::InvalidState(format!(
                        "lane {}: not a macro: {}",
                        self.id, self.target.param
                    )));
                }
            }
        }
        if !(0.0..=1.0).contains(&self.depth) || !(0.0..=1.0).contains(&self.humanize) {
            return Err(ProjectError::InvalidState(
                "lane depth/humanize 0..1".into(),
            ));
        }
        match &self.mode {
            AutomationMode::Lfo { period_s, .. } | AutomationMode::Walk { period_s } => {
                if !(*period_s > 0.0 && period_s.is_finite()) {
                    return Err(ProjectError::InvalidState("lane period".into()));
                }
            }
            AutomationMode::Keyframes { points } => {
                if points.windows(2).any(|w| w[1].0 < w[0].0)
                    || points.iter().any(|p| !p.0.is_finite() || !p.1.is_finite())
                {
                    return Err(ProjectError::InvalidState(
                        "keyframes must be finite and in time order".into(),
                    ));
                }
            }
            AutomationMode::Static => {}
        }
        Ok(())
    }
}

/// Numeric state of one layer inside a scene.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayerSnap {
    pub id: String,
    pub gain_db: f64,
    pub pan: f64,
    pub lowpass_hz: f64,
    pub highpass_hz: f64,
    pub mute: bool,
    pub source: LayerSource,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub layers: Vec<LayerSnap>,
    pub macros: Macros,
}

impl Snapshot {
    pub fn of(a: &Ambient) -> Self {
        Self {
            layers: a
                .layers
                .iter()
                .map(|l| LayerSnap {
                    id: l.id.clone(),
                    gain_db: l.gain_db,
                    pan: l.pan,
                    lowpass_hz: l.lowpass_hz,
                    highpass_hz: l.highpass_hz,
                    mute: l.mute,
                    source: l.source.clone(),
                })
                .collect(),
            macros: a.macros.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub name: String,
    /// Seconds on the program timeline.
    pub start_s: f64,
    pub duration_s: f64,
    /// Crossfade from the previous state at the scene's start, seconds.
    #[serde(default)]
    pub transition_s: f64,
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Program {
    #[serde(default)]
    pub scenes: Vec<Scene>,
    /// Loop region on the program timeline, seconds.
    #[serde(default)]
    pub loop_region: Option<(f64, f64)>,
}

impl Program {
    /// End of the last scene, seconds (0 without scenes).
    pub fn end(&self) -> f64 {
        self.scenes
            .iter()
            .map(|s| s.start_s + s.duration_s)
            .fold(0.0, f64::max)
    }

    pub fn validate(&self) -> Result<()> {
        for s in &self.scenes {
            if !(s.start_s >= 0.0 && s.duration_s > 0.0 && s.transition_s >= 0.0)
                || !s.start_s.is_finite()
                || !s.duration_s.is_finite()
            {
                return Err(ProjectError::InvalidState(format!(
                    "scene {} has an invalid time",
                    s.id
                )));
            }
        }
        if let Some((a, b)) = self.loop_region {
            if !(a >= 0.0 && b > a) {
                return Err(ProjectError::InvalidState("program loop region".into()));
            }
        }
        Ok(())
    }

    /// Map a timeline position into the loop region (if any) — the clock calls this.
    pub fn wrap(&self, t: f64) -> f64 {
        match self.loop_region {
            Some((a, b)) if b > a && t >= b => a + (t - a) % (b - a),
            _ => t,
        }
    }

    /// Scene index active at `t` (scenes sorted by start; the latest one that started).
    fn scene_at(&self, t: f64) -> Option<usize> {
        let mut best: Option<usize> = None;
        for (i, s) in self.scenes.iter().enumerate() {
            let inside = t >= s.start_s && t < s.start_s + s.duration_s;
            if inside
                && best
                    .map(|b| self.scenes[b].start_s <= s.start_s)
                    .unwrap_or(true)
            {
                best = Some(i);
            }
        }
        best
    }

    /// Scene that ended most recently before `t` (what a transition fades from).
    fn previous_scene(&self, i: usize) -> Option<&Scene> {
        let cur = &self.scenes[i];
        self.scenes
            .iter()
            .filter(|s| s.start_s + s.duration_s <= cur.start_s + 1e-9 && s.id != cur.id)
            .max_by(|a, b| {
                (a.start_s + a.duration_s)
                    .partial_cmp(&(b.start_s + b.duration_s))
                    .unwrap()
            })
    }

    /// The program's contribution at `t`: a snapshot (possibly mid-transition), or
    /// None outside every scene (the rack's own state applies).
    pub fn state_at(&self, t: f64) -> Option<Snapshot> {
        let i = self.scene_at(t)?;
        let cur = &self.scenes[i];
        let into = t - cur.start_s;
        if cur.transition_s > 0.0 && into < cur.transition_s {
            if let Some(prev) = self.previous_scene(i) {
                let r = (into / cur.transition_s).clamp(0.0, 1.0);
                return Some(lerp_snapshot(&prev.snapshot, &cur.snapshot, r));
            }
        }
        Some(cur.snapshot.clone())
    }
}

fn lerp(a: f64, b: f64, r: f64) -> f64 {
    a + (b - a) * r
}

fn lerp_source(a: &LayerSource, b: &LayerSource, r: f64) -> LayerSource {
    if a.kind() != b.kind() {
        return if r < 0.5 { a.clone() } else { b.clone() };
    }
    let mut out = b.clone();
    for p in source_params(b) {
        if let (Some(va), Some(vb)) = (source_param(a, p), source_param(b, p)) {
            set_source_param(&mut out, p, lerp(va, vb, r));
        }
    }
    out
}

fn lerp_macros(a: &Macros, b: &Macros, r: f64) -> Macros {
    Macros {
        intensity: lerp(a.intensity, b.intensity, r),
        brightness: lerp(a.brightness, b.brightness, r),
        motion: lerp(a.motion, b.motion, r),
        space: lerp(a.space, b.space, r),
        space_size: lerp(a.space_size, b.space_size, r),
        tonal_gravity: lerp(a.tonal_gravity, b.tonal_gravity, r),
        width: lerp(a.width, b.width, r),
        shimmer: lerp(a.shimmer, b.shimmer, r),
        // The transposition is a choice, not a continuum: switch halfway.
        shimmer_st: if r < 0.5 { a.shimmer_st } else { b.shimmer_st },
    }
}

/// Crossfade two snapshots: layers matched by id; a layer only on one side fades
/// its level from/to silence.
pub fn lerp_snapshot(a: &Snapshot, b: &Snapshot, r: f64) -> Snapshot {
    let mut layers = Vec::new();
    for lb in &b.layers {
        if let Some(la) = a.layers.iter().find(|l| l.id == lb.id) {
            layers.push(LayerSnap {
                id: lb.id.clone(),
                gain_db: lerp(la.gain_db, lb.gain_db, r),
                pan: lerp(la.pan, lb.pan, r),
                lowpass_hz: (la.lowpass_hz.ln() + (lb.lowpass_hz.ln() - la.lowpass_hz.ln()) * r)
                    .exp(),
                highpass_hz: (la.highpass_hz.ln()
                    + (lb.highpass_hz.ln() - la.highpass_hz.ln()) * r)
                    .exp(),
                mute: if r < 0.5 { la.mute } else { lb.mute },
                source: lerp_source(&la.source, &lb.source, r),
            });
        } else {
            let mut l = lb.clone();
            l.gain_db = lerp(-60.0, lb.gain_db, r);
            layers.push(l);
        }
    }
    for la in &a.layers {
        if !b.layers.iter().any(|l| l.id == la.id) {
            let mut l = la.clone();
            l.gain_db = lerp(la.gain_db, -60.0, r);
            layers.push(l);
        }
    }
    Snapshot {
        layers,
        macros: lerp_macros(&a.macros, &b.macros, r),
    }
}

/// Overlay a snapshot onto the rack's layers (structure stays: layers the snapshot
/// does not know keep their own values).
fn apply_snapshot(layers: &mut [AmbientLayer], macros: &mut Macros, snap: &Snapshot) {
    for l in layers.iter_mut() {
        if let Some(s) = snap.layers.iter().find(|s| s.id == l.id) {
            l.gain_db = s.gain_db;
            l.pan = s.pan;
            l.lowpass_hz = s.lowpass_hz;
            l.highpass_hz = s.highpass_hz;
            l.mute = s.mute;
            if s.source.kind() == l.source.kind() {
                l.source = s.source.clone();
            }
        }
    }
    *macros = snap.macros.clone();
}

/// The rack as it should sound at `program_t` on the program timeline and
/// `lane_t` seconds of automation time: rack → program scene → lanes.
pub fn effective(a: &Ambient, program_t: Option<f64>, lane_t: f64) -> (Vec<AmbientLayer>, Macros) {
    let mut layers = a.layers.clone();
    let mut macros = a.macros.clone();
    if let Some(t) = program_t {
        if let Some(snap) = a.program.state_at(t) {
            apply_snapshot(&mut layers, &mut macros, &snap);
        }
    }
    for lane in a.automation.iter().filter(|l| l.enabled) {
        match &lane.target.layer {
            Some(id) => {
                if let Some(l) = layers.iter_mut().find(|l| &l.id == id) {
                    if let Some(base) = layer_param(l, &lane.target.param) {
                        let v = lane.value(lane_t, base);
                        set_layer_param(l, &lane.target.param, v);
                    }
                }
            }
            None => {
                if let Some(base) = macro_param(&macros, &lane.target.param) {
                    let v = lane.value(lane_t, base);
                    set_macro_param(&mut macros, &lane.target.param, v);
                }
            }
        }
    }
    (layers, macros)
}

/// True when the rack has anything time-dependent (otherwise a clock is pointless).
pub fn is_animated(a: &Ambient) -> bool {
    a.automation
        .iter()
        .any(|l| l.enabled && !matches!(l.mode, AutomationMode::Static))
        || !a.program.scenes.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(id: &str, src: LayerSource) -> AmbientLayer {
        AmbientLayer {
            id: id.into(),
            name: id.into(),
            source: src,
            gain_db: -18.0,
            pan: 0.0,
            lowpass_hz: 20_000.0,
            highpass_hz: 10.0,
            mute: false,
            solo: false,
            seed: 3,
            license_ref: None,
            locked: false,
        }
    }

    fn rack() -> Ambient {
        let mut a = Ambient::default();
        a.layers
            .push(layer("w", LayerSource::default_for("wind").unwrap()));
        a.layers
            .push(layer("b", LayerSource::default_for("bells").unwrap()));
        a
    }

    #[test]
    fn lane_values_are_deterministic_bounded_and_survive_a_roundtrip() {
        let lanes = vec![
            AutomationLane {
                id: "l1".into(),
                target: ParamTarget {
                    layer: Some("w".into()),
                    param: "gust".into(),
                },
                mode: AutomationMode::Walk { period_s: 7.0 },
                depth: 1.0,
                humanize: 0.3,
                seed: 42,
                enabled: true,
            },
            AutomationLane {
                id: "l2".into(),
                target: ParamTarget {
                    layer: Some("b".into()),
                    param: "density".into(),
                },
                mode: AutomationMode::Lfo {
                    period_s: 60.0,
                    phase: 0.25,
                    shape: LfoShape::Triangle,
                },
                depth: 0.5,
                humanize: 0.0,
                seed: 1,
                enabled: true,
            },
            AutomationLane {
                id: "l3".into(),
                target: ParamTarget {
                    layer: None,
                    param: "space".into(),
                },
                mode: AutomationMode::Keyframes {
                    points: vec![(0.0, 0.1), (10.0, 0.9), (20.0, 0.3)],
                },
                depth: 1.0,
                humanize: 0.0,
                seed: 1,
                enabled: true,
            },
        ];
        let mut a = rack();
        a.automation = lanes;
        a.validate().unwrap();
        let (l1, m1) = effective(&a, None, 37.25);
        let (l2, m2) = effective(&a, None, 37.25);
        let text = serde_json::to_string(&a).unwrap();
        let back: Ambient = serde_json::from_str(&text).unwrap();
        let (l3, m3) = effective(&back, None, 37.25);
        assert_eq!(l1, l2);
        assert_eq!(l1, l3);
        assert_eq!(m1, m2);
        assert_eq!(m1, m3);
        // Bounds across time.
        for k in 0..2000 {
            let t = k as f64 * 0.37;
            let (ls, m) = effective(&a, None, t);
            let gust = layer_param(&ls[0], "gust").unwrap();
            assert!((0.0..=1.0).contains(&gust), "gust {gust} at {t}");
            let d = layer_param(&ls[1], "density").unwrap();
            assert!((0.0..=120.0).contains(&d));
            assert!((0.0..=1.0).contains(&m.space));
        }
        // The walk actually moves, the keyframes interpolate.
        let g0 = layer_param(&effective(&a, None, 0.0).0[0], "gust").unwrap();
        let g9 = layer_param(&effective(&a, None, 9.0).0[0], "gust").unwrap();
        assert!((g0 - g9).abs() > 0.02, "{g0} vs {g9}");
        assert!((effective(&a, None, 5.0).1.space - 0.5).abs() < 1e-9);
        assert!((effective(&a, None, 99.0).1.space - 0.3).abs() < 1e-9);
        // Unknown target is rejected.
        a.automation.push(AutomationLane {
            id: "bad".into(),
            target: ParamTarget {
                layer: Some("w".into()),
                param: "density".into(),
            },
            mode: AutomationMode::Static,
            depth: 0.5,
            humanize: 0.0,
            seed: 1,
            enabled: true,
        });
        assert!(a.validate().is_err());
    }

    #[test]
    fn scene_transitions_interpolate_without_jumps_and_hold_outside() {
        let mut a = rack();
        let mut quiet = a.clone();
        quiet.layers[0].gain_db = -40.0;
        quiet.layers[1].mute = true;
        quiet.macros.space = 0.1;
        let mut loud = a.clone();
        loud.layers[0].gain_db = -10.0;
        loud.macros.space = 0.7;
        if let LayerSource::Wind { gust, .. } = &mut loud.layers[0].source {
            *gust = 1.0;
        }
        a.program.scenes = vec![
            Scene {
                id: "s1".into(),
                name: "intro".into(),
                start_s: 0.0,
                duration_s: 60.0,
                transition_s: 0.0,
                snapshot: Snapshot::of(&quiet),
            },
            Scene {
                id: "s2".into(),
                name: "swell".into(),
                start_s: 60.0,
                duration_s: 60.0,
                transition_s: 20.0,
                snapshot: Snapshot::of(&loud),
            },
        ];
        a.validate().unwrap();
        assert_eq!(a.program.end(), 120.0);
        let g = |t: f64| effective(&a, Some(t), 0.0).0[0].gain_db;
        assert_eq!(g(30.0), -40.0);
        assert!((g(70.0) - -25.0).abs() < 1e-9, "midway: {}", g(70.0));
        assert_eq!(g(80.0), -10.0);
        assert_eq!(g(119.0), -10.0);
        // Outside every scene the rack's own state applies.
        assert_eq!(g(130.0), -18.0);
        // No jump bigger than the slope across the transition.
        let mut prev = g(60.0);
        for k in 1..=200 {
            let t = 60.0 + k as f64 * 0.1;
            let v = g(t);
            assert!((v - prev).abs() < 0.2, "jump at {t}: {prev} → {v}");
            prev = v;
        }
        let (ls, m) = effective(&a, Some(65.0), 0.0);
        if let LayerSource::Wind { gust, .. } = ls[0].source {
            assert!((gust - 0.7).abs() < 1e-9, "{gust}");
        }
        assert!((m.space - 0.25).abs() < 1e-9);
        assert!(ls[1].mute, "mute switches at the midpoint");
        a.program.loop_region = Some((10.0, 50.0));
        assert_eq!(a.program.wrap(55.0), 15.0);
        assert_eq!(a.program.wrap(5.0), 5.0);
        assert!(is_animated(&a));
        assert!(!is_animated(&rack()));
    }
}
