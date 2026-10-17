//! Factory presets ("Sound DNA", F1A.3): a handful of racks that sound musical out
//! of the box, built from the synthesized sources only. Names are shown through the
//! UI's message catalogue by `id`; `name` is the English fallback. Seeds are fixed so
//! a preset sounds the same every time it is loaded.

use serde::{Deserialize, Serialize};

use crate::ambient::{Ambient, AmbientLayer, LayerSource, Macros};

/// A saved rack with a name and tags. Factory ones live in code, user ones as files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub tags: Vec<String>,
    pub factory: bool,
    pub rack: Ambient,
}

/// Tag vocabulary (ids; the UI translates them).
pub const TAGS: [&str; 11] = [
    "forest",
    "river",
    "night",
    "fog",
    "mountain",
    "ethno",
    "meditation",
    "dark",
    "warm",
    "liquid",
    "no_key",
];

struct L {
    name: &'static str,
    source: LayerSource,
    gain_db: f64,
    pan: f64,
    lowpass_hz: f64,
    highpass_hz: f64,
    seed: u32,
}

fn layer(id_prefix: &str, i: usize, l: L) -> AmbientLayer {
    AmbientLayer {
        id: format!("{id_prefix}-{i}"),
        name: l.name.to_string(),
        source: l.source,
        gain_db: l.gain_db,
        pan: l.pan,
        lowpass_hz: l.lowpass_hz,
        highpass_hz: l.highpass_hz,
        mute: false,
        solo: false,
        seed: l.seed,
        license_ref: None,
        locked: false,
    }
}

