import { describe, expect, it } from "vitest";
import { codeForKey, midiForCode } from "./keyboard";

describe("keyboard everywhere", () => {
  it("maps the two computer rows to two octaves", () => {
    expect(codeForKey("z")).toBe(0);
    expect(codeForKey("M")).toBe(11);
    expect(codeForKey("q")).toBe(12);
    expect(codeForKey("p")).toBe(28);
    expect(codeForKey("ArrowLeft")).toBeNull();
  });
  it("snaps to the scale only when locked and a scale is known", () => {
    const dMinor = { tonic_pc: 2, degrees_cents: [0, 200, 300, 500, 700, 800, 1000], kind: null };
    expect(midiForCode(60, 1, null, true)).toBe(61);
    expect(midiForCode(60, 1, dMinor, false)).toBe(61);
    // C#4 is not in D minor: the nearest degree is D4 (or C4); never a note outside the mode.
    const snapped = midiForCode(60, 1, dMinor, true);
    expect([60, 62]).toContain(snapped);
    expect(midiForCode(60, 2, dMinor, true)).toBe(62);
  });
});
