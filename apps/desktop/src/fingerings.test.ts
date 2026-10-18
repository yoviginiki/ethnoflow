import { describe, expect, it } from "vitest";
import { applyOverrides, chartFor, fingeringsFor, HOLE_RE, KAVAL_D, nearestFingered } from "./fingerings";

describe("kaval-d fingerings (owner's table, F2.6)", () => {
  it("every fingering has 8 ASCII hole states and a sane register", () => {
    for (const f of KAVAL_D.fingerings) {
      expect(f.holes, `${f.midi}`).toMatch(HOLE_RE);
      expect(f.register).toBeGreaterThanOrEqual(0);
      expect(f.register).toBeLessThanOrEqual(4);
    }
  });

  it("covers the first and second register chromatically from D to B and the table's facts", () => {
    for (const base of [62, 74]) {
      for (const m of [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]) {
        expect(fingeringsFor(KAVAL_D, base + m).length, `midi ${base + m}`).toBeGreaterThan(0);
      }
    }
    // All closed = D1; A1 = thumb + index; E♭ only by half-covering hole 8.
    expect(fingeringsFor(KAVAL_D, 62)[0].holes).toBe("cccccccc");
    expect(fingeringsFor(KAVAL_D, 69)[0].holes).toBe("ccoooooo");
    expect(fingeringsFor(KAVAL_D, 63)[0].holes).toBe("ccccccch");
    // Third register: all closed gives A2; fourth register uses the closed little finger.
    expect(KAVAL_D.fingerings.some((f) => f.midi === 81 && f.register === 3 && f.holes === "cccccccc")).toBe(true);
    expect(fingeringsFor(KAVAL_D, 91)[0].holes).toBe("ccccoooc");
    // Kaba is an octave under the first register with the same fingerings.
    expect(fingeringsFor(KAVAL_D, 50)[0].holes).toBe("cccccccc");
    expect(fingeringsFor(KAVAL_D, 50)[0].register).toBe(0);
    // Alternatives come after the primary fingering.
    const a3 = fingeringsFor(KAVAL_D, 93);
    expect(a3[0].alt).toBeUndefined();
    expect(a3[1].alt).toBe(true);
  });

  it("finds the nearest fingered note for a measured pitch", () => {
    expect(nearestFingered(KAVAL_D, 69.3)).toEqual({ midi: 69, distance: expect.closeTo(0.3, 6) });
    expect(nearestFingered(KAVAL_D, 60.4)?.midi).toBe(59);
  });

  it("user overrides replace the factory entry of that note and register only", () => {
    const over = [{ midi: 69, register: 1, holes: "ccoxoooo" }];
    const c = applyOverrides(KAVAL_D, over);
    expect(fingeringsFor(c, 69).map((f) => f.holes)).toEqual(["ccoxoooo"]);
    // A different register of the same note name is untouched, as is every other note.
    expect(fingeringsFor(c, 81).length).toBe(fingeringsFor(KAVAL_D, 81).length);
    expect(fingeringsFor(c, 62)[0].holes).toBe("cccccccc");
    expect(c.fingerings.length).toBe(KAVAL_D.fingerings.length);
    expect(KAVAL_D.fingerings.find((f) => f.midi === 69)?.holes).toBe("ccoooooo");
    expect(chartFor("kaval", { "kaval-d": over })?.fingerings.some((f) => f.holes === "ccoxoooo")).toBe(true);
    expect(chartFor("flute")).toBeNull();
  });
});
