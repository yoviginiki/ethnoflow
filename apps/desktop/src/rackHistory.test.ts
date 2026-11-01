import { describe, expect, it } from "vitest";
import type { Ambient } from "./api";
import { DEFAULT_MACROS, defaultSource, makeLayer } from "./ambientPresets";
import { canRedo, canUndo, describeChange, jumpHistory, pushHistory, redoHistory, seedHistory, undoHistory, HISTORY_MAX, type ChangeWords, type RackHistory } from "./rackHistory";

const base: Ambient = { schema_version: 3, layers: [], macros: DEFAULT_MACROS, preset: null, tags: [], automation: [], program: { scenes: [], loop_region: null } };
const words: ChangeWords = {
  start: "start",
  layerAdded: (n) => `+${n}`,
  layerRemoved: (n) => `-${n}`,
  layerRenamed: (a, b) => `${a}→${b}`,
  layerParam: (n, p) => `${n}:${p}`,
  macro: (k) => `macro ${k}`,
  scenes: "scenes",
  automation: "lanes",
  many: "many",
};
const wind = makeLayer("Wind", defaultSource("wind"), { id: "w1", seed: 5 });

describe("rack history", () => {
  it("describes what changed", () => {
    expect(describeChange(null, base, words)).toBe("start");
    const withWind = { ...base, layers: [wind] };
    expect(describeChange(base, withWind, words)).toBe("+Wind");
    expect(describeChange(withWind, base, words)).toBe("-Wind");
    expect(describeChange(withWind, { ...withWind, layers: [{ ...wind, gain_db: -12 }] }, words)).toBe("Wind:gain_db");
    expect(describeChange(withWind, { ...withWind, layers: [{ ...wind, source: { ...wind.source, gust: 0.9 } as typeof wind.source }] }, words)).toBe("Wind:gust");
    expect(describeChange(withWind, { ...withWind, layers: [{ ...wind, name: "Breeze" }] }, words)).toBe("Wind→Breeze");
    expect(describeChange(base, { ...base, macros: { ...base.macros, brightness: 0.9 } }, words)).toBe("macro brightness");
    expect(describeChange(base, { ...base, program: { scenes: [], loop_region: [0, 10] } }, words)).toBe("scenes");
    expect(describeChange(base, { ...base, macros: { ...base.macros, brightness: 0.9, space: 0.1 } }, words)).toBe("macro brightness, macro space");
  });

  it("undo / redo walk the steps and a new change drops the redo branch", () => {
    let h: RackHistory = seedHistory({ entries: [], index: -1 }, base, "start", 0);
    const a = { ...base, layers: [wind] };
    const b = { ...a, macros: { ...a.macros, space: 0.9 } };
    h = pushHistory(h, a, "+Wind", 1000);
    h = pushHistory(h, b, "macro space", 3000);
    expect(h.entries.length).toBe(3);
    expect(canUndo(h)).toBe(true);
    expect(canRedo(h)).toBe(false);
    const u = undoHistory(h)!;
    expect(u.rack).toBe(a);
    expect(canRedo(u.history)).toBe(true);
    const r = redoHistory(u.history)!;
    expect(r.rack).toBe(b);
    const u2 = undoHistory(undoHistory(h)!.history)!;
    expect(u2.rack).toBe(base);
    expect(canUndo(u2.history)).toBe(false);
    const branched = pushHistory(u2.history, a, "+Wind again", 9000);
    expect(branched.entries.length).toBe(2);
    expect(canRedo(branched)).toBe(false);
    expect(jumpHistory(branched, 5)).toBeNull();
  });

  it("coalesces a knob dragged continuously and caps the length", () => {
    let h: RackHistory = seedHistory({ entries: [], index: -1 }, base, "start", 0);
    for (let i = 0; i < 5; i++) h = pushHistory(h, { ...base, macros: { ...base.macros, space: i / 10 } }, "macro space", 1000 + i * 100);
    expect(h.entries.length).toBe(2);
    expect(h.entries[1].rack.macros.space).toBe(0.4);
    h = pushHistory(h, { ...base, macros: { ...base.macros, space: 0.5 } }, "macro space", 5000);
    expect(h.entries.length).toBe(3);
    for (let i = 0; i < HISTORY_MAX + 10; i++) h = pushHistory(h, { ...base, macros: { ...base.macros, motion: i / 1000 } }, `m${i}`, 10_000 + i * 5000);
    expect(h.entries.length).toBe(HISTORY_MAX);
    expect(h.index).toBe(HISTORY_MAX - 1);
  });
});
