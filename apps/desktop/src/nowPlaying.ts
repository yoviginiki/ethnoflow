// What is sounding right now, for the "Now playing" bar (UX audit 2026-10-31, stage 2):
// the take player, the notes player, the background layers, the song transport, the
// metronome and the live follower. The take and notes players used to be polled only
// while "Play & record" was mounted, so leaving the screen froze their status while the
// sound went on; the polling lives here now, once per app, and "stop all" silences
// every engine from one place (the screen and the assistant bridge share it).
import { useEffect } from "react";
import { api, describeError } from "./api";
import { useStore } from "./store";
import { stopPlaying } from "./takeActions";

const POLL_MS = 50;

export interface NowPlaying {
  recording: boolean;
  take: { id: string | null; position_s: number; duration_s: number } | null;
  notes: { position_s: number; duration_s: number } | null;
  background: { drone: boolean; pad: boolean; ambient: boolean; rhythm: boolean; scapes: boolean; any: boolean };
  song: { bar: number; beat: number; seconds: number; source: string } | null;
  metronome: boolean;
  live: boolean;
  /** True when any of the above makes sound. */
  any: boolean;
}

/** A snapshot from the store: what sounds now. */
export function readNowPlaying(): NowPlaying {
  const s = useStore.getState();
  const bg = s.layers;
  const background = { drone: bg.drone, pad: bg.pad, ambient: bg.ambient, rhythm: bg.rhythm, scapes: bg.scapes, any: bg.drone || bg.pad || bg.ambient || bg.rhythm || bg.scapes };
  const take = s.mode === "playing" && s.player ? { id: s.loaded?.id ?? null, position_s: s.player.position_seconds, duration_s: s.player.duration_seconds } : null;
  const notes = s.notesPlayer ? { position_s: s.notesPlayer.position_seconds, duration_s: s.notesPlayer.duration_seconds } : null;
  const song = s.transport?.playing ? { bar: s.transport.bar, beat: s.transport.beat, seconds: s.transport.seconds, source: s.transport.source } : null;
  const recording = s.mode === "recording";
  const metronome = s.metronomeOn;
  const live = s.liveRunning;
  return { recording, take, notes, background, song, metronome, live, any: recording || take !== null || notes !== null || background.any || song !== null || metronome || live };
}

/**
 * Stop every engine that makes sound: take playback, the notes player, the background
 * layers (remembered so "play background" brings them back), the song transport, the
 * metronome, the live follower, a sample preview and an instrument audition. A running
 * recording is left alone: it is stopped from "Play & record", which also files the take.
 */
export async function stopAll(): Promise<void> {
  const s = useStore.getState();
  const jobs: Promise<unknown>[] = [];
  if (s.mode === "playing") jobs.push(stopPlaying());
  if (s.notesPlayer) {
    jobs.push(api.stopNotes().catch(() => undefined));
    s.setNotesPlayer(null);
  }
  if (s.layers.drone || s.layers.pad || s.layers.ambient || s.layers.rhythm || s.layers.scapes) {
    s.setLastLayers({ ...s.layers });
    s.setLayer("drone", false);
    s.setLayer("pad", false);
    s.setLayer("ambient", false);
    s.setLayer("rhythm", false);
    s.setLayer("scapes", false);
  }
  if (s.transport?.playing) jobs.push(api.transportStop().then((st) => useStore.getState().setTransport(st)).catch(() => undefined));
  if (s.metronomeOn) s.setMetronomeOn(false);
  if (s.liveRunning) {
    jobs.push(api.liveStop().catch(() => undefined));
    s.setLiveRunning(false);
  }
  jobs.push(api.samplePreviewStop().catch(() => undefined));
  jobs.push(api.instrumentAuditionStop().catch(() => undefined));
  await Promise.all(jobs);
}

/** Start the notes player on the loaded take's notes (the ♪ of "Play & record"). */
export async function playNotes(): Promise<void> {
  const st = useStore.getState();
  if (!st.analysis || st.analysis.notes.length === 0) return;
  const ps = await api.playNotes(st.prefs.selectedOutput, st.prefs.voice);
  st.setNotesPlayer(ps);
}

export async function stopNotes(): Promise<void> {
  await api.stopNotes().catch(() => undefined);
  useStore.getState().setNotesPlayer(null);
}

/**
 * Polls the take player while it plays and the notes player while it runs, wherever the
 * user is. Mount once (the Now-playing bar does).
 */
export function useNowPlayingPolls() {
  const playing = useStore((s) => s.mode === "playing");
  const notesOn = useStore((s) => s.notesPlayer !== null);

  useEffect(() => {
    if (!playing) return;
    let cancelled = false;
    const loop = async () => {
      if (cancelled) return;
      const st = useStore.getState();
      if (st.mode !== "playing") return;
      try {
        const p = await api.playbackStatus();
        st.setPlayer(p);
        if (!p || p.finished) {
          st.setMode("idle");
          st.setPlayer(null);
          st.setCursor(0);
          return;
        }
      } catch (e) {
        useStore.getState().setError(describeError(e));
        useStore.getState().setMode("idle");
        useStore.getState().setPlayer(null);
        return;
      }
      setTimeout(loop, POLL_MS);
    };
    void loop();
    return () => {
      cancelled = true;
    };
  }, [playing]);

  useEffect(() => {
    if (!notesOn) return;
    let cancelled = false;
    const tick = async () => {
      if (cancelled) return;
      try {
        const p = await api.notesPlaybackStatus();
        if (!p || p.finished) {
          useStore.getState().setNotesPlayer(null);
          return;
        }
        useStore.getState().setNotesPlayer(p);
      } catch (e) {
        useStore.getState().setError(describeError(e));
        useStore.getState().setNotesPlayer(null);
        return;
      }
      setTimeout(tick, POLL_MS);
    };
    void tick();
    return () => {
      cancelled = true;
    };
  }, [notesOn]);
}
