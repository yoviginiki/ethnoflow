import { describe, expect, it } from "vitest";
import type { Arrangement, Clip, Track } from "./api";
import { deleteTimeRange, insertTimeGap } from "./ripple";

const clip = (id: string, start: number, length: number, extra: Partial<Clip> = {}): Clip => ({ id, take_id: "t", start, offset: 0, length, gain_db: 0, fade_in: 0, fade_out: 0, mute: false, ...extra });
const track = (id: string, clips: Clip[]): Track => ({ id, name: id, gain_db: 0, pan: 0, mute: false, solo: false, lowpass_hz: 20000, highpass_hz: 10, send: 0, clips });
const arr = (): Arrangement => ({
  schema_version: 1,
  tracks: [track("a", [clip("a1", 0, 4), clip("a2", 6, 2)]), track("b", [clip("b1", 3, 5, { envelope: [{ at: 0, db: 0 }, { at: 2, db: -6 }, { at: 4, db: -12 }] })])],
  loop_region: [2, 4],
});

describe("sync-locked time edits", () => {
  it("deleting a range cuts across every track and pulls later clips left", () => {
    const out = deleteTimeRange(arr(), 2, 4);
    const a = out.tracks[0].clips;
    expect(a.map((c) => [c.start, c.length])).toEqual([
      [0, 2],
      [4, 2],
    ]);
    const b = out.tracks[1].clips;
    // b1 (3..8) loses 3..4: the rest starts at 2 with offset 1 and length 4.
    expect(b.length).toBe(1);
    expect(b[0].start).toBe(2);
    expect(b[0].offset).toBe(1);
    expect(b[0].length).toBe(4);
    // Envelope points before the cut are dropped, later ones re-timed.
    expect(b[0].envelope).toEqual([
      { at: 1, db: -6 },
      { at: 3, db: -12 },
    ]);
    expect(out.loop_region).toBeNull();
  });
  it("a clip spanning the whole range keeps both sides joined", () => {
    const out = deleteTimeRange(arr(), 1, 2);
    const a = out.tracks[0].clips;
    expect(a.map((c) => [c.start, c.offset, c.length])).toEqual([
      [0, 0, 1],
      [1, 2, 2],
      [5, 0, 2],
    ]);
  });
  it("inserting a gap splits what it crosses and pushes the rest right", () => {
    const out = insertTimeGap(arr(), 1, 2);
    const a = out.tracks[0].clips;
    expect(a.map((c) => [c.start, c.offset, c.length])).toEqual([
      [0, 0, 1],
      [3, 1, 3],
      [8, 0, 2],
    ]);
    expect(out.tracks[1].clips[0].start).toBe(5);
    expect(out.loop_region).toEqual([4, 6]);
  });
});
