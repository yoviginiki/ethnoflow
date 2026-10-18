// Rack helpers for the Atmosphere screen (F1A.1). Factory presets come from the
// engine (`project::ambient_presets`); this file only knows how to make new layers.

import type { AmbientLayer, LayerKind, LayerSource, Macros } from "./api";
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
  shimmer: 0,
  shimmer_st: 12,
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
    case "resonator":
      return { kind, density: 20, decay_s: 3, structure: 0.5, brightness: 0.5, strike: 0.5, tonal: true, register: [55, 79] };
    case "choir":
      return { kind, vowel: 0.6, breath: 0.4, spread: 8, brightness: 0.4, morph: 0.3, voices: 4, tonal: true, register: [43, 64] };
    case "bloom":
      return { kind, density: 16, note_s: 1.6, brightness: 0.3, attack_s: 0.25, phrase: 0.7, tonal: true, register: [60, 84] };
    case "cloud":
      return { kind, take_id: takeId, grain_ms: 120, density: 20, position: 0.3, spray: 0.1, scan: 0, pitch: 0, jitter: 10, reverse_p: 0.2, octave_p: 0.1 };
  }
}

export const KIND_LABEL: Record<LayerKind, StringMessageKey> = {
  noise: "kindNoise",
  wind: "kindWind",
  texture: "kindTexture",
  bells: "kindBells",
  pluck: "kindPluck",
  loop: "kindLoop",
  cloud: "kindCloud",
  resonator: "kindResonator",
  choir: "kindChoir",
  bloom: "kindBloom",
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
    locked: false,
    ...patch,
  };
}
