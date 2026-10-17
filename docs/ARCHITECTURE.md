# Architecture

Status: Phase 0 (feasibility) implemented. This document describes what exists and the
target shape that later phases grow into. Trade-offs are recorded in `DECISIONS.md`.

## 1. Process and thread model

```
┌────────────────────────────── Tauri process ──────────────────────────────┐
│  WebView (React/TS)                                                        │
│   UI state (Zustand) ── invoke() ──▶ Tauri commands (Rust, commands.rs)   │
│   polls status 20×/s ◀── JSON ◀──   AppState { recorder, player, take }    │
│                                             │                               │
│                                   audio-engine crate                        │
│                                             │                               │
│   ┌── stream thread (owns cpal::Stream, !Send) ──┐                          │
│   │   host audio thread: RT callback ───▶ ring buffer ───▶ drain thread ──▶ Vec<f32>  (record)
│   │   host audio thread: RT callback ◀── Arc<AudioBuffer> + atomic position         (play)
│   └──────────────────────────────────────────────┘                          │
└────────────────────────────────────────────────────────────────────────────┘
```

* **RT callback** (host-owned thread): sample-format conversion to f32, peak/clip
  counting, `push_slice` into a lock-free SPSC ring buffer. No allocation, no locks, no
  I/O. Overruns are counted, never block.
* **Drain thread**: pops from the ring buffer into a growable `Vec<f32>` (Phase 0) with a
  30-minute in-memory cap. Phase 1 replaces the Vec with a streaming WAV writer plus an
  in-memory peak cache, removing the cap.
* **Stream thread**: `cpal::Stream` is `!Send` on every platform, so each stream lives on
  a thread we own and is stopped through a channel. This is what lets Tauri hold the
  recorder/player behind `Mutex` in managed state.
* **UI**: never receives raw frames. `get_waveform(bins)` returns min/max pairs; status
  commands return small structs. The UI polls at 50 ms while recording/playing, which is
  enough for a meter and a playhead and keeps the engine free of UI callbacks. Events
  (push) can replace polling in Phase 1 without changing the engine.

## 2. Crate / package boundaries (target)

| path | status | responsibility |
|---|---|---|
| `crates/audio-engine` | **done (P0)** | device I/O, record, play, WAV, peaks, resample; later transport, mixing, metronome, DSP |
| `apps/desktop` | **done (P0)** | Tauri shell + commands; React UI |
| `packages/project-schema` | P1 | versioned `.ethnoflow` project model, migrations, validation |
| `crates/music-core` | P2 | note events, pitch↔note, scales/modes, rhythm, harmony rules |
| `services/analysis` | P2 | offline pitch/onset/segmentation, tonic/mode hypotheses (Rust first; Python only if a model requires it) |
| `crates/arranger` | P3/P4 | drone, voicings, bass, drum patterns, structure |
| `packages/lesson-content` | P6 | validated BG curriculum |
| `packages/ui` | P1+ | accessible components shared across screens |

Rules enforced across boundaries:
* Engine never depends on Tauri. Commands are a thin adapter (`commands.rs`).
* Types crossing the boundary are `serde` structs with snake_case and are mirrored by hand
  in `apps/desktop/src/api.ts`. (Phase 1: consider `tauri-specta`/`ts-rs` for generated
  types once the surface grows.)
* The immutable original (`assets/originals/`) is never modified; derived audio goes to
  `assets/derived/`.

## 3. Data flow for the Phase 0 vertical slice

1. `list_input_devices` → UI select (name-addressed devices; WASAPI/CoreAudio/ALSA names).
2. `start_recording(device)` → `Recorder::start` opens the device at its default config
   (sample rate, channels, sample format) and converts everything to f32.
