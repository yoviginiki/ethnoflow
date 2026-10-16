# Decision log (ADRs)

Format: context → decision → consequences. Status: Accepted unless noted.
Licences checked on crates.io / npm at the time of writing (2026-10-16).

## ADR-0001 Tauri 2 + React/TS + Rust engine (not Electron, not JUCE-first)

Context: master plan prefers Tauri 2. Alternatives: Electron (heavy, Node in the audio
path), JUCE/C++ (best audio tooling, weaker UI velocity, licence cost for closed source).
Decision: Tauri 2 (MIT/Apache-2.0) shell, React 19 + TypeScript + Vite 8 UI, Rust engine
in a separate crate with no Tauri dependency.
Consequences: audio code is testable without the UI; JUCE remains a fallback for the
engine only (ADR-0003) without touching the UI.

## ADR-0002 Windows first, cross-platform core from day 0

Context: owner's question about macOS/Linux. cpal covers WASAPI/ASIO, CoreAudio,
ALSA/Pulse/JACK; Tauri covers all three OSes.
Decision: primary release target is Windows 10/11 x64. The engine and CI stay
cross-platform; nothing platform-specific in `crates/`. macOS/Linux builds are a release
decision (signing/notarisation, packaging), not an engineering rewrite.
Cost: ~20–30 % extra QA per added platform (three WebViews, three packaging paths, Python
sidecar packaging later). Known losses: ASIO is Windows-only (irrelevant elsewhere); Linux
latency depends on the user's PipeWire/JACK setup; macOS needs a paid developer account
for a clean Gatekeeper experience.

## ADR-0003 cpal 0.16 as the audio backend (Apache-2.0)

Context: options were cpal, a JUCE sidecar process, or raw WASAPI bindings.
Decision: cpal. Phase 0 proved device enumeration, input and output streams, format
conversion and the `!Send` stream-thread pattern. Measured on Linux with an ALSA null
device; Windows WASAPI verification is the first task on the owner's laptop.
Fallback trigger: if WASAPI shared mode on the owner's hardware shows >40 ms round-trip
or unstable device switching that cpal cannot fix, evaluate (a) cpal's `asio` feature with
the Steinberg SDK (licence review needed), (b) a JUCE engine as a separate process over a
local IPC/shared-memory ring. The UI and command surface do not change in either case.

## ADR-0004 WAV via hound 3.5 (Apache-2.0/MIT); FLAC/MP3 deferred

Decision: hound for WAV read/write (float32, pcm24, pcm16). MP3 import (needed for the
owner's `re.mp3` test file) is Phase 1: candidate `symphonia` (MPL-2.0, pure Rust, MP3/FLAC/
OGG decoding) — no LAME, no patent-encumbered encoding, decode only.

## ADR-0005 Ring buffer: ringbuf 0.4 (MIT/Apache-2.0)

Decision: SPSC heap ring buffer between the RT callback and the drain thread; capacity
2 s @ 96 kHz stereo. Overruns are counted and surfaced in the UI, never silently dropped.

## ADR-0006 Linear resampler in Phase 0; band-limited later

Decision: a 40-line linear-interpolation resampler for playback when device rate ≠ take
rate. Good enough for monitoring. For export-quality SRC (stems at a project rate) use
`rubato` (MIT) in Phase 5. Tests cover length, channel independence and frequency
preservation.

## ADR-0007 Take kept in RAM with a 30-minute cap (Phase 0 only)

Context: the Xvfb test with an unclocked ALSA null device filled 2 GB in seconds and
exposed that an unbounded in-memory recorder is a real risk even if a real microphone only
produces ~11 MB/min.
Decision: `Recorder` has `max_seconds` (default 30 min); when hit, samples are dropped,
`capped` is set, and the UI auto-stops with a message.
Supersession: Phase 1 streams to a WAV file on the drain thread (crash-safe, no cap) and
keeps only peaks in memory.

## ADR-0008 Status by polling (50 ms), not events

Decision: the UI polls `recording_status`/`playback_status` while active. Simple,
sufficient for a meter and playhead, keeps the engine free of UI callbacks. Revisit in
Phase 1 if a >20 Hz meter or lower playhead jitter is wanted (Tauri events are a drop-in).

## ADR-0009 UI state in Zustand 5 (MIT); i18n as a flat BG/EN map

Decision: Zustand for the small UI store; strings in `src/i18n.ts` with BG default and EN
toggle. Phase 6 moves to a proper catalogue when lesson content arrives.

## ADR-0010 Native file dialogs via tauri-plugin-dialog; logging via tauri-plugin-log

Both MIT/Apache-2.0, first-party Tauri plugins. Capabilities limited to
save/open/message and log in `src-tauri/capabilities/default.json`.

## ADR-0011 No ML, no Python in Phase 0/1

