// Typed wrappers around the Tauri commands in src-tauri/src/{commands,project_cmds}.rs.
// Keep the shapes in sync with the Rust structs (serde snake_case).

import { invoke } from "@tauri-apps/api/core";

export interface DeviceInfo {
  name: string;
  is_default: boolean;
  default_sample_rate: number | null;
  default_channels: number | null;
  default_sample_format: string | null;
  sample_rates: number[];
  channel_counts: number[];
}

export interface AudioHostInfo {
  host: string;
  os: string;
}

export interface GridPosition {
  bar: number;
  beat: number;
  beat_fraction: number;
}

export interface MetronomeStatus {
  frame: number;
  sample_rate: number;
  position: GridPosition | null;
  stream_errors: number;
}

export interface RecordingStatus {
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
  device_lost: boolean;
  noise_floor: number | null;
  pitch_hz: number | null;
  pitch_confidence: number;
  metronome: MetronomeStatus | null;
  count_in_seconds: number;
}

export interface RecordingStarted {
  take_id: string;
  device: string;
  sample_rate: number;
  channels: number;
  count_in_seconds: number;
}

export type TakeSource = "microphone" | "imported_file" | "derived" | "midi_file";
export type TakeState = "recording" | "complete" | "recovered";
export type TakeHealth = "ok" | "missing" | "checksum_mismatch" | "unverified";

// Sample Lab (F1A.6): audio-engine/src/process.rs + project db TakeMeta
export interface Recipe {
  dc_remove: boolean;
  highpass_hz: number | null;
  gain_db: number;
  normalize_peak_dbfs: number | null;
  fade_in_s: number;
  fade_out_s: number;
  loop_crossfade_s: number;
  reverse: boolean;
}
export interface TakeMeta {
  category: string;
  location: string;
  notes: string;
  mic: string;
  license: string;
  tags: string[];
}
export interface SilenceReport {
  sound_start: number | null;
  sound_end: number | null;
  silences: [number, number][];
  threshold_dbfs: number;
}

export interface TakeView {
  id: string;
  number: number;
  label: string | null;
  source: TakeSource;
  state: TakeState;
  created_at_ms: number;
  sample_rate: number | null;
  channels: number | null;
  duration_seconds: number | null;
  trim_in_seconds: number;
  trim_out_seconds: number | null;
  original_name: string | null;
  device: string | null;
  overrun_samples: number;
  clipped_samples: number;
  health: TakeHealth | null;
  derived_from: string | null;
  recipe: Recipe;
  meta: TakeMeta;
}

export interface RecordingStopped {
  take: TakeView;
  frames_lost: number;
  write_error: string | null;
  device_lost: boolean;
}

export interface Timing {
  bpm: number | null;
  numerator: number | null;
  unit: number | null;
  grouping: number[];
}

export type ScaleKind =
  | "ionian"
  | "dorian"
  | "phrygian"
  | "lydian"
  | "mixolydian"
  | "aeolian"
  | "locrian"
  | "harmonic_minor"
  | "melodic_minor"
  | "phrygian_dominant"
  | "double_harmonic"
  | "major_pentatonic"
  | "minor_pentatonic";

export const SCALE_KINDS: ScaleKind[] = [
  "ionian",
  "dorian",
  "phrygian",
  "lydian",
  "mixolydian",
  "aeolian",
  "locrian",
  "harmonic_minor",
  "melodic_minor",
  "phrygian_dominant",
  "double_harmonic",
  "major_pentatonic",
  "minor_pentatonic",
];

export interface Scale {
  tonic_pc: number;
  degrees_cents: number[];
  kind: ScaleKind | null;
}

export type InstrumentType =
  | "kaval"
  | "shakuhachi"
  | "flute"
  | "recorder"
  | "gaida"
  | "voice"
  | "monochord"
  | "piano"
  | "midi_keyboard"
  | "other";
export const INSTRUMENT_TYPES: InstrumentType[] = [
  "kaval",
  "shakuhachi",
  "flute",
  "recorder",
  "gaida",
  "voice",
  "monochord",
  "piano",
  "midi_keyboard",
  "other",
];

export interface ProfileNote {
  midi: number;
  measured_hz: number;
  cents_offset: number;
  stability_cents: number;
  sample: string | null;
  measured_at_ms: number;
}

export interface InstrumentProfile {
  schema_version: number;
  id: string;
  version: number;
  instrument: InstrumentType;
  name: string;
  tuning: string;
  transpose_semitones: number;
  a4_hz: number;
  notes: ProfileNote[];
  created_at_ms: number;
  updated_at_ms: number;
}

export type Intonation = "profile" | "tempered";

export interface DroneSettings {
  root_midi: number;
  root_cents: number;
  fifth: number;
  octave: number;
  sub: number;
  warmth: number;
  movement: number;
  breath: number;
  detune_cents: number;
  cutoff_hz: number;
  level_db: number;
  intonation: Intonation;
}

