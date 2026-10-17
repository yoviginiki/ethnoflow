//! Background mixer buses (owner request, 2026-10-17: "not on/off, that is crude — knobs,
//! filters, how layers come in and go out, like a real mix"). The drone, the pad, the
//! atmosphere rack and the rhythm section each get a strip (trim, pan, low/high cut)
//! and fade times. There is no extra node in the graph: the strip is folded into every
//! member slot's own parameters whenever they are (re)applied, and the engine's
//! per-slot `fade_ms` turns switch-on / switch-off into a fade instead of a cut.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::ambient_cmds::AMBIENT_SLOT0;
use crate::commands::{err, CmdResult};
use crate::graph_cmds::{DRONE_SLOT, PAD_SLOT};
use crate::rhythm_cmds::{ARP_SLOT, DRUMS_SLOT};
use crate::state::{AppState, Inner};

pub const BUS_NAMES: [&str; 4] = ["drone", "pad", "ambient", "rhythm"];
pub const BUS_DRONE: usize = 0;
pub const BUS_PAD: usize = 1;
pub const BUS_AMBIENT: usize = 2;
pub const BUS_RHYTHM: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BusStrip {
    /// Trim added to every member's own level, dB (−40..+12).
    pub gain_db: f32,
    /// Added to every member's own pan (−1..1).
    pub pan: f32,
    /// The bus cuts above / below these; a member's own filter still applies.
    pub lowpass_hz: f32,
    pub highpass_hz: f32,
}

