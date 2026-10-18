//! Section follower (P2 Phase E / P3, first slice): the song map stops being a drawing.
//! While the song or the rhythm runs, the current section's **role, energy and density**
//! shape what plays: which rhythm parts sound, how dense the figures are, how loud each
//! background bus is. The user's settings stay the base — the follower only *removes*
//! (never switches on a part the user turned off) and *offsets* (bus trims), and every
//! effect is reported in the status so the UI can say what it did and why.
//!
//! Timing: the follower runs on the status polls (≤ 150 ms while something plays); a
//! section change lands within that. Offline renders and the song export do not follow
//! the map yet (recorded as a limitation).

use project::{Section, SongMap};
use serde::{Deserialize, Serialize};

use crate::rhythm_cmds::{ExtraKind, RhythmSettings};
use crate::state::Inner;

/// What the current section does to the parts and the buses.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionEffect {
    pub drums: bool,
    pub bass: bool,
    pub ostinato: bool,
    pub arp: bool,
    pub tapan: bool,
    pub chord_pad: bool,
    pub lead: bool,
    /// Multipliers on the generators' density / complexity (0..1).
    pub density_scale: f64,
    pub complexity_scale: f64,
    /// Seed offset for the melodic figures (a variation gets a different pattern).
    pub seed_offset: u32,
    /// Bus trims added to the user's mixer, dB: drone, pad, ambient, rhythm.
    pub bus_db: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionStatus {
    pub id: String,
    pub name: String,
    pub role: String,
    pub index: usize,
    /// 0..1 inside the section.
    pub progress: f64,
    pub energy: f64,
    pub density: f64,
    pub effect: SectionEffect,
}

fn all_on() -> SectionEffect {
    SectionEffect {
        drums: true,
        bass: true,
        ostinato: true,
        arp: true,
        tapan: true,
        chord_pad: true,
        lead: true,
        density_scale: 1.0,
        complexity_scale: 1.0,
        seed_offset: 0,
        bus_db: [0.0; 4],
    }
}

/// The effect of a section role at `progress` with its energy and density targets.
/// Rules are musical conventions, written out so they can be argued with:
/// intro/break thin the rhythm, build and outro ramp, drop and variation are full,
/// energy moves the rhythm bus (±3 dB around the role's level) and softens the pad.
pub fn effect(role: &str, energy: f64, density: f64, progress: f64) -> SectionEffect {
    let p = progress.clamp(0.0, 1.0);
    let e = energy.clamp(0.0, 1.0);
    let d = density.clamp(0.0, 1.0);
    let mut x = all_on();
    let role_rhythm_db: f32 = match role {
        "intro" => {
            x.drums = false;
            x.bass = false;
            x.arp = false;
            x.tapan = false;
            x.lead = false;
            x.density_scale = 0.6;
            -6.0
        }
        "establish" => {
            x.arp = false;
            x.lead = false;
            x.complexity_scale = 0.7;
            x.density_scale = 0.8;
            -3.0
        }
        "build" => {
            // Thin at the start, full at the end; the bus rises with it.
            x.arp = p > 0.5;
            x.lead = p > 0.25;
            x.density_scale = 0.5 + 0.5 * p;
            x.complexity_scale = 0.6 + 0.4 * p;
            -6.0 + 6.0 * p as f32
        }
        "drop" => 0.0,
        "variation" => {
            x.seed_offset = 1;
            0.0
        }
        "break" => {
            x.drums = false;
            x.bass = false;
            x.ostinato = false;
            x.arp = false;
            x.tapan = false;
            x.density_scale = 0.5;
            -4.0
        }
        "bridge" => {
            x.ostinato = false;
            x.tapan = false;
            x.complexity_scale = 0.5;
            x.density_scale = 0.7;
            -3.0
        }
        "outro" => {
            x.arp = false;
            x.lead = p < 0.5;
            x.drums = p < 0.75;
            x.tapan = p < 0.75;
            x.density_scale = 1.0 - 0.5 * p;
            -12.0 * p as f32
        }
        _ => -6.0 + 6.0 * e as f32,
    };
    x.density_scale = (x.density_scale * (0.5 + d)).clamp(0.1, 1.0);
    let energy_db = ((e - 0.5) * 6.0) as f32;
    x.bus_db = [
        if role == "drop" { -3.0 } else { 0.0 },        // drone
        (-3.0 * e as f32).max(-6.0),                    // pad: louder in quiet parts
        (-4.0 * e as f32).max(-6.0),                    // ambient
        (role_rhythm_db + energy_db).clamp(-24.0, 3.0), // rhythm
    ];
    x
}

