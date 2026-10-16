import { describe, expect, it } from "vitest";
import {
  formatBytes,
  formatDb,
  formatGrouping,
  formatTime,
  isTypingTarget,
  linearToDb,
  meterFraction,
  parseGrouping,
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