3. UI polls `recording_status` → peak (dBFS), seconds, overruns, clipped, `capped`.
4. `stop_recording` → joins the drain thread → `AudioBuffer` stored as the current *take*.
5. `get_waveform(1600)` → `compute_peaks` on the mono mix → canvas.
6. `start_playback(device, start_seconds)` → `adapt_for_device` (channel map + linear
   resample, once) → `Player::start`; UI polls `playback_status` for the playhead.
7. `export_wav(path, format)` → `write_wav` (float32 lossless, pcm24 for DAWs, pcm16).
8. `import_wav(path)` → `read_wav` → new take. Integration test proves export→import is
   bit-exact for float32 and within 1 LSB for PCM.

## 4. Audio format policy

* Internal: interleaved `f32` in [-1, 1], device-native sample rate (no hidden resampling
  on record). Recording sample rate is whatever the device reports; it is stored with the
  take and in the WAV header.
* Export: 32-bit float (lossless) or 24-bit PCM (DAW interchange) by user choice; 16-bit
  for compatibility.
* Playback: converted once to the output device's rate/channels (linear interpolation in
  P0; band-limited resampler planned, see ADR-0006).

## 5. Platform notes

* Windows: cpal uses WASAPI shared mode by default. ASIO is possible behind a cpal
  feature but requires the Steinberg SDK (licence/redistribution review) — not in P0.
* macOS: CoreAudio; Linux: ALSA (PipeWire/Pulse expose an ALSA "default"). Nothing in
  the engine is platform-specific; CI should run `cargo test` on all three.
* WebView differences (WebView2 / WebKit / WebKitGTK) only affect the UI layer.

## 6a. Ambient Playground on the audio graph (Phase 1A, 0.3.0)

Graph slots: 0 drone, 1 pad, 2 arrangement clips, 3 reserved, **4..12 the ambient
rack** (layer *i* → slot 4 + *i*), 16 total. One `GraphPlayer` stream serves all of
them; the stream stops when every owner has released its slots (`Inner::graph_idle`).

- **Sources** (`crates/audio-engine/src/graph/sources/`): each is a `Source` with an
  `Arc<…Params>` of smoothed atomics; `gen.rs` holds the shared seeded RNG, the
  sparse trigger (exponential gaps, events placed on their exact sample so a bounce
  equals playback), the bounded random walk and the SVF. Tonal sources read a
  `PitchSet` the shell fills from the project's scale.
- **Model** (`crates/project/src/ambient.rs`, `ambient.json`): layers with a tagged
  `LayerSource` per kind, macros, automation lanes and the scene program. Presets are
  the same structure (`ambient_presets.rs` in code, user files in the app config dir).
- **Shell** (`apps/desktop/src-tauri/src/ambient_cmds.rs`): `sync_live` rebuilds a slot
  only when its build key (kind + seed, or loop take/region/reverse) changes; every
  other edit is an atomic write. A 25 ms worker thread computes
  `program::effective(rack, program_t, lane_t)` — rack → scene snapshot (crossfaded)
  → lanes — and pushes it through the same `apply_*` functions. The offline bounce
  runs the same code per 512-frame block on a private `Graph`, with
  `render_with_stems` for per-layer stems (post-fader, pre-master).
- **Master**: gain → "space" reverb (two decorrelated Schroeder tails, bypassed at 0)
  → limiter → meters.
- **Sample Lab** (`audio_engine::process`): a `Recipe` is applied when a take is loaded
  (studio, clips, loops); originals are never rewritten; recipe and tags live in
  `project.db` (schema v3).

## 6. Known architectural debt after Phase 0

* In-memory take with a 30-minute cap → streaming writer in P1.
* Status polling → push events in P1 if the meter needs > 20 Hz.
* Hand-mirrored TS types → generated bindings when the command surface grows.
* Linear resampler → band-limited for export-quality SRC.
* No transport/metronome abstraction yet; `Player` is a one-shot buffer player. P1
  introduces a `Transport` that both recorder and player attach to (needed for punch-in,
  count-in and later multitrack).
