# Roadmap and phase gates

Each phase ends with a real demo and an `Implemented / Tested / Known Issues / Next /
User Decisions` report, then stops for approval. Phases are from the master plan §14;
this file tracks status and the concrete Phase 1 plan.

| phase | name | status |
|---|---|---|
| 0 | Discovery & feasibility | **done in sandbox (Linux/Xvfb); Windows verification pending on owner's laptop** |
| 1 | Core recording & project system | planned (below) — needs approval |
| 2 | Pitch, notation & practice | not started |
| 3 | Harmony / drone MVP | not started |
| 4 | Liquid DnB engine | not started |
| 5 | Mini DAW + export | not started |
| 6 | Beginner Academy v1 | not started |
| 7 | Intelligent variations | not started |
| 8 | Live accompanist R&D | not started |
| 9 | Release readiness | not started |

## Phase 0 — exit criteria vs. reality

Master plan exit: "installable app that records microphone and plays back WAV reliably".

| criterion | state |
|---|---|
| repository, docs, ADRs, CLAUDE.md | done |
| Tauri skeleton builds and runs | done (Linux release binary, Xvfb screenshots in `docs/phase0/`) |
| Rust audio spike: device → record → waveform → play → WAV | done; 37 automated tests green; hardware path exercised only through an ALSA null device |
| Python packaging spike | deferred to Phase 2 start (ADR-0011) — not needed by P0/P1 |
| installable Windows build | **not produced here** (Linux sandbox). `npm run tauri build` on Windows is step 4 of the hand-off in CLAUDE.md |
| real microphone test | **pending** on the owner's laptop |

Phase 0 is therefore *technically complete, physically unverified on Windows*. The
first hour on the laptop closes that gap before Phase 1 starts.

## Phase 1 — Core recording & project system (proposed plan)

Exit (master plan): record and reopen an original kaval take with no data loss.

### 1.1 Streaming recorder (replaces in-memory cap)
- Drain thread writes `assets/originals/<take-id>.wav` (32-bit float) incrementally;
  header finalised on stop; recovery pass on startup repairs a header left open by a crash.
- Peak cache computed incrementally during recording (live waveform while recording,
  not just a time bar).
- Tests: long-recording simulation (synthetic source, 30+ min equivalent), crash-recovery
  fixture (truncated WAV), overrun injection.

### 1.2 Project system (`packages/project-schema` + SQLite)
- `.ethnoflow` directory layout from master plan §10; `project.json` v1 with schema
  version, sample rate, tempo map placeholder, markers, takes list.
- SQLite (`rusqlite`, bundled) for metadata, checksums (BLAKE3) of originals, autosave
  every N seconds + on every state change; migrations table from day one.
- Tests: save/load roundtrip, migration v0→v1, missing-file relink, checksum mismatch
  detection.

### 1.3 Transport + metronome
- `Transport` abstraction (position, play/stop/record, loop range, tempo, time signature
  incl. 5/8, 7/8, 9/8 groupings); metronome click rendered in the engine (sample-accurate),
  count-in, mute during playback.
- Tests: click placement sample-accurate at several tempos/meters; loop boundaries.

### 1.4 Device settings & robustness
- Sample-rate/channel selection where the device supports it; input gain meter with
  noise-floor estimate; device test tone; feedback warning when input monitoring is on.
- Device disconnect/reconnect: stream error → clean stop, take preserved, UI message.
- Latency calibration tool (loopback click test) — measurement only, used in Phase 2.

### 1.5 Import
- MP3/FLAC/OGG decode via `symphonia` (ADR-0004); owner's `re.mp3` used locally only.
- Non-destructive trim (in/out points stored in the project; original untouched).

### 1.6 UI
- Home screen stub with the seven areas from master plan §9 (only "Свири и записвай"
  active, the rest clearly marked as not yet available — no fake buttons).
- Project open/new/recent; take list; take rename; delete with confirmation.
- Keyboard: Space play/stop, R record, Esc stop; screen-reader labels on controls.

### 1.7 Definition of done
- Record a real kaval take ≥ 3 min → close app → reopen → identical bytes (checksum).
- Kill the app mid-recording → reopen → the take up to the crash is recoverable.
- 30-minute continuous record + playback on the owner's laptop without dropouts
  (overrun counter stays 0) — logged in CHANGELOG with hardware details.
- Windows NSIS installer built from CI/script; clean-machine smoke test.

Estimated effort: 2–3 weeks of focused work.

### User decisions needed before Phase 1
1. Approve this plan (or reorder 1.1–1.6).
2. Confirm `re.mp3` may be used locally for import tests (never uploaded).
3. Default export format for "Export": 24-bit PCM (current) or 32-bit float?
4. Project location default: `%USERPROFILE%\Documents\EthnoFlow\` ?
5. Owner's laptop audio hardware (interface, mic, GPU) for the hardware log.
