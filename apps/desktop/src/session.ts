import { useEffect, useRef } from "react";
import { api, describeError, type Ambient as AmbientRack, type LibraryKind } from "./api";
import { useStore, type LayerName, type Prefs, type Screen } from "./store";

/** The preference keys a session (or a per-project autosave) carries. */
export const SESSION_PREF_KEYS = ["drone", "pad", "intent", "harmony", "rhythm", "masterDb", "stripControl"] as const;
export type SessionPrefKey = (typeof SESSION_PREF_KEYS)[number];
export type SessionPrefs = Partial<Pick<Prefs, SessionPrefKey>>;

export interface SessionPayload {
  v: 1;
  prefs: SessionPrefs;
  /** The atmosphere rack at save time (null when the project had none). */
  ambient: AmbientRack | null;
  /** Which background layers were playing. */
  layers: Record<LayerName, boolean>;
}

const LAYER_NAMES: LayerName[] = ["drone", "pad", "ambient", "rhythm"];

/** Pure: pick the session keys out of the preferences. */
export function pickSessionPrefs(prefs: Prefs): SessionPrefs {
  const out: SessionPrefs = {};
  for (const k of SESSION_PREF_KEYS) {
    const v = prefs[k];
    if (v !== undefined) (out as Record<string, unknown>)[k] = v;
  }
  return out;
}

/** Pure: merge a saved session's preferences over the current ones (unknown keys ignored). */
export function mergeSessionPrefs(prefs: Prefs, saved: unknown): Prefs {
  if (!saved || typeof saved !== "object") return prefs;
  const src = saved as Record<string, unknown>;
  const next: Record<string, unknown> = { ...prefs };
  for (const k of SESSION_PREF_KEYS) {
    const v = src[k];
    if (v === undefined || v === null) continue;
    const cur = prefs[k];
    // Objects merge over the defaults so older sessions keep working when fields are added.
    next[k] = cur && typeof cur === "object" && typeof v === "object" ? { ...(cur as object), ...(v as object) } : v;
  }
  return next as unknown as Prefs;
}

/** Pure: is this value a session payload we can read? */
export function isSessionPayload(p: unknown): p is SessionPayload {
  return !!p && typeof p === "object" && (p as SessionPayload).v === 1 && typeof (p as SessionPayload).prefs === "object";
}

/** Pure: a rack with the loop layers whose take is not in this project removed. */
export function rackForProject(rack: AmbientRack, takeIds: Set<string>): AmbientRack {
  const layers = rack.layers.filter((l) => l.source.kind !== "loop" || takeIds.has(l.source.take_id));
  const ids = new Set(layers.map((l) => l.id));
  return { ...rack, layers, automation: rack.automation.filter((a) => a.target.layer === null || ids.has(a.target.layer)) };
}

export function collectSession(): SessionPayload {
  const s = useStore.getState();
  return { v: 1, prefs: pickSessionPrefs(s.prefs), ambient: s.ambient, layers: { ...s.layers } };
}

/** Apply a saved session: preferences, the rack, and (optionally) which layers play. */
export async function applySession(payload: unknown, restoreLayers: boolean): Promise<void> {
  if (!isSessionPayload(payload)) throw new Error("not a session");
  const s = useStore.getState();
  const merged = mergeSessionPrefs(s.prefs, payload.prefs);
  for (const k of SESSION_PREF_KEYS) if (merged[k] !== s.prefs[k]) s.setPref(k, merged[k]);
  if (payload.ambient && s.project) {
    const rack = rackForProject(payload.ambient, new Set(s.project.takes.map((t) => t.id)));
    const v = await api.setAmbient(rack);
    useStore.getState().setAmbient(v.ambient);
  }
  if (restoreLayers) {
    const want = payload.layers ?? ({} as Record<LayerName, boolean>);
    const st = useStore.getState();
    for (const name of LAYER_NAMES) {
      const on = !!want[name];
      if (st.layers[name] !== on) st.setLayer(name, on);
    }
    st.setLastLayers({ ...st.lastLayers, ...want });
  }
}

/** Apply a library entry of any kind; returns the screen where the result lives. */
export async function applyLibraryPayload(kind: LibraryKind, payload: unknown): Promise<Screen> {
  const s = useStore.getState();
  switch (kind) {
    case "drone":
    case "pad":
    case "intent":
    case "harmony":
    case "rhythm": {
      const cur = s.prefs[kind] as object;
      s.setPref(kind, { ...cur, ...(payload as object) } as never);
      return kind === "rhythm" ? "rhythm" : "compose";
    }
    case "ambient": {
      if (!s.project) throw new Error("no project");
      const rack = rackForProject(payload as AmbientRack, new Set(s.project.takes.map((t) => t.id)));
      const v = await api.setAmbient(rack);
      useStore.getState().setAmbient(v.ambient);
      return "ambient";
    }
    case "session":
      await applySession(payload, true);
      return "compose";
    default:
      return "home";
  }
}

/**
 * Every project remembers its own settings: on open the project's `session.json`
 * (if any) replaces the session preferences; afterwards changes are written back
 * (debounced). Which layers play is remembered but never auto-started.
 */
export function useProjectSession() {
  const root = useStore((s) => s.project?.root ?? null);
  const prefs = useStore((s) => s.prefs);
  const layers = useStore((s) => s.layers);
  const ready = useRef<string | null>(null);
  const fail = (e: unknown) => useStore.getState().setError(describeError(e));

  useEffect(() => {
    ready.current = null;
    if (!root) return;
    let cancelled = false;
    api
      .sessionGet()
      .then(async (p) => {
        if (cancelled) return;
        if (isSessionPayload(p)) {
          const s = useStore.getState();
          const merged = mergeSessionPrefs(s.prefs, p.prefs);
          for (const k of SESSION_PREF_KEYS) if (merged[k] !== s.prefs[k]) s.setPref(k, merged[k]);
          if (p.layers) s.setLastLayers({ ...s.lastLayers, ...p.layers });
        }
        ready.current = root;
      })
      .catch((e) => {
        fail(e);
        ready.current = root;
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [root]);

  const sessionPrefs = SESSION_PREF_KEYS.map((k) => prefs[k]);
  useEffect(() => {
    if (!root || ready.current !== root) return;
    const id = window.setTimeout(() => {
      const s = useStore.getState();
      if (s.project?.root !== root) return;
      const anyOn = LAYER_NAMES.some((n) => s.layers[n]);
      const payload: SessionPayload = { v: 1, prefs: pickSessionPrefs(s.prefs), ambient: null, layers: anyOn ? { ...s.layers } : { ...s.lastLayers } };
      api.sessionSet(payload).catch(() => undefined);
    }, 1000);
    return () => window.clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [root, ...sessionPrefs, layers]);
}
