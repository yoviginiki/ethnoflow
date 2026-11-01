import { describe, expect, it } from "vitest";
import type { Clip, Track } from "./api";
import { chooseTake, overlappingClips, placeRecordedClip } from "./arrangeTakes";

const clip = (id: string, start: number, length: number, mute = false): Clip => ({ id, take_id: `t-${id}`, start, offset: 0, length, gain_db: 0, fade_in: 0, fade_out: 0, mute });
const track = (clips: Clip[]): Track => ({ id: "tr", name: "Kaval", gain_db: 0, pan: 0, mute: false, solo: false, lowpass_hz: 20000, highpass_hz: 10, send: 0, clips });

describe("takes on a track", () => {
  it("a new take at the same spot mutes the older ones and skips the measured latency", () => {
    const tr = track([clip("a", 0, 10), clip("b", 20, 5)]);
    const r = placeRecordedClip(tr, { id: "take3", duration_seconds: 12 }, 2, 130, "c");
    expect(r.clip.offset).toBeCloseTo(0.13);
    expect(r.clip.length).toBeCloseTo(11.87);
    expect(r.mutedIds).toEqual(["a"]);
    expect(r.track.clips.find((c) => c.id === "a")!.mute).toBe(true);
    expect(r.track.clips.find((c) => c.id === "b")!.mute).toBe(false);
    expect(r.track.clips.find((c) => c.id === "c")!.mute).toBe(false);
    // Without a measurement nothing is skipped; a tiny take keeps a playable length.
    expect(placeRecordedClip(tr, { id: "t", duration_seconds: 0.05 }, 0, null, "d").clip.length).toBe(0.1);
  });

  it("choosing a take unmutes it and mutes only the overlapping others", () => {
    const tr = track([clip("a", 0, 10, true), clip("b", 1, 8), clip("far", 30, 2)]);
    const next = chooseTake(tr, "a");
    expect(next.clips.map((c) => [c.id, c.mute])).toEqual([
      ["a", false],
      ["b", true],
      ["far", false],
    ]);
    expect(overlappingClips(next, next.clips[0]).map((c) => c.id)).toEqual(["a", "b"]);
  });
});
