//! Ambient Playground rack (F1A.1–F1A.3): up to eight layers of synthesized or
//! looped sources with global macros, saved as `ambient.json` next to
//! `project.json`. A preset ("Sound DNA", F1A.3) is the same structure saved on its
//! own; the seed makes a rack reproduce the same sound after reopening.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{ProjectError, Result};

pub const AMBIENT_SCHEMA_VERSION: u32 = 1;
pub const AMBIENT_FILE: &str = "ambient.json";
pub const MAX_AMBIENT_LAYERS: usize = 8;

/// What a layer plays. Controls are per source type, not universal knobs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayerSource {
    /// Noise colour: 0 white … 0.5 pink … 1 brown.
    Noise { color: f64 },
    Wind {
        gust: f64,
        speed: f64,
        center_hz: f64,
        resonance: f64,
    },
    /// Resonators on the pitch set (scale degrees in `register`, or a "no key" set).
    Texture {
        brightness: f64,
        motion: f64,
        resonance: f64,
        tonal: bool,
        register: (u8, u8),
    },
    Bells {
        /// Strikes per minute.
        density: f64,
        decay_s: f64,
        brightness: f64,
        velocity_spread: f64,
        tonal: bool,
        register: (u8, u8),
    },
    Pluck {
        density: f64,
        damping: f64,
        brightness: f64,
        velocity_spread: f64,
        tonal: bool,
        register: (u8, u8),
    },
    /// A region of a take, looped with a crossfade (the take stays immutable).
    Loop {
        take_id: String,
        /// Region in seconds of the take's playable audio; `end` 0 = to the end.
        start: f64,
        end: f64,
        crossfade: f64,
        rate: f64,
        reverse: bool,
    },
    /// Modal resonator (F1B.4): struck bodies tuned to the pitch set, harmonic ↔ bell.
    Resonator {
        /// Strikes per minute.
        density: f64,
        decay_s: f64,
        /// 0 harmonic (string / tube) … 1 inharmonic (bar / bowl / bell).
        structure: f64,
        brightness: f64,
        /// 0 soft mallet … 1 hard stick.
        strike: f64,
        tonal: bool,
        register: (u8, u8),
    },
    /// Choir drone (F1B.5): vowel formants on the pitch set — a vocal bed.
    Choir {
        /// 0..1 along a–e–i–o–u.
        vowel: f64,
        /// 0..1 breath swell depth.
        breath: f64,
        /// Per-voice detune, cents (0..30).
        spread: f64,
        brightness: f64,
        /// 0..1 how far and fast the vowel drifts by itself.
        morph: f64,
        /// Voices used (1..6), low to high on the pitch set.
        voices: u8,
        tonal: bool,
        register: (u8, u8),
    },
    /// Bloom (F1B.6): a generative melody on the pitch set — phrases with breaths.
    Bloom {
        /// Notes per minute.
        density: f64,
        /// Held length of a note, seconds.
        note_s: f64,
        brightness: f64,
        attack_s: f64,
        /// 0 random picks … 1 stepwise phrases with breaths.
        phrase: f64,
        tonal: bool,
        register: (u8, u8),
    },
    /// Granular cloud of a take (F1B.2): grains around a read position, transposed.
    Cloud {
        take_id: String,
        /// Grain length, ms (10..500).
        grain_ms: f64,
        /// Grains per second (0.5..80).
        density: f64,
        /// Read position 0..1 of the take; `spray` spreads the grain starts around it.
        position: f64,
        spray: f64,
        /// Drift of the position through the take, × real time (0 = frozen).
        scan: f64,
        /// Transposition in semitones (−24..24) and random detune per grain in cents.
        pitch: f64,
        jitter: f64,
        /// Probabilities 0..1: a grain plays backwards / an octave up.
        reverse_p: f64,
        octave_p: f64,
    },
}