Decision: the vertical slice needs none. Python packaging spike (PyInstaller sidecar vs.
pure-Rust inference via `ort`/`tract`) is scheduled for the start of Phase 2, when a
pitch model is actually chosen. Preference order: pure Rust DSP (YIN/pYIN) → ONNX in
Rust → Python sidecar only if a model has no other path.

## ADR-0012 GPU is not a dependency

Decision: no feature in Phases 0–6 requires a GPU. Phase 2 ML pitch models benefit from
one (5–10× faster offline analysis) but run on CPU for 1–2-minute takes. Local
generative models (Phase 7+) would require ≥8 GB VRAM and get their own ADR with
measurements before adoption.

## ADR-0013 Build the shell with `tauri build`, never bare `cargo build --release`

Found during Phase 0: a bare cargo release build does not embed the frontend. Scripts and
CLAUDE.md use the Tauri CLI.

## ADR-0014 Recordings stream to a checkpointed float32 WAV (supersedes ADR-0007)

Context: Phase 1 exit requires no data loss on crash and 30+ minute takes.
Decision: the drain thread writes a 32-bit float WAV through `hound::WavWriter` and calls
its `flush()` (header rewrite) every 1 s, so the file is a valid WAV up to the last
checkpoint at all times. `repair_wav_header` fixes the data/RIFF sizes from the actual
file length after a crash and cuts a torn trailing frame; sample bytes are never
rewritten. Files are opened with `create_new` — an existing original is never
overwritten. Memory holds only 10 ms min/max peak bins (~800 B/s) for the live waveform.
Limit: WAV's u32 sizes cap a take at ~3.1 h (48 kHz stereo); the recorder stops with a
message. RF64/W64 is not needed for the use case. No new dependency.
Known gap: after stop, the take is loaded into RAM for playback (player needs the whole
buffer); a streaming player is a Phase 5 concern.

## ADR-0015 Project = `<name>.ethnoflow/` folder; project.json + SQLite (`crates/project`)

Context: master plan §10 layout; Phase 1 needs save/reopen with no data loss, crash
recovery, checksums and migrations. The roadmap named `packages/project-schema`; the
format is owned by Rust, so it lives in `crates/project` (TS mirrors the types in
`api.ts`).
Decision:
- `project.json` (atomic write: temp + fsync + rename) holds the portable musical data:
  name, schema version, tempo map, time signatures (with groupings such as 2+2+3),
  markers. New projects have an empty tempo map and no meter — nothing is assumed.
- `project.db` (SQLite, rollback journal, `synchronous=FULL`) holds takes, BLAKE3
  checksums, trim points and settings; `schema_migrations` from v1. Every change is a
  committed transaction, so "autosave" is continuous rather than periodic.
- A take row is inserted in `recording` state *before* the file is created. On open,
  such rows are repaired (ADR-0014) and marked `recovered`; WAVs in `assets/originals/`
  without a row are adopted. Every original is verified against its checksum on open.
- Delete moves the original to `trash/` inside the project; nothing is destroyed.
- Relink of a missing original accepts only a byte-identical file (same checksum).
- Imports are copied byte-for-byte into `assets/originals/` (the source format is kept).
Newer `project.json`/`project.db` versions are refused with a clear error, never
downgraded. No locking between two app instances yet (single-user desktop).

## ADR-0016 rusqlite 0.40 (MIT, bundled SQLite: public domain) and blake3 1.8

- `rusqlite` with `bundled` compiles SQLite into the binary: no system DLL on Windows,
  same SQLite on all OSes; builds with MSVC (verified). `fallible_uint` makes u64 values
  error instead of silently wrapping. Widely used, actively maintained.
- `blake3` (CC0-1.0 OR Apache-2.0): checksums of originals; fast (~GB/s), pure Rust with
  SIMD, maintained by the BLAKE3 authors.
Supply chain: both from crates.io, versions pinned in Cargo.lock; no build-time
downloads (bundled SQLite source ships inside the crate).

## ADR-0017 symphonia 0.6 for import decoding (MPL-2.0), decode only

Implements ADR-0004's candidate. Features enabled: `mp3`, `flac`, `ogg`, `vorbis`, `wav`,
`pcm` (no AAC/ALAC/MP4 until needed). Pure Rust, no system codecs, builds on MSVC
(verified). MPL-2.0 is file-level copyleft: using the unmodified crate as a dependency
imposes no obligation on our code; if we ever patch symphonia's files, those patched files
must be published. No encoders — MP3 export stays out of scope (master plan: optional).
Behaviour: corrupt packets are skipped and logged; a mid-stream format change is refused.
The owner's `re.mp3` decodes (48 kHz stereo, 59.45 s); it is used only by a local test
that skips when the file is absent and is never committed or uploaded.
