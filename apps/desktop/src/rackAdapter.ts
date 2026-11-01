// The two racks (atmosphere, Soundscapes) share one editor: Ambient.tsx, the Modules view
// and the Program panel read and write "their" rack through this adapter instead of
// assuming the atmosphere. Presets are shared (same format), the engine keeps the racks
// apart (own slots, bus, file).
import { api, type Ambient, type AmbientStatus, type AmbientView } from "./api";
import { useStore, type LayerName } from "./store";

export type RackKind = "ambient" | "scapes";

type S = ReturnType<typeof useStore.getState>;

export interface RackApi {
  kind: RackKind;
  /** The background layer switch that plays this rack. */
  layer: LayerName;
  get: () => Promise<AmbientView>;
  set: (a: Ambient) => Promise<AmbientView>;
  status: () => Promise<AmbientStatus>;
  programSet: (playing: boolean | null, position: number | null) => Promise<AmbientStatus>;
  read: (s: S) => Ambient | null;
  readStatus: (s: S) => AmbientStatus | null;
  store: (a: Ambient | null) => void;
  storeStatus: (st: AmbientStatus | null) => void;
}

export const RACKS: Record<RackKind, RackApi> = {
  ambient: {
    kind: "ambient",
    layer: "ambient",
    get: () => api.ambientGet(),
    set: (a) => api.setAmbient(a),
    status: () => api.ambientStatus(),
    programSet: (p, pos) => api.ambientProgramSet(p, pos),
    read: (s) => s.ambient,
    readStatus: (s) => s.ambientStatus,
    store: (a) => useStore.getState().setAmbient(a),
    storeStatus: (st) => useStore.getState().setAmbientStatus(st),
  },
  scapes: {
    kind: "scapes",
    layer: "scapes",
    get: () => api.scapesGet(),
    set: (a) => api.setScapes(a),
    status: () => api.scapesStatus(),
    programSet: (p, pos) => api.scapesProgramSet(p, pos),
    read: (s) => s.scapes,
    readStatus: (s) => s.scapesStatus,
    store: (a) => useStore.getState().setScapes(a),
    storeStatus: (st) => useStore.getState().setScapesStatus(st),
  },
};