export interface DroneStatus {
  sample_rate: number;
  root_hz: number;
  root_cents_applied: number;
  fifth_cents_applied: number;
  intonation_used: Intonation;
  layer_peak: number;
  master_peak: number;
  limiter_gain: number;
  stream_errors: number;
}

export type Emotion = "calm" | "melancholy" | "light" | "dark";
export const EMOTIONS: Emotion[] = ["calm", "melancholy", "light", "dark"];

export interface PadMacro {
  brightness: number;
  movement: number;
  warmth: number;
  space: number;
  slowness: number;
}

export interface PadNote {
  midi: number;
  cents: number;
}

export interface PadSettings {
  notes: PadNote[];
  emotion: Emotion;
  x: number;
  y: number;
  level_db: number;
  lfo_hz: number;
  seed: number;
  intonation: Intonation;
}

export interface PadStatus {
  sample_rate: number;
  notes_hz: number[];
  intonation_used: Intonation;
  macro_applied: PadMacro;
  layer_peak: number;
  master_peak: number;
  limiter_gain: number;
  stream_errors: number;
}

// ---------- error-tolerant performance intelligence (F2.9–F2.11) ----------

export type NoteClass = "stable" | "ornament" | "transition" | "noise" | "probable_error";
export const NOTE_CLASSES: NoteClass[] = ["stable", "ornament", "transition", "noise", "probable_error"];
export type IntentMode = "off" | "stable" | "assisted" | "creative";
export type Reason =
  | "short"
  | "held"
  | "next_to_held_tone"
  | "in_scale"
  | "out_of_scale"
  | "low_confidence"
  | "slide"
  | "repeats_next_phrase"
  | "off_profile"
  | "matches_expected"
  | "differs_from_expected"
  | "isolated"
  | "user_marked"
  | "literal_mode";

export interface Locks {
  tonic: boolean;
  mode: boolean;
  chord: boolean;
  drone: boolean;
}

export interface IntentSettings {
  enabled: boolean;
  mode: IntentMode;
  tolerance: number;
  sensitivity: number;
  locks: Locks;
}

export interface StableTone {
  midi: number;
  cents: number;
  weight: number;
}

export interface Phrase {
  start: number;
  end: number;
  notes: number[];
  stable_tones: StableTone[];
  shape: "rising" | "falling" | "arch" | "valley" | "flat";
  breath_before: boolean;
  cause: "start" | "gap" | "long_tone_drop" | "leap" | "end";
  confidence: number;
}

export interface Classified {
  index: number;
  phrase: number;
  class: NoteClass;
  confidence: number;
  cents_from_scale: number | null;
  cents_from_profile: number | null;
  expected_midi: number | null;
  cents_from_expected: number | null;
  sustained: boolean;
  reasons: Reason[];
}

export interface IntentEstimate {
  phrase: number;
  tones: StableTone[];
  confidence: number;
  stable_notes: number;
  ornaments: number;
  transitions: number;
  noise: number;
  probable_errors: number;
}

export interface ChangeEvidence {
  tonic_pc: number;
  kind: ScaleKind;
  probability: number;
  consecutive_phrases: number;
  held_by_lock: boolean;
}

export interface TonalContext {
  phrase_index: number;
  tonic_pc: number;
  kind: ScaleKind;
  confidence: number;
  alternatives: { tonic_pc: number; kind: ScaleKind; score: number; probability: number }[];
  locked: Locks;
  evidence_for_change: ChangeEvidence | null;
  switched: boolean;
}

export interface PerformanceAnalysis {
  mode: IntentMode;
  phrases: Phrase[];
  classes: Classified[];
  intents: IntentEstimate[];
  contexts: TonalContext[];
  switches: number;
}

export interface Annotation {
  index: number;
  class: NoteClass;
}

// ---------- harmonic strategies (F3.4) ----------

export type Strategy = "drone_only" | "modal_pads" | "liquid_chords";
export type ChordShape = "open5" | "triad" | "seventh" | "add9" | "sus2" | "sus4" | "quartal";
export type ChordKind =
  | "major"
  | "minor"
  | "diminished"
  | "augmented"
  | "sus2"
  | "sus4"
  | "power"
  | "major7"
  | "minor7"
  | "dominant7"
  | "minor_major7"
  | "half_diminished7"
  | "diminished7";

export interface ChordTone {
  midi: number;
  cents: number;
}

export type HarmonyReason =
  | { kind: "tonic" }
  | { kind: "covers_melody"; tones: number[]; share: number }
  | { kind: "smooth_voice_leading"; semitones: number }
  | { kind: "kept_low_confidence"; confidence: number }
  | { kind: "kept_chord_lock" }
  | { kind: "kept_hold"; phrases_left: number }
  | { kind: "changed_for_clash"; share: number }
  | { kind: "tension_target"; tension: number }
  | { kind: "avoids_clash"; pc: number }
  | { kind: "first_chord" };