fn rack(id: &str, name: &str, tags: &[&str], macros: Macros, layers: Vec<L>) -> Preset {
    Preset {
        id: id.to_string(),
        name: name.to_string(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        factory: true,
        rack: Ambient {
            schema_version: crate::ambient::AMBIENT_SCHEMA_VERSION,
            layers: layers
                .into_iter()
                .enumerate()
                .map(|(i, l)| layer(id, i, l))
                .collect(),
            macros,
            preset: Some(id.to_string()),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            automation: Vec::new(),
            program: crate::program::Program::default(),
        },
    }
}

fn m(space: f64, size: f64, brightness: f64, motion: f64) -> Macros {
    Macros {
        intensity: 1.0,
        brightness,
        motion,
        space,
        space_size: size,
        tonal_gravity: 0.5,
        width: 1.0,
        shimmer: 0.0,
        shimmer_st: 12.0,
    }
}

const LP_OFF: f64 = 20_000.0;
const HP_OFF: f64 = 10.0;

fn wind(gust: f64, speed: f64, center_hz: f64, resonance: f64) -> LayerSource {
    LayerSource::Wind {
        gust,
        speed,
        center_hz,
        resonance,
    }
}
fn noise(color: f64) -> LayerSource {
    LayerSource::Noise { color }
}
fn texture(
    brightness: f64,
    motion: f64,
    resonance: f64,
    tonal: bool,
    lo: u8,
    hi: u8,
) -> LayerSource {
    LayerSource::Texture {
        brightness,
        motion,
        resonance,
        tonal,
        register: (lo, hi),
    }
}
fn bells(density: f64, decay_s: f64, brightness: f64, tonal: bool, lo: u8, hi: u8) -> LayerSource {
    LayerSource::Bells {
        density,
        decay_s,
        brightness,
        velocity_spread: 0.6,
        tonal,
        register: (lo, hi),
    }
}
fn pluck(density: f64, damping: f64, brightness: f64, tonal: bool, lo: u8, hi: u8) -> LayerSource {
    LayerSource::Pluck {
        density,
        damping,
        brightness,
        velocity_spread: 0.5,
        tonal,
        register: (lo, hi),
    }
}

/// The shipped presets. Quality over quantity: each is a listening scene, not a demo
/// of every knob. Tonal layers follow the project's mode when it has one.
pub fn factory_presets() -> Vec<Preset> {
    vec![
        rack(
            "mountain_mist",
            "Mountain mist",
            &["mountain", "fog", "ethno"],
            m(0.35, 0.7, 0.5, 0.5),
            vec![
                L {
                    name: "Wind",
                    source: wind(0.5, 0.2, 400.0, 0.4),
                    gain_db: -20.0,
                    pan: -0.3,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 11,
                },
                L {
                    name: "Mist",
                    source: noise(0.75),
                    gain_db: -28.0,
                    pan: 0.2,
                    lowpass_hz: 2000.0,
                    highpass_hz: HP_OFF,
                    seed: 12,
                },
                L {
                    name: "Texture",
                    source: texture(0.3, 0.6, 0.6, true, 48, 72),
                    gain_db: -18.0,
                    pan: 0.25,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 13,
                },
                L {
                    name: "Bells",
                    source: bells(6.0, 8.0, 0.35, true, 64, 88),
                    gain_db: -16.0,
                    pan: 0.4,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 14,
                },
                L {
                    name: "Strings",
                    source: pluck(8.0, 0.25, 0.45, true, 48, 72),
                    gain_db: -18.0,
                    pan: -0.35,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 15,
                },
            ],
        ),
        rack(
            "forest_morning",
            "Forest morning",
            &["forest", "warm"],
            m(0.3, 0.55, 0.55, 0.6),
            vec![
                L {
                    name: "Leaves",
                    source: noise(0.45),
                    gain_db: -30.0,
                    pan: -0.2,
                    lowpass_hz: 6000.0,
                    highpass_hz: 300.0,
                    seed: 21,
                },
                L {
                    name: "Breeze",
                    source: wind(0.4, 0.4, 900.0, 0.35),
                    gain_db: -24.0,
                    pan: 0.35,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 22,
                },
                L {
                    name: "Strings",
                    source: pluck(14.0, 0.3, 0.6, true, 55, 79),
                    gain_db: -17.0,
                    pan: -0.4,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 23,
                },
                L {
                    name: "Bells",
                    source: bells(9.0, 4.0, 0.5, true, 72, 96),
                    gain_db: -20.0,
                    pan: 0.5,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 24,
                },
                L {
                    name: "Warmth",
                    source: texture(0.2, 0.4, 0.5, true, 43, 60),
                    gain_db: -20.0,
                    pan: 0.0,
                    lowpass_hz: 3000.0,
                    highpass_hz: HP_OFF,
                    seed: 25,
                },
            ],
        ),
        rack(
            "river",
            "River",
            &["river", "no_key"],
            m(0.3, 0.6, 0.5, 0.7),
            vec![
                L {
                    name: "Water",
                    source: noise(0.55),
                    gain_db: -20.0,
                    pan: 0.0,
                    lowpass_hz: 5000.0,
                    highpass_hz: 120.0,
                    seed: 31,
                },
                L {
                    name: "Current",
                    source: wind(0.8, 0.6, 1400.0, 0.3),
                    gain_db: -22.0,
                    pan: -0.25,
                    lowpass_hz: LP_OFF,
                    highpass_hz: 200.0,
                    seed: 32,
                },
                L {
                    name: "Eddies",
                    source: wind(0.9, 0.8, 2600.0, 0.5),
                    gain_db: -26.0,
                    pan: 0.4,
                    lowpass_hz: LP_OFF,
                    highpass_hz: 400.0,
                    seed: 33,
                },
                L {
                    name: "Stones",
                    source: texture(0.4, 0.8, 0.5, false, 52, 76),
                    gain_db: -24.0,
                    pan: 0.15,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 34,
                },
            ],
        ),
        rack(
            "night",
            "Night",
            &["night", "dark"],
            m(0.45, 0.85, 0.4, 0.5),
            vec![
                L {
                    name: "Deep",
                    source: noise(1.0),
                    gain_db: -30.0,
                    pan: 0.0,
                    lowpass_hz: 600.0,
                    highpass_hz: HP_OFF,
                    seed: 41,
                },
                L {
                    name: "Wind",
                    source: wind(0.3, 0.1, 250.0, 0.7),
                    gain_db: -22.0,
                    pan: -0.5,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 42,
                },
                L {
                    name: "Texture",
                    source: texture(0.1, 0.7, 0.8, true, 40, 64),
                    gain_db: -22.0,
                    pan: 0.1,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 43,
                },
                L {
                    name: "Strings",
                    source: pluck(5.0, 0.2, 0.35, true, 52, 76),
                    gain_db: -20.0,
                    pan: 0.45,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 44,
                },
                L {
                    name: "Bells",
                    source: bells(3.0, 12.0, 0.2, true, 72, 96),
                    gain_db: -22.0,
                    pan: -0.4,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 45,
                },
            ],
        ),
        rack(
            "fog_no_key",
            "Fog, no key",
            &["fog", "no_key", "meditation"],
            m(0.4, 0.8, 0.45, 0.5),
            vec![
                L {
                    name: "Fog",
                    source: noise(0.9),
                    gain_db: -24.0,
                    pan: 0.0,
                    lowpass_hz: 900.0,
                    highpass_hz: HP_OFF,
                    seed: 51,
                },
                L {
                    name: "Wind",
                    source: wind(0.7, 0.35, 700.0, 0.55),
                    gain_db: -18.0,
                    pan: 0.3,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 52,
                },
                L {
                    name: "Texture",
                    source: texture(0.2, 0.7, 0.7, false, 40, 64),
                    gain_db: -18.0,
                    pan: -0.3,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 53,
                },
                L {
                    name: "Bells",
                    source: bells(4.0, 10.0, 0.5, false, 60, 84),
                    gain_db: -20.0,
                    pan: 0.5,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 54,
                },
            ],
        ),
        rack(
            "meditation",
            "Meditation",
            &["meditation", "ethno", "warm"],
            m(0.5, 0.9, 0.45, 0.3),
            vec![
                L {
                    name: "Ground",
                    source: texture(0.15, 0.3, 0.85, true, 36, 55),
                    gain_db: -16.0,
                    pan: 0.0,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 61,
                },
                L {
                    name: "Air",
                    source: noise(0.7),
                    gain_db: -32.0,
                    pan: 0.0,
                    lowpass_hz: 1500.0,
                    highpass_hz: HP_OFF,
                    seed: 62,
                },
                L {
                    name: "Bells",
                    source: bells(2.5, 15.0, 0.3, true, 67, 91),
                    gain_db: -18.0,
                    pan: 0.3,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 63,
                },
                L {
                    name: "Strings",
                    source: pluck(3.0, 0.1, 0.3, true, 55, 72),
                    gain_db: -22.0,
                    pan: -0.3,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 64,
                },
            ],
        ),
        rack(
            "warm_hearth",
            "Warm hearth",
            &["warm", "night"],
            m(0.25, 0.5, 0.45, 0.4),
            vec![
                L {
                    name: "Embers",
                    source: noise(0.85),
                    gain_db: -26.0,
                    pan: 0.0,
                    lowpass_hz: 1200.0,
                    highpass_hz: 60.0,
                    seed: 71,
                },
                L {
                    name: "Crackle",
                    source: bells(20.0, 0.4, 0.9, false, 72, 96),
                    gain_db: -30.0,
                    pan: -0.2,
                    lowpass_hz: LP_OFF,
                    highpass_hz: 1000.0,
                    seed: 72,
                },
                L {
                    name: "Strings",
                    source: pluck(10.0, 0.35, 0.4, true, 48, 67),
                    gain_db: -16.0,
                    pan: 0.3,
                    lowpass_hz: 6000.0,
                    highpass_hz: HP_OFF,
                    seed: 73,
                },
                L {
                    name: "Texture",
                    source: texture(0.2, 0.4, 0.6, true, 43, 62),
                    gain_db: -20.0,
                    pan: -0.3,
                    lowpass_hz: 4000.0,
                    highpass_hz: HP_OFF,
                    seed: 74,
                },
            ],
        ),
        rack(
            "liquid_glass",
            "Liquid glass",
            &["liquid", "river"],
            m(0.45, 0.75, 0.65, 0.7),
            vec![
                L {
                    name: "Glass",
                    source: bells(14.0, 5.0, 0.7, true, 72, 96),
                    gain_db: -18.0,
                    pan: 0.35,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 81,
                },
                L {
                    name: "Drops",
                    source: pluck(24.0, 0.45, 0.8, true, 67, 91),
                    gain_db: -20.0,
                    pan: -0.4,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 82,
                },
                L {
                    name: "Shimmer",
                    source: texture(0.8, 0.8, 0.7, true, 60, 84),
                    gain_db: -22.0,
                    pan: 0.1,
                    lowpass_hz: LP_OFF,
                    highpass_hz: 200.0,
                    seed: 83,
                },
                L {
                    name: "Flow",
                    source: wind(0.6, 0.5, 1800.0, 0.4),
                    gain_db: -28.0,
                    pan: -0.1,
                    lowpass_hz: LP_OFF,
                    highpass_hz: 300.0,
                    seed: 84,
                },
                L {
                    name: "Floor",
                    source: texture(0.1, 0.3, 0.8, true, 36, 52),
                    gain_db: -20.0,
                    pan: 0.0,
                    lowpass_hz: 2000.0,
                    highpass_hz: HP_OFF,
                    seed: 85,
                },
            ],
        ),
        rack(
            "dark_cave",
            "Dark cave",
            &["dark", "no_key", "fog"],
            m(0.6, 1.0, 0.3, 0.4),
            vec![
                L {
                    name: "Depth",
                    source: noise(1.0),
                    gain_db: -22.0,
                    pan: 0.0,
                    lowpass_hz: 300.0,
                    highpass_hz: HP_OFF,
                    seed: 91,
                },
                L {
                    name: "Draught",
                    source: wind(0.5, 0.15, 160.0, 0.8),
                    gain_db: -20.0,
                    pan: -0.4,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 92,
                },
                L {
                    name: "Drips",
                    source: bells(5.0, 3.0, 0.6, false, 72, 96),
                    gain_db: -24.0,
                    pan: 0.5,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 93,
                },
                L {
                    name: "Stone",
                    source: texture(0.1, 0.6, 0.9, false, 36, 55),
                    gain_db: -20.0,
                    pan: 0.2,
                    lowpass_hz: LP_OFF,
                    highpass_hz: HP_OFF,
                    seed: 94,
                },
            ],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_presets_are_valid_unique_tagged_and_enough() {
        let all = factory_presets();
        assert!(all.len() >= 8, "{} presets", all.len());
        let mut ids = std::collections::HashSet::new();
        for p in &all {
            assert!(ids.insert(p.id.clone()), "duplicate id {}", p.id);
            p.rack
                .validate()
                .unwrap_or_else(|e| panic!("{}: {e}", p.id));
            assert!(!p.rack.layers.is_empty() && p.rack.layers.len() <= 6);
            assert!(!p.tags.is_empty());
            for t in &p.tags {
                assert!(TAGS.contains(&t.as_str()), "{}: unknown tag {t}", p.id);
            }
            assert_eq!(p.rack.preset.as_deref(), Some(p.id.as_str()));
            // Levels leave headroom: no layer hotter than −14 dB in a shipped scene.
            assert!(p.rack.layers.iter().all(|l| l.gain_db <= -14.0), "{}", p.id);
        }
        // Every tag is used by at least one preset, so the browser has no empty tag.
        for tag in TAGS {
            assert!(
                all.iter().any(|p| p.tags.iter().any(|t| t == tag)),
                "tag {tag} unused"
            );
        }
        // A preset round-trips through JSON (save → reopen identical).
        let p = &all[0];
        let text = serde_json::to_string_pretty(&p.rack).unwrap();
        let back: Ambient = serde_json::from_str(&text).unwrap();
        assert_eq!(&back, &p.rack);
    }
}
