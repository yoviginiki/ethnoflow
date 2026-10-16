// UI state. Only reduced data (status, peaks, info) lives here — never raw audio frames.

import { create } from "zustand";
import type { DeviceInfo, PlayerStatus, RecorderStatus, TakeInfo, WaveformPeaks, WavFormat } from "./api";
import type { Lang } from "./i18n";

export type Mode = "idle" | "recording" | "playing";

interface Store {
  lang: Lang;
  setLang: (l: Lang) => void;

  inputDevices: DeviceInfo[];
  outputDevices: DeviceInfo[];
  selectedInput: string | null;
  selectedOutput: string | null;
  setDevices: (i: DeviceInfo[], o: DeviceInfo[]) => void;
  setSelectedInput: (n: string | null) => void;
  setSelectedOutput: (n: string | null) => void;

  mode: Mode;
  setMode: (m: Mode) => void;
  recorder: RecorderStatus | null;
  setRecorder: (s: RecorderStatus | null) => void;
  player: PlayerStatus | null;
  setPlayer: (s: PlayerStatus | null) => void;

  take: TakeInfo | null;
  peaks: WaveformPeaks | null;
  setTake: (t: TakeInfo | null, p: WaveformPeaks | null) => void;

  exportFormat: WavFormat;
  setExportFormat: (f: WavFormat) => void;

  /** Peak hold for the input meter, decays in the UI tick. */
  meterDb: number;
  setMeterDb: (db: number) => void;

  message: string | null;
  error: string | null;
  setMessage: (m: string | null) => void;
  setError: (e: string | null) => void;
}

export const useStore = create<Store>((set) => ({
  lang: "bg",
  setLang: (lang) => set({ lang }),

  inputDevices: [],
  outputDevices: [],
  selectedInput: null,
  selectedOutput: null,
  setDevices: (inputDevices, outputDevices) => set({ inputDevices, outputDevices }),
  setSelectedInput: (selectedInput) => set({ selectedInput }),
  setSelectedOutput: (selectedOutput) => set({ selectedOutput }),

  mode: "idle",
  setMode: (mode) => set({ mode }),
  recorder: null,
  setRecorder: (recorder) => set({ recorder }),
  player: null,
  setPlayer: (player) => set({ player }),

  take: null,
  peaks: null,
  setTake: (take, peaks) => set({ take, peaks }),

  exportFormat: "pcm24",
  setExportFormat: (exportFormat) => set({ exportFormat }),

  meterDb: -120,
  setMeterDb: (meterDb) => set({ meterDb }),

  message: null,
  error: null,
  setMessage: (message) => set({ message, error: null }),
  setError: (error) => set({ error }),
}));
