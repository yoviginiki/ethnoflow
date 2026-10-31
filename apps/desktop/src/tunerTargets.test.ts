import { describe, expect, it } from "vitest";
import { clampA4, lockRange, needleAngle, needsLock, readTuner, smooth, targetsFor } from "./tunerTargets";

describe("tuner targets", () => {
  it("guitar has six strings low E2 to high E4, labelled by string number", () => {
    const g = targetsFor("guitar", null);
    expect(g.map((t) => t.midi)).toEqual([40, 45, 50, 55, 59, 64]);
    expect(g[0].label).toBe("6 · E2");
    expect(g[5].label).toBe("1 · E4");
  });
  it("shakuhachi in D starts on Ro = D4 and has the kan octave", () => {
    const s = targetsFor("shakuhachi-d", null);
    expect(s[0]).toEqual({ midi: 62, label: "ロ Ro" });
    expect(s.find((t) => t.midi === 74)?.label).toContain("kan");
  });
  it("kaval in D is chromatic D4..A6", () => {
    const k = targetsFor("kaval-d", null);
    expect(k[0].midi).toBe(62);
    expect(k[k.length - 1].midi).toBe(93);
    expect(k.length).toBe(32);
  });
  it("free and monochord have no fixed targets; monochord asks for a lock", () => {
    expect(targetsFor("free", null)).toEqual([]);
    expect(targetsFor("monochord", null)).toEqual([]);
    expect(needsLock("monochord")).toBe(true);
    expect(needsLock("guitar")).toBe(false);
  });
  it("a locked note replaces the instrument's set", () => {
    expect(targetsFor("guitar", 50)).toEqual([{ midi: 50, label: null }]);
    const [lo, hi] = lockRange("monochord");
    expect(lo).toBeLessThan(hi);
  });
});

describe("readTuner", () => {
  it("picks the nearest string and signs the cents", () => {
    // 10 cents sharp of A2 (45).
    const r = readTuner(45.1, targetsFor("guitar", null));
    expect(r.target.midi).toBe(45);
    expect(r.cents).toBeCloseTo(10, 6);
    expect(r.direction).toBe(1);
  });
  it("in tune within ±5 cents, too low below", () => {
    expect(readTuner(62.03, targetsFor("kaval-d", null)).direction).toBe(0);
    expect(readTuner(61.8, targetsFor("kaval-d", null)).direction).toBe(-1);
  });
  it("without targets the nearest chromatic note is the target, even far from an instrument's range", () => {
    const r = readTuner(30.4, []);
    expect(r.target.midi).toBe(30);
    expect(r.cents).toBeCloseTo(40, 6);
  });
  it("a shakuhachi reading names the hole", () => {
    expect(readTuner(69.02, targetsFor("shakuhachi-d", null)).target.label).toBe("チ Chi");
  });
});

describe("gauge helpers", () => {
  it("needle angle is linear and clamped at ±50 cents", () => {
    expect(needleAngle(0)).toBe(0);
    expect(needleAngle(25)).toBe(22.5);
    expect(needleAngle(-80)).toBe(-45);
  });
  it("smoothing starts at the first value and moves part of the way", () => {
    expect(smooth(null, 10)).toBe(10);
    expect(smooth(0, 10, 0.5)).toBe(5);
  });
  it("A4 is clamped to 400..480 and NaN falls back", () => {
    expect(clampA4(432)).toBe(432);
    expect(clampA4(10)).toBe(400);
    expect(clampA4(Number.NaN)).toBe(440);
    expect(clampA4(441.26)).toBe(441.3);
  });
});
