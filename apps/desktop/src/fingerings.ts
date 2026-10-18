import kavalD from "../../../packages/lesson-content/fingerings/kaval-d.json";

/** One fingering: 8 holes in the order T 2 3 4 | 5 6 7 8 (c closed, o open, x optional, h half). */
export interface Fingering {
  midi: number;
  register: number;
  holes: string;
  alt?: boolean;
  note?: { bg: string; en: string } | null;
}

export interface FingeringChart {
  instrument: string;
  name: { bg: string; en: string };
  holes: string[];
  legend: Record<string, { bg: string; en: string }>;
  registers: Record<string, { bg: string; en: string }>;
  fingerings: Fingering[];
}

export const KAVAL_D: FingeringChart = kavalD as FingeringChart;

/**
 * The user's corrections on top of a chart: an override replaces every factory entry of
 * the same note and register (so one corrected primary fingering is enough), keeps the
 * other notes, and is sorted by note. The factory file never changes.
 */
export function applyOverrides(chart: FingeringChart, overrides: Fingering[] | undefined): FingeringChart {
  if (!overrides || overrides.length === 0) return chart;
  const replaced = new Set(overrides.map((o) => `${o.midi}:${o.register}`));
  const kept = chart.fingerings.filter((f) => !replaced.has(`${f.midi}:${f.register}`));
  const fingerings = [...kept, ...overrides].sort((a, b) => a.midi - b.midi || a.register - b.register || Number(!!a.alt) - Number(!!b.alt));
  return { ...chart, fingerings };
}

/** Chart id by profile instrument type (F2.6); only the kaval in D exists so far. */
export function chartIdFor(instrument: string | null | undefined): string | null {
  return instrument === "kaval" ? KAVAL_D.instrument : null;
}

/** Charts by profile instrument type, with the user's overrides applied when given. */
export function chartFor(instrument: string | null | undefined, overrides?: Record<string, Fingering[]>): FingeringChart | null {
  const id = chartIdFor(instrument);
  if (!id) return null;
  return applyOverrides(KAVAL_D, overrides?.[id]);
}

/** Primary fingering(s) for a MIDI note: the non-alternative one first, then alternatives. */
export function fingeringsFor(chart: FingeringChart, midi: number): Fingering[] {
  const all = chart.fingerings.filter((f) => f.midi === midi);
  return [...all.filter((f) => !f.alt), ...all.filter((f) => f.alt)];
}

/** Nearest note that has a fingering (ties go down), with its distance in semitones. */
export function nearestFingered(chart: FingeringChart, midiFrac: number): { midi: number; distance: number } | null {
  let best: { midi: number; distance: number } | null = null;
  for (const f of chart.fingerings) {
    const d = Math.abs(f.midi - midiFrac);
    if (!best || d < best.distance - 1e-9) best = { midi: f.midi, distance: d };
  }
  return best;
}

export const HOLE_RE = /^[cohx]{8}$/;
