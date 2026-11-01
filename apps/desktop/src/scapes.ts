// Soundscapes 2, stage S1 (docs/design/SOUNDSCAPES_GENERATIVE_PLAN.md): a "journey" is a
// whole composition built on the atmosphere engine that already exists — a rack of layers,
// a program of four scenes (Intro → Development → Climax → Outro) with transitions, and
// "walk" automation lanes that make the macros breathe. Nothing here is a new synth: a
// world is a recipe, buildJourney turns it into an ordinary Ambient rack the engine
// plays, exports and lets the user edit in Atmosphere (Layers › Program).
import type { Ambient, AmbientLayer, AutomationLane, LayerSource, Macros, Scene } from "./api";
import { DEFAULT_MACROS, defaultSource, makeLayer, newId } from "./ambientPresets";
import type { StringMessageKey } from "./i18n";

export type WorldId = "ancient-forest" | "deep-space" | "cyber-rain" | "rhodope-night" | "mountain-morning" | "thracian-steppe";
export type PhaseRole = "intro" | "development" | "climax" | "outro";
export const PHASE_ROLES: PhaseRole[] = ["intro", "development", "climax", "outro"];
/** Share of the journey each phase takes. */
export const PHASE_SHARE: Record<PhaseRole, number> = { intro: 0.22, development: 0.33, climax: 0.25, outro: 0.2 };

/** What a layer does in the arc: beds stay, textures fade in, events and pulses arrive late. */
export type LayerRole = "bed" | "texture" | "events" | "pulse";

interface WorldLayer {
  role: LayerRole;
  name: StringMessageKey;
  source: LayerSource;
  gain_db: number;
  lowpass_hz?: number;
  highpass_hz?: number;
  pan?: number;
}

export interface World {
  id: WorldId;
  title: StringMessageKey;
  desc: StringMessageKey;
  macros: Partial<Macros>;
  layers: WorldLayer[];
}

/** Gain offset (dB) per layer role and phase; -60 = the layer is silent in that phase. */
const PHASE_GAIN: Record<PhaseRole, Record<LayerRole, number>> = {
  intro: { bed: 0, texture: -6, events: -60, pulse: -60 },
  development: { bed: 0, texture: -2, events: -8, pulse: -6 },
  climax: { bed: 1, texture: 0, events: 0, pulse: 0 },
  outro: { bed: -3, texture: -10, events: -60, pulse: -60 },
};

/** Macro deltas per phase on top of the world's base macros. */
const PHASE_MACROS: Record<PhaseRole, { intensity: number; brightness: number; space: number; motion: number }> = {
  intro: { intensity: 0.6, brightness: -0.15, space: 0.1, motion: -0.2 },
  development: { intensity: 0.8, brightness: 0, space: 0, motion: 0 },
  climax: { intensity: 1, brightness: 0.15, space: -0.05, motion: 0.2 },
  outro: { intensity: 0.5, brightness: -0.25, space: 0.2, motion: -0.3 },
};

const src = <K extends LayerSource["kind"]>(kind: K, patch: Partial<Extract<LayerSource, { kind: K }>>): LayerSource => ({ ...defaultSource(kind), ...patch }) as LayerSource;

