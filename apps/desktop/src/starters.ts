import type { Ambient as AmbientRack, AmbientLayer, LayerKind, LayerSource, Note, Scale, Timing } from "./api";
import { DEFAULT_MACROS, defaultSource, makeLayer } from "./ambientPresets";
import type { SessionPayload } from "./session";
import { DEFAULT_DRONE, DEFAULT_HARMONY, DEFAULT_PAD, DEFAULT_RHYTHM } from "./store";

/**
 * "Start from a melody" (owner request 2026-10-17): after the analysis of a take, propose
 * several complete starting points (drone + pad + rhythm + atmosphere + which layers
 * play) tuned to the melody's key, tempo and register. Pure functions: nothing here
 * touches the app; the Start screen applies a starter only when the user presses Try.
 * Nothing is assumed about the material: the key comes from the analysis or the
 * project, the tempo from the analysis or the project, and a fallback is labelled as
 * a suggestion in the facts.
 */

export interface MelodyRegister {
  low: number;
  high: number;
  median: number;
  count: number;
}

/** Register of the analysed melody (confident notes of at least 80 ms). */
export function melodyRegister(notes: Note[]): MelodyRegister | null {
  const m = notes
    .filter((n) => n.confidence >= 0.5 && n.end - n.start >= 0.08)
    .map((n) => n.midi)
    .sort((a, b) => a - b);
  if (m.length === 0) return null;
  return { low: m[0], high: m[m.length - 1], median: m[Math.floor(m.length / 2)], count: m.length };
}

export type StarterId = "quiet_ambient" | "dark_drone" | "liquid_dnb" | "ambient_dnb" | "ethno_pulse";
export const STARTER_IDS: StarterId[] = ["quiet_ambient", "dark_drone", "liquid_dnb", "ambient_dnb", "ethno_pulse"];

export type StarterFact =
  | { kind: "key"; scale: Scale }
  | { kind: "no_key" }
  | { kind: "tempo_detected" | "tempo_project" | "tempo_suggested" | "tempo_doubled"; bpm: number }
  | { kind: "meter_project" | "meter_suggested"; numerator: number; unit: number; grouping: number[] }
  | { kind: "pad_below"; low: number; high: number }
  | { kind: "register"; low: number; high: number }
  | { kind: "register_unknown" };

export interface StarterContext {
  scale: Scale | null;
  timing: Timing;
  /** Best tempo candidate of the analysis, if any. */
  detectedBpm: number | null;
  register: MelodyRegister | null;
  /** The project's current rack: schema, program and so on are kept; layers and macros are replaced. */
  baseRack: AmbientRack;
  /** Layer names in the UI language. */
  names: Record<LayerKind, string>;
}

