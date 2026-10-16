// UI state. Only reduced data (status, peaks, info) lives here — never raw audio frames.
// Per-user preferences (language, devices, export format, metronome toggles) persist in
// localStorage; project data lives in the project on disk.

import { create } from "zustand";
import type {
  DeviceInfo,
  LoadedTakeInfo,
  OpenReport,
  PlayerStatus,
  ProjectView,
  RecordingStatus,
  WaveformPeaks,
  WavFormat,
} from "./api";
import type { Lang } from "./i18n";

export type Mode = "idle" | "recording" | "playing";
export type Screen = "home" | "play";

export interface Prefs {
  lang: Lang;
  selectedInput: string | null;
  selectedOutput: string | null;
  sampleRate: number | null;
  exportFormat: WavFormat;
  clickRec: boolean;
  countIn: number;
  clickPlay: boolean;
  loop: boolean;
}

const PREFS_KEY = "ethnoflow.prefs.v1";

export const DEFAULT_PREFS: Prefs = {
  lang: "bg",
  selectedInput: null,
  selectedOutput: null,
  sampleRate: null,
  exportFormat: "float32",
  clickRec: false,
  countIn: 0,
  clickPlay: false,
  loop: false,
};

function loadPrefs(): Prefs {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    if (!raw) return DEFAULT_PREFS;
    return { ...DEFAULT_PREFS, ...(JSON.parse(raw) as Partial<Prefs>) };
  } catch {
    return DEFAULT_PREFS;
  }
}

function savePrefs(p: Prefs) {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(p));
  } catch {
    /* storage unavailable: preferences just don't persist */
  }
}

interface Store {
  prefs: Prefs;
  setPref: <K extends keyof Prefs>(k: K, v: Prefs[K]) => void;

  screen: Screen;
  setScreen: (s: Screen) => void;

  inputDevices: DeviceInfo[];
  outputDevices: DeviceInfo[];
  host: string;
  setDevices: (i: DeviceInfo[], o: DeviceInfo[], host: string) => void;

  project: ProjectView | null;
  report: OpenReport | null;
  setProject: (p: ProjectView | null, report?: OpenReport | null) => void;

  loaded: LoadedTakeInfo | null;
  peaks: WaveformPeaks | null;
  setLoaded: (l: LoadedTakeInfo | null, p: WaveformPeaks | null) => void;
  setPeaks: (p: WaveformPeaks | null) => void;

  mode: Mode;
  setMode: (m: Mode) => void;
  rec: RecordingStatus | null;
  setRec: (s: RecordingStatus | null) => void;
  player: PlayerStatus | null;
  setPlayer: (s: PlayerStatus | null) => void;

  /** Cursor in seconds within the loaded (trimmed) take. */
  cursor: number;
  setCursor: (s: number) => void;
  /** Selected region [start, end) in seconds of the loaded take, or null. */
  selection: [number, number] | null;
  setSelection: (sel: [number, number] | null) => void;

  /** Label of the command in flight (shown as a busy indicator), or null. */
  pending: string | null;
  setPending: (p: string | null) => void;

  /** Peak hold for the input meter, decays in the UI tick. */
  meterDb: number;
  setMeterDb: (db: number) => void;

  message: string | null;
  error: string | null;
  setMessage: (m: string | null) => void;
  setError: (e: string | null) => void;
}

export const useStore = create<Store>((set, get) => ({
  prefs: loadPrefs(),
  setPref: (k, v) => {
    const prefs = { ...get().prefs, [k]: v };
    savePrefs(prefs);
    set({ prefs });
  },

  screen: "home",
  setScreen: (screen) => set({ screen }),

  inputDevices: [],
  outputDevices: [],
  host: "",
  setDevices: (inputDevices, outputDevices, host) => set({ inputDevices, outputDevices, host }),

  project: null,
  report: null,
  setProject: (project, report) =>
    set(report === undefined ? { project } : { project, report }),

  loaded: null,
  peaks: null,
  setLoaded: (loaded, peaks) => set({ loaded, peaks, cursor: 0, selection: null }),
  setPeaks: (peaks) => set({ peaks }),

  mode: "idle",
  setMode: (mode) => set({ mode }),
  rec: null,
  setRec: (rec) => set({ rec }),
  player: null,
  setPlayer: (player) => set({ player }),

  cursor: 0,
  setCursor: (cursor) => set({ cursor }),
  selection: null,
  setSelection: (selection) => set({ selection }),

  pending: null,
  setPending: (pending) => set({ pending }),

  meterDb: -120,
  setMeterDb: (meterDb) => set({ meterDb }),

  message: null,
  error: null,
  setMessage: (message) => set({ message, error: null }),
  setError: (error) => set({ error }),
}));