export const WORLDS: World[] = [
  {
    id: "ancient-forest",
    title: "wAncientForest",
    desc: "wAncientForestDesc",
    macros: { brightness: 0.35, space: 0.35, space_size: 0.7, tonal_gravity: 0.6, motion: 0.5 },
    layers: [
      { role: "bed", name: "kindTexture", source: src("texture", { brightness: 0.25, motion: 0.4, register: [41, 60] }), gain_db: -14 },
      { role: "bed", name: "kindChoir", source: src("choir", { voices: 3, breath: 0.5, register: [36, 55] }), gain_db: -22 },
      { role: "texture", name: "kindWind", source: src("wind", { gust: 0.4, speed: 0.2, center_hz: 500 }), gain_db: -24 },
      { role: "events", name: "kindPluck", source: src("pluck", { density: 6, damping: 0.5, register: [55, 79] }), gain_db: -18 },
      { role: "events", name: "kindBells", source: src("bells", { tension: 0.1, density: 3, decay_s: 6, register: [67, 91] }), gain_db: -22 },
      { role: "pulse", name: "kindResonator", source: src("resonator", { density: 12, structure: 0.2, strike: 0.3, register: [43, 55] }), gain_db: -22 },
    ],
  },
  {
    id: "deep-space",
    title: "wDeepSpace",
    desc: "wDeepSpaceDesc",
    macros: { brightness: 0.2, space: 0.6, space_size: 1, tonal_gravity: 0.3, shimmer: 0.3, motion: 0.3 },
    layers: [
      { role: "bed", name: "kindNoise", source: src("noise", { color: 0.9 }), gain_db: -30, lowpass_hz: 1200 },
      { role: "bed", name: "kindTexture", source: src("texture", { brightness: 0.15, motion: 0.3, register: [36, 55] }), gain_db: -16 },
      { role: "texture", name: "kindChoir", source: src("choir", { voices: 2, breath: 0.8, morph: 0.6, register: [31, 50] }), gain_db: -24 },
      { role: "events", name: "kindResonator", source: src("resonator", { density: 4, structure: 0.9, decay_s: 8, brightness: 0.5, register: [60, 84] }), gain_db: -20 },
      { role: "events", name: "kindBells", source: src("bells", { tension: 0.1, density: 2, decay_s: 9, brightness: 0.3, register: [72, 96] }), gain_db: -24 },
    ],
  },
  {
    id: "cyber-rain",
    title: "wCyberRain",
    desc: "wCyberRainDesc",
    macros: { brightness: 0.55, space: 0.4, space_size: 0.5, motion: 0.7, width: 0.9 },
    layers: [
      { role: "bed", name: "kindNoise", source: src("noise", { color: 0.5 }), gain_db: -26, lowpass_hz: 6000 },
      { role: "bed", name: "kindTexture", source: src("texture", { brightness: 0.5, motion: 0.7, register: [48, 67] }), gain_db: -16 },
      { role: "texture", name: "kindWind", source: src("wind", { gust: 0.8, speed: 0.6, center_hz: 2000, resonance: 0.8 }), gain_db: -28 },
      { role: "events", name: "kindPluck", source: src("pluck", { density: 10, damping: 0.2, brightness: 0.7, register: [60, 84] }), gain_db: -18 },
      { role: "pulse", name: "kindResonator", source: src("resonator", { density: 30, structure: 0.1, strike: 0.8, decay_s: 1.5, register: [36, 48] }), gain_db: -20 },
    ],
  },
  {
    id: "rhodope-night",
    title: "wRhodopeNight",
    desc: "wRhodopeNightDesc",
    macros: { brightness: 0.3, space: 0.45, space_size: 0.8, tonal_gravity: 0.7, motion: 0.4 },
    layers: [
      { role: "bed", name: "kindChoir", source: src("choir", { vowel: 0.4, breath: 0.5, voices: 4, register: [43, 64] }), gain_db: -18 },
      { role: "bed", name: "kindTexture", source: src("texture", { brightness: 0.3, motion: 0.4, register: [48, 67] }), gain_db: -16 },
      { role: "texture", name: "kindWind", source: src("wind", { gust: 0.3, speed: 0.15, center_hz: 400 }), gain_db: -26 },
      { role: "events", name: "kindBloom", source: src("bloom", { density: 8, note_s: 2.5, attack_s: 0.4, phrase: 0.8, register: [67, 86] }), gain_db: -18 },
      { role: "events", name: "kindBells", source: src("bells", { tension: 0.1, density: 2, decay_s: 7, register: [72, 91] }), gain_db: -26 },
      { role: "pulse", name: "kindResonator", source: src("resonator", { density: 24, structure: 0.3, strike: 0.6, decay_s: 0.8, register: [36, 48] }), gain_db: -22 },
    ],
  },
  {
    id: "mountain-morning",
    title: "wMountainMorning",
    desc: "wMountainMorningDesc",
    macros: { brightness: 0.6, space: 0.3, space_size: 0.6, motion: 0.5, width: 1, shimmer: 0.15 },
    layers: [
      { role: "bed", name: "kindTexture", source: src("texture", { brightness: 0.45, motion: 0.5, register: [55, 74] }), gain_db: -16 },
      { role: "texture", name: "kindWind", source: src("wind", { gust: 0.5, speed: 0.3, center_hz: 800 }), gain_db: -22 },
      { role: "texture", name: "kindChoir", source: src("choir", { voices: 5, breath: 0.3, register: [48, 69] }), gain_db: -22 },
      { role: "events", name: "kindBells", source: src("bells", { tension: 0.1, density: 6, decay_s: 4, brightness: 0.6, register: [72, 96] }), gain_db: -20 },
      { role: "events", name: "kindBloom", source: src("bloom", { density: 10, note_s: 1.2, attack_s: 0.1, phrase: 0.5, register: [67, 91] }), gain_db: -20 },
      { role: "pulse", name: "kindPluck", source: src("pluck", { density: 18, damping: 0.3, register: [55, 74] }), gain_db: -22 },
    ],
  },
  {
    id: "thracian-steppe",
    title: "wThracianSteppe",
    desc: "wThracianSteppeDesc",
    macros: { brightness: 0.4, space: 0.4, space_size: 0.7, tonal_gravity: 0.8, motion: 0.4 },
    layers: [
      { role: "bed", name: "kindChoir", source: src("choir", { vowel: 0.7, voices: 3, breath: 0.6, register: [36, 55] }), gain_db: -18 },
      { role: "bed", name: "kindTexture", source: src("texture", { brightness: 0.35, motion: 0.4, register: [43, 62] }), gain_db: -16 },
      { role: "texture", name: "kindNoise", source: src("noise", { color: 0.7 }), gain_db: -32, lowpass_hz: 3000 },
      { role: "events", name: "kindResonator", source: src("resonator", { density: 8, structure: 0.5, strike: 0.5, register: [55, 79] }), gain_db: -20 },
      { role: "events", name: "kindBloom", source: src("bloom", { density: 6, note_s: 3, attack_s: 0.5, phrase: 0.9, register: [62, 84] }), gain_db: -20 },
      { role: "pulse", name: "kindPluck", source: src("pluck", { density: 28, damping: 0.7, register: [43, 55] }), gain_db: -22 },
    ],
  },
];

