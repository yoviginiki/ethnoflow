// UI state. Only reduced data (status, peaks, info) lives here — never raw audio frames.
// Per-user preferences (language, devices, export format, metronome toggles) persist in
// localStorage; project data lives in the project on disk.

import { create } from "zustand";
import type {
  AnalysisView,
  DeviceInfo,
  LoadedTakeInfo,
  MidiTakeView,
  MonitorStatus,
  Note,
  Voice,
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
  tuner: boolean;
  notesView: "roll" | "falling";
  keyNames: boolean;
  voice: Voice;
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
  tuner: false,
  notesView: "roll",
  keyNames: true,
  voice: "flute",
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

  /** Pitch analysis of the loaded take (contour, notes, hypotheses), or null. */
  analysis: AnalysisView | null;
  /** New analysis (load/analyse): clears selection and undo. */
  setAnalysis: (a: AnalysisView | null) => void;
  /** Same analysis after an edit: keeps selection and undo. */
  patchAnalysis: (a: AnalysisView) => void;
  /** Index into analysis.notes, or null. */
  selectedNote: number | null;
  setSelectedNote: (i: number | null) => void;
  /** Undo stack of note lists (most recent last). */
  notesUndo: Note[][];
  pushUndo: (notes: Note[]) => void;
  popUndo: () => Note[] | undefined;
  notesRedo: Note[][];
  pushRedo: (notes: Note[]) => void;
  popRedo: () => Note[] | undefined;

  /** Live tuner reading while idle (input monitor), or null. */
  monitor: MonitorStatus | null;
  setMonitor: (m: MonitorStatus | null) => void;

  /** Loaded take is a MIDI file: its tracks and render options. */
  midi: MidiTakeView | null;
  setMidi: (m: MidiTakeView | null) => void;
  /** "Play the notes" synth playback status, or null when not playing. */
  notesPlayer: PlayerStatus | null;
  setNotesPlayer: (p: PlayerStatus | null) => void;

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
  setLoaded: (loaded, peaks) =>
    set({ loaded, peaks, cursor: 0, selection: null, analysis: null, selectedNote: null, notesUndo: [], midi: null, notesPlayer: null }),
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

  analysis: null,
  setAnalysis: (analysis) => set({ analysis, selectedNote: null, notesUndo: [], notesRedo: [] }),
  patchAnalysis: (analysis) =>
    set((st) => ({
      analysis,
      selectedNote: st.selectedNote !== null && st.selectedNote < analysis.notes.length ? st.selectedNote : null,
    })),
  selectedNote: null,
  setSelectedNote: (selectedNote) => set({ selectedNote }),
  notesUndo: [],
  pushUndo: (notes) => set({ notesUndo: [...get().notesUndo.slice(-49), notes], notesRedo: [] }),
  popUndo: () => {
    const stack = get().notesUndo;
    if (stack.length === 0) return undefined;
    set({ notesUndo: stack.slice(0, -1) });
    return stack[stack.length - 1];
  },
  notesRedo: [],
  pushRedo: (notes) => set({ notesRedo: [...get().notesRedo.slice(-49), notes] }),
  popRedo: () => {
    const stack = get().notesRedo;
    if (stack.length === 0) return undefined;
    set({ notesRedo: stack.slice(0, -1) });
    return stack[stack.length - 1];
  },

  monitor: null,
  setMonitor: (monitor) => set({ monitor }),

  midi: null,
  setMidi: (midi) => set({ midi }),
  notesPlayer: null,
  setNotesPlayer: (notesPlayer) => set({ notesPlayer }),

  meterDb: -120,
  setMeterDb: (meterDb) => set({ meterDb }),

  message: null,
  error: null,
  setMessage: (message) => set({ message, error: null }),
  setError: (error) => set({ error }),
}));
