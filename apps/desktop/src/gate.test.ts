import { describe, expect, it } from "vitest";
import type { Note } from "./api";
import { gateVerdict, glide, pitchRange, timeWindow, toleranceFor, xOfTime, yOfMidi } from "./gate";

const n = (midi: number, start: number, end: number): Note => ({ midi, start, end, cents: 0, confidence: 1, velocity: 0.8, bend: [] });

describe("gate geometry", () => {
  it("pitch range pads the notes and is at least an octave", () => {
    // 62..64 padded by 4 = 58..68 (10 semitones) → widened around 63 to a full octave.
    expect(pitchRange([n(62, 0, 1), n(64, 1, 2)])).toEqual([57, 69]);
    expect(pitchRange([n(62, 0, 1), n(81, 1, 2)])).toEqual([58, 85]);
    expect(pitchRange([])).toEqual([55, 79]);
  });
  it("higher pitch is higher on screen (smaller y)", () => {
    const r: [number, number] = [50, 70];
    expect(yOfMidi(50, r, 200)).toBe(200);
    expect(yOfMidi(70, r, 200)).toBe(0);
    expect(yOfMidi(60, r, 200)).toBe(100);
  });
  it("time maps left to right in the window", () => {
    const w = timeWindow(10);
    expect(w).toEqual([8.5, 14]);
    expect(xOfTime(8.5, w, 550)).toBe(0);
    expect(xOfTime(10, w, 550)).toBeCloseTo(150, 6);
  });
  it("glide moves towards the target and snaps when close", () => {
    expect(glide(0, 10, 0.05)).toBeCloseTo(3, 6);
    expect(glide(9.9995, 10, 0.05)).toBe(10);
    expect(glide(0, 10, 1)).toBe(10);
  });
});

describe("gate verdict", () => {
  it("silent without a pitch or a target", () => {
    expect(gateVerdict(null, [62], 30).verdict).toBe("silent");
    expect(gateVerdict(62, [], 30).verdict).toBe("silent");
  });
  it("passes inside the opening, blocked high or low outside, with the signed cents", () => {
    const pass = gateVerdict(62.2, [62], 30);
    expect(pass.verdict).toBe("pass");
    expect(pass.cents).toBeCloseTo(20, 6);
    expect(gateVerdict(62.45, [62], 30).verdict).toBe("high");
    const low = gateVerdict(61.5, [62], 30);
    expect(low.verdict).toBe("low");
    expect(low.cents).toBeCloseTo(-50, 6);
  });
  it("a chord target passes on its nearest note", () => {
    expect(gateVerdict(67.1, [64, 67], 20).verdict).toBe("pass");
  });
  it("tolerance follows the trainer levels", () => {
    expect(toleranceFor("beginner")).toBe(30);
    expect(toleranceFor("advanced")).toBe(10);
  });
});
