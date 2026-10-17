// "Surprise me" (F1A.3): randomize the rack inside musically compatible bounds.
// Locked layers are never touched; kinds are kept (the scene keeps its character);
// levels move at most ±6 dB; tonal layers stay tonal; macros move gently. Seeded so
// a test can check the locks and the bounds.

import type { Ambient, AmbientLayer, LayerSource, Macros } from "./api";

/** mulberry32: small seeded generator for the UI (the engine has its own in Rust). */
export function mulberry(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
/** Move `v` by up to ±`spread` and keep it inside [lo, hi]. */
const nudge = (rnd: () => number, v: number, spread: number, lo: number, hi: number) => clamp(v + (rnd() * 2 - 1) * spread, lo, hi);

function randomizeSource(rnd: () => number, s: LayerSource): LayerSource {
  switch (s.kind) {
    case "noise":
      return { ...s, color: nudge(rnd, s.color, 0.3, 0, 1) };
    case "wind":
      return {
        ...s,
        gust: nudge(rnd, s.gust, 0.3, 0, 1),
        speed: nudge(rnd, s.speed, 0.25, 0, 1),
        center_hz: clamp(s.center_hz * Math.pow(2, (rnd() * 2 - 1) * 0.8), 80, 6000),
        resonance: nudge(rnd, s.resonance, 0.25, 0, 1),
      };
    case "texture":
      return {
        ...s,
        brightness: nudge(rnd, s.brightness, 0.3, 0, 1),
        motion: nudge(rnd, s.motion, 0.3, 0, 1),
        resonance: nudge(rnd, s.resonance, 0.2, 0, 1),
      };
    case "bells":
      return {
        ...s,
        density: clamp(s.density * Math.pow(2, (rnd() * 2 - 1) * 0.7), 0.5, 40),
        decay_s: clamp(s.decay_s * Math.pow(2, (rnd() * 2 - 1) * 0.6), 0.5, 20),
        brightness: nudge(rnd, s.brightness, 0.3, 0, 1),
        velocity_spread: nudge(rnd, s.velocity_spread, 0.2, 0, 1),
      };
    case "pluck":
      return {
        ...s,
        density: clamp(s.density * Math.pow(2, (rnd() * 2 - 1) * 0.7), 0.5, 60),
        damping: nudge(rnd, s.damping, 0.25, 0, 1),
        brightness: nudge(rnd, s.brightness, 0.3, 0, 1),
        velocity_spread: nudge(rnd, s.velocity_spread, 0.2, 0, 1),
      };
    case "loop":
      // A loop keeps its region (the user chose it); only the rate breathes a little.
      return { ...s, rate: clamp(s.rate * Math.pow(2, (rnd() * 2 - 1) * 0.17), 0.25, 4) };
    case "resonator":
      return {
        ...s,
        density: clamp(s.density * Math.pow(2, (rnd() * 2 - 1) * 0.7), 0.5, 60),
        decay_s: clamp(s.decay_s * Math.pow(2, (rnd() * 2 - 1) * 0.6), 0.3, 20),
        structure: nudge(rnd, s.structure, 0.3, 0, 1),
        brightness: nudge(rnd, s.brightness, 0.3, 0, 1),
        strike: nudge(rnd, s.strike, 0.3, 0, 1),
      };
    case "cloud":
      // The take and the pitch stay (the user chose them); the grain texture breathes.
      return {
        ...s,
        grain_ms: clamp(s.grain_ms * Math.pow(2, (rnd() * 2 - 1) * 0.6), 10, 500),
        density: clamp(s.density * Math.pow(2, (rnd() * 2 - 1) * 0.6), 0.5, 80),
        position: nudge(rnd, s.position, 0.2, 0, 1),
        spray: nudge(rnd, s.spray, 0.2, 0, 1),
        jitter: nudge(rnd, s.jitter, 10, 0, 100),
        reverse_p: nudge(rnd, s.reverse_p, 0.2, 0, 1),
        octave_p: nudge(rnd, s.octave_p, 0.15, 0, 1),
      };
  }
}

export function randomizeLayer(rnd: () => number, l: AmbientLayer): AmbientLayer {
  if (l.locked) return l;
  return {
    ...l,
    source: randomizeSource(rnd, l.source),
    gain_db: nudge(rnd, l.gain_db, 6, -40, -6),
    pan: nudge(rnd, l.pan, 0.4, -1, 1),
    seed: 1 + Math.floor(rnd() * 1_000_000),
  };
}

export function randomizeMacros(rnd: () => number, m: Macros): Macros {
  return {
    ...m,
    brightness: nudge(rnd, m.brightness, 0.15, 0.2, 0.8),
    motion: nudge(rnd, m.motion, 0.2, 0.2, 0.9),
    space: nudge(rnd, m.space, 0.15, 0, 0.7),
    space_size: nudge(rnd, m.space_size, 0.15, 0.2, 1),
    shimmer: nudge(rnd, m.shimmer, 0.15, 0, 0.6),
  };
}

/** The whole rack: unlocked layers and the gentle macros; intensity/width untouched. */
export function randomizeRack(rack: Ambient, seed: number): Ambient {
  const rnd = mulberry(seed);
  return { ...rack, layers: rack.layers.map((l) => randomizeLayer(rnd, l)), macros: randomizeMacros(rnd, rack.macros), preset: null };
}
