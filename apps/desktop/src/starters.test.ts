import { describe, expect, it } from "vitest";
import type { Ambient, Note, Scale } from "./api";
import { melodyRegister, proposeStarters, STARTER_IDS, type StarterContext } from "./starters";

const baseRack = { schema_version: 1, layers: [], macros: {}, preset: null, tags: [], automation: [], program: { scenes: [], loop_region: null } } as unknown as Ambient;
const names = { noise: "Шум", wind: "Вятър", texture: "Текстура", bells: "Камбани", pluck: "Струни", loop: "Запис", cloud: "Облак", resonator: "Резонатор", choir: "Хор", bloom: "Блум" } as const;
const note = (midi: number): Note => ({ start: 0, end: 0.5, midi, cents: 0, confidence: 0.9, velocity: 0.8, bend: [] });

describe("starters", () => {
  it("measures the melody register from confident, long enough notes", () => {
    expect(melodyRegister([])).toBeNull();
    const r = melodyRegister([note(62), note(69), note(74), { ...note(40), confidence: 0.1 }]);
    expect(r).toEqual({ low: 62, high: 74, median: 69, count: 3 });
  });

  it("proposes every starter, keyed to the analysed scale, with the pad below the melody", () => {
    const scale: Scale = { tonic_pc: 6, degrees_cents: [0, 200, 300, 500, 700, 800, 1000], kind: "aeolian" };
    const ctx: StarterContext = { scale, timing: { bpm: null, numerator: null, unit: null, grouping: [] }, detectedBpm: 86, register: { low: 62, high: 81, median: 71, count: 20 }, baseRack, names };
    const st = proposeStarters(ctx);
    expect(st.map((s) => s.id)).toEqual(STARTER_IDS);
    for (const s of st) {
      expect(s.session.prefs.drone?.rootMidi).toBe(54);
      expect(s.session.prefs.pad?.rootMidi).toBe(54);
      expect(s.session.ambient?.layers.length).toBeGreaterThan(0);
      expect(s.session.prefs.harmony?.registerHigh).toBeLessThanOrEqual(71 + 6);
      for (const l of s.session.ambient!.layers) expect(l.gain_db).toBeLessThanOrEqual(-16);
    }
    const dnb = st.find((s) => s.id === "liquid_dnb")!;
    // 86 BPM detected → doubled for drum & bass, and said so.
    expect(dnb.timing?.bpm).toBe(172);
    expect(dnb.facts).toContainEqual({ kind: "tempo_doubled", bpm: 172 });
    expect(dnb.session.layers.rhythm).toBe(true);
  });

  it("never assumes a key: without a scale the tonal layers stay off and the facts say so", () => {
    const ctx: StarterContext = { scale: null, timing: { bpm: 120, numerator: 7, unit: 8, grouping: [3, 2, 2] }, detectedBpm: null, register: null, baseRack, names };
    const st = proposeStarters(ctx);
    const quiet = st.find((s) => s.id === "quiet_ambient")!;
    expect(quiet.session.layers.drone).toBe(false);
    expect(quiet.facts).toContainEqual({ kind: "no_key" });
    expect(quiet.facts).toContainEqual({ kind: "register_unknown" });
    const ethno = st.find((s) => s.id === "ethno_pulse")!;
    // The project's own meter is kept, never replaced by a guess.
    expect(ethno.timing).toEqual({ bpm: 120, numerator: 7, unit: 8, grouping: [3, 2, 2] });
    expect(ethno.facts).toContainEqual({ kind: "meter_project", numerator: 7, unit: 8, grouping: [3, 2, 2] });
    const dnb = st.find((s) => s.id === "liquid_dnb")!;
    expect(dnb.facts).toContainEqual({ kind: "tempo_suggested", bpm: 172 });
  });
});
