// History of rack edits (owner 1.11.2026: "искам някъде history на промените, timeline,
// за да мога да връщам назад/напред, ако много се обърка след моята намеса"). One list per
// rack (atmosphere, Soundscapes): every change is a step with a label (which layer, which
// knob, new layer, scene, journey, preset…); Undo / Redo walk the list, "jump" restores any
// step. Kept in memory for the session (the project file always holds the current state).
import { create } from "zustand";
import type { Ambient, AmbientLayer } from "./api";
import type { RackKind } from "./rackAdapter";

export const HISTORY_MAX = 200;
/** Continuous knob moves with the same label inside this window collapse into one step. */
export const COALESCE_MS = 800;

export interface HistoryEntry {
  rack: Ambient;
  label: string;
  at: number;
}

export interface RackHistory {
  /** Chronological: entries[i].rack is the state after change i; entries[0] is the start. */
  entries: HistoryEntry[];
  /** The step the rack is at now (normally the last one). */
  index: number;
}

export const EMPTY_HISTORY: RackHistory = { entries: [], index: -1 };

/** Labels for describeChange; the UI passes translated names. */
export interface ChangeWords {
  start: string;
  layerAdded: (name: string) => string;
  layerRemoved: (name: string) => string;
  layerRenamed: (from: string, to: string) => string;
  layerParam: (name: string, param: string) => string;
  macro: (key: string) => string;
  scenes: string;
  automation: string;
  many: string;
}

function layerDiff(a: AmbientLayer, b: AmbientLayer): string[] {
  const keys: string[] = [];
  if (a.name !== b.name) keys.push("name");
  for (const k of ["gain_db", "pan", "lowpass_hz", "highpass_hz", "mute", "solo", "seed", "locked"] as const) if (a[k] !== b[k]) keys.push(k);
  const sa = a.source as unknown as Record<string, unknown>;
  const sb = b.source as unknown as Record<string, unknown>;
  for (const k of new Set([...Object.keys(sa), ...Object.keys(sb)])) if (JSON.stringify(sa[k]) !== JSON.stringify(sb[k])) keys.push(k);
  return keys;
}

/** One short label for what changed between two racks. */
export function describeChange(prev: Ambient | null, next: Ambient, w: ChangeWords): string {
  if (!prev) return w.start;
  const found: string[] = [];
  const prevIds = new Set(prev.layers.map((l) => l.id));
  const nextIds = new Set(next.layers.map((l) => l.id));
  for (const l of next.layers) if (!prevIds.has(l.id)) found.push(w.layerAdded(l.name));
  for (const l of prev.layers) if (!nextIds.has(l.id)) found.push(w.layerRemoved(l.name));
  for (const l of next.layers) {
    const p = prev.layers.find((x) => x.id === l.id);
    if (!p) continue;
    const keys = layerDiff(p, l);
    if (keys.length === 0) continue;
    if (keys.includes("name")) found.push(w.layerRenamed(p.name, l.name));
    else found.push(w.layerParam(l.name, keys.length === 1 ? keys[0] : `${keys[0]} +${keys.length - 1}`));
  }
  const pm = prev.macros as unknown as Record<string, number>;
  const nm = next.macros as unknown as Record<string, number>;
  for (const k of Object.keys(nm)) if (pm[k] !== nm[k]) found.push(w.macro(k));
  if (JSON.stringify(prev.program) !== JSON.stringify(next.program)) found.push(w.scenes);
  if (JSON.stringify(prev.automation) !== JSON.stringify(next.automation)) found.push(w.automation);
  if (found.length === 0) return w.many;
  if (found.length === 1) return found[0];
  if (found.length === 2) return `${found[0]}, ${found[1]}`;
  return `${found[0]} (+${found.length - 1})`;
}

/** Record a new state. Drops the redo branch; coalesces a repeated label inside COALESCE_MS. */
export function pushHistory(h: RackHistory, rack: Ambient, label: string, at = Date.now()): RackHistory {
  let entries = h.index >= 0 ? h.entries.slice(0, h.index + 1) : [];
  const last = entries[entries.length - 1];
  if (last && last.label === label && at - last.at < COALESCE_MS && entries.length > 1) {
    entries = [...entries.slice(0, -1), { rack, label, at: last.at }];
  } else {
    entries = [...entries, { rack, label, at }];
    if (entries.length > HISTORY_MAX) entries = entries.slice(entries.length - HISTORY_MAX);
  }
  return { entries, index: entries.length - 1 };
}

/** Make sure the start state is recorded before the first change. */
export function seedHistory(h: RackHistory, rack: Ambient, startLabel: string, at = Date.now()): RackHistory {
  if (h.entries.length > 0) return h;
  return { entries: [{ rack, label: startLabel, at }], index: 0 };
}

export function canUndo(h: RackHistory): boolean {
  return h.index > 0;
}
export function canRedo(h: RackHistory): boolean {
  return h.index >= 0 && h.index < h.entries.length - 1;
}

/** Move to step `index`; returns the rack to apply, or null when out of range. */
export function jumpHistory(h: RackHistory, index: number): { history: RackHistory; rack: Ambient } | null {
  if (index < 0 || index >= h.entries.length) return null;
  return { history: { ...h, index }, rack: h.entries[index].rack };
}
export function undoHistory(h: RackHistory) {
  return canUndo(h) ? jumpHistory(h, h.index - 1) : null;
}
export function redoHistory(h: RackHistory) {
  return canRedo(h) ? jumpHistory(h, h.index + 1) : null;
}

interface HistoryStore {
  byRack: Record<RackKind, RackHistory>;
  set: (kind: RackKind, h: RackHistory) => void;
  reset: () => void;
}

/** Session memory of both racks' histories (not persisted: the project holds the current state). */
export const useHistoryStore = create<HistoryStore>((set) => ({
  byRack: { ambient: EMPTY_HISTORY, scapes: EMPTY_HISTORY },
  set: (kind, h) => set((s) => ({ byRack: { ...s.byRack, [kind]: h } })),
  reset: () => set({ byRack: { ambient: EMPTY_HISTORY, scapes: EMPTY_HISTORY } }),
}));

/** Record a change from anywhere (editor, journey, bridge): seeds the start state first. */
export function recordChange(kind: RackKind, prev: Ambient | null, next: Ambient, label: string, startLabel: string) {
  const st = useHistoryStore.getState();
  let h = st.byRack[kind];
  if (h.entries.length === 0 && prev) h = seedHistory(h, prev, startLabel);
  st.set(kind, pushHistory(h, next, label));
}
