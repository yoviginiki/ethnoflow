# EthnoFlow Studio

Desktop music learning, recording and accompaniment studio for ethno melodies.
Windows first, cross-platform core. Specification: `docs/MASTER_PLAN.md`.

**Status: Phase 0 (feasibility) — vertical slice implemented and tested in a Linux
sandbox; Windows verification pending. See `docs/ROADMAP.md`.**

## Quick start (Windows, development)

```powershell
cargo test --workspace                 # Rust engine tests
cd apps\desktop
npm install
npm run tauri dev                      # live app with hot reload
npm run tauri build                    # release + NSIS/MSI installers
```

For Claude Code, start with `CLAUDE.md`.

## Layout

- `crates/audio-engine` — Rust audio I/O, WAV, peaks (37 tests)
- `apps/desktop` — Tauri 2 + React/TypeScript UI (BG/EN)
- `docs/` — vision, architecture, ADRs, roadmap, risks, test plan, changelog
- `qa/fixtures` — synthetic test audio (generated)
- `scripts/release` — Windows build and smoke scripts
