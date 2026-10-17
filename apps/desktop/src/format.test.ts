import { describe, expect, it } from "vitest";
import {
  centsFromScale,
  formatBytes,
  formatDb,
  formatGrouping,
  formatTime,
  hzToMidi,
  isTypingTarget,
  linearToDb,
  meterFraction,
  midiToHz,
  noteParts,
  parseGrouping,
  snapToBars,
  takeName,
} from "./format";

describe("grouping", () => {
  it("parses additive groupings", () => {
    expect(parseGrouping("2+2+3")).toEqual([2, 2, 3]);
    expect(parseGrouping(" 3 + 2 ")).toEqual([3, 2]);
    expect(parseGrouping("")).toEqual([]);
  });
  it("rejects malformed input", () => {
    expect(parseGrouping("2++3")).toBeNull();
    expect(parseGrouping("0+7")).toBeNull();
    expect(parseGrouping("2,2,3")).toBeNull();
    expect(parseGrouping("a")).toBeNull();
  });
  it("round-trips", () => {
    expect(formatGrouping(parseGrouping("2+2+2+3")!)).toBe("2+2+2+3");
  });
});

describe("pitch helpers", () => {
  it("converts Hz and MIDI at a reference", () => {
    expect(hzToMidi(440)).toBeCloseTo(69, 9);
    expect(hzToMidi(442, 442)).toBeCloseTo(69, 9);
    expect(midiToHz(60)).toBeCloseTo(261.6256, 3);
  });
  it("splits a fractional MIDI number into note parts", () => {
    expect(noteParts(69)).toEqual({ pc: 9, midi: 69, octave: 4, cents: 0 });
    expect(noteParts(60.3).cents).toBeCloseTo(30, 9);
    expect(noteParts(60.5)).toMatchObject({ midi: 61, cents: -50 });
    expect(noteParts(59.7)).toMatchObject({ pc: 0, midi: 60, octave: 4 });
  });
  it("measures distance from a scale degree", () => {
    const dDorian = { tonic_pc: 2, degrees_cents: [0, 200, 300, 500, 700, 900, 1000] };
    expect(centsFromScale(dDorian, 62)).toBe(0);
    expect(centsFromScale(dDorian, 62.1)).toBeCloseTo(10, 9);
    // C♯ sits a semitone from both C and D: the first degree checked (D, the tonic) wins.
    expect(Math.abs(centsFromScale(dDorian, 61))).toBe(100);
    // C♯ + 40 cents is 60 cents below D (the nearest degree), not 40 above C♯.
    expect(centsFromScale(dDorian, 73.4)).toBeCloseTo(-60, 9);
  });
});

describe("snapToBars", () => {
  it("snaps both edges, keeps at least one bar and stays inside the take", () => {
    expect(snapToBars([0.9, 3.2], 2, 10)).toEqual([0, 4]);
    expect(snapToBars([1.2, 3.2], 2, 10)).toEqual([2, 4]);
    expect(snapToBars([1.9, 2.1], 2, 10)).toEqual([2, 4]);
    expect(snapToBars([8.9, 9.9], 2, 9)).toEqual([8, 10]);
    expect(snapToBars([0, 1], 0, 10)).toEqual([0, 1]);
  });
});

describe("takeName", () => {
  it("prefers the label, falls back to the number", () => {
    expect(takeName({ label: "Хоро", number: 3 }, "Запис")).toBe("Хоро");
    expect(takeName({ label: null, number: 3 }, "Запис")).toBe("Запис 3");
    expect(takeName({ label: "  ", number: 4 }, "Take")).toBe("Take 4");
  });
});

describe("isTypingTarget", () => {
  it("detects form fields only", () => {
    expect(isTypingTarget({ tagName: "INPUT" } as unknown as EventTarget)).toBe(true);
    expect(isTypingTarget({ tagName: "SELECT" } as unknown as EventTarget)).toBe(true);
    expect(isTypingTarget({ tagName: "DIV", isContentEditable: false } as unknown as EventTarget)).toBe(false);
    expect(isTypingTarget(null)).toBe(false);
  });
});

describe("formatTime", () => {
  it("formats minutes, seconds and tenths", () => {
    expect(formatTime(0)).toBe("0:00.0");
    expect(formatTime(5.27)).toBe("0:05.2");
    expect(formatTime(65.99)).toBe("1:05.9");
    expect(formatTime(600)).toBe("10:00.0");
  });
  it("clamps invalid input", () => {
    expect(formatTime(-3)).toBe("0:00.0");
    expect(formatTime(NaN)).toBe("0:00.0");
  });
});

describe("dB helpers", () => {
  it("converts linear to dB with a floor", () => {
    expect(linearToDb(1)).toBeCloseTo(0, 6);
    expect(linearToDb(0.5)).toBeCloseTo(-6.0206, 3);
    expect(linearToDb(0)).toBe(-120);
  });
  it("formats the floor as -∞", () => {
    expect(formatDb(-120)).toBe("-∞ dB");
    expect(formatDb(-6.02)).toBe("-6.0 dB");
  });
  it("maps dB to a meter fraction", () => {
    expect(meterFraction(0)).toBe(1);
    expect(meterFraction(-60)).toBe(0);
    expect(meterFraction(-30)).toBeCloseTo(0.5, 6);
    expect(meterFraction(-120)).toBe(0);
    expect(meterFraction(3)).toBe(1);
  });
});

describe("formatBytes", () => {
  it("picks sensible units", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(5 * 1024 * 1024)).toBe("5.00 MB");
  });
});
