// Chord progressions by genre with a generator and hand editing (owner 1.11.2026: "искам
// различните прогресии, които да идват от drum секцията… RnB, rock, DnB, liquid, ambient…
// но прогресиите да имат генератор"). The progression lives in the session; a follower
// (layers.ts) pushes the chord of the current bar into the pad, the drone root and the
// rhythm section while the song transport or the rhythm runs. Pure functions here: the
// generator is deterministic for a seed, chords come from the project's scale degrees,
// never from an assumed key.
import type { PadNote, Scale } from "./api";
import type { StringMessageKey } from "./i18n";

export type Genre = "rnb" | "rock" | "pop" | "dnb" | "liquid" | "ambient" | "ethno" | "jazz";
export const GENRES: Genre[] = ["rnb", "rock", "pop", "dnb", "liquid", "ambient", "ethno", "jazz"];

export type ChordQuality = "triad" | "power" | "sus2" | "sus4" | "7" | "maj7" | "9" | "add9";
export const QUALITIES: ChordQuality[] = ["triad", "power", "sus2", "sus4", "7", "maj7", "9", "add9"];

export interface ProgStep {
  /** Scale degree 1..7 (1 = the tonic of the project's scale). */
  degree: number;
  quality: ChordQuality;
  /** How many bars this chord holds. */
  bars: number;
}

export interface Progression {
  genre: Genre;
  seed: number;
  /** Whole length in bars (the steps repeat to fill it). */
  bars: number;
  steps: ProgStep[];
  /** The follower pushes the chord of the current bar into pad / drone / rhythm. */
  follow: boolean;
  /** True once the user edited a step by hand (a new generation asks before replacing). */
  edited: boolean;
}

export const DEFAULT_PROGRESSION: Progression = { genre: "liquid", seed: 1, bars: 8, steps: [], follow: false, edited: false };

export interface GenreSpec {
  id: Genre;
  title: StringMessageKey;
  desc: StringMessageKey;
  /** Bars per chord by default. */
  barsPerChord: number;
  /** Candidate progressions as degree lists; the seed picks one and may substitute. */
  patterns: number[][];
  /** Chord qualities the genre likes, by weight (the seed picks per step). */
  qualities: [ChordQuality, number][];
  /** Drum style of the rhythm section that fits (set when "also set the drums" is on). */
  drumStyle: "liquid" | "halftime" | "ethno";
  /** Suggested rhythm bass figure density 0..1 and tempo hint for the user. */
  tempoHint: [number, number];
}

export const GENRE_SPECS: GenreSpec[] = [
  { id: "rnb", title: "gRnb", desc: "gRnbDesc", barsPerChord: 2, patterns: [[2, 5, 1, 6], [1, 6, 2, 5], [4, 3, 6, 2], [1, 3, 4, 5]], qualities: [["7", 3], ["maj7", 3], ["9", 2], ["add9", 1]], drumStyle: "halftime", tempoHint: [70, 95] },
  { id: "rock", title: "gRock", desc: "gRockDesc", barsPerChord: 1, patterns: [[1, 5, 6, 4], [1, 4, 5, 5], [6, 4, 1, 5], [1, 7, 4, 1]], qualities: [["power", 4], ["triad", 2], ["sus4", 1]], drumStyle: "liquid", tempoHint: [110, 140] },
  { id: "pop", title: "gPop", desc: "gPopDesc", barsPerChord: 1, patterns: [[1, 5, 6, 4], [6, 4, 1, 5], [1, 6, 4, 5], [4, 1, 5, 6]], qualities: [["triad", 4], ["add9", 2], ["sus2", 1]], drumStyle: "liquid", tempoHint: [95, 125] },
  { id: "dnb", title: "gDnb", desc: "gDnbDesc", barsPerChord: 4, patterns: [[1, 6], [1, 7], [1, 3], [1, 4]], qualities: [["7", 2], ["triad", 2], ["sus2", 1]], drumStyle: "liquid", tempoHint: [168, 176] },
  { id: "liquid", title: "gLiquid", desc: "gLiquidDesc", barsPerChord: 2, patterns: [[1, 4, 7, 6], [1, 6, 4, 5], [2, 1, 6, 7], [1, 3, 6, 4]], qualities: [["maj7", 3], ["7", 3], ["9", 2], ["add9", 1]], drumStyle: "liquid", tempoHint: [170, 176] },
  { id: "ambient", title: "gAmbient", desc: "gAmbientDesc", barsPerChord: 8, patterns: [[1], [1, 4], [1, 6], [1, 2]], qualities: [["sus2", 3], ["add9", 3], ["maj7", 1], ["triad", 1]], drumStyle: "halftime", tempoHint: [60, 80] },
  { id: "ethno", title: "gEthno", desc: "gEthnoDesc", barsPerChord: 2, patterns: [[1, 7, 6, 7], [1, 7, 1, 2], [1, 4, 7, 1], [1, 2, 1, 7]], qualities: [["triad", 3], ["power", 2], ["sus4", 2]], drumStyle: "ethno", tempoHint: [100, 160] },
  { id: "jazz", title: "gJazz", desc: "gJazzDesc", barsPerChord: 1, patterns: [[2, 5, 1, 1], [1, 6, 2, 5], [3, 6, 2, 5], [2, 5, 1, 4]], qualities: [["7", 3], ["maj7", 3], ["9", 3]], drumStyle: "halftime", tempoHint: [90, 160] },
];

