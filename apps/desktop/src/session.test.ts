import { describe, expect, it } from "vitest";
import type { Ambient } from "./api";
import { DEFAULT_PREFS } from "./store";
import { isSessionPayload, mergeSessionPrefs, pickSessionPrefs, rackForProject } from "./session";

describe("session", () => {
  it("picks only the session keys", () => {
    const p = pickSessionPrefs(DEFAULT_PREFS);
    expect(Object.keys(p).sort()).toEqual(["drone", "harmony", "intent", "masterDb", "mixer", "pad", "rhythm", "stripControl"]);
  });

  it("merges saved objects over the defaults and ignores unknown keys", () => {
    const merged = mergeSessionPrefs(DEFAULT_PREFS, { drone: { rootMidi: 62 }, masterDb: -6, lang: "en", bogus: 1 });
    expect(merged.drone.rootMidi).toBe(62);
    expect(merged.drone.intonation).toBe(DEFAULT_PREFS.drone.intonation);
    expect(merged.masterDb).toBe(-6);
    expect(merged.lang).toBe(DEFAULT_PREFS.lang);
    expect((merged as unknown as Record<string, unknown>).bogus).toBeUndefined();
  });

  it("leaves prefs alone for garbage", () => {
    expect(mergeSessionPrefs(DEFAULT_PREFS, null)).toBe(DEFAULT_PREFS);
  });

  it("merges nested objects at every depth so a partial bridge command keeps the other fields", () => {
    const merged = mergeSessionPrefs(DEFAULT_PREFS, { rhythm: { ostinato: { on: true }, instruments: { bass: "factory:bass.wood" } } });
    expect(merged.rhythm.ostinato.on).toBe(true);
    expect(merged.rhythm.ostinato.steps).toBe(DEFAULT_PREFS.rhythm.ostinato.steps);
    expect(merged.rhythm.drums).toEqual(DEFAULT_PREFS.rhythm.drums);
    expect(merged.rhythm.instruments?.bass).toBe("factory:bass.wood");
    // Arrays replace, they do not merge by index.
    const g = mergeSessionPrefs(DEFAULT_PREFS, { rhythm: { drums: { ...DEFAULT_PREFS.rhythm.drums } } });
    expect(g.rhythm.drums).toEqual(DEFAULT_PREFS.rhythm.drums);
    expect(mergeSessionPrefs(DEFAULT_PREFS, "x")).toBe(DEFAULT_PREFS);
    expect(isSessionPayload({ v: 2, prefs: {} })).toBe(false);
    expect(isSessionPayload({ v: 1, prefs: {}, ambient: null, layers: {} })).toBe(true);
  });

  it("drops loop layers whose take is not in the project, and their lanes", () => {
    const rack = {
      layers: [
        { id: "a", source: { kind: "loop", take_id: "t1" } },
        { id: "b", source: { kind: "loop", take_id: "missing" } },
        { id: "d", source: { kind: "cloud", take_id: "missing" } },
        { id: "c", source: { kind: "wind" } },
      ],
      automation: [
        { target: { layer: "b", param: "gain_db" } },
        { target: { layer: null, param: "intensity" } },
      ],
    } as unknown as Ambient;
    const out = rackForProject(rack, new Set(["t1"]));
    expect(out.layers.map((l) => l.id)).toEqual(["a", "c"]);
    expect(out.automation.length).toBe(1);
  });
});
