import { type Ambient as AmbientRack } from "./api";
import { RACKS, type RackKind } from "./rackAdapter";

const COMMIT_MS = 80;
const pending: Record<RackKind, AmbientRack | null> = { ambient: null, scapes: null };
const timers: Record<RackKind, number | null> = { ambient: null, scapes: null };

/**
 * Local state now, save + apply shortly after (knobs fire continuously). One debouncer
 * per rack for every screen that edits it (the Modules view, Library loads), so two
 * quick edits never race each other on the way to the shell. `kind` picks the rack —
 * before 2026-11-01 (evening) this always wrote the atmosphere, so editing modules on
 * the Soundscapes tab changed the atmosphere instead (owner: "they became the same again").
 */
export function commitRack(next: AmbientRack, fail: (e: unknown) => void, kind: RackKind = "ambient"): void {
  const R = RACKS[kind];
  R.store(next);
  pending[kind] = next;
  if (timers[kind] !== null) return;
  timers[kind] = window.setTimeout(() => {
    timers[kind] = null;
    const p = pending[kind];
    pending[kind] = null;
    if (!p) return;
    R.set(p)
      .then((v) => {
        // Keep newer local edits if any arrived while saving.
        if (pending[kind] === null) R.store(v.ambient);
      })
      .catch(fail);
  }, COMMIT_MS);
}

/** Save what is still pending (on unmount / before switching screens). */
export function flushRack(kind?: RackKind): void {
  for (const k of (kind ? [kind] : ["ambient", "scapes"]) as RackKind[]) {
    if (timers[k] === null) continue;
    window.clearTimeout(timers[k]!);
    timers[k] = null;
    const p = pending[k];
    pending[k] = null;
    if (p) void RACKS[k].set(p).catch(() => undefined);
  }
}