export interface Starter {
  id: StarterId;
  session: SessionPayload;
  /** Timing to write into the project before trying (null = keep the project's). */
  timing: Timing | null;
  facts: StarterFact[];
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

function root(ctx: StarterContext): number | null {
  return ctx.scale ? 48 + ctx.scale.tonic_pc : null;
}

/** Where the pad and texture sit: just below the melody, never on top of it. */
function padRegister(ctx: StarterContext): { low: number; high: number; fact: StarterFact } {
  if (!ctx.register) return { low: 48, high: 72, fact: { kind: "register_unknown" } };
  const low = clamp(ctx.register.median - 19, 36, 60);
  return { low, high: low + 24, fact: { kind: "pad_below", low, high: low + 24 } };
}

function dnbTempo(ctx: StarterContext): { bpm: number; fact: StarterFact } {
  const b = ctx.detectedBpm ?? ctx.timing.bpm;
  if (b !== null) {
    if (b >= 150 && b <= 185) return { bpm: Math.round(b), fact: { kind: ctx.detectedBpm !== null ? "tempo_detected" : "tempo_project", bpm: Math.round(b) } };
    if (b >= 75 && b <= 93) return { bpm: Math.round(b * 2), fact: { kind: "tempo_doubled", bpm: Math.round(b * 2) } };
  }
  return { bpm: 172, fact: { kind: "tempo_suggested", bpm: 172 } };
}

function layer(ctx: StarterContext, kind: LayerKind, src: Partial<LayerSource>, patch: Partial<AmbientLayer>, name?: string): AmbientLayer {
  return makeLayer(name ?? ctx.names[kind], { ...defaultSource(kind), ...src } as LayerSource, patch);
}

function rack(ctx: StarterContext, layers: AmbientLayer[], macros: Partial<typeof DEFAULT_MACROS>): AmbientRack {
  return { ...ctx.baseRack, layers, macros: { ...DEFAULT_MACROS, ...macros }, preset: null, automation: [] };
}

function keyFacts(ctx: StarterContext): StarterFact[] {
  const f: StarterFact[] = ctx.scale ? [{ kind: "key", scale: ctx.scale }] : [{ kind: "no_key" }];
  if (ctx.register) f.push({ kind: "register", low: ctx.register.low, high: ctx.register.high });
  return f;
}

export function proposeStarters(ctx: StarterContext): Starter[] {
  const r = root(ctx);
  const tonal = r !== null;
  const pr = padRegister(ctx);
  const med = ctx.register?.median ?? 64;
  const texReg: [number, number] = [clamp(med - 24, 36, 72), clamp(med - 5, 48, 84)];
  const bellReg: [number, number] = [clamp(med - 7, 48, 84), clamp(med + 12, 60, 100)];
  const harmony = { ...DEFAULT_HARMONY, registerLow: pr.low, registerHigh: pr.high };
  const out: Starter[] = [];

  // 1. Quiet ambient: drone + pad + a breathing texture, bells far away.
  out.push({
    id: "quiet_ambient",
    timing: null,
    facts: [...keyFacts(ctx), pr.fact],
    session: {
      v: 1,
      prefs: {
        drone: { ...DEFAULT_DRONE, rootMidi: r, fifth: 0.6, octave: 0.3, sub: 0.1, warmth: 0.5, movement: 0.15, breath: 0.4, levelDb: -14 },
        pad: { ...DEFAULT_PAD, rootMidi: r, voicing: "sus2", emotion: "calm", x: 0.4, y: 0.5, levelDb: -16, lfoHz: 0.06 },
        rhythm: { ...DEFAULT_RHYTHM, drums: { ...DEFAULT_RHYTHM.drums, on: false }, bass: { ...DEFAULT_RHYTHM.bass, on: false } },
        harmony,
        masterDb: 0,
      },
      ambient: rack(
        ctx,
        [
          layer(ctx, "texture", { brightness: 0.35, motion: 0.3, resonance: 0.4, tonal, register: texReg }, { gain_db: -20, pan: -0.2 }),
          layer(ctx, "wind", { gust: 0.4, speed: 0.3, center_hz: 320, resonance: 0.3 }, { gain_db: -26, pan: 0.3 }),
          layer(ctx, "bells", { density: 2, decay_s: 12, brightness: 0.4, velocity_spread: 0.6, tonal, register: bellReg }, { gain_db: -22, pan: 0.4 }),
        ],
        { space: 0.6, space_size: 0.8, motion: 0.4, brightness: 0.5 },
      ),
      layers: { drone: tonal, pad: tonal, ambient: true, rhythm: false },
    },
  });

  // 2. Dark drone: sub and octave, dark pad, low texture, rare plucks.
  out.push({
    id: "dark_drone",
    timing: null,
    facts: [...keyFacts(ctx), pr.fact],
    session: {
      v: 1,
      prefs: {
        drone: { ...DEFAULT_DRONE, rootMidi: r, fifth: 0.3, octave: 0.5, sub: 0.5, warmth: 0.7, movement: 0.25, breath: 0.2, cutoffHz: 1200, levelDb: -12 },
        pad: { ...DEFAULT_PAD, rootMidi: r, voicing: "fifth", emotion: "dark", x: 0.3, y: 0.3, levelDb: -18, lfoHz: 0.05 },
        rhythm: { ...DEFAULT_RHYTHM, drums: { ...DEFAULT_RHYTHM.drums, on: false }, bass: { ...DEFAULT_RHYTHM.bass, on: false } },
        harmony: { ...harmony, tension: 0.45 },
        masterDb: 0,
      },
      ambient: rack(
        ctx,
        [
          layer(ctx, "noise", { color: 0.2 }, { gain_db: -30, lowpass_hz: 1500 }),
          layer(ctx, "texture", { brightness: 0.2, motion: 0.2, resonance: 0.5, tonal, register: [clamp(med - 31, 36, 60), clamp(med - 12, 44, 72)] }, { gain_db: -18, pan: 0.15 }),
          layer(ctx, "pluck", { density: 3, damping: 0.7, brightness: 0.3, velocity_spread: 0.5, tonal, register: texReg }, { gain_db: -24, pan: -0.35 }),
        ],
        { space: 0.5, space_size: 0.9, motion: 0.3, brightness: 0.3 },
      ),
      layers: { drone: tonal, pad: tonal, ambient: true, rhythm: false },
    },
  });

  // 3. Liquid DnB: rolling drums, Reese bass with a slow wobble, light pad, airy textures.
  const dnb = dnbTempo(ctx);
  const dnbTiming: Timing = { bpm: dnb.bpm, numerator: 4, unit: 4, grouping: [] };
  out.push({
    id: "liquid_dnb",
    timing: dnbTiming,
    facts: [...keyFacts(ctx), dnb.fact, pr.fact],
    session: {
      v: 1,
      prefs: {
        drone: { ...DEFAULT_DRONE, rootMidi: r, levelDb: -20 },
        pad: { ...DEFAULT_PAD, rootMidi: r, voicing: "scale_triad", emotion: "light", x: 0.6, y: 0.55, levelDb: -16, lfoHz: 0.1 },
        rhythm: {
          ...DEFAULT_RHYTHM,
          drums: { ...DEFAULT_RHYTHM.drums, on: true, style: "liquid", complexity: 0.45, humanize: 0.3, level_db: -10 },
          bass: { ...DEFAULT_RHYTHM.bass, on: true, voice: 2, density: 0.5, syncopation: 0.4, wobble_div: 8, wobble_depth: 0.4, sidechain: 0.7, level_db: -12 },
          ostinato: { ...DEFAULT_RHYTHM.ostinato, on: false },
          arp: { ...DEFAULT_RHYTHM.arp, on: false },
        },
        harmony,
        masterDb: 0,
      },
      ambient: rack(
        ctx,
        [
          layer(ctx, "texture", { brightness: 0.55, motion: 0.4, resonance: 0.3, tonal, register: [clamp(med - 12, 48, 72), clamp(med + 7, 60, 90)] }, { gain_db: -22, pan: -0.25 }),
          layer(ctx, "wind", { gust: 0.3, speed: 0.4, center_hz: 1500, resonance: 0.2 }, { gain_db: -28, pan: 0.3 }),
        ],
        { space: 0.4, space_size: 0.6, motion: 0.5, brightness: 0.6 },
      ),
      layers: { drone: false, pad: tonal, ambient: true, rhythm: true },
    },
  });

  // 4. Ambient DnB: half-time drums under a wide, slow atmosphere (the owner's "new style").
  out.push({
    id: "ambient_dnb",
    timing: dnbTiming,
    facts: [...keyFacts(ctx), dnb.fact, pr.fact],
    session: {
      v: 1,
      prefs: {
        drone: { ...DEFAULT_DRONE, rootMidi: r, fifth: 0.4, octave: 0.3, sub: 0.3, warmth: 0.6, movement: 0.2, breath: 0.3, levelDb: -16 },
        pad: { ...DEFAULT_PAD, rootMidi: r, voicing: "sus4", emotion: "calm", x: 0.45, y: 0.6, levelDb: -14, lfoHz: 0.07 },
        rhythm: {
          ...DEFAULT_RHYTHM,
          drums: { ...DEFAULT_RHYTHM.drums, on: true, style: "halftime", complexity: 0.3, humanize: 0.4, level_db: -12 },
          bass: { ...DEFAULT_RHYTHM.bass, on: true, voice: 0, density: 0.3, syncopation: 0.2, sidechain: 0.6, level_db: -12 },
          ostinato: { ...DEFAULT_RHYTHM.ostinato, on: false },
          arp: { ...DEFAULT_RHYTHM.arp, on: false },
        },
        harmony,
        masterDb: 0,
      },
      ambient: rack(
        ctx,
        [
          layer(ctx, "bells", { density: 3, decay_s: 15, brightness: 0.45, velocity_spread: 0.6, tonal, register: bellReg }, { gain_db: -22, pan: 0.35 }),
          layer(ctx, "texture", { brightness: 0.4, motion: 0.35, resonance: 0.4, tonal, register: texReg }, { gain_db: -20, pan: -0.2 }),
          layer(ctx, "wind", { gust: 0.35, speed: 0.3, center_hz: 500, resonance: 0.3 }, { gain_db: -26, pan: 0.1 }),
        ],
        { space: 0.7, space_size: 0.9, motion: 0.4, brightness: 0.45, width: 0.8 },
      ),
      layers: { drone: tonal, pad: tonal, ambient: true, rhythm: true },
    },
  });

  // 5. Ethno pulse: the project's meter (or a suggested 7/8), plucked bass, ostinato.
  const hasMeter = ctx.timing.numerator !== null && ctx.timing.unit !== null;
  const bpm = ctx.detectedBpm ?? ctx.timing.bpm;
  const ethnoTiming: Timing = hasMeter
    ? { ...ctx.timing, bpm: bpm ?? ctx.timing.bpm }
    : { bpm: bpm ?? 120, numerator: 7, unit: 8, grouping: [3, 2, 2] };
  const ethnoFacts: StarterFact[] = [...keyFacts(ctx)];
  if (bpm !== null) ethnoFacts.push({ kind: ctx.detectedBpm !== null ? "tempo_detected" : "tempo_project", bpm: Math.round(bpm) });
  else ethnoFacts.push({ kind: "tempo_suggested", bpm: 120 });
  ethnoFacts.push(hasMeter ? { kind: "meter_project", numerator: ethnoTiming.numerator!, unit: ethnoTiming.unit!, grouping: ethnoTiming.grouping } : { kind: "meter_suggested", numerator: 7, unit: 8, grouping: [3, 2, 2] });
  ethnoFacts.push(pr.fact);
  out.push({
    id: "ethno_pulse",
    timing: ethnoTiming,
    facts: ethnoFacts,
    session: {
      v: 1,
      prefs: {
        drone: { ...DEFAULT_DRONE, rootMidi: r, fifth: 0.6, octave: 0.2, warmth: 0.45, movement: 0.2, breath: 0.3, levelDb: -14 },
        pad: { ...DEFAULT_PAD, rootMidi: r, voicing: "scale_triad", emotion: "calm", x: 0.5, y: 0.5, levelDb: -16 },
        rhythm: {
          ...DEFAULT_RHYTHM,
          drums: { ...DEFAULT_RHYTHM.drums, on: true, style: "ethno", complexity: 0.5, humanize: 0.35, level_db: -12 },
          bass: { ...DEFAULT_RHYTHM.bass, on: true, voice: 1, density: 0.45, syncopation: 0.3, passing: 0.4, sidechain: 0.3, level_db: -13 },
          ostinato: { ...DEFAULT_RHYTHM.ostinato, on: true, steps: 8, step_beats: 0.5, density: 0.6, timbre: 2, level_db: -16, register_low: clamp(med - 12, 48, 72), register_high: clamp(med + 12, 60, 96) },
          arp: { ...DEFAULT_RHYTHM.arp, on: false },
        },
        harmony,
        masterDb: 0,
      },
      ambient: rack(
        ctx,
        [
          layer(ctx, "pluck", { density: 6, damping: 0.5, brightness: 0.5, velocity_spread: 0.5, tonal, register: bellReg }, { gain_db: -24, pan: 0.3 }),
          layer(ctx, "wind", { gust: 0.3, speed: 0.35, center_hz: 400, resonance: 0.25 }, { gain_db: -28, pan: -0.3 }),
        ],
        { space: 0.35, space_size: 0.5, motion: 0.4, brightness: 0.5 },
      ),
      layers: { drone: tonal, pad: tonal, ambient: true, rhythm: true },
    },
  });

  return out;
}
