import kavalD from "../../../packages/lesson-content/fingerings/kaval-d.json";

/** One fingering: 8 holes in the order T 2 3 4 | 5 6 7 8 (c closed, o open, x optional, h half). */
export interface Fingering {
  midi: number;
  register: number;
  holes: string;
  alt?: boolean;
  note?: { bg: string; en: string };
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

/** Charts by profile instrument type (F2.6); only the kaval in D exists so far. */
export function chartFor(instrument: string | null | undefined): FingeringChart | null {
  return instrument === "kaval" ? KAVAL_D : null;
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