export interface ChordEvent {
  phrase: number;
  start: number;
  end: number;
  root_pc: number;
  kind: ChordKind | null;
  shape: ChordShape;
  tones: ChordTone[];
  tension: number;
  melody_share: number;
  why: HarmonyReason[];
}

export interface BassEvent {
  phrase: number;
  start: number;
  end: number;
  midi: number;
}

export interface ScoreBreakdown {
  melodic_compatibility: number;
  modal_fidelity: number;
  voice_leading: number;
  tension_fit: number;
  rhythmic_fit: number;
  register_separation: number;
  variety: number;
  total: number;
}

export interface HarmonyProposal {
  strategy: Strategy;
  seed: number;
  tonic_pc: number;
  kind: ScaleKind;
  drone: ChordTone[];
  chords: ChordEvent[];
  bass: BassEvent[];
  score: ScoreBreakdown;
  confidence: number;
}

export interface HarmonyUiParams {
  tension: number;
  movement: number;
  richness: number;
  seed: number;
  locks: Locks;
  register_low: number;
  register_high: number;
}

export interface HarmonyExportResult {
  path: string;
  chords: number;
  bass_notes: number;
}

export interface ProfileDiagnosis {
  notes: number;
  mean_offset_cents: number;
  spread_cents: number;
  mean_stability_cents: number;
}

export interface ProfileView extends InstrumentProfile {
  diagnosis: ProfileDiagnosis | null;
  range: { midi: number; measured: boolean }[];
}

export interface ProfileSummary {
  id: string;
  version: number;
  instrument: InstrumentType;
  name: string;
  tuning: string;
  notes: number;
}

export interface CalibrationResult {
  note: ProfileNote;
  detected_midi: number;
  detected_cents: number;
  held_seconds: number;
  profile: ProfileView;
}

// Ambient Playground (F1A.1/F1A.2): project/src/ambient.rs
export type LayerSource =
  | { kind: "noise"; color: number }
  | { kind: "wind"; gust: number; speed: number; center_hz: number; resonance: number }
  | { kind: "texture"; brightness: number; motion: number; resonance: number; tonal: boolean; register: [number, number] }
  | { kind: "bells"; density: number; decay_s: number; brightness: number; velocity_spread: number; tonal: boolean; register: [number, number] }
  | { kind: "pluck"; density: number; damping: number; brightness: number; velocity_spread: number; tonal: boolean; register: [number, number] }
  | { kind: "loop"; take_id: string; start: number; end: number; crossfade: number; rate: number; reverse: boolean }
  | { kind: "cloud"; take_id: string; grain_ms: number; density: number; position: number; spray: number; scan: number; pitch: number; jitter: number; reverse_p: number; octave_p: number };
export type LayerKind = LayerSource["kind"];

export interface AmbientLayer {
  id: string;
  name: string;
  source: LayerSource;
  gain_db: number;
  pan: number;
  lowpass_hz: number;
  highpass_hz: number;
  mute: boolean;
  solo: boolean;
  seed: number;
  license_ref: string | null;
  /** "Keep what I like": randomize never changes a locked layer. */
  locked: boolean;
}

export interface Macros {
  intensity: number;
  brightness: number;
  motion: number;
  space: number;
  space_size: number;
  tonal_gravity: number;
  width: number;
}

// Automation (F1A.4) and scenes (F1A.5): project/src/program.rs
export interface ParamTarget {
  /** Layer id, or null for a macro. */
  layer: string | null;
  param: string;
}
export type AutomationMode =
  | { mode: "static" }
  | { mode: "lfo"; period_s: number; phase: number; shape: "sine" | "triangle" }
  | { mode: "walk"; period_s: number }
  | { mode: "keyframes"; points: [number, number][] };
export interface AutomationLane {
  id: string;
  target: ParamTarget;
  mode: AutomationMode;
  depth: number;
  humanize: number;
  seed: number;
  enabled: boolean;
}
export interface LayerSnap {
  id: string;
  gain_db: number;
  pan: number;
  lowpass_hz: number;
  highpass_hz: number;
  mute: boolean;
  source: LayerSource;
}
export interface Snapshot {
  layers: LayerSnap[];
  macros: Macros;
}
export interface Scene {
  id: string;
  name: string;
  start_s: number;
  duration_s: number;
  transition_s: number;
  snapshot: Snapshot;
}
export interface Program {
  scenes: Scene[];
  loop_region: [number, number] | null;
}

export interface Ambient {
  schema_version: number;
  layers: AmbientLayer[];
  macros: Macros;
  preset: string | null;
  tags: string[];
  automation: AutomationLane[];
  program: Program;
}

export interface AmbientExportResult {
  path: string;
  bytes: number;
  format: WavFormat;
  seconds: number;
  stems: string[];
  render_ms: number;
}

export interface PresetSummary {
  id: string;
  name: string;
  tags: string[];
  factory: boolean;
  layer_count: number;
}

export const AMBIENT_TAGS = ["forest", "river", "night", "fog", "mountain", "ethno", "meditation", "dark", "warm", "liquid", "no_key"] as const;
export type AmbientTag = (typeof AMBIENT_TAGS)[number];

