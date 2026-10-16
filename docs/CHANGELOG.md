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
_(to be filled in on the owner's laptop — see CLAUDE.md hand-off)_
