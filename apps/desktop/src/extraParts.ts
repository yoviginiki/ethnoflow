import type { ExtraKind, ExtraPart } from "./api";

/** Default factory preset per Instrument Rack kind (P1 R3). */
export const EXTRA_DEFAULT_PRESET: Record<ExtraKind, string> = { tapan: "factory:tapan.folk", chord_pad: "factory:chordpad.warm", lead: "factory:lead.kaval" };

/** A new rack part with musical defaults per kind (registers: pad low-mid, lead high). */
export function newExtraPart(kind: ExtraKind, existing: ExtraPart[]): ExtraPart {
  let n = 1;
  while (existing.some((e) => e.id === `x${n}`)) n++;
  const base: ExtraPart = { id: `x${n}`, kind, on: true, level_db: -14, instrument: EXTRA_DEFAULT_PRESET[kind], density: 0.5, humanize: 0.2, fill_every_bars: 4, gap: 0.1, bars_per_chord: 1, step_beats: 0.5, phrase_bars: 2, leaps: 0.2, register_low: 60, register_high: 84, seed: 1 + Math.floor(Math.random() * 1_000_000), locked: false };
  if (kind === "chord_pad") return { ...base, register_low: 48, register_high: 67, level_db: -18 };
  if (kind === "lead") return { ...base, register_low: 62, register_high: 86, density: 0.55, level_db: -16 };
  return base;
}
