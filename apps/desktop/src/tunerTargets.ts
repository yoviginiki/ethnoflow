// Tuner targets per instrument (pure; tested in tunerTargets.test.ts).
//
// A tuner shows how far the sound is from a *target*. The target set depends on what
// is being tuned: a guitar has six strings, a shakuhachi five holes (plus the upper
// octave), a kaval plays the chromatic scale across its registers, a monochord bed has
// many strings on one note the owner chooses. "Free" is a chromatic tuner.

export type TunerInstrument = "free" | "kaval-d" | "shakuhachi-d" | "guitar" | "monochord";
export const TUNER_INSTRUMENTS: TunerInstrument[] = ["free", "kaval-d", "shakuhachi-d", "guitar", "monochord"];

/** Reference pitches offered as one-click choices (Hz). 440 is the standard; 432 is a common alternative. */
export const A4_CHOICES = [440, 432] as const;
export const A4_MIN = 400;
export const A4_MAX = 480;

/** The green zone of the gauge: within this the needle reads "in tune" (cents). */
export const IN_TUNE_CENTS = 5;
/** The gauge spans ±GAUGE_CENTS. */
export const GAUGE_CENTS = 50;

export interface TunerTarget {
  midi: number;
  /** Instrument-specific name: "6 · E2" for a guitar string, "ロ Ro" for a shakuhachi hole; null for plain notes. */
  label: string | null;
}

/** Standard guitar tuning, string 6 (low E2) to string 1 (high E4). */
const GUITAR_STANDARD: readonly number[] = [40, 45, 50, 55, 59, 64];
/** Shakuhachi 1.8 (D): otsu (lower) register holes, then kan (upper octave). */
const SHAKUHACHI_D: readonly { midi: number; name: string }[] = [
  { midi: 62, name: "ロ Ro" },
  { midi: 65, name: "ツ Tsu" },
  { midi: 67, name: "レ Re" },
  { midi: 69, name: "チ Chi" },
  { midi: 72, name: "リ Ri" },
  { midi: 74, name: "ロ Ro (kan)" },
  { midi: 77, name: "ツ Tsu (kan)" },
  { midi: 79, name: "レ Re (kan)" },
  { midi: 81, name: "チ Chi (kan)" },
  { midi: 84, name: "リ Ri (kan)" },
];
/** Kaval in D: the three registers used in practice, D4..A6, every chromatic note. */
const KAVAL_D_RANGE: readonly [number, number] = [62, 93];

/** Target notes an instrument can be tuned to; empty means "any chromatic note". */
export function targetsFor(inst: TunerInstrument, lockMidi: number | null): TunerTarget[] {
  if (lockMidi !== null) return [{ midi: lockMidi, label: null }];
  switch (inst) {
    case "guitar":
      return GUITAR_STANDARD.map((m, i) => ({ midi: m, label: `${6 - i} · ${noteLabel(m)}` }));
    case "shakuhachi-d":
      return SHAKUHACHI_D.map((h) => ({ midi: h.midi, label: h.name }));
    case "kaval-d": {
      const out: TunerTarget[] = [];
      for (let m = KAVAL_D_RANGE[0]; m <= KAVAL_D_RANGE[1]; m++) out.push({ midi: m, label: null });
      return out;
    }
    case "monochord":
    case "free":
      return [];
  }
}

/** Does this instrument need the user to pick the note (a bed of strings on one tone)? */
export function needsLock(inst: TunerInstrument): boolean {
  return inst === "monochord";
}

/** MIDI range offered in the "lock" note picker for an instrument (inclusive). */
export function lockRange(inst: TunerInstrument): [number, number] {
  switch (inst) {
    case "guitar":
      return [36, 76];
    case "shakuhachi-d":
      return [57, 88];
    case "kaval-d":
      return [57, 96];
    case "monochord":
      return [24, 72];
    case "free":
      return [24, 108];
  }
}

export interface TunerReading {
  target: TunerTarget;
  /** Signed distance from the target in cents (+ = sharp). */
  cents: number;
  /** -1 too low (go up), 0 in tune, +1 too high (go down). */
  direction: -1 | 0 | 1;
}

/**
 * Nearest target to a measured pitch (fractional MIDI), with the signed cents and the
 * direction to correct. With no targets the nearest chromatic note is used.
 */
export function readTuner(midiFrac: number, targets: TunerTarget[], inTuneCents = IN_TUNE_CENTS): TunerReading {
  let best: TunerTarget | null = null;
  let bestCents = 0;
  if (targets.length === 0) {
    const m = Math.round(midiFrac);
    best = { midi: m, label: null };
    bestCents = (midiFrac - m) * 100;
  } else {
    for (const tg of targets) {
      const c = (midiFrac - tg.midi) * 100;
      if (best === null || Math.abs(c) < Math.abs(bestCents)) {
        best = tg;
        bestCents = c;
      }
    }
  }
  const direction: -1 | 0 | 1 = Math.abs(bestCents) <= inTuneCents ? 0 : bestCents < 0 ? -1 : 1;
  return { target: best!, cents: bestCents, direction };
}

/** Needle angle in degrees for a cents reading: ±GAUGE_CENTS → ±maxDeg, clamped. */
export function needleAngle(cents: number, maxDeg = 45): number {
  const c = Math.max(-GAUGE_CENTS, Math.min(GAUGE_CENTS, cents));
  return (c / GAUGE_CENTS) * maxDeg;
}

/** Exponential smoothing for the needle (alpha per 50 ms tick). */
export function smooth(prev: number | null, next: number, alpha = 0.35): number {
  return prev === null ? next : prev + (next - prev) * alpha;
}

const NAMES_EN = ["C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B"];
function noteLabel(midi: number): string {
  return `${NAMES_EN[((midi % 12) + 12) % 12]}${Math.floor(midi / 12) - 1}`;
}

/** Clamp a typed reference pitch into the supported range; NaN → fallback. */
export function clampA4(v: number, fallback = 440): number {
  if (!Number.isFinite(v)) return fallback;
  return Math.max(A4_MIN, Math.min(A4_MAX, Math.round(v * 10) / 10));
}
