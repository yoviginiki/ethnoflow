// Loading and playing a take, shared by "Play & record" (the ▶ on every row of the take
// list, the main transport) and the assistant bridge (`load_take` / `play_take`), so the
// terminal goes through exactly the path the owner clicks (UX audit 2026-10-31, stage 1).
import { api, type TakeView } from "./api";
import { useStore } from "./store";

export const WAVEFORM_BINS = 1600;

/** The most recent take of the project (highest number), or null. */
export function latestTake(takes: TakeView[]): TakeView | null {
  return takes.reduce<TakeView | null>((a, b) => (a && a.number > b.number ? a : b), null);
}

/** Stop playback if it runs, then load the take with its waveform, analysis and MIDI view. */
export async function loadTake(id: string): Promise<void> {
  const st = useStore.getState();
  if (st.mode === "playing") {
    await api.stopPlayback().catch(() => undefined);
    st.setMode("idle");
    st.setPlayer(null);
  }
  const info = await api.loadTake(id);
  const peaks = await api.getWaveform(WAVEFORM_BINS);
  useStore.getState().setLoaded(info, peaks);
  useStore.getState().setAnalysis(await api.getAnalysis());
  useStore.getState().setMidi(await api.midiTake());
}

/**
 * Play the loaded take from `from` (default: the cursor, kept inside the selection when
 * there is one; from the start when the cursor sits at the end). `click` adds the
 * metronome. Sound comes out of the chosen output.
 */
export async function playLoaded(from: number | undefined, click: boolean): Promise<void> {
  const st = useStore.getState();
  if (!st.loaded) return;
  const dur = st.loaded.duration_seconds;
  const sel = st.selection;
  let start = from ?? st.cursor;
  if (from === undefined && sel && (start < sel[0] || start >= sel[1])) start = sel[0];
  if (start >= dur - 0.01) start = sel ? sel[0] : 0;
  const ps = await api.startPlayback({
    device: st.prefs.selectedOutput,
    start_seconds: start,
    loop_seconds: st.prefs.loop ? (sel ?? [0, dur]) : null,
    click,
  });
  st.setPlayer(ps);
  st.setMode("playing");
}

/**
 * Home's "Continue": open "Play & record" with the latest take loaded, or empty and ready
 * to record when the project has no takes yet. Shared with the bridge op `continue`.
 */
export async function continueLast(): Promise<string | null> {
  const st = useStore.getState();
  const latest = st.project ? latestTake(st.project.takes) : null;
  st.setScreen("play");
  if (latest && st.loaded?.id !== latest.id) await loadTake(latest.id);
  return latest?.id ?? null;
}

/** Stop playback and leave the cursor where it stopped. */
export async function stopPlaying(): Promise<void> {
  const st = useStore.getState();
  try {
    await api.stopPlayback();
  } finally {
    if (st.player) st.setCursor(Math.min(st.player.position_seconds, st.loaded?.duration_seconds ?? 0));
    st.setMode("idle");
    st.setPlayer(null);
  }
}