export interface AmbientView {
  ambient: Ambient;
  playing: boolean;
}

export interface AmbientStatus {
  playing: boolean;
  sample_rate: number;
  layer_peaks: number[];
  master_peak: number;
  limiter_gain: number;
  stream_errors: number;
  pitches_hz: number[][];
  program_position: number;
  program_playing: boolean;
  program_end: number;
  lane_time: number;
}

// Rhythm section (Phase 4): src-tauri/src/rhythm_cmds.rs
export type GrooveStyle = "liquid" | "halftime" | "ethno";
export type ArpDirection = "up" | "down" | "up_down" | "random";
export interface Motif {
  cells: ([number, number] | null)[];
}
export interface DrumsSettings {
  on: boolean;
  style: GrooveStyle;
  complexity: number;
  humanize: number;
  swing: number;
  fill_every_bars: number;
  seed: number;
  level_db: number;
  kick_hz: number;
  decay: number;
  brightness: number;
  locked: boolean;
}
export interface BassSettings {
  on: boolean;
  voice: number;
  density: number;
  syncopation: number;
  passing: number;
  register_low: number;
  register_high: number;
  seed: number;
  level_db: number;
  cutoff_hz: number;
  resonance: number;
  drive: number;
  glide_s: number;
  sidechain: number;
  wobble_div: number;
  wobble_depth: number;
  warmth: number;
  locked: boolean;
}
export interface OstinatoSettings {
  on: boolean;
  steps: number;
  step_beats: number;
  density: number;
  probability: number;
  variation_every_bars: number;
  swing: number;
  seed: number;
  level_db: number;
  register_low: number;
  register_high: number;
  timbre: number;
  release_s: number;
  locked: boolean;
  motif: Motif | null;
}
export interface ArpSettings {
  on: boolean;
  direction: ArpDirection;
  octaves: number;
  step_beats: number;
  gate: number;
  density: number;
  swing: number;
  seed: number;
  level_db: number;
  register_low: number;
  timbre: number;
  release_s: number;
  locked: boolean;
}
/** What the user edits (persisted in prefs); chord/root/intonation are added at send time. */
export interface RhythmPrefs {
  drums: DrumsSettings;
  bass: BassSettings;
  ostinato: OstinatoSettings;
  arp: ArpSettings;
  bars: number;
}
export interface RhythmSettings extends RhythmPrefs {
  chord: PadNote[] | null;
  root_midi: number | null;
  intonation: Intonation;
}
export interface RhythmStatus {
  playing: boolean;
  sample_rate: number;
  bpm: number;
  beat: number;
  bar: number;
  beat_in_bar: number;
  peaks: [number, number, number, number];
  master_peak: number;
  limiter_gain: number;
  stream_errors: number;
  motif: Motif | null;
  root_pc: number | null;
  chord_pcs: number[];
}
export interface TempoCandidate {
  bpm: number;
  offset_s: number;
  fit: number;
}
export type CounterRegister = "below" | "above";
export interface CounterUiParams {
  register: CounterRegister;
  adventure: number;
  tension: number;
  min_rest_s: number;
  seed: number;
}

export interface Marker {
  at_seconds: number;
  name: string;
}
export const SECTION_NAMES = ["Intro", "A", "B", "Build", "Drop", "Break", "Outro"] as const;

export interface ProjectView {
  name: string;
  root: string;
  timing: Timing;
  takes: TakeView[];
  markers: Marker[];
  loaded_take: string | null;
  scale: Scale | null;
  a4_hz: number;
  instrument: InstrumentProfile | null;
}

export interface Note {
  start: number;
  end: number;
  midi: number;
  cents: number;
  confidence: number;
  velocity: number;
  bend: [number, number][];
}

export interface ContourView {
  t: number;
  midi: number | null;
  confidence: number;
}

export interface PcShare {
  pc: number;
  share: number;
}

export interface Decisive {
  pc_for: number;
  share_for: number;
  pc_against: number;
  share_against: number;
  rival_tonic_pc: number;
  rival_kind: ScaleKind;
}

/** Facts behind a hypothesis (F3.1); the UI words them. Shares are 0..1 of sounding time. */
export interface Evidence {
  tonic_share: number;
  fifth_share: number;
  coverage: number;
  decisive: Decisive | null;
  off_scale: PcShare[];
}

export interface Hypothesis {
  tonic_pc: number;
  kind: ScaleKind;
  score: number;
  probability: number;
  evidence: Evidence | null;
}

export interface AnalysisView {
  take_id: string;
  a4_hz: number;
  contour: ContourView[];
  notes: Note[];
  edited: boolean;
  hypotheses: Hypothesis[];
}

export interface MonitorStatus {
  sample_rate: number;
  channels: number;
  peak: number;
  clipped_samples: number;
  stream_errors: number;
  device_lost: boolean;
  pitch_hz: number | null;
  pitch_confidence: number;
}