impl Default for BusStrip {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            pan: 0.0,
            lowpass_hz: 20_000.0,
            highpass_hz: 10.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BusState {
    pub strip: BusStrip,
    /// While fading out (the UI stops the sources when the fade is over).
    pub muted: bool,
    /// Gain ramp length of the member slots, ms (0 = the engine's 20 ms).
    pub fade_ms: u32,
}

pub fn bus_index(name: &str) -> CmdResult<usize> {
    BUS_NAMES
        .iter()
        .position(|n| *n == name)
        .ok_or_else(|| err("invalid_argument", format!("unknown bus {name}")))
}

/// Graph slots a bus owns.
pub fn bus_slots(bus: usize) -> std::ops::Range<usize> {
    match bus {
        BUS_DRONE => DRONE_SLOT..DRONE_SLOT + 1,
        BUS_PAD => PAD_SLOT..PAD_SLOT + 1,
        BUS_AMBIENT => AMBIENT_SLOT0..AMBIENT_SLOT0 + project::MAX_AMBIENT_LAYERS,
        _ => DRUMS_SLOT..ARP_SLOT + 1,
    }
}

/// The gain a member slot gets: its own level through the bus trim, −120 while the bus
/// fades out, and never above +12 dB.
pub fn bus_gain(s: &Inner, bus: usize, own_db: f32) -> f32 {
    let b = &s.bus[bus];
    if b.muted || own_db <= -119.0 {
        -120.0
    } else {
        (own_db + b.strip.gain_db).clamp(-120.0, 12.0)
    }
}

/// Pan and filters of a member slot through the bus (the stricter filter wins).
pub fn apply_bus_strip(
    s: &Inner,
    bus: usize,
    layer: &audio_engine::LayerParams,
    own_pan: f32,
    own_lowpass_hz: f32,
    own_highpass_hz: f32,
) {
    let b = &s.bus[bus];
    layer.pan.set((own_pan + b.strip.pan).clamp(-1.0, 1.0));
    layer
        .lowpass_hz
        .set(own_lowpass_hz.min(b.strip.lowpass_hz).clamp(40.0, 20_000.0));
    layer.highpass_hz.set(
        own_highpass_hz
            .max(b.strip.highpass_hz)
            .clamp(10.0, 8_000.0),
    );
    layer
        .fade_ms
        .store(b.fade_ms, std::sync::atomic::Ordering::Relaxed);
}

/// Push a bus into its running members (and the fade length into all its slots).
pub fn refresh(s: &mut Inner, bus: usize) {
    if let Some(g) = s.graph.as_ref() {
        for slot in bus_slots(bus) {
            g.params().layers[slot]
                .fade_ms
                .store(s.bus[bus].fade_ms, std::sync::atomic::Ordering::Relaxed);
        }
    }
    match bus {
        BUS_DRONE => {
            if let (Some(g), Some(set)) = (s.graph.as_ref(), s.drone_settings.as_ref()) {
                let l = &g.params().layers[DRONE_SLOT];
                l.gain_db.set(bus_gain(s, BUS_DRONE, set.level_db));
                apply_bus_strip(s, BUS_DRONE, l, 0.0, 20_000.0, 10.0);
            }
        }
        BUS_PAD => {
            if let (Some(g), Some(set)) = (s.graph.as_ref(), s.pad_settings.as_ref()) {
                let l = &g.params().layers[PAD_SLOT];
                l.gain_db.set(bus_gain(s, BUS_PAD, set.level_db));
                apply_bus_strip(s, BUS_PAD, l, 0.0, 20_000.0, 10.0);
            }
        }
        BUS_AMBIENT => crate::ambient_cmds::reapply_strips(s),
        _ => crate::rhythm_cmds::reapply(s),
    }
}

fn validate(strip: &BusStrip) -> CmdResult<()> {
    if !(-40.0..=12.0).contains(&strip.gain_db)
        || !(-1.0..=1.0).contains(&strip.pan)
        || !(40.0..=20_000.0).contains(&strip.lowpass_hz)
        || !(10.0..=8_000.0).contains(&strip.highpass_hz)
        || !strip.gain_db.is_finite()
    {
        return Err(err("invalid_argument", "bus strip out of range"));
    }
    Ok(())
}

/// The four strips, in `BUS_NAMES` order.
#[tauri::command]
pub async fn bus_get(state: State<'_, AppState>) -> CmdResult<Vec<BusStrip>> {
    let s = state.lock();
    Ok(s.bus.iter().map(|b| b.strip).collect())
}

/// Set a bus strip; takes effect at once on whatever plays in it.
#[tauri::command]
pub async fn bus_set(state: State<'_, AppState>, name: String, strip: BusStrip) -> CmdResult<()> {
    validate(&strip)?;
    let bus = bus_index(&name)?;
    let mut s = state.lock();
    s.bus[bus].strip = strip;
    refresh(&mut s, bus);
    Ok(())
}

/// Set the fade length of a bus and whether it is fading out. The UI calls this with
/// `muted = false` just before starting a layer (fade-in) and with `muted = true` when
/// switching it off, then stops the sources once the fade is over.
#[tauri::command]
pub async fn bus_fade(
    state: State<'_, AppState>,
    name: String,
    fade_ms: u32,
    muted: bool,
) -> CmdResult<()> {
    let bus = bus_index(&name)?;
    let mut s = state.lock();
    s.bus[bus].fade_ms = fade_ms.min(120_000);
    s.bus[bus].muted = muted;
    refresh(&mut s, bus);
    Ok(())
}

/// Peak per bus (max of its slots), in `BUS_NAMES` order; zeros when nothing plays.
#[tauri::command]
pub async fn bus_status(state: State<'_, AppState>) -> CmdResult<Vec<f32>> {
    let s = state.lock();
    let Some(g) = s.graph.as_ref() else {
        return Ok(vec![0.0; 4]);
    };
    let m = g.params().meters(2);
    Ok((0..4)
        .map(|b| {
            bus_slots(b)
                .filter_map(|slot| m.layers.get(slot).copied())
                .fold(0.0f32, f32::max)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_and_gains() {
        assert_eq!(bus_index("ambient").unwrap(), BUS_AMBIENT);
        assert!(bus_index("x").is_err());
        assert_eq!(bus_slots(BUS_AMBIENT).len(), project::MAX_AMBIENT_LAYERS);
        assert_eq!(bus_slots(BUS_RHYTHM), DRUMS_SLOT..ARP_SLOT + 1);
        let mut s = Inner::default();
        s.bus[BUS_PAD].strip.gain_db = -6.0;
        assert_eq!(bus_gain(&s, BUS_PAD, -12.0), -18.0);
        assert_eq!(bus_gain(&s, BUS_PAD, -120.0), -120.0);
        s.bus[BUS_PAD].muted = true;
        assert_eq!(bus_gain(&s, BUS_PAD, -12.0), -120.0);
        assert!(validate(&BusStrip {
            gain_db: 20.0,
            ..BusStrip::default()
        })
        .is_err());
    }
}
