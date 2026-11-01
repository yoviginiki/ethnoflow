// Takes on a track (home-studio item 2, owner 1.11.2026): recording over the background
// again and again at the same place keeps every take as a clip on the same track; only
// the chosen one sounds (the others are muted, never deleted), the recorded clip is
// shifted by the measured round-trip latency so it lands where it was played, and
// "one more take" records at the same spot. Pure helpers, tested without audio.
import type { Clip, Track } from "./api";

export const MIN_CLIP_S = 0.1;

/** Clips of the track that overlap `clip` in time (the take group), itself included. */
export function overlappingClips(track: Track, clip: Clip): Clip[] {
  const a0 = clip.start;
  const a1 = clip.start + clip.length;
  return track.clips.filter((c) => c.start < a1 - 1e-6 && c.start + c.length > a0 + 1e-6);
}

/**
 * The clip for a take just recorded at timeline `at`: the first `latencyMs` of the take
 * are skipped (the microphone heard the background that much late), the other clips the
 * new one overlaps are muted so only the newest take sounds.
 */
export function placeRecordedClip(track: Track, take: { id: string; duration_seconds: number | null }, at: number, latencyMs: number | null, id: string): { track: Track; clip: Clip; mutedIds: string[] } {
  const dur = take.duration_seconds ?? 0;
  const offset = latencyMs && latencyMs > 0 ? Math.min(latencyMs / 1000, Math.max(0, dur - MIN_CLIP_S)) : 0;
  const clip: Clip = { id, take_id: take.id, start: at, offset, length: Math.max(MIN_CLIP_S, dur - offset), gain_db: 0, fade_in: 0, fade_out: 0, mute: false };
  const probe = { ...track, clips: [...track.clips, clip] };
  const overlapped = overlappingClips(probe, clip).filter((c) => c.id !== id);
  const mutedIds = overlapped.filter((c) => !c.mute).map((c) => c.id);
  const clips = track.clips.map((c) => (mutedIds.includes(c.id) ? { ...c, mute: true } : c));
  return { track: { ...track, clips: [...clips, clip] }, clip, mutedIds };
}

/** Make `clipId` the sounding take of its group: unmute it, mute the overlapping others. */
export function chooseTake(track: Track, clipId: string): Track {
  const clip = track.clips.find((c) => c.id === clipId);
  if (!clip) return track;
  const group = new Set(overlappingClips(track, clip).map((c) => c.id));
  return { ...track, clips: track.clips.map((c) => (group.has(c.id) ? { ...c, mute: c.id !== clipId } : c)) };
}