export type Voice = "sine" | "triangle" | "flute";
export const VOICES: Voice[] = ["flute", "sine", "triangle"];

export interface MidiRender {
  tempo_factor: number;
  muted_tracks: number[];
  voice: Voice;
  transpose: number;
}

export interface TrackInfo {
  index: number;
  name: string | null;
  program: number | null;
  channels: number[];
  note_count: number;
}

export interface MidiTakeView {
  tracks: TrackInfo[];
  tempo_from_file: boolean;
  bpm: number;
  time_signature: [number, number] | null;
  render: MidiRender;
  duration_seconds: number;
}

export type MidiEvent =
  | { kind: "note_on"; channel: number; note: number; velocity: number; t_us: number }
  | { kind: "note_off"; channel: number; note: number; t_us: number }
  | { kind: "other"; status: number; t_us: number };

export type Level = "beginner" | "intermediate" | "advanced";
export const LEVELS: Level[] = ["beginner", "intermediate", "advanced"];

export type Verdict =
  | { kind: "hit"; cents: number; ms: number }
  | { kind: "wrong_pitch"; cents: number; ms: number }
  | { kind: "early"; cents: number; ms: number }
  | { kind: "late"; cents: number; ms: number }
  | { kind: "missed" };

export interface NoteResult {
  index: number;
  played: number | null;
  verdict: Verdict;
}

export interface TrainerSummary {
  expected: number;
  hits: number;
  wrong_pitch: number;
  early: number;
  late: number;
  missed: number;
  extra: number;
  mean_abs_cents: number;
  mean_abs_ms: number;
}

export interface TrainerReport {
  per_note: NoteResult[];
  extra_played: number[];
  summary: TrainerSummary;
}

export interface MidiExportResult {
  path: string;
  bytes: number;
  notes: number;
  tempo_bpm: number;
  technical_tempo: boolean;
  readme_path: string | null;
}

export interface TakeCheck {
  id: string;
  health: TakeHealth;
}

export interface OpenReport {
  recovered: string[];
  unrecoverable: string[];
  dropped_empty: number;
  checks: TakeCheck[];
}

export interface ProjectOpened {
  project: ProjectView;
  report: OpenReport;
}

export interface RecentProject {
  path: string;
  name: string;
  exists: boolean;
}

export interface LoadedTakeInfo {
  id: string;
  sample_rate: number;
  channels: number;
  frames: number;
  duration_seconds: number;
  peak_dbfs: number;
  rms_dbfs: number;
  clipped_samples: number;
  offset_seconds: number;
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
  looping: boolean;
}

export type WavFormat = "float32" | "pcm24" | "pcm16";

export interface ExportResult {
  path: string;
  bytes: number;
  format: WavFormat;
}

export interface RecordRequest {
  input: string | null;
  output: string | null;
  sample_rate: number | null;
  click: boolean;
  count_in_bars: number;
}

export interface PlayRequest {
  device: string | null;
  start_seconds: number;
  loop_seconds: [number, number] | null;
  click: boolean;
}

export interface CmdError {
  code: string;
  message: string;
}

export function isCmdError(e: unknown): e is CmdError {
  return typeof e === "object" && e !== null && "code" in e && "message" in e;
}

// ---------- arrangement (F5.1) ----------

export interface Clip {
  id: string;
  take_id: string;
  /** Timeline position of the clip's first sample, seconds. */
  start: number;
  /** Seconds into the take's (trimmed) audio where the clip begins. */
  offset: number;
  length: number;
  gain_db: number;
  fade_in: number;
  fade_out: number;
  mute: boolean;
}

export interface Track {
  id: string;
  name: string;
  gain_db: number;
  pan: number;
  mute: boolean;
  solo: boolean;
  clips: Clip[];
}

export interface Arrangement {
  schema_version: number;
  tracks: Track[];
  loop_region: [number, number] | null;
}

export interface ArrangementView {
  arrangement: Arrangement;
  duration_seconds: number;
}

export interface ArrangeStatus {
  playing: boolean;
  position_seconds: number;
  duration_seconds: number;
  finished: boolean;
  sample_rate: number;
  layer_peak: number;
  master_peak: number;
  limiter_gain: number;
  stream_errors: number;
}

export function describeError(e: unknown): string {
  if (isCmdError(e)) return `${e.code}: ${e.message}`;
  if (e instanceof Error) return e.message;
  return String(e);
}

export const IMPORT_EXTENSIONS = ["wav", "mp3", "flac", "ogg", "mid"];

/** One background bus strip (drone, pad, atmosphere, rhythm): trim, pan, cuts. */
export interface BusStrip {
  gain_db: number;
  pan: number;
  lowpass_hz: number;
  highpass_hz: number;
}

