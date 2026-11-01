// Sync-locked time edits across every track (Audacity programme, stage A4, 2026-11-01):
// delete a span of time or insert a gap, and everything later moves together on all
// tracks, so the song stays in time. Pure functions on the arrangement document.
import type { Arrangement, Clip, Track } from "./api";

const MIN_CLIP_S = 0.05;

function uid(): string {
  return `c-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

/** The part of `c` after `cut` seconds into it, as a new clip starting at `at`. */
function tail(c: Clip, cut: number, at: number): Clip {
  const env = (c.envelope ?? []).filter((p) => p.at >= cut).map((p) => ({ at: p.at - cut, db: p.db }));
  return { ...c, id: uid(), start: at, offset: c.offset + cut, length: c.length - cut, fade_in: 0, envelope: env };
}

/** The part of `c` before `cut` seconds into it (same id, same start). */
function head(c: Clip, cut: number): Clip {
  const env = (c.envelope ?? []).filter((p) => p.at <= cut);
  return { ...c, length: cut, fade_out: 0, envelope: env };
}

/** Remove the time [a, b) from every track: clips inside go, clips across it are cut, later clips move left. */
export function deleteTimeRange(arr: Arrangement, a: number, b: number): Arrangement {
  if (!(b > a)) return arr;
  const span = b - a;
  const tracks: Track[] = arr.tracks.map((tr) => {
    const clips: Clip[] = [];
    for (const c of tr.clips) {
      const end = c.start + c.length;
      if (end <= a) {
        clips.push(c);
      } else if (c.start >= b) {
        clips.push({ ...c, start: c.start - span });
      } else if (c.start < a && end > b) {
        // Spans the whole range: keep both sides, the right one joins the left seamlessly.
        clips.push(head(c, a - c.start));
        clips.push(tail(c, b - c.start, a));
      } else if (c.start < a) {
        if (a - c.start >= MIN_CLIP_S) clips.push(head(c, a - c.start));
      } else if (end > b) {
        if (end - b >= MIN_CLIP_S) clips.push(tail(c, b - c.start, a));
      }
      // else: entirely inside [a, b) → removed
    }
    return { ...tr, clips };
  });
  return { ...arr, tracks, loop_region: null };
}

/** Insert `seconds` of empty time at `at` on every track: later clips move right, clips across `at` are split. */
export function insertTimeGap(arr: Arrangement, at: number, seconds: number): Arrangement {
  if (!(seconds > 0)) return arr;
  const tracks: Track[] = arr.tracks.map((tr) => {
    const clips: Clip[] = [];
    for (const c of tr.clips) {
      const end = c.start + c.length;
      if (end <= at) clips.push(c);
      else if (c.start >= at) clips.push({ ...c, start: c.start + seconds });
      else {
        const cut = at - c.start;
        if (cut >= MIN_CLIP_S) clips.push(head(c, cut));
        if (c.length - cut >= MIN_CLIP_S) clips.push(tail(c, cut, at + seconds));
      }
    }
    return { ...tr, clips };
  });
  const loop = arr.loop_region ? ([arr.loop_region[0] >= at ? arr.loop_region[0] + seconds : arr.loop_region[0], arr.loop_region[1] >= at ? arr.loop_region[1] + seconds : arr.loop_region[1]] as [number, number]) : null;
  return { ...arr, tracks, loop_region: loop };
}
