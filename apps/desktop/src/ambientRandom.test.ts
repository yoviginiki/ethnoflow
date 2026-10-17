import { describe, expect, it } from "vitest";
import type { Ambient, AmbientLayer } from "./api";
import { randomizeRack } from "./ambientRandom";

function layer(id: string, patch: Partial<AmbientLayer> = {}): AmbientLayer {
  return {
    id,
    name: id,
    source: { kind: "bells", density: 8, decay_s: 5, brightness: 0.4, velocity_spread: 0.5, tonal: true, register: [60, 84] },
    gain_db: -18,
    pan: 0.2,
    lowpass_hz: 20000,
    highpass_hz: 10,
    mute: false,
    solo: false,
    seed: 7,
    license_ref: null,
    locked: false,
    ...patch,
  };
}

const rack: Ambient = {
  schema_version: 1,
  layers: [
    layer("a"),
    layer("b", { locked: true }),
    layer("c", { source: { kind: "wind", gust: 0.5, speed: 0.3, center_hz: 600, resonance: 0.5 } }),
    layer("d", { source: { kind: "loop", take_id: "t1", start: 1, end: 3, crossfade: 0.1, rate: 1, reverse: false } }),
  ],
  macros: { intensity: 1, brightness: 0.5, motion: 0.5, space: 0.3, space_size: 0.6, tonal_gravity: 0.5, width: 1 },
  preset: "mountain_mist",
  tags: [],
  automation: [],
  program: { scenes: [], loop_region: null },
};

describe("randomizeRack (F1A.3)", () => {
  it("never touches a locked layer and keeps kinds, tonality and regions", () => {
    const out = randomizeRack(rack, 42);
    expect(out.layers[1]).toEqual(rack.layers[1]);
    expect(out.layers.map((l) => l.source.kind)).toEqual(["bells", "bells", "wind", "loop"]);
    const a = out.layers[0].source;
    if (a.kind === "bells") {
      expect(a.tonal).toBe(true);
      expect(a.register).toEqual([60, 84]);
    }
    const d = out.layers[3].source;
    if (d.kind === "loop") {
      expect([d.take_id, d.start, d.end]).toEqual(["t1", 1, 3]);
    }
    expect(out.preset).toBeNull();
    expect(out.macros.intensity).toBe(1);
    expect(out.macros.width).toBe(1);
  });
  it("stays inside musical bounds and actually changes the unlocked layers", () => {
    for (let seed = 1; seed < 40; seed++) {
      const out = randomizeRack(rack, seed);
      for (const l of out.layers) {
        expect(l.gain_db).toBeGreaterThanOrEqual(-40);
        expect(l.gain_db).toBeLessThanOrEqual(-6);
        expect(Math.abs(l.gain_db - -18)).toBeLessThanOrEqual(6);
        expect(Math.abs(l.pan)).toBeLessThanOrEqual(1);
        if (l.source.kind === "bells") {
          expect(l.source.density).toBeGreaterThan(0);
          expect(l.source.density).toBeLessThanOrEqual(40);
        }
      }
    }
    const out = randomizeRack(rack, 5);
    expect(out.layers[0]).not.toEqual(rack.layers[0]);
    expect(randomizeRack(rack, 5)).toEqual(out);
  });
});
