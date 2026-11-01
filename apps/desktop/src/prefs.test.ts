import { describe, expect, it } from "vitest";
import { DEFAULT_MIXER, fillPrefs, type MixerPrefs } from "./store";

describe("prefs hydration", () => {
  it("fills mixer strips an older save lacks (the fifth strip, scapes)", () => {
    const old = { drone: DEFAULT_MIXER.drone, pad: DEFAULT_MIXER.pad, ambient: { ...DEFAULT_MIXER.ambient, gain_db: -6 }, rhythm: DEFAULT_MIXER.rhythm } as unknown as MixerPrefs;
    const p = fillPrefs({ mixer: old, lang: "en" });
    expect(p.lang).toBe("en");
    expect(p.mixer.ambient.gain_db).toBe(-6);
    expect(p.mixer.scapes).toEqual(DEFAULT_MIXER.scapes);
  });

  it("keeps the defaults when nothing is saved", () => {
    expect(fillPrefs({}).mixer).toEqual(DEFAULT_MIXER);
  });
});
