// Typed wrappers around the Tauri commands in src-tauri/src/commands.rs.
// Keep the shapes in sync with the Rust structs (serde snake_case).

import { invoke } from "@tauri-apps/api/core";

export interface DeviceInfo {
  name: string;
  is_default: boolean;
  default_sample_rate: number | null;
  default_channels: number | null;
  default_sample_format: string | null;
}

export interface AudioHostInfo {
  host: string;
  os: string;
}

export interface RecordingStarted {
  device: string;
  sample_rate: number;
  channels: number;
}

export interface RecorderStatus {
  sample_rate: number;
  channels: number;
  frames_captured: number;
  seconds_captured: number;
  peak: number;
  overrun_samples: number;
  clipped_samples: number;
  stream_errors: number;
  capped: boolean;
  max_seconds: number;
  write_failed: boolean;
}

export type TakeSource = "none" | "microphone" | "imported_file";

export interface TakeInfo {
  source: TakeSource;
  label: string | null;
  sample_rate: number;
  channels: number;
  frames: number;
  duration_seconds: number;
  peak_dbfs: number;
  rms_dbfs: number;
  clipped_samples: number;
}

export interface WaveformPeaks {
  sample_rate: number;
  frames: number;
  duration_seconds: number;
  frames_per_bin: number;
  min: number[];
  max: number[];
}

export interface PlayerStatus {
  position_frames: number;
  position_seconds: number;
  total_frames: number;
  duration_seconds: number;
  finished: boolean;
  sample_rate: number;
  channels: number;
  stream_errors: number;
}

export type WavFormat = "float32" | "pcm24" | "pcm16";

export interface ExportResult {
  path: string;
  bytes: number;
  format: WavFormat;
}

export interface CmdError {
  code: string;
  message: string;
}

export function isCmdError(e: unknown): e is CmdError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

export function describeError(e: unknown): string {
  if (isCmdError(e)) return `${e.code}: ${e.message}`;
  if (e instanceof Error) return e.message;
  return String(e);
}

export const api = {
  audioHost: () => invoke<AudioHostInfo>("audio_host"),
  listInputDevices: () => invoke<DeviceInfo[]>("list_input_devices"),
  listOutputDevices: () => invoke<DeviceInfo[]>("list_output_devices"),

  startRecording: (device: string | null) =>
    invoke<RecordingStarted>("start_recording", { device }),
  recordingStatus: () => invoke<RecorderStatus | null>("recording_status"),
  stopRecording: () => invoke<TakeInfo>("stop_recording"),
  recordingWaveform: (bins: number) =>
    invoke<WaveformPeaks | null>("recording_waveform", { bins }),

  takeInfo: () => invoke<TakeInfo | null>("take_info"),
  getWaveform: (bins: number) => invoke<WaveformPeaks | null>("get_waveform", { bins }),
  clearTake: () => invoke<void>("clear_take"),

  startPlayback: (device: string | null, startSeconds: number) =>
    invoke<PlayerStatus>("start_playback", { device, startSeconds }),
  playbackStatus: () => invoke<PlayerStatus | null>("playback_status"),
  stopPlayback: () => invoke<void>("stop_playback"),

  exportWav: (path: string, format: WavFormat) =>
    invoke<ExportResult>("export_wav", { path, format }),
  importWav: (path: string) => invoke<TakeInfo>("import_wav", { path }),
};