export const WORLD_IDS: WorldId[] = WORLDS.map((w) => w.id);
export function worldById(id: string): World | null {
  return WORLDS.find((w) => w.id === id) ?? null;
}

/** Tag that marks a rack as a journey of a world (the Soundscapes tab recognises it). */
export const JOURNEY_TAG = "scapes";
export function journeyWorld(rack: Ambient | null): WorldId | null {
  const tag = rack?.tags.find((x) => x.startsWith(`${JOURNEY_TAG}:`));
  const id = tag?.slice(JOURNEY_TAG.length + 1);
  return id && WORLD_IDS.includes(id as WorldId) ? (id as WorldId) : null;
}

export interface JourneyOptions {
  /** Whole length in minutes (1..60). */
  minutes: number;
  /** Seed for the layers' seeds and the lanes: same seed = same journey. */
  seed: number;
  /** Start again from Intro when Outro ends. */
  loop: boolean;
  /** Scene names in the user's language, by phase. */
  names: Record<PhaseRole, string>;
  /** The rack to keep schema_version from (the project's current rack). */
  base: Ambient;
  /** Layer names in the user's language (the kind labels). */
  layerName: (key: StringMessageKey) => string;
}

/** Deterministic small PRNG (mulberry32) so a seed reproduces a journey. */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const clamp01 = (v: number) => Math.max(0, Math.min(1, v));