impl LayerSource {
    /// The take a layer reads, if it reads one (loop, cloud).
    pub fn take_id(&self) -> Option<&str> {
        match self {
            LayerSource::Loop { take_id, .. } | LayerSource::Cloud { take_id, .. } => {
                Some(take_id.as_str())
            }
            _ => None,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            LayerSource::Noise { .. } => "noise",
            LayerSource::Wind { .. } => "wind",
            LayerSource::Texture { .. } => "texture",
            LayerSource::Bells { .. } => "bells",
            LayerSource::Pluck { .. } => "pluck",
            LayerSource::Loop { .. } => "loop",
            LayerSource::Cloud { .. } => "cloud",
            LayerSource::Resonator { .. } => "resonator",
            LayerSource::Choir { .. } => "choir",
            LayerSource::Bloom { .. } => "bloom",
        }
    }

    /// Sources that follow the project's scale (tonal gravity applies to them).
    pub fn is_tonal(&self) -> bool {
        matches!(
            self,
            LayerSource::Texture { tonal: true, .. }
                | LayerSource::Bells { tonal: true, .. }
                | LayerSource::Pluck { tonal: true, .. }
                | LayerSource::Resonator { tonal: true, .. }
                | LayerSource::Choir { tonal: true, .. }
                | LayerSource::Bloom { tonal: true, .. }
        )
    }

    /// Sensible defaults per kind (what "add layer" creates).
    pub fn default_for(kind: &str) -> Option<LayerSource> {
        Some(match kind {
            "noise" => LayerSource::Noise { color: 0.6 },
            "wind" => LayerSource::Wind {
                gust: 0.6,
                speed: 0.3,
                center_hz: 600.0,
                resonance: 0.5,
            },
            "texture" => LayerSource::Texture {
                brightness: 0.3,
                motion: 0.5,
                resonance: 0.6,
                tonal: true,
                register: (48, 72),
            },
            "bells" => LayerSource::Bells {
                density: 10.0,
                decay_s: 5.0,
                brightness: 0.4,
                velocity_spread: 0.5,
                tonal: true,
                register: (60, 84),
            },
            "pluck" => LayerSource::Pluck {
                density: 15.0,
                damping: 0.3,
                brightness: 0.5,
                velocity_spread: 0.5,
                tonal: true,
                register: (48, 72),
            },
            _ => return None,
        })
    }

