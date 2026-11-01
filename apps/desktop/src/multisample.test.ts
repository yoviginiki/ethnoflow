import { describe, expect, it } from "vitest";
import type { SampleEntry } from "./api";
import { noteFromName, planZones, zonesRegister } from "./multisample";

const entry = (name: string, folder = "C:\\lib\\piano"): SampleEntry => ({ path: `${folder}\\${name}`, name, folder, ext: "wav", bytes: 1, modified_ms: 0, duration_s: 1, sample_rate: 48000, channels: 1 });

describe("multisample names", () => {
  it("reads note names with octaves, accidentals and bare MIDI numbers", () => {
    expect(noteFromName("Piano_C4.wav")).toBe(60);
    expect(noteFromName("cello A#2.wav")).toBe(46);
    expect(noteFromName("flute-Bb3.flac")).toBe(58);
    expect(noteFromName("C-1.wav")).toBe(0);
    expect(noteFromName("F#-1.wav")).toBe(6);
    expect(noteFromName("060.wav")).toBe(60);
    expect(noteFromName("kick 01.wav")).toBeNull();
    expect(noteFromName("bells.wav")).toBeNull();
    expect(noteFromName("cello.wav")).toBeNull();
    expect(noteFromName("A4_long_take.wav")).toBe(69);
    // Library file names that are not notes: paper sizes, counters, catalogue numbers.
    expect(noteFromName("A4 Printing Paper Rattle Page Turn Tail.wav")).toBeNull();
    expect(noteFromName("ice, block of ice crushed, heavy-015.wav")).toBeNull();
    expect(noteFromName("DSGNBass_Bass Drop & Downer Fast 16_344 Audio_Bass Drops.wav")).toBeNull();
    expect(noteFromName("Moroccan Countryside Ambience Sigturesamples.org 12.wav")).toBeNull();
    expect(noteFromName("Piano_060.wav")).toBe(60);
  });

  it("plans one zone per note inside the folder, sorted by pitch, and a register around them", () => {
    const z = planZones([entry("Piano_G3.wav"), entry("Piano_C4.wav"), entry("Piano_C4_alt.wav"), entry("noise.wav"), entry("Harp_C5.wav", "C:\\lib\\harp")], "C:\\lib\\piano");
    expect(z.map((x) => [x.entry.name, x.root_midi])).toEqual([
      ["Piano_G3.wav", 55],
      ["Piano_C4.wav", 60],
    ]);
    expect(zonesRegister(z)).toEqual([48, 67]);
    expect(zonesRegister([])).toEqual([48, 72]);
  });
});
