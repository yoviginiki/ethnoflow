import { api, type Ambient as AmbientRack } from "./api";
import { useStore } from "./store";

const COMMIT_MS = 80;
let pending: AmbientRack | null = null;
let timer: number | null = null;

/**
 * Local state now, save + apply shortly after (knobs fire continuously). One
 * debouncer for every screen that edits the rack (Soundscapes, Library loads), so
 * two quick edits never race each other on the way to the shell.
 */
export function commitRack(next: AmbientRack, fail: (e: unknown) => void): void {
  useStore.getState().setAmbient(next);
  pending = next;
  if (timer !== null) return;
  timer = window.setTimeout(() => {
    timer = null;
    const p = pending;
    pending = null;
    if (!p) return;
    api
      .setAmbient(p)
      .then((v) => {
        // Keep newer local edits if any arrived while saving.
        if (pending === null) useStore.getState().setAmbient(v.ambient);
      })
      .catch(fail);
  }, COMMIT_MS);
}

/** Save what is still pending (on unmount / before switching screens). */
export function flushRack(): void {
  if (timer === null) return;
  window.clearTimeout(timer);
  timer = null;
  const p = pending;
  pending = null;
  if (p) void api.setAmbient(p).catch(() => undefined);
}
