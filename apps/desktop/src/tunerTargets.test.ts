import { describe, expect, it } from "vitest";
import {
  BASS_MIN_HZ,
  clampA4,
  CONF_ENTER,
  CONF_STAY,
  HOLD_MS,
  holdTone,
  lockRange,
  midiToHz,
  monitorMinHz,
  needleAngle,
  needsLock,
  readTuner,
  smooth,
  smoothPitch,
  targetsFor,
} from "./tunerTargets";

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

describe("monitorMinHz (bass range for the sound bed)", () => {
  it("the monochord bed always listens from 30 Hz; its lock range covers the owner's 34–40 Hz", () => {
    expect(monitorMinHz("monochord", null, 440)).toBe(BASS_MIN_HZ);
    expect(monitorMinHz("monochord", 26, 440)).toBe(BASS_MIN_HZ);
    const [lo] = lockRange("monochord");
    expect(midiToHz(lo, 440)).toBeLessThan(34);
  });
  it("other instruments use the engine's default unless a low note is locked", () => {
    expect(monitorMinHz("kaval-d", null, 440)).toBeNull();
    expect(monitorMinHz("guitar", 40, 440)).toBe(BASS_MIN_HZ); // low E2 = 82 Hz: under 100, the longer window helps
    expect(monitorMinHz("guitar", 50, 440)).toBeNull(); // D3 = 147 Hz
    expect(monitorMinHz("guitar", null, 440)).toBeNull();
    expect(monitorMinHz("free", 26, 440)).toBe(BASS_MIN_HZ); // D1 = 36.7 Hz
    expect(monitorMinHz("free", 26, 432)).toBe(BASS_MIN_HZ);
  });
  it("midiToHz follows the chosen A4", () => {
    expect(midiToHz(69, 440)).toBe(440);
    expect(midiToHz(26, 440)).toBeCloseTo(36.71, 1);
    expect(midiToHz(26, 432)).toBeCloseTo(36.04, 1);
  });
});

describe("holdTone (no blinking)", () => {
  it("keeps the last tone for HOLD_MS after a dropped frame, dimmed, then lets go", () => {
    const t0 = holdTone(null, 440, 0.9, 1000);
    expect(t0).toEqual({ hz: 440, confidence: 0.9, at: 1000, live: true });
    const t1 = holdTone(t0, null, 0, 1050);
    expect(t1).toEqual({ hz: 440, confidence: 0.9, at: 1000, live: false });
    const t2 = holdTone(t1, null, 0, 1000 + HOLD_MS - 1);
    expect(t2?.hz).toBe(440);
    expect(holdTone(t2, null, 0, 1000 + HOLD_MS)).toBeNull();
  });
  it("enters at CONF_ENTER and stays down to CONF_STAY (hysteresis)", () => {
    expect(holdTone(null, 440, CONF_ENTER - 0.01, 0)).toBeNull();
    const live = holdTone(null, 440, CONF_ENTER, 0);
    expect(live?.live).toBe(true);
    const still = holdTone(live, 441, CONF_STAY + 0.01, 50);
    expect(still).toEqual({ hz: 441, confidence: CONF_STAY + 0.01, at: 50, live: true });
    const dropped = holdTone(still, 441, CONF_STAY - 0.01, 100);
    expect(dropped?.live).toBe(false);
    expect(dropped?.hz).toBe(441);
  });
  it("a new tone replaces a held one at once", () => {
    const held = holdTone(holdTone(null, 440, 0.9, 0), null, 0, 50);
    expect(holdTone(held, 330, 0.8, 100)).toEqual({ hz: 330, confidence: 0.8, at: 100, live: true });
  });
});

describe("smoothPitch", () => {
  it("filters small motion but snaps on a jump to another note", () => {
    expect(smoothPitch(null, 69.2)).toBe(69.2);
    expect(smoothPitch(69.0, 69.2)).toBeCloseTo(69.07, 2);
    expect(smoothPitch(69.0, 71.0)).toBe(71.0);
    expect(smoothPitch(69.0, 57.0)).toBe(57.0);
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