/// The user's settings through a section: parts only ever switch *off*, figures get
/// sparser, variations take a different seed. Levels stay (the buses carry the dynamics).
pub fn overlay(base: &RhythmSettings, fx: &SectionEffect) -> RhythmSettings {
    let mut s = base.clone();
    s.drums.on &= fx.drums;
    s.bass.on &= fx.bass;
    s.ostinato.on &= fx.ostinato;
    s.arp.on &= fx.arp;
    s.drums.complexity = (s.drums.complexity * fx.complexity_scale).clamp(0.0, 1.0);
    s.ostinato.density = (s.ostinato.density * fx.density_scale).clamp(0.05, 1.0);
    s.arp.density = (s.arp.density * fx.density_scale).clamp(0.05, 1.0);
    if fx.seed_offset > 0 {
        if !s.ostinato.locked {
            s.ostinato.seed = s.ostinato.seed.wrapping_add(fx.seed_offset);
            s.ostinato.motif = None;
        }
        if !s.arp.locked {
            s.arp.seed = s.arp.seed.wrapping_add(fx.seed_offset);
        }
    }
    for x in s.extras.iter_mut() {
        let keep = match x.kind {
            ExtraKind::Tapan => fx.tapan,
            ExtraKind::ChordPad => fx.chord_pad,
            ExtraKind::Lead => fx.lead,
        };
        x.on &= keep;
        if matches!(x.kind, ExtraKind::Lead | ExtraKind::Tapan) {
            x.density = (x.density * fx.density_scale).clamp(0.0, 1.0);
        }
        if fx.seed_offset > 0 && !x.locked {
            x.seed = x.seed.wrapping_add(fx.seed_offset);
        }
    }
    s
}

/// The section under a tick and how far into it we are.
pub fn section_at(map: &SongMap, tick: u64) -> Option<(usize, &Section, f64)> {
    map.sections.iter().enumerate().find_map(|(i, sec)| {
        if tick >= sec.start_tick && tick < sec.end_tick {
            let len = (sec.end_tick - sec.start_tick).max(1) as f64;
            Some((i, sec, (tick - sec.start_tick) as f64 / len))
        } else {
            None
        }
    })
}

/// Current section status from the song position (None without a map or outside it).
pub fn current(s: &Inner) -> Option<SectionStatus> {
    let map = crate::transport_cmds::song_map(s)?;
    let (seconds, _) = crate::transport_cmds::current_seconds(s);
    let tick = map.seconds_to_tick(seconds);
    let (index, sec, progress) = section_at(&map, tick)?;
    Some(SectionStatus {
        id: sec.id.clone(),
        name: sec.name.clone(),
        role: sec.role.clone(),
        index,
        progress,
        energy: sec.energy,
        density: sec.density,
        effect: effect(&sec.role, sec.energy, sec.density, progress),
    })
}

/// Key that changes when the overlay should be recomputed (section, and the ramp step
/// for roles that evolve inside the section).
pub fn overlay_key(st: &SectionStatus) -> String {
    let step = match st.role.as_str() {
        "build" | "outro" => (st.progress * 4.0).floor() as u32,
        _ => 0,
    };
    format!("{}:{}", st.id, step)
}

