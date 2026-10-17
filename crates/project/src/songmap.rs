//! Song map (Programs P2 Phase C / P3 Phase 1): the song's form in **ticks**, with a
//! tempo map and a meter map, so sections, cues and transitions have one canonical
//! timebase; seconds are derived, never the source of truth. Sections carry the musical
//! targets the Composer and the Performance Director read (energy, density, tension,
//! roles). Markers in seconds (F4.3) migrate into sections at the project's tempo.

use serde::{Deserialize, Serialize};

use crate::error::{ProjectError, Result};
use crate::manifest::Marker;

pub const SONG_MAP_SCHEMA_VERSION: u32 = 1;
/// Pulses per quarter note.
pub const PPQ: u64 = 960;
pub const SECTION_ROLES: [&str; 9] = [
    "intro",
    "establish",
    "build",
    "drop",
    "variation",
    "break",
    "bridge",
    "outro",
    "custom",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempoSegment {
    /// Tick where this tempo starts (the first segment starts at 0).
    pub tick: u64,
    pub bpm: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeterSegment {
    pub tick: u64,
    pub numerator: u32,
    pub unit: u32,
    #[serde(default)]
    pub grouping: Vec<u32>,
}

impl MeterSegment {
    /// Ticks per bar: `numerator` notes of `1/unit`.
    pub fn bar_ticks(&self) -> u64 {
        (PPQ * 4 * self.numerator as u64) / self.unit.max(1) as u64
    }
    pub fn beat_ticks(&self) -> u64 {
        (PPQ * 4) / self.unit.max(1) as u64
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: String,
    /// One of `SECTION_ROLES`.
    pub role: String,
    pub name: String,
    pub start_tick: u64,
    pub end_tick: u64,
    /// Musical targets 0..1 the generators read (energy level, event density, tension).
    #[serde(default = "half")]
    pub energy: f64,
    #[serde(default = "half")]
    pub density: f64,
    #[serde(default = "half")]
    pub tension: f64,
    /// Roles that play here (kaval, drone, pad, bass, drums, ambience …); empty = unspecified.
    #[serde(default)]
    pub roles: Vec<String>,
    #[serde(default)]
    pub transition_out: Option<String>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub notes: String,
}

fn half() -> f64 {
    0.5
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SongMap {
    #[serde(default = "schema_version")]
    pub schema_version: u32,
    #[serde(default = "ppq")]
    pub ppq: u64,
    pub tempo_map: Vec<TempoSegment>,
    pub meter_map: Vec<MeterSegment>,
    #[serde(default)]
    pub sections: Vec<Section>,
}

fn schema_version() -> u32 {
    SONG_MAP_SCHEMA_VERSION
}
fn ppq() -> u64 {
    PPQ
}

impl SongMap {
    /// An empty map at one tempo and meter.
    pub fn new(bpm: f64, numerator: u32, unit: u32, grouping: Vec<u32>) -> Self {
        Self {
            schema_version: SONG_MAP_SCHEMA_VERSION,
            ppq: PPQ,
            tempo_map: vec![TempoSegment { tick: 0, bpm }],
            meter_map: vec![MeterSegment {
                tick: 0,
                numerator,
                unit,
                grouping,
            }],
            sections: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version > SONG_MAP_SCHEMA_VERSION {
            return Err(ProjectError::InvalidState(
                "song map schema is newer".into(),
            ));
        }
        if self.ppq != PPQ {
            return Err(ProjectError::InvalidState("song map ppq".into()));
        }
        if self.tempo_map.first().map(|t| t.tick) != Some(0)
            || self.meter_map.first().map(|m| m.tick) != Some(0)
        {
            return Err(ProjectError::InvalidState(
                "tempo and meter maps must start at tick 0".into(),
            ));
        }
        for w in self.tempo_map.windows(2) {
            if w[1].tick <= w[0].tick {
                return Err(ProjectError::InvalidState(
                    "tempo map not increasing".into(),
                ));
            }
        }
        for t in &self.tempo_map {
            if !(20.0..=400.0).contains(&t.bpm) {
                return Err(ProjectError::InvalidState("bpm 20..400".into()));
            }
        }
        for w in self.meter_map.windows(2) {
            if w[1].tick <= w[0].tick {
                return Err(ProjectError::InvalidState(
                    "meter map not increasing".into(),
                ));
            }
        }
        for m in &self.meter_map {
            if !(1..=32).contains(&m.numerator) || ![1u32, 2, 4, 8, 16].contains(&m.unit) {
                return Err(ProjectError::InvalidState("meter".into()));
            }
            if !m.grouping.is_empty() && m.grouping.iter().sum::<u32>() != m.numerator {
                return Err(ProjectError::InvalidState(
                    "grouping must sum to the numerator".into(),
                ));
            }
        }
        let mut ids = std::collections::HashSet::new();
        let mut prev_end = 0u64;
        for s in &self.sections {
            if s.id.is_empty() || !ids.insert(s.id.as_str()) {
                return Err(ProjectError::InvalidState(
                    "section ids must be unique".into(),
                ));
            }
            if !SECTION_ROLES.contains(&s.role.as_str()) {
                return Err(ProjectError::InvalidState(format!(
                    "section role {}",
                    s.role
                )));
            }
            if s.end_tick <= s.start_tick {
                return Err(ProjectError::InvalidState(format!(
                    "section {} has no length",
                    s.id
                )));
            }
            if s.start_tick < prev_end {
                return Err(ProjectError::InvalidState(format!(
                    "section {} overlaps the previous one",
                    s.id
                )));
            }
            for v in [s.energy, s.density, s.tension] {
                if !(0.0..=1.0).contains(&v) {
                    return Err(ProjectError::InvalidState("section targets 0..1".into()));
                }
            }
            prev_end = s.end_tick;
        }
        Ok(())
    }

    // ---------- timebase ----------

    pub fn tempo_at(&self, tick: u64) -> f64 {
        self.tempo_map
            .iter()
            .rev()
            .find(|t| t.tick <= tick)
            .map(|t| t.bpm)
            .unwrap_or(120.0)
    }

    pub fn meter_at(&self, tick: u64) -> &MeterSegment {
        self.meter_map
            .iter()
            .rev()
            .find(|m| m.tick <= tick)
            .unwrap_or(&self.meter_map[0])
    }

    /// Seconds from the start, walking the tempo map.
    pub fn tick_to_seconds(&self, tick: u64) -> f64 {
        let mut secs = 0.0;
        for (i, seg) in self.tempo_map.iter().enumerate() {
            if seg.tick >= tick {
                break;
            }
            let end = self
                .tempo_map
                .get(i + 1)
                .map(|n| n.tick)
                .unwrap_or(u64::MAX)
                .min(tick);
            secs += (end - seg.tick) as f64 / self.ppq as f64 * 60.0 / seg.bpm;
        }
        secs
    }

    /// Tick at a time (fractional ticks truncated).
    pub fn seconds_to_tick(&self, seconds: f64) -> u64 {
        let mut left = seconds.max(0.0);
        let mut tick = 0u64;
        for (i, seg) in self.tempo_map.iter().enumerate() {
            let next = self.tempo_map.get(i + 1).map(|n| n.tick);
            let seg_ticks = next.map(|n| n - seg.tick);
            let secs_per_tick = 60.0 / seg.bpm / self.ppq as f64;
            match seg_ticks {
                Some(n) if left >= n as f64 * secs_per_tick => {
                    left -= n as f64 * secs_per_tick;
                    tick = seg.tick + n;
                }
                _ => return seg.tick + (left / secs_per_tick) as u64,
            }
        }
        tick
    }

    /// (bar, beat, ticks into the beat), bars and beats counted from 1, walking the meter map.
    pub fn tick_to_bar_beat(&self, tick: u64) -> (u64, u64, u64) {
        let mut bar = 0u64;
        for (i, seg) in self.meter_map.iter().enumerate() {
            let end = self
                .meter_map
                .get(i + 1)
                .map(|n| n.tick)
                .unwrap_or(u64::MAX);
            if tick < end {
                let into = tick - seg.tick;
                let bt = seg.bar_ticks();
                let b = into / bt;
                let rem = into - b * bt;
                let beat = rem / seg.beat_ticks();
                return (bar + b + 1, beat + 1, rem - beat * seg.beat_ticks());
            }
            bar += (end - seg.tick) / seg.bar_ticks();
        }
        (bar + 1, 1, 0)
    }

    /// Tick at the start of a bar (1-based), walking the meter map.
    pub fn bar_to_tick(&self, bar: u64) -> u64 {
        let mut remaining = bar.saturating_sub(1);
        for (i, seg) in self.meter_map.iter().enumerate() {
            let end = self.meter_map.get(i + 1).map(|n| n.tick);
            let bars_here = end.map(|e| (e - seg.tick) / seg.bar_ticks());
            match bars_here {
                Some(n) if remaining >= n => remaining -= n,
                _ => return seg.tick + remaining * seg.bar_ticks(),
            }
        }
        0
    }

    pub fn end_tick(&self) -> u64 {
        self.sections.last().map(|s| s.end_tick).unwrap_or(0)
    }

    // ---------- migration / editing ----------

    /// Sections from seconds-based markers at one tempo: each marker opens a section that
    /// lasts until the next marker; the last one gets `tail_bars`. Names map to roles.
    pub fn from_markers(
        bpm: f64,
        numerator: u32,
        unit: u32,
        grouping: Vec<u32>,
        markers: &[Marker],
        tail_bars: u64,
    ) -> Self {
        let mut map = Self::new(bpm, numerator, unit, grouping);
        let mut ms: Vec<&Marker> = markers.iter().collect();
        ms.sort_by(|a, b| a.at_seconds.partial_cmp(&b.at_seconds).unwrap());
        let bt = map.meter_map[0].bar_ticks();
        let snap = |t: u64| ((t + bt / 2) / bt) * bt;
        for (i, m) in ms.iter().enumerate() {
            let start = snap(map.seconds_to_tick(m.at_seconds));
            let end = ms
                .get(i + 1)
                .map(|n| snap(map.seconds_to_tick(n.at_seconds)))
                .unwrap_or(start + tail_bars.max(1) * bt);
            if end <= start {
                continue;
            }
            let role = role_for_name(&m.name);
            map.sections.push(Section {
                id: format!("s{}", i + 1),
                role: role.into(),
                name: m.name.clone(),
                start_tick: start,
                end_tick: end,
                energy: default_energy(role),
                density: default_energy(role),
                tension: 0.5,
                roles: Vec::new(),
                transition_out: None,
                locked: false,
                notes: String::new(),
            });
        }
        map
    }

    /// Split the section containing `tick` at a bar boundary (no-op at its edges).
    pub fn split_at(&mut self, tick: u64) -> bool {
        let bt = self.meter_at(tick).bar_ticks();
        let tick = (tick / bt) * bt;
        let Some(i) = self
            .sections
            .iter()
            .position(|s| s.start_tick < tick && tick < s.end_tick)
        else {
            return false;
        };
        let mut right = self.sections[i].clone();
        right.id = format!("{}b{}", right.id, tick);
        right.start_tick = tick;
        right.locked = false;
        self.sections[i].end_tick = tick;
        self.sections.insert(i + 1, right);
        true
    }

    /// Merge the section at `index` with the next one.
    pub fn merge_with_next(&mut self, index: usize) -> bool {
        if index + 1 >= self.sections.len() {
            return false;
        }
        let next = self.sections.remove(index + 1);
        self.sections[index].end_tick = next.end_tick;
        true
    }
}

pub fn role_for_name(name: &str) -> &'static str {
    let n = name.trim().to_lowercase();
    match n.as_str() {
        "intro" | "интро" => "intro",
        "build" | "градеж" => "build",
        "drop" | "дроп" => "drop",
        "break" | "breakdown" | "пауза" => "break",
        "outro" | "аутро" | "край" => "outro",
        "bridge" | "мост" => "bridge",
        "a" => "establish",
        "b" => "variation",
        _ => "custom",
    }
}

fn default_energy(role: &str) -> f64 {
    match role {
        "intro" => 0.2,
        "establish" => 0.4,
        "build" => 0.6,
        "drop" => 0.9,
        "variation" => 0.8,
        "break" => 0.3,
        "bridge" => 0.5,
        "outro" => 0.2,
        _ => 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_seconds_bars_round_trip_across_tempo_and_meter_changes() {
        let mut m = SongMap::new(120.0, 4, 4, vec![]);
        // 120 BPM for 8 bars, then 90 BPM; 4/4 for 4 bars then 7/8 (3+2+2).
        m.tempo_map.push(TempoSegment {
            tick: 8 * 4 * PPQ,
            bpm: 90.0,
        });
        m.meter_map.push(MeterSegment {
            tick: 4 * 4 * PPQ,
            numerator: 7,
            unit: 8,
            grouping: vec![3, 2, 2],
        });
        m.validate().unwrap();
        // 8 bars of 4/4 at 120 = 16 s.
        assert!((m.tick_to_seconds(8 * 4 * PPQ) - 16.0).abs() < 1e-9);
        // One more 4/4 bar at 90 BPM = 2.6667 s.
        assert!((m.tick_to_seconds(9 * 4 * PPQ) - (16.0 + 4.0 * 60.0 / 90.0)).abs() < 1e-9);
        for secs in [0.0, 3.3, 15.999, 16.0, 20.5, 42.0] {
            let t = m.seconds_to_tick(secs);
            assert!((m.tick_to_seconds(t) - secs).abs() < 0.002, "{secs}");
        }
        // Bars: the fifth bar starts at 4 bars of 4/4; the sixth is one 7/8 bar later.
        assert_eq!(m.bar_to_tick(5), 4 * 4 * PPQ);
        assert_eq!(m.bar_to_tick(6), 4 * 4 * PPQ + 7 * PPQ / 2);
        assert_eq!(m.tick_to_bar_beat(m.bar_to_tick(6)), (6, 1, 0));
        let (bar, beat, _) = m.tick_to_bar_beat(4 * 4 * PPQ + PPQ);
        assert_eq!((bar, beat), (5, 3), "two eighth beats into the 7/8 bar");
    }

    #[test]
    fn markers_migrate_into_bar_snapped_sections_with_roles() {
        let markers = vec![
            Marker {
                at_seconds: 0.0,
                name: "Intro".into(),
            },
            Marker {
                at_seconds: 16.1,
                name: "Build".into(),
            },
            Marker {
                at_seconds: 31.9,
                name: "Drop".into(),
            },
            Marker {
                at_seconds: 64.0,
                name: "Outro".into(),
            },
        ];
        let m = SongMap::from_markers(120.0, 4, 4, vec![], &markers, 8);
        m.validate().unwrap();
        let roles: Vec<&str> = m.sections.iter().map(|s| s.role.as_str()).collect();
        assert_eq!(roles, vec!["intro", "build", "drop", "outro"]);
        // 16.1 s at 120 BPM ≈ bar 9 (8 bars × 2 s): snapped to the bar line.
        assert_eq!(m.sections[1].start_tick, 8 * 4 * PPQ);
        assert_eq!(m.sections[2].start_tick, 16 * 4 * PPQ);
        assert_eq!(m.sections[3].end_tick, 32 * 4 * PPQ + 8 * 4 * PPQ);
        assert!(m.sections[2].energy > m.sections[0].energy);
    }

    #[test]
    fn validation_rejects_overlaps_bad_roles_and_split_merge_keep_it_valid() {
        let markers = vec![
            Marker {
                at_seconds: 0.0,
                name: "A".into(),
            },
            Marker {
                at_seconds: 8.0,
                name: "B".into(),
            },
        ];
        let mut m = SongMap::from_markers(120.0, 4, 4, vec![], &markers, 4);
        m.validate().unwrap();
        assert!(
            m.split_at(2 * 4 * PPQ + 3),
            "split inside the first section (snaps to bar 3)"
        );
        m.validate().unwrap();
        assert_eq!(m.sections.len(), 3);
        assert_eq!(m.sections[1].start_tick, 2 * 4 * PPQ);
        assert!(m.merge_with_next(0));
        m.validate().unwrap();
        assert_eq!(m.sections.len(), 2);
        let mut bad = m.clone();
        bad.sections[1].start_tick = bad.sections[0].end_tick - 1;
        assert!(bad.validate().is_err(), "overlap");
        let mut bad2 = m.clone();
        bad2.sections[0].role = "chorus".into();
        assert!(bad2.validate().is_err(), "unknown role");
        // Older JSON without the newer fields still loads.
        let text = r#"{"tempo_map":[{"tick":0,"bpm":100.0}],"meter_map":[{"tick":0,"numerator":4,"unit":4}],"sections":[{"id":"x","role":"drop","name":"D","start_tick":0,"end_tick":3840}]}"#;
        let old: SongMap = serde_json::from_str(text).unwrap();
        old.validate().unwrap();
        assert_eq!(old.sections[0].energy, 0.5);
    }
}
