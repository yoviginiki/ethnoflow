# Changelog

## 0.0.1 — Phase 0 (2026-10-16, built and tested in a Linux sandbox)

### Implemented
- Cargo workspace; `crates/audio-engine` (cpal 0.16, hound, ringbuf): device
  enumeration, recorder (RT callback → ring buffer → drain thread, peak/clip/overrun
  counters, 30-min in-memory cap), player (channel mapping + resampling once, atomic
  playhead), WAV float32/pcm24/pcm16 read/write, waveform peak reduction, linear resampler,
  `!Send` stream-thread wrapper.
- `apps/desktop`: Tauri 2 shell with 14 typed commands; React 19 + TS + Vite 8 UI
  (BG/EN), device selects, dBFS meter with peak hold, canvas waveform with time ticks and
  playhead, click-to-seek, record/stop/play, native import/export dialogs, export format
  choice, status/error banners, light/dark theme, keyboard focus styles.
- Fixtures generator (`gen_fixtures`), playback clock diagnostic (`play_timing`).
- Docs: vision, architecture, 13 ADRs, roadmap with Phase 1 plan, risks, test plan;
  CLAUDE.md with Windows commands and hand-off checklist.

### Tested
- 37 automated tests green (34 unit + 3 integration); 6 vitest; tsc and clippy clean.
- Real cpal stream path exercised through an ALSA `null` device: record→stop→buffer,
  play→finish with correct position; Tauri app launched under Xvfb, driven with xdotool:
  idle → record → stop; import fixture → waveform rendered → play (screenshots in
  `docs/phase0/`).

### Known issues
- Not verified on Windows / real microphone / real output clock (sandbox has no audio
  hardware). The ALSA null device has no clock, so timing, latency and overrun behaviour
  with real devices are unmeasured.
- No Windows installer produced here; `npm run tauri build` on the laptop is required.
- Take is held in RAM (30-min cap) until the Phase 1 streaming writer.
- Live waveform during recording shows only an elapsed-time bar; peaks appear after stop.
- Device list showed no named devices in the sandbox (ALSA null only) — verify WASAPI
  names appear on Windows.

### Phase 0 Windows verification
Run on the owner's laptop, 2026-10-16.

**Machine:** Windows 11 Pro 10.0.26200; Intel Core i5-12450H; 16 GB RAM; NVIDIA GeForce
RTX 4050 Laptop GPU + Intel UHD Graphics. Audio: Realtek High Definition Audio (SST) with
Nahimic APO components; active endpoints `Microphone Array (Realtek(R) Audio)` and
`Speakers (Realtek(R) Audio)`. No external audio interface. Microphone and headphones
used for the manual test: not reported.

**Toolchain:** Rust 1.99.1 (stable-x86_64-pc-windows-msvc, rustup 1.29.1), VS 2022
Build Tools 17.14 (VC workload), Node.js v24.20.0 / npm 11.19.0 (CLAUDE.md asks for 22 LTS;
no problem observed).

| step | result |
|---|---|
| `cargo test --workspace -- --nocapture` | 37/37 pass (34 unit + 3 integration); device tests ran against real WASAPI: `recorded 480 frames @ 48000 Hz x2 (overruns 0)`, `played … position_frames: 9600, finished: true, stream_errors: 0`, `device test: 9120 frames @ 48000 Hz x2 → 73028 bytes` |
| `play_timing` (3.0 s tone) | 3.35 s (first, cold), then 3.27 / 3.05 / 3.06 s. Device clock is correct; the extra ~0.3 s on cold runs is WASAPI stream open, which the example's timer includes. No code change. |
| `npm install` / `typecheck` / `npm test` | 0 vulnerabilities; tsc clean; 6/6 vitest pass |
| `npm run tauri dev` | app launched; owner reports recording works. Device names in the selects, meter behaviour, overrun count, clipping, latency feel, headphone playback, 24-bit export → re-import: **not reported in detail** |
| `npm run tauri build` | success (release compile 3 m 14 s). Present: `target/release/bundle/nsis/EthnoFlow Studio_0.0.1_x64-setup.exe` (1.76 MiB), `target/release/bundle/msi/EthnoFlow Studio_0.0.1_x64_en-US.msi` (2.45 MiB) |
| install + repeat manual test from installed app | owner installed the NSIS build and reports it works (no detailed measurements) |

**Issues found:** none in code. Open: manual-test details above not yet recorded; Nahimic
APO on this Realtek path may add latency (unmeasured).
