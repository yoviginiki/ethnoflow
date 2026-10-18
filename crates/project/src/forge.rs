//! Instrument Forge presets (Program P1, R1): an instrument is a document with an
//! identity (id, name, category, tags, author, parent) and a *voice* — the sound
//! parameters of one of the engines. The pattern a part plays is not part of the
//! instrument; the rhythm settings reference an instrument by id per part. Factory
//! presets are read-only and live in code; user presets are library entries.

use serde::{Deserialize, Serialize};

use crate::error::{ProjectError, Result};

pub const INSTRUMENT_SCHEMA_VERSION: u32 = 1;
pub const CATEGORIES: [&str; 6] = ["drums", "bass", "synth", "tapan", "chordpad", "lead"];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case")]
pub enum InstrumentVoice {
    /// The synthesized kit: kick fundamental, overall decay, hat/snare brightness and a
    /// level per drum (kick, snare, ghost, closed hat, open hat, shaker), dB.
    Drums {
        kick_hz: f64,
        decay: f64,
        brightness: f64,
        #[serde(default = "default_levels")]
        levels_db: [f64; 6],
    },
    /// The mono bass: voice 0 sub / 1 pluck / 2 Reese, filter, drive, glide, sidechain,
    /// tempo-synced wobble, warmth, and the designer's envelopes.
    Bass {
        voice: u8,
        cutoff_hz: f64,
        resonance: f64,
        drive: f64,
        glide_s: f64,
        sidechain: f64,
        wobble_div: u32,
        wobble_depth: f64,
        warmth: f64,
        attack_s: f64,
        decay_s: f64,
        sustain: f64,
        release_s: f64,
        filter_env: f64,
        pitch_env_st: f64,
        pitch_env_s: f64,
        microvar: f64,
    },
    /// The polyphonic pattern synth (ostinato / arpeggio): timbre 0 sine, 1 triangle,
    /// 2 flute; attack, release, tone (low-pass), dynamics (velocity → loudness).
    Synth {
        timbre: u8,
        attack_s: f64,
        release_s: f64,
        tone_hz: f64,
        dynamics: f64,
    },
    /// The tapan (P1 R3): boom tuning and decay, skin noise in the boom, rim brightness,
    /// a level per stroke (boom, rim, ghost), dB.
    Tapan {
        tune_hz: f64,
        decay: f64,
        skin: f64,
        rim_tone: f64,
        #[serde(default = "default_tapan_levels")]
        levels_db: [f64; 3],
    },
    /// The chord pad (P1 R3): wave 0 saw / 1 triangle, attack, release, detune between the
    /// two oscillators (cents), low-pass cutoff, slow movement, dynamics.
    ChordPad {
        wave: u8,
        attack_s: f64,
        release_s: f64,
        detune_cents: f64,
        cutoff_hz: f64,
        movement: f64,
        dynamics: f64,
    },
    /// The lead (P1 R3): wave 0 saw / 1 pulse / 2 triangle, glide, vibrato rate and depth,
    /// filter cutoff / resonance / envelope amount, attack, release, drive.
    Lead {
        wave: u8,
        glide_s: f64,
        vibrato_hz: f64,
        vibrato_cents: f64,
        cutoff_hz: f64,
        resonance: f64,
        env_amount: f64,
        attack_s: f64,
        release_s: f64,
        drive: f64,
    },
}

fn default_tapan_levels() -> [f64; 3] {
    [0.0, 0.0, -10.0]
}

fn default_levels() -> [f64; 6] {
    [0.0, 0.0, -8.0, 0.0, 0.0, 0.0]
}

