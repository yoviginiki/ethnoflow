import { describe, expect, it } from "vitest";
import type { SongMap } from "./api";
import { barToTick, barTicks, secondsToTick, tickToBar, tickToSeconds } from "./SongMapPanel";

const PPQ = 960;
const map: SongMap = {
  schema_version: 1,
  ppq: PPQ,
  tempo_map: [
    { tick: 0, bpm: 120 },
    { tick: 8 * 4 * PPQ, bpm: 90 },
  ],
  meter_map: [
    { tick: 0, numerator: 4, unit: 4, grouping: [] },
    { tick: 4 * 4 * PPQ, numerator: 7, unit: 8, grouping: [3, 2, 2] },
  ],
  sections: [],
};

describe("song map helpers (mirror the Rust model)", () => {
  it("converts ticks to seconds across a tempo change and back", () => {
    expect(tickToSeconds(map, 8 * 4 * PPQ)).toBeCloseTo(16, 9);
    expect(tickToSeconds(map, 9 * 4 * PPQ)).toBeCloseTo(16 + (4 * 60) / 90, 9);
    for (const secs of [0, 3.3, 15.999, 16, 20.5, 42]) {
      expect(tickToSeconds(map, secondsToTick(map, secs))).toBeCloseTo(secs, 2);
    }
  });

  it("counts bars across a meter change", () => {
    expect(barTicks(map, 0)).toBe(4 * PPQ);
    expect(barTicks(map, 4 * 4 * PPQ)).toBe((7 * PPQ) / 2);
    expect(barToTick(map, 5)).toBe(4 * 4 * PPQ);
    expect(barToTick(map, 6)).toBe(4 * 4 * PPQ + (7 * PPQ) / 2);
    expect(tickToBar(map, barToTick(map, 6))).toBe(6);
    expect(tickToBar(map, 4 * 4 * PPQ + PPQ)).toBe(5);
  });
});
