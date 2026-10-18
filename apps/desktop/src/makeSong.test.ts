import { describe, expect, it } from "vitest";
import type { Phrase } from "./api";
import { sectionsFromPhrases } from "./makeSong";

const phrase = (start: number, end: number): Phrase => ({ start, end, notes: [], stable_tones: [], shape: "flat", breath_before: true, cause: "gap", confidence: 0.8 }) as unknown as Phrase;

describe("make the song: sections from phrases", () => {
  it("builds intro → build → drop → break → outro on bar lines inside the take", () => {
    const bpm = 174;
    const barS = (60 / bpm) * 4;
    const phrases = [phrase(0.3, 6.4), phrase(7.2, 12.6), phrase(13.6, 20.2), phrase(21.5, 25.0), phrase(27.0, 35.0), phrase(36.0, 43.0), phrase(45.0, 52.0)];
    const s = sectionsFromPhrases(phrases, 59.4, barS, 960, bpm);
    expect(s.map((x) => x.role)).toEqual(["intro", "build", "drop", "break", "outro"]);
    const tps = (bpm / 60) * 960;
    for (let i = 0; i < s.length; i++) {
      expect(s[i].end_tick).toBeGreaterThan(s[i].start_tick);
      if (i > 0) expect(s[i].start_tick).toBe(s[i - 1].end_tick);
      // Every start sits on a bar line.
      const bars = s[i].start_tick / tps / barS;
      expect(Math.abs(bars - Math.round(bars))).toBeLessThan(1e-6);
    }
    expect(s[0].start_tick).toBe(0);
    expect(s[s.length - 1].end_tick).toBe(Math.round(59.4 * tps));
    // The intro ends where the second phrase starts (snapped), not deep into the take.
    expect(s[0].end_tick / tps).toBeLessThanOrEqual(12);
    expect(s[2].energy).toBe(1);
  });

  it("a short take with one phrase still gets a usable map", () => {
    const bpm = 120;
    const barS = 2;
    const s = sectionsFromPhrases([phrase(0.2, 5)], 9, barS, 960, bpm);
    expect(s.length).toBeGreaterThanOrEqual(2);
    expect(s[0].role).toBe("intro");
    expect(s[s.length - 1].end_tick).toBe(Math.round(9 * (bpm / 60) * 960));
  });
});