impl InstrumentVoice {
    pub fn category(&self) -> &'static str {
        match self {
            InstrumentVoice::Drums { .. } => "drums",
            InstrumentVoice::Bass { .. } => "bass",
            InstrumentVoice::Synth { .. } => "synth",
            InstrumentVoice::Tapan { .. } => "tapan",
            InstrumentVoice::ChordPad { .. } => "chordpad",
            InstrumentVoice::Lead { .. } => "lead",
        }
    }

    pub fn validate(&self) -> Result<()> {
        let unit = |v: f64, what: &str| -> Result<()> {
            if !(0.0..=1.0).contains(&v) {
                return Err(ProjectError::InvalidState(format!("{what} must be 0..1")));
            }
            Ok(())
        };
        match self {
            InstrumentVoice::Drums {
                kick_hz,
                decay,
                brightness,
                levels_db,
            } => {
                if !(35.0..=90.0).contains(kick_hz) {
                    return Err(ProjectError::InvalidState("kick_hz 35..90".into()));
                }
                unit(*decay, "decay")?;
                unit(*brightness, "brightness")?;
                if levels_db.iter().any(|l| !(-60.0..=6.0).contains(l)) {
                    return Err(ProjectError::InvalidState("drum level −60..6 dB".into()));
                }
                Ok(())
            }
            InstrumentVoice::Bass {
                voice,
                cutoff_hz,
                resonance,
                drive,
                glide_s,
                sidechain,
                wobble_div,
                wobble_depth,
                warmth,
                attack_s,
                decay_s,
                sustain,
                release_s,
                filter_env,
                pitch_env_st,
                pitch_env_s,
                microvar,
            } => {
                if *voice > 2
                    || !(60.0..=8000.0).contains(cutoff_hz)
                    || !(0.0..=0.95).contains(resonance)
                    || !(0.0..=1.0).contains(glide_s)
                    || !(0.0005..=2.0).contains(attack_s)
                    || !(0.01..=4.0).contains(decay_s)
                    || !(0.01..=2.0).contains(release_s)
                    || !(-24.0..=24.0).contains(pitch_env_st)
                    || !(0.005..=1.0).contains(pitch_env_s)
                    || ![0u32, 1, 2, 4, 8, 16].contains(wobble_div)
                {
                    return Err(ProjectError::InvalidState("bass parameters".into()));
                }
                unit(*drive, "drive")?;
                unit(*sidechain, "sidechain")?;
                unit(*wobble_depth, "wobble_depth")?;
                unit(*warmth, "warmth")?;
                unit(*sustain, "sustain")?;
                unit(*filter_env, "filter_env")?;
                unit(*microvar, "microvar")
            }
            InstrumentVoice::Synth {
                timbre,
                attack_s,
                release_s,
                tone_hz,
                dynamics,
            } => {
                if *timbre > 2
                    || !(0.001..=2.0).contains(attack_s)
                    || !(0.01..=4.0).contains(release_s)
                    || !(200.0..=20_000.0).contains(tone_hz)
                {
                    return Err(ProjectError::InvalidState("synth parameters".into()));
                }
                unit(*dynamics, "dynamics")
            }
            InstrumentVoice::Tapan {
                tune_hz,
                decay,
                skin,
                rim_tone,
                levels_db,
            } => {
                if !(45.0..=110.0).contains(tune_hz) {
                    return Err(ProjectError::InvalidState("tapan tune 45..110 Hz".into()));
                }
                if levels_db.iter().any(|l| !(-60.0..=6.0).contains(l)) {
                    return Err(ProjectError::InvalidState("tapan level −60..6 dB".into()));
                }
                unit(*decay, "decay")?;
                unit(*skin, "skin")?;
                unit(*rim_tone, "rim_tone")
            }
            InstrumentVoice::ChordPad {
                wave,
                attack_s,
                release_s,
                detune_cents,
                cutoff_hz,
                movement,
                dynamics,
            } => {
                if *wave > 1
                    || !(0.005..=4.0).contains(attack_s)
                    || !(0.05..=8.0).contains(release_s)
                    || !(0.0..=30.0).contains(detune_cents)
                    || !(200.0..=12_000.0).contains(cutoff_hz)
                {
                    return Err(ProjectError::InvalidState("chord pad parameters".into()));
                }
                unit(*movement, "movement")?;
                unit(*dynamics, "dynamics")
            }
            InstrumentVoice::Lead {
                wave,
                glide_s,
                vibrato_hz,
                vibrato_cents,
                cutoff_hz,
                resonance,
                env_amount,
                attack_s,
                release_s,
                drive,
            } => {
                if *wave > 2
                    || !(0.0..=1.0).contains(glide_s)
                    || !(0.5..=9.0).contains(vibrato_hz)
                    || !(0.0..=100.0).contains(vibrato_cents)
                    || !(200.0..=12_000.0).contains(cutoff_hz)
                    || !(0.0..=0.95).contains(resonance)
                    || !(0.001..=2.0).contains(attack_s)
                    || !(0.01..=4.0).contains(release_s)
                {
                    return Err(ProjectError::InvalidState("lead parameters".into()));
                }
                unit(*env_amount, "env_amount")?;
                unit(*drive, "drive")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstrumentPreset {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    /// "drums" | "bass" | "synth" — always equal to the voice's category.
    pub category: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub author: String,
    /// The preset this one was forked from ("Edit copy"), if any.
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub factory: bool,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub created_ms: i64,
    #[serde(default)]
    pub updated_ms: i64,
    pub voice: InstrumentVoice,
}

fn schema_version() -> u32 {
    INSTRUMENT_SCHEMA_VERSION
}

impl InstrumentPreset {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version > INSTRUMENT_SCHEMA_VERSION {
            return Err(ProjectError::InvalidState(format!(
                "instrument schema {} is newer than this build ({})",
                self.schema_version, INSTRUMENT_SCHEMA_VERSION
            )));
        }
        if self.name.trim().is_empty() || self.name.chars().count() > 80 {
            return Err(ProjectError::InvalidState("instrument name".into()));
        }
        if self.category != self.voice.category() {
            return Err(ProjectError::InvalidState(format!(
                "category {} does not match the voice ({})",
                self.category,
                self.voice.category()
            )));
        }
        self.voice.validate()
    }

    /// Bring an older document up to the current schema (nothing to migrate yet; the
    /// field defaults cover v1 additions).
    pub fn migrate(mut self) -> Self {
        self.schema_version = INSTRUMENT_SCHEMA_VERSION;
        self
    }

    /// A user copy of this preset: new identity, same voice, lineage kept.
    pub fn fork(&self, id: String, name: String, now_ms: i64) -> InstrumentPreset {
        InstrumentPreset {
            schema_version: INSTRUMENT_SCHEMA_VERSION,
            id,
            name,
            category: self.category.clone(),
            tags: self.tags.clone(),
            author: "user".into(),
            parent_id: Some(self.id.clone()),
            factory: false,
            notes: String::new(),
            created_ms: now_ms,
            updated_ms: now_ms,
            voice: self.voice.clone(),
        }
    }
}

fn factory(
    id: &str,
    name: &str,
    tags: &[&str],
    notes: &str,
    voice: InstrumentVoice,
) -> InstrumentPreset {
    InstrumentPreset {
        schema_version: INSTRUMENT_SCHEMA_VERSION,
        id: format!("factory:{id}"),
        name: name.into(),
        category: voice.category().into(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        author: "EthnoFlow".into(),
        parent_id: None,
        factory: true,
        notes: notes.into(),
        created_ms: 0,
        updated_ms: 0,
        voice,
    }
}

/// Bass voice fields with the engine's defaults; presets use struct-update syntax.
#[derive(Debug, Clone, Copy)]
pub struct BassSpec {
    pub voice: u8,
    pub cutoff: f64,
    pub res: f64,
    pub drive: f64,
    pub warmth: f64,
    pub wobble_div: u32,
    pub wobble_depth: f64,
    pub attack: f64,
    pub decay: f64,
    pub sustain: f64,
    pub release: f64,
    pub filter_env: f64,
    pub pitch_st: f64,
    pub pitch_s: f64,
    pub microvar: f64,
}

impl Default for BassSpec {
    fn default() -> Self {
        Self {
            voice: 0,
            cutoff: 400.0,
            res: 0.2,
            drive: 0.3,
            warmth: 0.5,
            wobble_div: 0,
            wobble_depth: 0.5,
            attack: 0.004,
            decay: 0.2,
            sustain: 1.0,
            release: 0.08,
            filter_env: 0.0,
            pitch_st: 0.0,
            pitch_s: 0.05,
            microvar: 0.0,
        }
    }
}

impl From<BassSpec> for InstrumentVoice {
    fn from(b: BassSpec) -> Self {
        InstrumentVoice::Bass {
            voice: b.voice,
            cutoff_hz: b.cutoff,
            resonance: b.res,
            drive: b.drive,
            glide_s: 0.08,
            sidechain: 0.6,
            wobble_div: b.wobble_div,
            wobble_depth: b.wobble_depth,
            warmth: b.warmth,
            attack_s: b.attack,
            decay_s: b.decay,
            sustain: b.sustain,
            release_s: b.release,
            filter_env: b.filter_env,
            pitch_env_st: b.pitch_st,
            pitch_env_s: b.pitch_s,
            microvar: b.microvar,
        }
    }
}

/// The bass voice with the engine's defaults (what the section played before presets).
pub fn legacy_bass(voice: u8) -> InstrumentVoice {
    BassSpec {
        voice,
        ..BassSpec::default()
    }
    .into()
}

/// Curated, genuinely different factory presets (the four legacy sounds first).
/// Names are English identifiers; the UI localises known ids.
pub fn factory_instruments() -> Vec<InstrumentPreset> {
    let d = BassSpec::default();
    vec![
        // ---- drums
        factory("drums.liquid", "Liquid kit", &["liquid", "dnb"], "The section's original kit: 50 Hz kick, medium decay, balanced hats.",
            InstrumentVoice::Drums { kick_hz: 50.0, decay: 0.5, brightness: 0.5, levels_db: default_levels() }),
        factory("drums.dark", "Dark halftime kit", &["halftime", "dark", "deep"], "Low 42 Hz kick with a long tail, dull hats, snare forward: for breakdowns and deep rollers.",
            InstrumentVoice::Drums { kick_hz: 42.0, decay: 0.75, brightness: 0.25, levels_db: [1.0, 1.5, -10.0, -4.0, -6.0, -8.0] }),
        factory("drums.ethno", "Ethno kit", &["ethno", "7/8", "bright"], "Tighter 60 Hz kick, short decay, bright hats and a loud shaker for additive meters.",
            InstrumentVoice::Drums { kick_hz: 60.0, decay: 0.35, brightness: 0.65, levels_db: [0.0, -1.0, -6.0, -2.0, -3.0, 2.0] }),
        factory("drums.soft", "Soft wooden kit", &["ambient", "soft", "organic"], "Quiet, rounded: kick 48 Hz, short, very dark hats, ghost notes forward. For ambient DnB under a kaval.",
            InstrumentVoice::Drums { kick_hz: 48.0, decay: 0.4, brightness: 0.15, levels_db: [-2.0, -3.0, -5.0, -8.0, -10.0, -6.0] }),
        // ---- bass
        factory("bass.sub", "Sub", &["sub", "clean"], "Pure foundation: sine with a little triangle warmth, no drive to speak of.",
            BassSpec { voice: 0, ..d }.into()),
        factory("bass.pluck", "Pluck", &["pluck", "liquid"], "Saw through a closing filter: short decay, filter envelope open, light drive.",
            BassSpec { voice: 1, cutoff: 900.0, res: 0.3, drive: 0.2, attack: 0.002, decay: 0.25, sustain: 0.3, release: 0.12, filter_env: 0.7, ..d }.into()),
        factory("bass.reese", "Reese", &["reese", "liquid", "wobble"], "Two detuned saws, slow 1/8 wobble, drive: the liquid roller.",
            BassSpec { voice: 2, cutoff: 500.0, res: 0.25, drive: 0.5, wobble_div: 8, wobble_depth: 0.4, ..d }.into()),
        factory("bass.deep", "Deep sine", &["sub", "ambient", "soft"], "Very low and round: 250 Hz cutoff, maximum warmth, slow attack so it swells under pads.",
            BassSpec { voice: 0, cutoff: 250.0, drive: 0.1, warmth: 0.95, attack: 0.06, release: 0.3, ..d }.into()),
        factory("bass.growl", "Growl", &["reese", "neuro", "aggressive"], "Resonant, driven Reese with a fast filter envelope and a pitch dip: for the second drop.",
            BassSpec { voice: 2, cutoff: 350.0, res: 0.6, drive: 0.8, wobble_div: 4, wobble_depth: 0.6, decay: 0.3, sustain: 0.6, filter_env: 0.6, pitch_st: -5.0, pitch_s: 0.08, microvar: 0.2, ..d }.into()),
        factory("bass.wood", "Wooden pluck", &["pluck", "ethno", "organic"], "Short, dry, slightly unstable tuning (microvariation) like a plucked folk bass.",
            BassSpec { voice: 1, cutoff: 700.0, res: 0.15, drive: 0.35, warmth: 0.7, attack: 0.001, decay: 0.12, sustain: 0.0, release: 0.06, filter_env: 0.4, pitch_st: 3.0, pitch_s: 0.02, microvar: 0.5, ..d }.into()),
        // ---- synth
        factory("synth.breath", "Breath", &["flute", "soft"], "The original flute-like voice: slow-ish attack, airy, medium release.",
            InstrumentVoice::Synth { timbre: 2, attack_s: 0.01, release_s: 0.15, tone_hz: 6000.0, dynamics: 0.6 }),
        factory("synth.glass", "Glass", &["sine", "bell", "ambient"], "Sine with a long release and open tone: bell-like ostinatos.",
            InstrumentVoice::Synth { timbre: 0, attack_s: 0.004, release_s: 0.7, tone_hz: 9000.0, dynamics: 0.5 }),
        factory("synth.marimba", "Marimba", &["triangle", "percussive", "ethno"], "Triangle, instant attack, short release, darker tone, strong dynamics: wooden mallets.",
            InstrumentVoice::Synth { timbre: 1, attack_s: 0.001, release_s: 0.22, tone_hz: 3500.0, dynamics: 0.9 }),
        factory("synth.pad", "Soft pad", &["pad", "slow", "ambient"], "Slow attack and long release on the flute voice: an arpeggio becomes a wash.",
            InstrumentVoice::Synth { timbre: 2, attack_s: 0.25, release_s: 1.6, tone_hz: 4000.0, dynamics: 0.3 }),
        // ---- tapan (R3)
        factory("tapan.folk", "Tapan", &["tapan", "ethno", "7/8"], "The village drum: 70 Hz boom with a felt beater, medium decay, wooden rim.",
            InstrumentVoice::Tapan { tune_hz: 70.0, decay: 0.5, skin: 0.45, rim_tone: 0.5, levels_db: [0.0, -1.0, -10.0] }),
        factory("tapan.deep", "Deep tapan", &["tapan", "deep", "slow"], "Big, low (52 Hz), long and dark: for slow pieces and halftime, ghosts almost gone.",
            InstrumentVoice::Tapan { tune_hz: 52.0, decay: 0.85, skin: 0.25, rim_tone: 0.3, levels_db: [1.0, -3.0, -16.0] }),
        factory("tapan.tight", "Tight tapan", &["tapan", "fast", "bright"], "Small and dry (88 Hz), hard beater, bright metal rim: for fast horos and DnB.",
            InstrumentVoice::Tapan { tune_hz: 88.0, decay: 0.28, skin: 0.7, rim_tone: 0.85, levels_db: [-1.0, 0.0, -6.0] }),
        // ---- chord pad (R3)
        factory("chordpad.warm", "Warm pad", &["pad", "saw", "warm"], "Two detuned saws, slow attack, 1.4 kHz low-pass with gentle movement: the classic bed.",
            InstrumentVoice::ChordPad { wave: 0, attack_s: 0.8, release_s: 1.6, detune_cents: 7.0, cutoff_hz: 1400.0, movement: 0.3, dynamics: 0.4 }),
        factory("chordpad.glass", "Glass pad", &["pad", "triangle", "bright", "ambient"], "Triangles, little detune, open filter with more movement: thin and shining.",
            InstrumentVoice::ChordPad { wave: 1, attack_s: 0.3, release_s: 2.5, detune_cents: 4.0, cutoff_hz: 5000.0, movement: 0.5, dynamics: 0.3 }),
        factory("chordpad.dark", "Dark pad", &["pad", "saw", "dark", "deep"], "Wide detune, very slow attack and a 700 Hz filter: a low fog under the kaval.",
            InstrumentVoice::ChordPad { wave: 0, attack_s: 1.5, release_s: 3.0, detune_cents: 12.0, cutoff_hz: 700.0, movement: 0.15, dynamics: 0.2 }),
        // ---- lead (R3)
        factory("lead.kaval", "Kaval-like lead", &["lead", "triangle", "flute"], "Triangle with a little glide, delayed vibrato, soft attack and an open filter: a breathy answer to the melody.",
            InstrumentVoice::Lead { wave: 2, glide_s: 0.05, vibrato_hz: 5.5, vibrato_cents: 15.0, cutoff_hz: 3000.0, resonance: 0.1, env_amount: 0.3, attack_s: 0.03, release_s: 0.25, drive: 0.1 }),
        factory("lead.saw", "Saw lead", &["lead", "saw", "liquid"], "Saw through a resonant filter with a strong envelope: the liquid DnB lead.",
            InstrumentVoice::Lead { wave: 0, glide_s: 0.03, vibrato_hz: 5.0, vibrato_cents: 8.0, cutoff_hz: 1500.0, resonance: 0.35, env_amount: 0.6, attack_s: 0.005, release_s: 0.15, drive: 0.3 }),
        factory("lead.hollow", "Hollow lead", &["lead", "pulse", "dark"], "Pulse wave, long glide, deep vibrato, resonant low filter and drive: hollow and vocal.",
            InstrumentVoice::Lead { wave: 1, glide_s: 0.12, vibrato_hz: 6.0, vibrato_cents: 20.0, cutoff_hz: 900.0, resonance: 0.5, env_amount: 0.5, attack_s: 0.01, release_s: 0.3, drive: 0.5 }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_presets_are_valid_distinct_and_round_trip() {
        let all = factory_instruments();
        assert!(all.len() >= 21);
        for c in CATEGORIES {
            assert!(all.iter().filter(|p| p.category == c).count() >= 3, "≥ 3 presets for {c}");
        }
        let mut ids = std::collections::HashSet::new();
        for p in &all {
            p.validate().unwrap_or_else(|e| panic!("{}: {e}", p.id));
            assert!(ids.insert(p.id.clone()), "duplicate id {}", p.id);
            assert!(p.factory);
            let text = serde_json::to_string(p).unwrap();
            let back: InstrumentPreset = serde_json::from_str(&text).unwrap();
            assert_eq!(&back, p);
        }
        // Genuinely different kicks: fundamental, decay and brightness all move.
        let drums: Vec<_> = all.iter().filter(|p| p.category == "drums").collect();
        for a in &drums {
            for b in &drums {
                if a.id == b.id {
                    continue;
                }
                if let (
                    InstrumentVoice::Drums {
                        kick_hz: ka,
                        decay: da,
                        brightness: ba,
                        ..
                    },
                    InstrumentVoice::Drums {
                        kick_hz: kb,
                        decay: db,
                        brightness: bb,
                        ..
                    },
                ) = (&a.voice, &b.voice)
                {
                    let diff = ((ka - kb).abs() >= 2.0) as u8
                        + ((da - db).abs() >= 0.05) as u8
                        + ((ba - bb).abs() >= 0.1) as u8;
                    assert!(
                        diff >= 2,
                        "{} vs {} differ in fewer than two drum traits",
                        a.id,
                        b.id
                    );
                }
            }
        }
    }

    #[test]
    fn validation_catches_bad_documents_and_fork_keeps_lineage() {
        let mut p = factory_instruments()[0].clone();
        p.category = "bass".into();
        assert!(p.validate().is_err(), "category must match the voice");
        let mut q = factory_instruments()[5].clone();
        if let InstrumentVoice::Bass { cutoff_hz, .. } = &mut q.voice {
            *cutoff_hz = 9.0;
        }
        assert!(q.validate().is_err());
        let f = factory_instruments()[6].fork("u1".into(), "Моят Reese".into(), 5);
        assert!(!f.factory);
        assert_eq!(f.parent_id.as_deref(), Some("factory:bass.reese"));
        assert_eq!(f.voice, factory_instruments()[6].voice);
        f.validate().unwrap();
        // Older documents without the newer fields still load.
        let old = r#"{"id":"x","name":"Old","category":"drums","voice":{"engine":"drums","kick_hz":50.0,"decay":0.5,"brightness":0.5}}"#;
        let d: InstrumentPreset = serde_json::from_str(old).unwrap();
        d.migrate().validate().unwrap();
    }
}
