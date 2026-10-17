// UI state. Only reduced data (status, peaks, info) lives here — never raw audio frames.
// Per-user preferences (language, devices, export format, metronome toggles) persist in
// localStorage; project data lives in the project on disk.

import { create } from "zustand";
import type {
  Ambient,
  AmbientStatus,
  AnalysisView,
  RhythmPrefs,
  RhythmStatus,
  DeviceInfo,
  DroneStatus,
  Emotion,
  HarmonyProposal,
  IntentSettings,
  Intonation,
  PadNote,
  PadStatus,
  PerformanceAnalysis,
  LoadedTakeInfo,
  MidiEvent,
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
export type Screen = "home" | "play" | "compose" | "studio" | "ambient" | "rhythm" | "library";
export type LayerName = "drone" | "pad" | "ambient" | "rhythm";
export type LayerSwitches = Record<LayerName, boolean>;

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
  /** Snap the selection (loop) to bar boundaries on MIDI takes with a known meter. */
  snapBar: boolean;
  /** MIDI input port name, or null for none. */
  midiInput: string | null;
  /** Drone settings (F3.2); `rootMidi` null until the user picks one (no assumed key). */
  drone: DronePrefs;
  /** Pad settings (F3.3). */
  pad: PadPrefs;
  /** Error-tolerant performance intelligence (F2.11): switch, mode, sliders, locks. */
  intent: IntentSettings;
  /** Harmony proposal sliders (F3.4); locks are shared with `intent`. */
  harmony: HarmonyPrefs;
  /** Master level of the background (drone + pad) in dB. */
  masterDb: number;
  /** Ambient preset ids marked as favourites / recently loaded (F1A.3). */
  favPresets: string[];
  recentPresets: string[];
  /** Rhythm section settings (Phase 4). */
  rhythm: RhythmPrefs;
  /** Level controls on channel strips: vertical sliders or rotary knobs. */
  stripControl: "slider" | "knob";
}

export const DEFAULT_RHYTHM: RhythmPrefs = {
  drums: { on: true, style: "liquid", complexity: 0.4, humanize: 0.3, swing: 0, fill_every_bars: 4, seed: 1, level_db: -10, kick_hz: 50, decay: 0.5, brightness: 0.5, locked: false },
  bass: { on: true, voice: 0, density: 0.4, syncopation: 0.3, passing: 0.3, register_low: 31, register_high: 43, seed: 1, level_db: -12, cutoff_hz: 400, resonance: 0.2, drive: 0.3, glide_s: 0.08, sidechain: 0.6, wobble_div: 0, wobble_depth: 0.5, warmth: 0.5, locked: false },
  ostinato: { on: false, steps: 8, step_beats: 0.5, density: 0.6, probability: 1, variation_every_bars: 4, swing: 0, seed: 1, level_db: -16, register_low: 55, register_high: 79, timbre: 2, release_s: 0.15, locked: false, motif: null },
  arp: { on: false, direction: "up", octaves: 1, step_beats: 0.25, gate: 0.6, density: 1, swing: 0, seed: 1, level_db: -18, register_low: 60, timbre: 0, release_s: 0.12, locked: false },
  bars: 4,
};

export interface HarmonyPrefs {
  tension: number;
  movement: number;
  richness: number;
  seed: number;
  registerLow: number;
  registerHigh: number;
}

export const DEFAULT_HARMONY: HarmonyPrefs = {
  tension: 0.3,
  movement: 0.5,
  richness: 0.5,
  seed: 1,
  registerLow: 48,
  registerHigh: 72,
};

export const DEFAULT_INTENT: IntentSettings = {
  enabled: true,
  mode: "assisted",
  tolerance: 0.5,
  sensitivity: 0.5,
  locks: { tonic: false, mode: false, chord: false, drone: false },
};

export type Voicing = "fifth" | "sus2" | "sus4" | "fifth9" | "scale_triad" | "scale_7th";
export const VOICINGS: Voicing[] = ["fifth", "sus2", "sus4", "fifth9", "scale_triad", "scale_7th"];

export interface PadPrefs {
  rootMidi: number | null;
  voicing: Voicing;
  emotion: Emotion;
  x: number;
  y: number;
  levelDb: number;
  lfoHz: number;
  seed: number;
  intonation: Intonation;
}

export const DEFAULT_PAD: PadPrefs = {
  rootMidi: null,
  voicing: "fifth",
  emotion: "calm",
  x: 0.5,
  y: 0.5,
  levelDb: -14,
  lfoHz: 0.08,
  seed: 1,
  intonation: "profile",
};

export interface DronePrefs {
  rootMidi: number | null;
  rootCents: number;
  fifth: number;
  octave: number;
  sub: number;
  warmth: number;
  movement: number;
  breath: number;
  detuneCents: number;
  cutoffHz: number;
  levelDb: number;
  intonation: Intonation;
}

