// Starter racks for the Atmosphere screen (F1A.1; the preset browser with files and
// tags is F1A.3). All sources are synthesized and original; tonal layers follow the
// project's scale and fall back to a "no key" set when there is none.

import type { Ambient, AmbientLayer, LayerKind, LayerSource, Macros } from "./api";
import type { StringMessageKey } from "./i18n";

export const MAX_AMBIENT_LAYERS = 8;

export const DEFAULT_MACROS: Macros = {
  intensity: 1,
  brightness: 0.5,
  motion: 0.5,
  space: 0.25,
  space_size: 0.6,
  tonal_gravity: 0.5,
  width: 1,
};

export function newId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID().slice(0, 8) : Math.random().toString(36).slice(2, 10);
}

/** What "add layer" creates per kind (mirrors `LayerSource::default_for` in Rust). */
export function defaultSource(kind: LayerKind, takeId = ""): LayerSource {
  switch (kind) {
    case "noise":
      return { kind, color: 0.6 };
    case "wind":
      return { kind, gust: 0.6, speed: 0.3, center_hz: 600, resonance: 0.5 };
    case "texture":
      return { kind, brightness: 0.3, motion: 0.5, resonance: 0.6, tonal: true, register: [48, 72] };
    case "bells":
      return { kind, density: 10, decay_s: 5, brightness: 0.4, velocity_spread: 0.5, tonal: true, register: [60, 84] };
    case "pluck":
      return { kind, density: 15, damping: 0.3, brightness: 0.5, velocity_spread: 0.5, tonal: true, register: [48, 72] };
    case "loop":
      return { kind, take_id: takeId, start: 0, end: 0, crossfade: 0.1, rate: 1, reverse: false };
  }
}

export const KIND_LABEL: Record<LayerKind, StringMessageKey> = {
  noise: "kindNoise",
  wind: "kindWind",
  texture: "kindTexture",
  bells: "kindBells",
  pluck: "kindPluck",
  loop: "kindLoop",
};

export function makeLayer(name: string, source: LayerSource, patch: Partial<AmbientLayer> = {}): AmbientLayer {
  return {
    id: newId(),
    name,
    source,
    gain_db: -18,
    pan: 0,
    lowpass_hz: 20000,
    highpass_hz: 10,
    mute: false,
    solo: false,
    seed: 1 + Math.floor(Math.random() * 1_000_000),
    license_ref: null,
    ...patch,
  };
}

export type StarterKey = "starterMountain" | "starterNoKey" | "starterNight";

/** Three starter racks; names are i18n keys so the strips read in the UI language. */
export function starter(key: StarterKey, names: Record<LayerKind, string>): Ambient {
  const L = makeLayer;
  let layers: AmbientLayer[];
  let macros: Macros = { ...DEFAULT_MACROS };
  switch (key) {
    case "starterMountain":
      layers = [
        L(names.wind, { kind: "wind", gust: 0.5, speed: 0.2, center_hz: 400, resonance: 0.4 }, { gain_db: -20, pan: -0.3 }),
        L(names.noise, { kind: "noise", color: 0.75 }, { gain_db: -28, lowpass_hz: 2000, pan: 0.2 }),
        L(names.texture, { kind: "texture", brightness: 0.3, motion: 0.6, resonance: 0.6, tonal: true, register: [48, 72] }, { gain_db: -18, pan: 0.25 }),
        L(names.bells, { kind: "bells", density: 6, decay_s: 8, brightness: 0.35, velocity_spread: 0.6, tonal: true, register: [64, 88] }, { gain_db: -16, pan: 0.4 }),
        L(names.pluck, { kind: "pluck", density: 8, damping: 0.25, brightness: 0.45, velocity_spread: 0.5, tonal: true, register: [48, 72] }, { gain_db: -18, pan: -0.35 }),
      ];
      macros = { ...macros, space: 0.35, space_size: 0.7 };
      break;
    case "starterNoKey":
      layers = [
        L(names.noise, { kind: "noise", color: 0.9 }, { gain_db: -24, lowpass_hz: 900 }),
        L(names.wind, { kind: "wind", gust: 0.7, speed: 0.35, center_hz: 700, resonance: 0.55 }, { gain_db: -18, pan: 0.3 }),
        L(names.texture, { kind: "texture", brightness: 0.2, motion: 0.7, resonance: 0.7, tonal: false, register: [40, 64] }, { gain_db: -18, pan: -0.3 }),
        L(names.bells, { kind: "bells", density: 4, decay_s: 10, brightness: 0.5, velocity_spread: 0.6, tonal: false, register: [60, 84] }, { gain_db: -20, pan: 0.5 }),
      ];
      macros = { ...macros, space: 0.4, space_size: 0.8 };
      break;
    case "starterNight":
      layers = [
        L(names.noise, { kind: "noise", color: 1 }, { gain_db: -30, lowpass_hz: 600 }),
        L(names.wind, { kind: "wind", gust: 0.3, speed: 0.1, center_hz: 250, resonance: 0.7 }, { gain_db: -22, pan: -0.5 }),
        L(names.texture, { kind: "texture", brightness: 0.1, motion: 0.7, resonance: 0.8, tonal: true, register: [40, 64] }, { gain_db: -22, pan: 0.1 }),
        L(names.pluck, { kind: "pluck", density: 5, damping: 0.2, brightness: 0.35, velocity_spread: 0.6, tonal: true, register: [52, 76] }, { gain_db: -20, pan: 0.45 }),
        L(names.bells, { kind: "bells", density: 3, decay_s: 12, brightness: 0.2, velocity_spread: 0.7, tonal: true, register: [72, 96] }, { gain_db: -22, pan: -0.4 }),
      ];
      macros = { ...macros, brightness: 0.4, space: 0.45, space_size: 0.85 };
      break;
  }
  return { schema_version: 1, layers, macros, preset: key };
}
