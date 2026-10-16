# CLAUDE.md — EthnoFlow Studio

Working rules for Claude Code in this repository. The product specification is
`ETHNOFLOW_CLAUDE_CODE_MASTER_PLAN.md` (keep a copy at `docs/MASTER_PLAN.md`); this file
is the operational layer on top of it.

## What this is

Windows-first (cross-platform core) desktop app: music academy + live instrument lab +
mini DAW + rule-based accompanist for ethno melodies. Tauri 2 shell, React/TS UI, Rust
audio engine. **Phase 0 is complete; Phase 1 starts only after the owner's approval.**
Current phase and definition of done: `docs/ROADMAP.md`.

## Repository layout

```
apps/desktop/            Tauri 2 + React + TypeScript + Vite (src/ = UI, src-tauri/ = shell)
crates/audio-engine/     Rust: devices, recorder (streams to disk), player, WAV, peaks, resampler
crates/project/          Rust: .ethnoflow folder, project.json, SQLite, takes, recovery
qa/fixtures/             generated synthetic WAVs (gitignored) + README
docs/                    PRODUCT_VISION, ARCHITECTURE, ROADMAP, DECISIONS (ADRs), RISKS,
                         TEST_PLAN, CHANGELOG, phase0/ screenshots
scripts/release/         Windows build + smoke scripts
```

Crates planned but not yet created (do not stub them with fake code): `crates/music-core`,
`crates/arranger`, `crates/plugin-host` (CLAP, ADR-0018 proposed), `services/analysis`,
`packages/lesson-content`. (`packages/project-schema` became `crates/project`, ADR-0015.)

Product concept for the instrument layer (Phase 2+): `docs/INSTRUMENT_MACHINE_CONCEPT.md`.
Where it differs from the master plan, §6 of that file records the resolution; the rule is
always "the assistant proposes, the user decides".

## Prerequisites (Windows dev machine)

- Rust stable (rustup), MSVC build tools ("Desktop development with C++")
- Node.js 22 LTS + npm
- WebView2 runtime (present on Windows 10/11; the installer bootstraps it otherwise)
- For installers: NSIS and/or WiX are downloaded automatically by `tauri build`
- No Python is needed for Phase 0/1. No toolchain is ever required on end-user machines.

## Commands

All from the repo root unless noted.

```powershell
# Rust engine: build + tests (no audio device needed; device tests self-skip)
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

# Diagnostics on a real machine (expect ≈3.0 s wall time for 3 s of audio)
cargo run -p ethnoflow-audio-engine --example play_timing
cargo run -p ethnoflow-audio-engine --example gen_fixtures      # writes qa/fixtures/*.wav

# Desktop app (from apps/desktop)
npm install
npm run typecheck          # tsc --noEmit
npm test                   # vitest (pure TS units)
npm run tauri dev          # live app: Vite + Rust, hot reload for the UI
npm run tauri build        # release build + NSIS/MSI installers (see below)
npm run tauri build -- --no-bundle   # release binary only
```

Installer output after `npm run tauri build` (only if the command succeeded):
`target/release/bundle/nsis/EthnoFlow Studio_<version>_x64-setup.exe` and
`target/release/bundle/msi/EthnoFlow Studio_<version>_x64_en-US.msi`.
Never claim an installer exists unless that path is present.

**Build gotcha:** build the shell with `npm run tauri build` or `npm run tauri dev`, never
`cargo build --release` alone — the latter does not embed the frontend and the window shows
"Could not connect to localhost".

## Hard rules for changes

1. Audio callbacks (`build_input_stream` / `build_output_stream` closures) never allocate,
   lock, log or touch files. Share state with atomics and ring buffers only.
2. Raw audio frames never cross the Tauri command boundary or enter React state. Send
   reduced data: `WaveformPeaks`, status structs, `TakeInfo`.
3. The original recording is immutable. Any processing returns a new `AudioBuffer`.
4. `cpal::Stream` is `!Send`; keep streams inside `stream_thread::StreamThread`.
5. No mock implementations in production paths. If something is not real, it is behind
   a feature flag and the UI says so. No "Generate" buttons that fake output.
6. Never hard-code D minor, 174 BPM or 4/4 as assumptions about user material.
7. Do not add a dependency without a line in `docs/DECISIONS.md` (license, Windows
   support, maintenance, supply-chain note).
8. Small commits, one concern each. Conventional prefix: `engine:`, `desktop:`, `docs:`,
   `qa:`, `release:`.
9. After each step: run `cargo test --workspace`, `npm run typecheck`, `npm test`. Report
   failures as failures. A test that cannot run is reported as "not run: <reason>", never
   as PASS.
10. Ask before: paid APIs, uploads of user audio, destructive file operations, or changing
    the stack (Tauri/React/cpal). Propose an ADR instead of migrating.
11. UI strings go through `apps/desktop/src/i18n.ts` (BG + EN). No bare literals in JSX.
12. The phase gate is real: finish the phase's definition of done, write the
    `Implemented / Tested / Known Issues / Next / User Decisions` report, then stop.

## Phase 0 → Phase 1 hand-off (first thing to do on the Windows laptop)

1. `cargo test --workspace` — all green; device tests should now actually record/play.
2. `cargo run -p ethnoflow-audio-engine --example play_timing` — expect ≈3.0 s.
3. `cd apps/desktop && npm install && npm run tauri dev` — record a kaval/flute take with a
   real microphone, check the meter, stop, see the waveform, play it back with headphones,
   export 24-bit WAV, re-import it. Note the actual latency feel, any overruns, device names.
4. `npm run tauri build` — confirm the NSIS installer path above exists; install on the
   same machine and repeat step 3 from the installed app.
5. Record findings in `docs/CHANGELOG.md` under "Phase 0 Windows verification", then wait
   for approval of Phase 1 (`docs/ROADMAP.md`).