export const DEFAULT_DRONE: DronePrefs = {
  rootMidi: null,
  rootCents: 0,
  fifth: 0.6,
  octave: 0.3,
  sub: 0,
  warmth: 0.4,
  movement: 0.2,
  breath: 0.3,
  detuneCents: 6,
  cutoffHz: 2500,
  levelDb: -12,
  intonation: "profile",
};

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
  snapBar: true,
  midiInput: null,
  drone: DEFAULT_DRONE,
  pad: DEFAULT_PAD,
  intent: DEFAULT_INTENT,
  harmony: DEFAULT_HARMONY,
  masterDb: 0,
  favPresets: [],
  recentPresets: [],
  rhythm: DEFAULT_RHYTHM,
  stripControl: "slider",
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
  /** Notes currently held on the MIDI keyboard (lights, trainer). */
  midiHeld: number[];
  /** Most recent MIDI events batch (consumed by the trainer). */
  midiEvents: MidiEvent[];
  setMidiState: (held: number[], events: MidiEvent[]) => void;
  midiPorts: string[];
  setMidiPorts: (p: string[]) => void;
  /** "Play the notes" synth playback status, or null when not playing. */
  notesPlayer: PlayerStatus | null;
  setNotesPlayer: (p: PlayerStatus | null) => void;

  /** Peak hold for the input meter, decays in the UI tick. */
  meterDb: number;
  setMeterDb: (db: number) => void;

  /** Background layers on the audio graph (F3.2/F3.3); run by `useBackgroundLayers`. */
  layers: LayerSwitches;
  setLayer: (name: LayerName, on: boolean) => void;
  /** Which layers were on before "Stop background", so "Play background" brings them back. */
  lastLayers: LayerSwitches;
  setLastLayers: (l: LayerSwitches) => void;
  /** The project's ambient rack (F1A.1), loaded when a project opens / the screen mounts. */
  ambient: Ambient | null;
  setAmbient: (a: Ambient | null) => void;
  ambientStatus: AmbientStatus | null;
  setAmbientStatus: (s: AmbientStatus | null) => void;
  rhythmStatus: RhythmStatus | null;
  setRhythmStatus: (s: RhythmStatus | null) => void;
  /** Standalone metronome switch (the click without recording/playback). */
  metronomeOn: boolean;
  setMetronomeOn: (on: boolean) => void;
  /** Sleep timer: epoch ms at which the background fades out, or null. */
  sleepAt: number | null;
  setSleepAt: (t: number | null) => void;
  droneStatus: DroneStatus | null;
  setDroneStatus: (s: DroneStatus | null) => void;
  padStatus: PadStatus | null;
  setPadStatus: (s: PadStatus | null) => void;
  /** Compose First asked for a recording: the studio starts it on arrival. */
  pendingRecord: boolean;
  setPendingRecord: (p: boolean) => void;
  /** F2.11 analysis of the loaded take's notes under the current intent settings. */
  performance: PerformanceAnalysis | null;
  setPerformance: (p: PerformanceAnalysis | null) => void;
  /** F3.4 proposals for the loaded take (three strategies), or null. */
  harmony: HarmonyProposal[] | null;
  setHarmony: (h: HarmonyProposal[] | null) => void;
  /** A/B slots: index into `harmony` armed on each side, and which side plays. */
  armed: { a: number | null; b: number | null; side: "a" | "b" };
  setArmed: (a: { a: number | null; b: number | null; side: "a" | "b" }) => void;
  /** While an armed proposal follows playback: the chord the pad should play and the
   *  drone root — overrides the pad voicing / drone root prefs. */
  padChord: PadNote[] | null;
  droneRootOverride: number | null;
  setFollow: (chord: PadNote[] | null, droneRoot: number | null) => void;

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

  layers: { drone: false, pad: false, ambient: false, rhythm: false },
  setLayer: (name, on) => set({ layers: { ...get().layers, [name]: on } }),
  lastLayers: { drone: true, pad: false, ambient: false, rhythm: false },
  setLastLayers: (lastLayers) => set({ lastLayers }),
  ambient: null,
  setAmbient: (ambient) => set({ ambient }),
  ambientStatus: null,
  setAmbientStatus: (ambientStatus) => set({ ambientStatus }),
  rhythmStatus: null,
  setRhythmStatus: (rhythmStatus) => set({ rhythmStatus }),
  metronomeOn: false,
  setMetronomeOn: (metronomeOn) => set({ metronomeOn }),
  sleepAt: null,
  setSleepAt: (sleepAt) => set({ sleepAt }),
  droneStatus: null,
  setDroneStatus: (droneStatus) => set({ droneStatus }),
  padStatus: null,
  setPadStatus: (padStatus) => set({ padStatus }),
  pendingRecord: false,
  setPendingRecord: (pendingRecord) => set({ pendingRecord }),
  performance: null,
  setPerformance: (performance) => set({ performance }),
  harmony: null,
  setHarmony: (harmony) => set({ harmony }),
  armed: { a: null, b: null, side: "a" },
  setArmed: (armed) => set({ armed }),
  padChord: null,
  droneRootOverride: null,
  setFollow: (padChord, droneRootOverride) => set({ padChord, droneRootOverride }),

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
  midiHeld: [],
  midiEvents: [],
  setMidiState: (midiHeld, midiEvents) => set({ midiHeld, midiEvents }),
  midiPorts: [],
  setMidiPorts: (midiPorts) => set({ midiPorts }),
  notesPlayer: null,
  setNotesPlayer: (notesPlayer) => set({ notesPlayer }),

  meterDb: -120,
  setMeterDb: (meterDb) => set({ meterDb }),

  message: null,
  error: null,
  setMessage: (message) => set({ message, error: null }),
  setError: (error) => set({ error }),
}));
