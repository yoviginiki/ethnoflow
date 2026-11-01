import { describe, expect, it } from "vitest";
import type { Scale } from "./api";
import { chordNotes, chordRootMidi, generateProgression, GENRES, GENRE_SPECS, stepAtBar, stepLabel, totalBars } from "./progression";

const dMinor: Scale = { tonic_pc: 2, degrees_cents: [0, 200, 300, 500, 700, 800, 1000], kind: "aeolian" };
const cMajor: Scale = { tonic_pc: 0, degrees_cents: [0, 200, 400, 500, 700, 900, 1100], kind: "ionian" };

describe("progressions", () => {
  it("every genre generates steps that fill the bars, deterministic per seed", () => {
    for (const g of GENRES) {
      const p = generateProgression(g, 7, 16);
      expect(p.genre).toBe(g);
      expect(totalBars(p)).toBe(16);
      for (const st of p.steps) {
        expect(st.degree).toBeGreaterThanOrEqual(1);
        expect(st.degree).toBeLessThanOrEqual(7);
        expect(st.bars).toBeGreaterThan(0);
      }
      expect(generateProgression(g, 7, 16)).toEqual(p);
      expect(GENRE_SPECS.find((s) => s.id === g)).toBeTruthy();
    }
    expect(generateProgression("rock", 1, 8).steps.map((s) => s.degree)).not.toEqual(generateProgression("rock", 2, 8).steps.map((s) => s.degree));
  });

  it("the genre shapes the harmony: rock holds a chord per bar, ambient for eight, dnb vamps on two chords", () => {
    expect(generateProgression("rock", 3, 8).steps.every((s) => s.bars === 1)).toBe(true);
    expect(generateProgression("ambient", 3, 16).steps[0].bars).toBe(8);
    const dnb = generateProgression("dnb", 3, 16);
    expect(new Set(dnb.steps.map((s) => s.degree)).size).toBeLessThanOrEqual(2);
    expect(generateProgression("jazz", 3, 8).steps.every((s) => ["7", "maj7", "9"].includes(s.quality))).toBe(true);
  });

  it("stepAtBar repeats the progression and chord notes come from the scale, never an assumed key", () => {
    const p = generateProgression("liquid", 5, 8);
    expect(stepAtBar(p, 0)?.index).toBe(0);
    expect(stepAtBar(p, 8)?.index).toBe(0);
    expect(stepAtBar(p, 2)?.index).toBe(1);
    // D minor: degree 1 triad = D F A (62 65 69 around C3+2 = 50 → 50 53 57).
    const i = chordNotes(dMinor, { degree: 1, quality: "triad", bars: 1 });
    expect(i.map((n) => n.midi)).toEqual([50, 53, 57]);
    expect(chordRootMidi(dMinor, { degree: 5, quality: "triad", bars: 1 })).toBe(57);
    // C major: V7 = G B D F.
    expect(chordNotes(cMajor, { degree: 5, quality: "7", bars: 1 }).map((n) => n.midi)).toEqual([55, 59, 62, 65]);
    expect(chordNotes(cMajor, { degree: 1, quality: "power", bars: 1 }).map((n) => n.midi)).toEqual([48, 55, 60]);
    expect(stepLabel(dMinor, { degree: 1, quality: "triad", bars: 1 })).toBe("i");
    expect(stepLabel(cMajor, { degree: 5, quality: "7", bars: 1 })).toBe("V7");
    expect(stepLabel(dMinor, { degree: 6, quality: "maj7", bars: 1 })).toBe("VImaj7");
  });
});
