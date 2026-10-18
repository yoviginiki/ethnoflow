import { describe, expect, it } from "vitest";
import { newExtraPart } from "./extraParts";

describe("newExtraPart", () => {
  it("gives unique ids and kind-specific registers and presets", () => {
    const a = newExtraPart("tapan", []);
    const b = newExtraPart("chord_pad", [a]);
    const c = newExtraPart("lead", [a, b]);
    expect(new Set([a.id, b.id, c.id]).size).toBe(3);
    expect(a.instrument).toBe("factory:tapan.folk");
    expect(b.register_high).toBeLessThan(c.register_low + 10);
    expect(c.register_low).toBeGreaterThan(b.register_low);
    expect([a, b, c].every((x) => x.on && x.level_db <= 0 && x.register_low < x.register_high)).toBe(true);
    // A gap in the ids is reused, never a duplicate.
    const d = newExtraPart("lead", [c]);
    expect(d.id).not.toBe(c.id);
  });
});