export function genreSpec(id: string): GenreSpec | null {
  return GENRE_SPECS.find((g) => g.id === id) ?? null;
}

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

function pickWeighted<T>(items: [T, number][], r: number): T {
  const total = items.reduce((s, [, w]) => s + w, 0);
  let x = r * total;
  for (const [it, w] of items) {
    x -= w;
    if (x <= 0) return it;
  }
  return items[items.length - 1][0];
}

/** Generate a progression for a genre and seed; `bars` is the whole length. */
export function generateProgression(genre: Genre, seed: number, bars: number, barsPerChord?: number): Progression {
  const spec = genreSpec(genre) ?? GENRE_SPECS[0];
  const next = rng(seed);
  const pattern = spec.patterns[Math.floor(next() * spec.patterns.length)];
  const per = Math.max(1, barsPerChord ?? spec.barsPerChord);
  const total = Math.max(1, Math.min(64, bars));
  const steps: ProgStep[] = [];
  let at = 0;
  let i = 0;
  while (at < total) {
    let degree = pattern[i % pattern.length];
    // A small chance to substitute a relative (vi ↔ IV, ii ↔ IV) keeps repeats alive.
    const sub = next();
    if (sub < 0.12 && degree === 6) degree = 4;
    else if (sub < 0.2 && degree === 4 && spec.id !== "ethno") degree = 2;
    const quality = pickWeighted(spec.qualities, next());
    const len = Math.min(per, total - at);
    steps.push({ degree, quality, bars: len });
    at += len;
    i++;
  }
  return { genre: spec.id, seed, bars: total, steps, follow: false, edited: false };
}

/** The step (and its index) that sounds in `bar` (0-based); the progression repeats. */
export function stepAtBar(p: Progression, bar: number): { index: number; step: ProgStep; barInStep: number } | null {
  if (p.steps.length === 0) return null;
  const total = p.steps.reduce((s, st) => s + st.bars, 0);
  if (total <= 0) return null;
  let b = ((bar % total) + total) % total;
  for (let i = 0; i < p.steps.length; i++) {
    if (b < p.steps[i].bars) return { index: i, step: p.steps[i], barInStep: b };
    b -= p.steps[i].bars;
  }
  return null;
}

/** Scale degree (1-based, any octave) → cents above the tonic, octave-wrapped. */
function degreeCents(scale: Scale, degree1: number): number {
  const n = scale.degrees_cents.length;
  const idx = ((degree1 - 1) % n + n) % n;
  const oct = Math.floor((degree1 - 1) / n);
  return scale.degrees_cents[idx] + oct * 1200;
}

/** Chord tones of a step in the project's scale, as pad notes around `rootMidiBase` (default C3 + tonic). */
export function chordNotes(scale: Scale, step: ProgStep, rootMidiBase?: number): PadNote[] {
  const base = rootMidiBase ?? 48 + scale.tonic_pc;
  const d = step.degree;
  // Stacked scale thirds: degree, +2, +4 (+6 = 7th, +8 = 9th); sus / power / add9 replace members.
  let members: number[];
  switch (step.quality) {
    case "power":
      members = [d, d + 4, d + 7];
      break;
    case "sus2":
      members = [d, d + 1, d + 4];
      break;
    case "sus4":
      members = [d, d + 3, d + 4];
      break;
    case "7":
    case "maj7":
      members = [d, d + 2, d + 4, d + 6];
      break;
    case "9":
      members = [d, d + 2, d + 4, d + 6, d + 8];
      break;
    case "add9":
      members = [d, d + 2, d + 4, d + 8];
      break;
    default:
      members = [d, d + 2, d + 4];
  }
  const rootCents = degreeCents(scale, d);
  return members.map((m) => {
    const cents = degreeCents(scale, m) - rootCents;
    const semis = Math.round(cents / 100);
    return { midi: base + Math.round(rootCents / 100) + semis, cents: Math.round(cents - semis * 100) };
  });
}

/** Root of a step as MIDI (for the drone and the rhythm's root), near C3 + tonic. */
export function chordRootMidi(scale: Scale, step: ProgStep, rootMidiBase?: number): number {
  const base = rootMidiBase ?? 48 + scale.tonic_pc;
  return base + Math.round(degreeCents(scale, step.degree) / 100);
}

/** Roman-numeral style label of a step in the scale (I, ii, bVII…), with the quality. */
export function stepLabel(scale: Scale | null, step: ProgStep): string {
  const roman = ["I", "II", "III", "IV", "V", "VI", "VII"][((step.degree - 1) % 7 + 7) % 7] ?? String(step.degree);
  const suffix = { triad: "", power: "5", sus2: "sus2", sus4: "sus4", "7": "7", maj7: "maj7", "9": "9", add9: "add9" }[step.quality];
  if (!scale) return `${roman}${suffix}`;
  // Minor-ish third (≤ 350 c above the chord root) → lower-case numeral.
  const n = scale.degrees_cents.length;
  const third = degreeCents(scale, step.degree + 2) - degreeCents(scale, step.degree);
  const minorish = n >= 5 && third <= 350;
  return `${minorish ? roman.toLowerCase() : roman}${suffix}`;
}

export function totalBars(p: Progression): number {
  return p.steps.reduce((s, st) => s + st.bars, 0);
}