// ---------- Preset library (shell library_cmds.rs) ----------
export const LIBRARY_KINDS = ["session", "module", "drone", "pad", "rhythm", "harmony", "intent", "ambient", "background"] as const;
export type LibraryKind = (typeof LIBRARY_KINDS)[number];
export type LibrarySortBy = "date_desc" | "date_asc" | "name" | "kind";
export interface LibrarySummary {
  id: string;
  kind: string;
  name: string;
  tags: string[];
  notes: string;
  project: string;
  created_ms: number;
  updated_ms: number;
}
export interface LibraryEntry extends LibrarySummary {
  payload: unknown;
}
export interface LibraryFacets {
  projects: string[];
  tags: string[];
  counts: [string, number][];
}
export interface NewLibraryEntry {
  kind: LibraryKind;
  name: string;
  tags: string[];
  notes: string;
  payload: unknown;
}

export const api = {
  audioHost: () => invoke<AudioHostInfo>("audio_host"),
  listInputDevices: () => invoke<DeviceInfo[]>("list_input_devices"),
  listOutputDevices: () => invoke<DeviceInfo[]>("list_output_devices"),
  playTestTone: (device: string | null) => invoke<void>("play_test_tone", { device }),

  defaultProjectsDir: () => invoke<string>("default_projects_dir"),
  recentProjects: () => invoke<RecentProject[]>("recent_projects"),
  createProject: (name: string, parent: string | null) =>
    invoke<ProjectView>("create_project", { name, parent }),
  openProject: (path: string) => invoke<ProjectOpened>("open_project", { path }),
  closeProject: () => invoke<void>("close_project"),
  projectState: () => invoke<ProjectView | null>("project_state"),
  setTiming: (timing: Timing) => invoke<ProjectView>("set_timing", { timing }),

  loadTake: (id: string) => invoke<LoadedTakeInfo>("load_take", { id }),
  getWaveform: (bins: number) => invoke<WaveformPeaks | null>("get_waveform", { bins }),
  renameTake: (id: string, label: string | null) =>
    invoke<ProjectView>("rename_take", { id, label }),
  deleteTake: (id: string) => invoke<ProjectView>("delete_take", { id }),
  setTrim: (id: string, inSeconds: number, outSeconds: number | null) =>
    invoke<ProjectView>("set_trim", { id, inSeconds, outSeconds }),
  relinkTake: (id: string, path: string) => invoke<ProjectView>("relink_take", { id, path }),
  setTakeRecipe: (id: string, recipe: Recipe) => invoke<ProjectView>("set_take_recipe", { id, recipe }),
  setTakeMeta: (id: string, meta: TakeMeta) => invoke<ProjectView>("set_take_meta", { id, meta }),
  analyzeSilence: (id: string, thresholdDbfs: number | null) => invoke<SilenceReport>("analyze_silence", { id, thresholdDbfs }),
  snapTrimToZero: (id: string) => invoke<ProjectView>("snap_trim_to_zero", { id }),
  verifyTakes: () => invoke<ProjectView>("verify_takes"),
  importAudio: (path: string) => invoke<ProjectView>("import_audio", { path }),
  saveSelectionAsTake: (inSeconds: number, outSeconds: number, label: string | null) =>
    invoke<ProjectView>("save_selection_as_take", { inSeconds, outSeconds, label }),
  exportWav: (path: string, format: WavFormat) =>
    invoke<ExportResult>("export_wav", { path, format }),

  analyzeTake: () => invoke<AnalysisView>("analyze_take"),
  getAnalysis: () => invoke<AnalysisView | null>("get_analysis"),
  updateNotes: (notes: Note[]) => invoke<AnalysisView>("update_notes", { notes }),
  resetNotes: () => invoke<AnalysisView>("reset_notes"),
  setScale: (scale: Scale | null) => invoke<ProjectView>("set_scale", { scale }),
  setReference: (a4Hz: number | null) => invoke<ProjectView>("set_reference", { a4Hz }),
  exportMidi: (path: string) => invoke<MidiExportResult>("export_midi", { path }),
  midiTake: () => invoke<MidiTakeView | null>("midi_take"),
  setMidiRender: (render: MidiRender) => invoke<MidiTakeView>("set_midi_render", { render }),
  playNotes: (device: string | null, voice: Voice | null) =>
    invoke<PlayerStatus>("play_notes", { device, voice }),
  notesPlaybackStatus: () => invoke<PlayerStatus | null>("notes_playback_status"),
  stopNotes: () => invoke<void>("stop_notes"),
  sendNotesToPlayer: (label: string | null) =>
    invoke<{ project: ProjectView; take_id: string }>("send_notes_to_player", { label }),
  listMidiPorts: () => invoke<{ name: string }[]>("list_midi_ports"),
  openMidiInput: (name: string) => invoke<void>("open_midi_input", { name }),
  closeMidiInput: () => invoke<void>("close_midi_input"),
  pollMidiEvents: () => invoke<{ name: string; events: MidiEvent[] } | null>("poll_midi_events"),
  startLiveSynth: (device: string | null, voice: Voice | null) =>
    invoke<{ sample_rate: number; active_voices: number; stream_errors: number }>("start_live_synth", { device, voice }),
  stopLiveSynth: () => invoke<void>("stop_live_synth"),
  synthNote: (midi: number, on: boolean, velocity: number | null) => invoke<void>("synth_note", { midi, on, velocity }),
  synthAllOff: () => invoke<void>("synth_all_off"),
  startDrone: (device: string | null, settings: DroneSettings) => invoke<DroneStatus>("start_drone", { device, settings }),
  setDrone: (settings: DroneSettings) => invoke<DroneStatus>("set_drone", { settings }),
  stopDrone: () => invoke<void>("stop_drone"),
  droneStatus: () => invoke<DroneStatus | null>("drone_status"),
  startPad: (device: string | null, settings: PadSettings) => invoke<PadStatus>("start_pad", { device, settings }),
  setPad: (settings: PadSettings) => invoke<PadStatus>("set_pad", { settings }),
  stopPad: () => invoke<void>("stop_pad"),
  padStatus: () => invoke<PadStatus | null>("pad_status"),
  setGraphMaster: (db: number) => invoke<void>("set_graph_master", { db }),
  performanceAnalysis: (settings: IntentSettings) => invoke<PerformanceAnalysis | null>("performance_analysis", { settings }),
  annotateNote: (settings: IntentSettings, index: number, cls: NoteClass | null) =>
    invoke<PerformanceAnalysis | null>("annotate_note", { settings, index, class: cls }),
  intentAnnotations: () => invoke<Annotation[]>("intent_annotations"),
  harmonyProposals: (intent: IntentSettings, params: HarmonyUiParams) => invoke<HarmonyProposal[]>("harmony_proposals", { intent, params }),
  exportHarmonyMidi: (proposal: HarmonyProposal, path: string) => invoke<HarmonyExportResult>("export_harmony_midi", { proposal, path }),
  evaluatePerformance: (expected: Note[], played: Note[], level: Level, latencyMs: number) =>
    invoke<TrainerReport>("evaluate_performance", { expected, played, level, latencyMs }),
  listProfiles: () => invoke<ProfileSummary[]>("list_profiles"),
  getProfile: (id: string) => invoke<ProfileView>("get_profile", { id }),
  createProfile: (req: { instrument: InstrumentType; name: string; tuning: string; transpose_semitones: number; a4_hz: number | null }) =>
    invoke<ProfileView>("create_profile", { req }),
  deleteProfile: (id: string) => invoke<ProfileSummary[]>("delete_profile", { id }),
  setProjectInstrument: (id: string | null) => invoke<ProjectView>("set_project_instrument", { id }),
  calibrateNote: (id: string, midi: number, device: string | null, sampleRate: number | null) =>
    invoke<CalibrationResult>("calibrate_note", { id, midi, device, sampleRate }),
  removeProfileNote: (id: string, midi: number) => invoke<ProfileView>("remove_profile_note", { id, midi }),
  playProfileNote: (id: string, midi: number, device: string | null, exact: boolean) =>
    invoke<void>("play_profile_note", { id, midi, device, exact }),
  startMonitor: (device: string | null, sampleRate: number | null) =>
    invoke<void>("start_monitor", { device, sampleRate }),
  stopMonitor: () => invoke<void>("stop_monitor"),
  monitorStatus: () => invoke<MonitorStatus | null>("monitor_status"),

  startRecording: (req: RecordRequest) => invoke<RecordingStarted>("start_recording", { req }),
  recordingStatus: () => invoke<RecordingStatus | null>("recording_status"),
  recordingWaveform: (bins: number) =>
    invoke<WaveformPeaks | null>("recording_waveform", { bins }),
  stopRecording: () => invoke<RecordingStopped>("stop_recording"),

  startPlayback: (req: PlayRequest) => invoke<PlayerStatus>("start_playback", { req }),
  playbackStatus: () => invoke<PlayerStatus | null>("playback_status"),
  setPlaybackLoop: (loopSeconds: [number, number] | null) =>
    invoke<void>("set_playback_loop", { loopSeconds }),
  setPlaybackClick: (on: boolean) => invoke<void>("set_playback_click", { on }),
  startMetronome: (device: string | null) => invoke<MetronomeStatus>("start_metronome", { device }),
  stopMetronome: () => invoke<void>("stop_metronome"),
  metronomeStatus: () => invoke<MetronomeStatus | null>("metronome_status"),
  stopPlayback: () => invoke<void>("stop_playback"),

  ambientGet: () => invoke<AmbientView>("ambient_get"),
  setAmbient: (ambient: Ambient) => invoke<AmbientView>("set_ambient", { ambient }),
  ambientPlay: (device: string | null) => invoke<AmbientStatus>("ambient_play", { device }),
  ambientStop: () => invoke<void>("ambient_stop"),
  ambientStatus: () => invoke<AmbientStatus>("ambient_status"),
  backgroundFadeOut: (seconds: number) => invoke<void>("background_fade_out", { seconds }),
  ambientProgramSet: (playing: boolean | null, position: number | null) => invoke<AmbientStatus>("ambient_program_set", { playing, position }),
  exportAmbient: (path: string, format: WavFormat | null, seconds: number | null, stems: boolean, sampleRate: number | null) =>
    invoke<AmbientExportResult>("export_ambient", { path, format, seconds, stems, sampleRate }),
  startRhythm: (device: string | null, settings: RhythmSettings) => invoke<RhythmStatus>("start_rhythm", { device, settings }),
  setRhythm: (settings: RhythmSettings) => invoke<RhythmStatus>("set_rhythm", { settings }),
  stopRhythm: () => invoke<void>("stop_rhythm"),
  rhythmStatus: () => invoke<RhythmStatus>("rhythm_status"),
  rhythmRestart: () => invoke<RhythmStatus>("rhythm_restart"),
  rhythmRegenerate: (settings: RhythmSettings) => invoke<RhythmSettings>("rhythm_regenerate", { settings }),
  rhythmToTakes: (settings: RhythmSettings) => invoke<{ project: ProjectView; created: string[] }>("rhythm_to_takes", { settings }),
  setMarkers: (markers: Marker[]) => invoke<ProjectView>("set_markers", { markers }),
  estimateTakeTempo: (bpmLow: number | null, bpmHigh: number | null) => invoke<TempoCandidate[]>("estimate_take_tempo", { bpmLow, bpmHigh }),
  countermelodyToTake: (intent: IntentSettings, params: CounterUiParams, proposal: HarmonyProposal | null, label: string | null) =>
    invoke<{ project: ProjectView; take_id: string }>("countermelody_to_take", { intent, params, proposal, label }),
  ambientPresets: () => invoke<PresetSummary[]>("ambient_presets"),
  ambientPresetLoad: (id: string) => invoke<Ambient>("ambient_preset_load", { id }),
  ambientPresetSave: (name: string, tags: string[], rack: Ambient) => invoke<PresetSummary[]>("ambient_preset_save", { name, tags, rack }),
  ambientPresetDelete: (id: string) => invoke<PresetSummary[]>("ambient_preset_delete", { id }),
  arrangementGet: () => invoke<ArrangementView>("arrangement_get"),
  setArrangement: (arrangement: Arrangement) => invoke<ArrangementView>("set_arrangement", { arrangement }),
  arrangementPlay: (device: string | null, fromSeconds: number | null) =>
    invoke<ArrangeStatus>("arrangement_play", { device, fromSeconds }),
  arrangementPause: () => invoke<ArrangeStatus | null>("arrangement_pause"),
  arrangementSeek: (seconds: number) => invoke<ArrangeStatus | null>("arrangement_seek", { seconds }),
  arrangementRelease: () => invoke<void>("arrangement_release"),
  arrangementStatus: () => invoke<ArrangeStatus | null>("arrangement_status"),
  takePeaks: (takeId: string, bins: number) => invoke<WaveformPeaks>("take_peaks", { takeId, bins }),
  exportArrangementMix: (path: string, format: WavFormat | null, sampleRate: number | null) =>
    invoke<ExportResult>("export_arrangement_mix", { path, format, sampleRate }),

  // Preset library
  libraryList: (kind: LibraryKind | null, project: string | null, query: string | null, sortBy: LibrarySortBy) =>
    invoke<LibrarySummary[]>("library_list", { kind, project, query, sortBy }),
  librarySave: (entry: NewLibraryEntry) => invoke<LibrarySummary>("library_save", { entry }),
  libraryGet: (id: string) => invoke<LibraryEntry>("library_get", { id }),
  libraryUpdate: (id: string, name: string, tags: string[], notes: string) =>
    invoke<LibrarySummary>("library_update", { id, name, tags, notes }),
  libraryReplacePayload: (id: string, payload: unknown) => invoke<LibrarySummary>("library_replace_payload", { id, payload }),
  libraryDelete: (id: string) => invoke<void>("library_delete", { id }),
  libraryFacets: () => invoke<LibraryFacets>("library_facets"),
  // Background mixer buses (bus_cmds.rs)
  busGet: () => invoke<BusStrip[]>("bus_get"),
  busSet: (name: string, strip: BusStrip) => invoke<void>("bus_set", { name, strip }),
  busFade: (name: string, fadeMs: number, muted: boolean) => invoke<void>("bus_fade", { name, fadeMs, muted }),
  busStatus: () => invoke<number[]>("bus_status"),
  // Assistant bridge (docs/ASSISTANT_BRIDGE.md)
  assistantDir: () => invoke<string>("assistant_dir"),
  assistantPoll: () => invoke<{ name: string; body: unknown }[]>("assistant_poll"),
  assistantReply: (name: string, reply: unknown) => invoke<void>("assistant_reply", { name, reply }),
  assistantWriteState: (snapshot: unknown) => invoke<void>("assistant_write_state", { snapshot }),
  // Per-project remembered session (session.json in the project folder)
  sessionGet: () => invoke<unknown | null>("session_get"),
  sessionSet: (session: unknown) => invoke<void>("session_set", { session }),
};
