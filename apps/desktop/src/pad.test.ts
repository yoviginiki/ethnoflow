import { describe, expect, it } from "vitest";
import { voicingNotes } from "./PadPanel";
import type { Scale } from "./api";

describe("voicingNotes (F3.3)", () => {
  it("builds mode-neutral voicings from any root without a scale", () => {
    expect(voicingNotes("fifth", 62, null)).toEqual([62, 69, 74].map((midi) => ({ midi, cents: 0 })));
    expect(voicingNotes("sus4", 50, null)?.map((n) => n.midi)).toEqual([50, 55, 57]);
    expect(voicingNotes("fifth9", 60, null)?.map((n) => n.midi)).toEqual([60, 67, 74]);
    expect(voicingNotes("scale_triad", 62, null)).toBeNull();
  });
  it("takes scale degrees with their cents, including non-tempered ones", () => {
    const hijaz: Scale = { tonic_pc: 4, degrees_cents: [0, 100, 400, 500, 700, 800, 1000], kind: "phrygian_dominant" };
    expect(voicingNotes("scale_triad", 64, hijaz)?.map((n) => n.midi)).toEqual([64, 68, 71]);
    const custom: Scale = { tonic_pc: 2, degrees_cents: [0, 150, 350, 500, 700, 850, 1050], kind: null };
    const seventh = voicingNotes("scale_7th", 62, custom)!;
    expect(seventh.map((n) => n.midi)).toEqual([62, 66, 69, 73]);
    expect(seventh.map((n) => Math.round(n.cents))).toEqual([0, -50, 0, -50]);
    expect(voicingNotes("scale_7th", 62, { ...custom, degrees_cents: [0, 200, 400, 700, 900] })).toBeNull();
  });
});