function phaseMacros(base: Macros, role: PhaseRole): Macros {
  const d = PHASE_MACROS[role];
  return { ...base, intensity: clamp01(d.intensity), brightness: clamp01(base.brightness + d.brightness), space: clamp01(base.space + d.space), motion: clamp01(base.motion + d.motion) };
}

/** Build the whole journey as an atmosphere rack: layers, scenes, breathing lanes. */
export function buildJourney(world: World, opts: JourneyOptions): Ambient {
  const minutes = Math.max(1, Math.min(60, opts.minutes));
  const total = minutes * 60;
  const next = rng(opts.seed);
  const layers: AmbientLayer[] = world.layers.map((wl, i) =>
    makeLayer(opts.layerName(wl.name), wl.source, {
      gain_db: wl.gain_db,
      pan: wl.pan ?? Math.round((next() * 2 - 1) * 0.4 * 100) / 100,
      lowpass_hz: wl.lowpass_hz ?? 20000,
      highpass_hz: wl.highpass_hz ?? 10,
      seed: 1 + Math.floor(next() * 1_000_000) + i,
    }),
  );
  const roles: LayerRole[] = world.layers.map((wl) => wl.role);
  const macros: Macros = { ...DEFAULT_MACROS, ...world.macros };

  let at = 0;
  const scenes: Scene[] = PHASE_ROLES.map((role, i) => {
    const duration_s = Math.round(total * PHASE_SHARE[role]);
    const scene: Scene = {
      id: newId(),
      name: opts.names[role],
      start_s: at,
      duration_s,
      transition_s: i === 0 ? 0 : Math.min(20, Math.round(duration_s / 3)),
      snapshot: {
        layers: layers.map((l, k) => {
          const off = PHASE_GAIN[role][roles[k]];
          return { id: l.id, gain_db: off <= -60 ? -60 : Math.max(-60, l.gain_db + off), pan: l.pan, lowpass_hz: l.lowpass_hz, highpass_hz: l.highpass_hz, mute: false, source: l.source };
        }),
        macros: phaseMacros(macros, role),
      },
    };
    at += duration_s;
    return scene;
  });

  const lane = (param: "brightness" | "space" | "motion", period_s: number, depth: number): AutomationLane => ({
    id: newId(),
    target: { layer: null, param },
    mode: { mode: "walk", period_s },
    depth,
    humanize: 0.2,
    seed: 1 + Math.floor(next() * 1_000_000),
    enabled: true,
  });
  const automation: AutomationLane[] = [lane("brightness", 70, 0.2), lane("space", 95, 0.2), lane("motion", 50, 0.25)];

  return {
    schema_version: opts.base.schema_version,
    layers,
    macros: scenes[0].snapshot.macros,
    preset: null,
    tags: [JOURNEY_TAG, `${JOURNEY_TAG}:${world.id}`],
    automation,
    program: { scenes, loop_region: opts.loop ? [0, at] : null },
  };
}

export interface PhaseAt {
  index: number;
  role: PhaseRole;
  name: string;
  /** 0..1 inside the phase. */
  progress: number;
  /** Seconds left in the phase. */
  remaining_s: number;
  end_s: number;
}

/** Which phase of a journey plays at `position_s` (null when the rack has no scenes). */
export function phaseAt(rack: Ambient | null, position_s: number): PhaseAt | null {
  const scenes = rack?.program.scenes ?? [];
  if (scenes.length === 0) return null;
  const end_s = scenes.reduce((m, sc) => Math.max(m, sc.start_s + sc.duration_s), 0);
  const p = Math.max(0, Math.min(position_s, end_s));
  let index = scenes.findIndex((sc) => p >= sc.start_s && p < sc.start_s + sc.duration_s);
  if (index < 0) index = scenes.length - 1;
  const sc = scenes[index];
  const role = PHASE_ROLES[index] ?? "outro";
  return { index, role, name: sc.name, progress: sc.duration_s > 0 ? (p - sc.start_s) / sc.duration_s : 1, remaining_s: Math.max(0, sc.start_s + sc.duration_s - p), end_s };
}
