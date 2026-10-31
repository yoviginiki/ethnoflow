// "Gate" lesson game geometry and verdicts (pure; tested in gate.test.ts).
//
// Time runs left → right, pitch bottom → top. Every expected note is a wall at its
// start with an opening at its pitch; the ball (the player's live pitch) passes only
// through the opening. Too high or too low: the ball is blocked and the song waits.

import type { Note } from "./api";

export type GateVerdict = "silent" | "pass" | "high" | "low";

/** Pitch range to draw, in MIDI, padded so the lowest and highest notes are inside. */
export function pitchRange(notes: Note[], pad = 4): [number, number] {
  if (notes.length === 0) return [55, 79];
  let lo = Infinity;
  let hi = -Infinity;
  for (const n of notes) {
    if (n.midi < lo) lo = n.midi;
    if (n.midi > hi) hi = n.midi;
  }
  lo -= pad;
  hi += pad;
  if (hi - lo < 12) {
    const mid = (lo + hi) / 2;
    lo = mid - 6;
    hi = mid + 6;
  }
  return [lo, hi];
}

/** Vertical position (0 = top, height = bottom) of a fractional MIDI pitch. */
export function yOfMidi(midiFrac: number, range: [number, number], height: number): number {
  const [lo, hi] = range;
  const f = (midiFrac - lo) / (hi - lo);
  return height - f * height;
}

/**
 * Where is the ball relative to the target's opening? `tolCents` is the half-height of
 * the opening (the trainer's level tolerance). A chord target passes on any of its notes.
 */
export function gateVerdict(midiFrac: number | null, targetMidis: number[], tolCents: number): { verdict: GateVerdict; cents: number | null } {
  if (midiFrac === null || targetMidis.length === 0) return { verdict: "silent", cents: null };
  let best = Infinity;
  for (const m of targetMidis) {
    const c = (midiFrac - m) * 100;
    if (Math.abs(c) < Math.abs(best)) best = c;
  }
  if (Math.abs(best) <= tolCents) return { verdict: "pass", cents: best };
  return { verdict: best > 0 ? "high" : "low", cents: best };
}

/** Visible time window around the ball: `before` seconds to its left, `after` to its right. */
export function timeWindow(position: number, before = 1.5, after = 4): [number, number] {
  return [position - before, position + after];
}

export function xOfTime(t: number, window: [number, number], width: number): number {
  const [a, b] = window;
  return ((t - a) / (b - a)) * width;
}

/** Glide the drawn position towards the target position (per animation frame). */
export function glide(current: number, target: number, dtSeconds: number, speed = 6): number {
  const k = Math.min(1, dtSeconds * speed);
  const next = current + (target - current) * k;
  return Math.abs(target - next) < 1e-3 ? target : next;
}

/** Trainer level → half-height of the opening in cents (mirrors TrainerPanel). */
export function toleranceFor(level: "beginner" | "intermediate" | "advanced"): number {
  return level === "beginner" ? 30 : level === "intermediate" ? 20 : 10;
}