    fn validate(&self) -> Result<()> {
        let unit = |v: f64, what: &str| -> Result<()> {
            if !(0.0..=1.0).contains(&v) {
                return Err(ProjectError::InvalidState(format!("{what} must be 0..1")));
            }
            Ok(())
        };
        let register = |r: (u8, u8)| -> Result<()> {
            if r.0 > 127 || r.1 > 127 || r.1 < r.0 {
                return Err(ProjectError::InvalidState("register".into()));
            }
            Ok(())
        };
        match self {
            LayerSource::Noise { color } => unit(*color, "color"),
            LayerSource::Wind {
                gust,
                speed,
                center_hz,
                resonance,
            } => {
                unit(*gust, "gust")?;
                unit(*speed, "speed")?;
                unit(*resonance, "resonance")?;
                if !(80.0..=6000.0).contains(center_hz) {
                    return Err(ProjectError::InvalidState("wind centre".into()));
                }
                Ok(())
            }
            LayerSource::Texture {
                brightness,
                motion,
                resonance,
                register: r,
                ..
            } => {
                unit(*brightness, "brightness")?;
                unit(*motion, "motion")?;
                unit(*resonance, "resonance")?;
                register(*r)
            }
            LayerSource::Bells {
                density,
                decay_s,
                brightness,
                velocity_spread,
                register: r,
                ..
            } => {
                if !(0.0..=120.0).contains(density) || !(0.2..=30.0).contains(decay_s) {
                    return Err(ProjectError::InvalidState("bells density/decay".into()));
                }
                unit(*brightness, "brightness")?;
                unit(*velocity_spread, "velocity_spread")?;
                register(*r)
            }
            LayerSource::Pluck {
                density,
                damping,
                brightness,
                velocity_spread,
                register: r,
                ..
            } => {
                if !(0.0..=240.0).contains(density) {
                    return Err(ProjectError::InvalidState("pluck density".into()));
                }
                unit(*damping, "damping")?;
                unit(*brightness, "brightness")?;
                unit(*velocity_spread, "velocity_spread")?;
                register(*r)
            }
            LayerSource::Resonator {
                density,
                decay_s,
                structure,
                brightness,
                strike,
                register: r,
                ..
            } => {
                if !(0.0..=120.0).contains(density) || !(0.2..=30.0).contains(decay_s) {
                    return Err(ProjectError::InvalidState("resonator density/decay".into()));
                }
                unit(*structure, "structure")?;
                unit(*brightness, "brightness")?;
                unit(*strike, "strike")?;
                register(*r)
            }
            LayerSource::Choir {
                vowel,
                breath,
                spread,
                brightness,
                morph,
                voices,
                register: r,
                ..
            } => {
                if !(0.0..=30.0).contains(spread) || !(1..=6).contains(voices) {
                    return Err(ProjectError::InvalidState("choir spread/voices".into()));
                }
                unit(*vowel, "vowel")?;
                unit(*breath, "breath")?;
                unit(*brightness, "brightness")?;
                unit(*morph, "morph")?;
                register(*r)
            }
            LayerSource::Bloom {
                density,
                note_s,
                brightness,
                attack_s,
                phrase,
                register: r,
                ..
            } => {
                if !(0.0..=120.0).contains(density)
                    || !(0.2..=8.0).contains(note_s)
                    || !(0.005..=2.0).contains(attack_s)
                {
                    return Err(ProjectError::InvalidState(
                        "bloom density/note/attack".into(),
                    ));
                }
                unit(*brightness, "brightness")?;
                unit(*phrase, "phrase")?;
                register(*r)
            }
            LayerSource::Cloud {
                take_id,
                grain_ms,
                density,
                position,
                spray,
                scan,
                pitch,
                jitter,
                reverse_p,
                octave_p,
            } => {
                if take_id.is_empty()
                    || !(10.0..=500.0).contains(grain_ms)
                    || !(0.5..=80.0).contains(density)
                    || !(0.0..=2.0).contains(scan)
                    || !(-24.0..=24.0).contains(pitch)
                    || !(0.0..=100.0).contains(jitter)
                {
                    return Err(ProjectError::InvalidState("cloud parameters".into()));
                }
                unit(*position, "position")?;
                unit(*spray, "spray")?;
                unit(*reverse_p, "reverse_p")?;
                unit(*octave_p, "octave_p")
            }
            LayerSource::Loop {
                take_id,
                start,
                end,
                crossfade,
                rate,
                ..
            } => {
                if take_id.is_empty()
                    || *start < 0.0
                    || *end < 0.0
                    || (*end != 0.0 && *end <= *start)
                    || *crossfade < 0.0
                    || !(0.25..=4.0).contains(rate)
                {
                    return Err(ProjectError::InvalidState("loop region/rate".into()));
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AmbientLayer {
    pub id: String,
    pub name: String,
    pub source: LayerSource,
    #[serde(default)]
    pub gain_db: f64,
    /// −1 … +1.
    #[serde(default)]
    pub pan: f64,
    #[serde(default = "default_lowpass")]
    pub lowpass_hz: f64,
    #[serde(default = "default_highpass")]
    pub highpass_hz: f64,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub solo: bool,
    #[serde(default = "default_seed")]
    pub seed: u32,
    /// Licence record of the audio an audio loop uses (F1A.8), if any.
    #[serde(default)]
    pub license_ref: Option<String>,
    /// "Keep what I like": randomize never changes a locked layer (F1A.3).
    #[serde(default)]
    pub locked: bool,
}

fn default_lowpass() -> f64 {
    20_000.0
}
fn default_highpass() -> f64 {
    10.0
}
fn default_seed() -> u32 {
    1
}

/// Global macros (F1A.2). 0.5 is neutral for the bipolar ones.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Macros {
    /// 0..1 overall intensity (scales every layer's level).
    pub intensity: f64,
    /// 0..1 dark … bright (shifts every layer's low-pass).
    pub brightness: f64,
    /// 0..1 still … lively (scales the slow movement inside the sources).
    pub motion: f64,
    /// 0..1 reverb mix on the master.
    pub space: f64,
    /// 0..1 reverb size.
    pub space_size: f64,
    /// 0..1 atonal … tonal balance between the tonal and the textural layers.
    pub tonal_gravity: f64,
    /// 0..1 stereo width (scales the pans).
    pub width: f64,
    /// 0..1 shimmer: transposed feedback in the master space (F1B.3).
    #[serde(default)]
    pub shimmer: f64,
    /// Shimmer transposition in semitones (0..24; 12 octave, 7 fifth, 19 octave + fifth).
    #[serde(default = "default_shimmer_st")]
    pub shimmer_st: f64,
}

fn default_shimmer_st() -> f64 {
    12.0
}

impl Default for Macros {
    fn default() -> Self {
        Self {
            intensity: 1.0,
            brightness: 0.5,
            motion: 0.5,
            space: 0.25,
            space_size: 0.6,
            tonal_gravity: 0.5,
            width: 1.0,
            shimmer: 0.0,
            shimmer_st: 12.0,
        }
    }
}

impl Macros {
    pub fn validate(&self) -> Result<()> {
        for v in [
            self.intensity,
            self.brightness,
            self.motion,
            self.space,
            self.space_size,
            self.tonal_gravity,
            self.width,
            self.shimmer,
        ] {
            if !(0.0..=1.0).contains(&v) {
                return Err(ProjectError::InvalidState("macro must be 0..1".into()));
            }
        }
        if !(0.0..=24.0).contains(&self.shimmer_st) {
            return Err(ProjectError::InvalidState(
                "shimmer_st must be 0..24".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ambient {
    pub schema_version: u32,
    #[serde(default)]
    pub layers: Vec<AmbientLayer>,
    #[serde(default)]
    pub macros: Macros,
    /// Name of the preset this rack came from, if any (informational).
    #[serde(default)]
    pub preset: Option<String>,
    /// Tags when saved as a preset (ids from `ambient_presets::TAGS`).
    #[serde(default)]
    pub tags: Vec<String>,
    /// Automation lanes (F1A.4): functions of time on layer or macro parameters.
    #[serde(default)]
    pub automation: Vec<crate::program::AutomationLane>,
    /// Scenes on a timeline (F1A.5).
    #[serde(default)]
    pub program: crate::program::Program,
}

impl Default for Ambient {
    fn default() -> Self {
        Self {
            schema_version: AMBIENT_SCHEMA_VERSION,
            layers: Vec::new(),
            macros: Macros::default(),
            preset: None,
            tags: Vec::new(),
            automation: Vec::new(),
            program: crate::program::Program::default(),
        }
    }
}

impl Ambient {
    pub fn load(root: &Path) -> Result<Self> {
        Self::load_file(&root.join(AMBIENT_FILE))
    }

    /// Load a rack or a preset from any path (missing file = empty rack).
    pub fn load_file(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path)?;
        let a: Ambient = serde_json::from_str(&text)?;
        if a.schema_version > AMBIENT_SCHEMA_VERSION {
            return Err(ProjectError::NewerSchema {
                what: "ambient",
                found: a.schema_version,
                supported: AMBIENT_SCHEMA_VERSION,
            });
        }
        Ok(a)
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        self.save_file(&root.join(AMBIENT_FILE))
    }

    /// Atomic save: temp file, then rename.
    pub fn save_file(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    pub fn validate(&self) -> Result<()> {
        if self.layers.len() > MAX_AMBIENT_LAYERS {
            return Err(ProjectError::InvalidState(format!(
                "at most {MAX_AMBIENT_LAYERS} layers"
            )));
        }
        let mut ids = std::collections::HashSet::new();
        for l in &self.layers {
            if l.id.is_empty() || !ids.insert(l.id.as_str()) {
                return Err(ProjectError::InvalidState(
                    "layer ids must be unique".into(),
                ));
            }
            if !(-1.0..=1.0).contains(&l.pan)
                || !(-60.0..=12.0).contains(&l.gain_db)
                || !(40.0..=20_000.0).contains(&l.lowpass_hz)
                || !(10.0..=8_000.0).contains(&l.highpass_hz)
            {
                return Err(ProjectError::InvalidState(format!(
                    "layer {} level/pan/filter out of range",
                    l.id
                )));
            }
            l.source.validate()?;
        }
        let mut lane_ids = std::collections::HashSet::new();
        for lane in &self.automation {
            if lane.id.is_empty() || !lane_ids.insert(lane.id.as_str()) {
                return Err(ProjectError::InvalidState("lane ids must be unique".into()));
            }
            lane.validate(&self.layers)?;
        }
        self.program.validate()?;
        self.macros.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rack() -> Ambient {
        let mut a = Ambient::default();
        for (i, kind) in ["noise", "wind", "texture", "bells", "pluck"]
            .iter()
            .enumerate()
        {
            a.layers.push(AmbientLayer {
                id: format!("l{i}"),
                name: kind.to_string(),
                source: LayerSource::default_for(kind).unwrap(),
                gain_db: -12.0,
                pan: (i as f64 - 2.0) * 0.3,
                lowpass_hz: 20_000.0,
                highpass_hz: 10.0,
                mute: false,
                solo: false,
                seed: 7 + i as u32,
                license_ref: None,
                locked: false,
            });
        }
        a.layers.push(AmbientLayer {
            id: "loop".into(),
            name: "river".into(),
            source: LayerSource::Loop {
                take_id: "take-1".into(),
                start: 1.0,
                end: 0.0,
                crossfade: 0.1,
                rate: 1.0,
                reverse: false,
            },
            gain_db: -18.0,
            pan: 0.0,
            lowpass_hz: 20_000.0,
            highpass_hz: 10.0,
            mute: false,
            solo: false,
            seed: 1,
            license_ref: Some("own-recording".into()),
            locked: false,
        });
        a.macros.space = 0.4;
        a
    }

    #[test]
    fn save_and_reopen_is_identical_and_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Ambient::load(dir.path()).unwrap(), Ambient::default());
        let a = rack();
        a.validate().unwrap();
        a.save(dir.path()).unwrap();
        let b = Ambient::load(dir.path()).unwrap();
        assert_eq!(a, b, "snapshot round-trip");
        assert!(!dir.path().join("ambient.json.tmp").exists());
        // Tagged kinds are readable JSON for presets.
        let text = std::fs::read_to_string(dir.path().join(AMBIENT_FILE)).unwrap();
        assert!(text.contains("\"kind\": \"bells\""));
    }

    #[test]
    fn validation_rejects_bad_values_and_newer_schema() {
        let mut a = rack();
        a.layers[0].pan = 1.5;
        assert!(a.validate().is_err());
        let mut a = rack();
        a.layers[1].id = "l0".into();
        assert!(a.validate().is_err());
        let mut a = rack();
        a.layers[3].source = LayerSource::Bells {
            density: 500.0,
            decay_s: 1.0,
            brightness: 0.5,
            velocity_spread: 0.5,
            tonal: true,
            register: (60, 72),
        };
        assert!(a.validate().is_err());
        let mut a = rack();
        a.macros.width = 2.0;
        assert!(a.validate().is_err());
        for _ in 0..3 {
            a.layers.push(a.layers[0].clone());
        }
        assert!(a.validate().is_err(), "too many layers");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(AMBIENT_FILE),
            r#"{"schema_version": 99, "layers": []}"#,
        )
        .unwrap();
        assert!(matches!(
            Ambient::load(dir.path()),
            Err(ProjectError::NewerSchema { .. })
        ));
    }

    #[test]
    fn old_files_without_new_fields_still_load() {
        let text = r#"{"schema_version":1,"layers":[{"id":"a","name":"n","source":{"kind":"noise","color":0.2}}]}"#;
        let a: Ambient = serde_json::from_str(text).unwrap();
        assert_eq!(a.layers[0].lowpass_hz, 20_000.0);
        assert_eq!(a.layers[0].seed, 1);
        assert_eq!(a.macros, Macros::default());
        assert!(a.validate().is_ok());
        assert!(LayerSource::default_for("piano").is_none());
        assert!(a.layers[0].source.kind() == "noise" && !a.layers[0].source.is_tonal());
    }
}
