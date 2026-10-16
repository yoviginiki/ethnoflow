import { describe, expect, it } from "vitest";
import { formatBytes, formatDb, formatTime, linearToDb, meterFraction } from "./format";

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