/// Called from the status polls: pushes the current section into the buses and, when
/// the section (or ramp step) changed, re-syncs the rhythm from the user's base settings.
pub fn tick(s: &mut Inner) -> Option<SectionStatus> {
    let follow = s.follow_map;
    let st = if follow { current(s) } else { None };
    let desired: [f32; 4] = st.as_ref().map(|x| x.effect.bus_db).unwrap_or([0.0; 4]);
    let mut changed_bus = false;
    for (i, db) in desired.iter().enumerate() {
        if (s.bus[i].section_db - db).abs() > 1e-6 {
            s.bus[i].section_db = *db;
            changed_bus = true;
        }
    }
    if changed_bus {
        for i in 0..4 {
            crate::bus_cmds::refresh(s, i);
        }
    }
    let key = st.as_ref().map(overlay_key).unwrap_or_default();
    let needs_sync = s
        .rhythm
        .as_ref()
        .is_some_and(|live| live.section_key != key);
    if needs_sync {
        if let Some(base) = s.rhythm.as_ref().map(|l| l.base.clone()) {
            if let Err(e) = crate::rhythm_cmds::sync(s, base) {
                log::warn!("section follow: {}", e.message);
            }
        }
    }
    st
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roles_thin_or_fill_the_rhythm_and_build_ramps() {
        let intro = effect("intro", 0.2, 0.5, 0.0);
        assert!(!intro.drums && !intro.bass && intro.ostinato && intro.chord_pad);
        let drop = effect("drop", 0.9, 0.8, 0.5);
        assert!(drop.drums && drop.bass && drop.ostinato && drop.arp && drop.lead);
        assert!(drop.bus_db[3] > intro.bus_db[3], "drop louder than intro");
        let b0 = effect("build", 0.6, 0.6, 0.0);
        let b1 = effect("build", 0.6, 0.6, 0.99);
        assert!(!b0.arp && b1.arp);
        assert!(b1.bus_db[3] > b0.bus_db[3] && b1.density_scale > b0.density_scale);
        let brk = effect("break", 0.3, 0.3, 0.5);
        assert!(!brk.drums && !brk.bass && brk.chord_pad && brk.lead);
        let var = effect("variation", 0.8, 0.8, 0.5);
        assert_eq!(var.seed_offset, 1);
        // Energy moves the rhythm bus within a role.
        assert!(effect("drop", 1.0, 0.8, 0.5).bus_db[3] > effect("drop", 0.0, 0.8, 0.5).bus_db[3]);
        // Quiet parts get a louder pad.
        assert!(effect("intro", 0.1, 0.5, 0.0).bus_db[1] > effect("drop", 0.9, 0.5, 0.0).bus_db[1]);
    }

    #[test]
    fn overlay_never_switches_on_what_the_user_turned_off_and_varies_seeds() {
        let mut base = crate::rhythm_cmds::render_tests::settings();
        base.arp.on = false;
        base.ostinato.on = true;
        base.ostinato.seed = 10;
        let full = overlay(&base, &effect("drop", 0.9, 0.9, 0.5));
        assert!(!full.arp.on, "arp stays off");
        assert!(full.drums.on && full.bass.on);
        let intro = overlay(&base, &effect("intro", 0.2, 0.5, 0.0));
        assert!(!intro.drums.on && !intro.bass.on && intro.ostinato.on);
        assert!(intro.ostinato.density < base.ostinato.density);
        let var = overlay(&base, &effect("variation", 0.8, 0.8, 0.5));
        assert_eq!(var.ostinato.seed, 11);
        base.ostinato.locked = true;
        let locked = overlay(&base, &effect("variation", 0.8, 0.8, 0.5));
        assert_eq!(locked.ostinato.seed, 10, "a locked figure keeps its seed");
    }

    #[test]
    fn section_lookup_and_keys() {
        let mut map = SongMap::new(120.0, 4, 4, Vec::new());
        let bt = map.meter_map[0].bar_ticks();
        map.sections.push(Section {
            id: "a".into(),
            role: "build".into(),
            name: "Build".into(),
            start_tick: 0,
            end_tick: 4 * bt,
            energy: 0.6,
            density: 0.6,
            tension: 0.5,
            roles: Vec::new(),
            transition_out: None,
            locked: false,
            notes: String::new(),
        });
        map.sections.push(Section {
            id: "b".into(),
            role: "drop".into(),
            name: "Drop".into(),
            start_tick: 4 * bt,
            end_tick: 8 * bt,
            energy: 0.9,
            density: 0.9,
            tension: 0.5,
            roles: Vec::new(),
            transition_out: None,
            locked: false,
            notes: String::new(),
        });
        let (i, sec, p) = section_at(&map, bt).unwrap();
        assert_eq!((i, sec.id.as_str()), (0, "a"));
        assert!((p - 0.25).abs() < 1e-9);
        assert_eq!(section_at(&map, 5 * bt).unwrap().1.id, "b");
        assert!(section_at(&map, 9 * bt).is_none());
        let st = current_from(&map, 3 * bt);
        assert_eq!(overlay_key(&st), "a:3");
        let st2 = current_from(&map, 6 * bt);
        assert_eq!(overlay_key(&st2), "b:0");
    }

    fn current_from(map: &SongMap, tick: u64) -> SectionStatus {
        let (index, sec, progress) = section_at(map, tick).unwrap();
        SectionStatus {
            id: sec.id.clone(),
            name: sec.name.clone(),
            role: sec.role.clone(),
            index,
            progress,
            energy: sec.energy,
            density: sec.density,
            effect: effect(&sec.role, sec.energy, sec.density, progress),
        }
    }
}
