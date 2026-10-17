# Changelog

## Unreleased — Phase 2: pitch, notation & practice (in progress, 2026-10-16)

### Implemented
- **`crates/music-core`** (pure): Hz ↔ MIDI at a configurable A4; scales (7 modes,
  harmonic/melodic minor, hijaz, double harmonic, pentatonics, user-defined cents);
  tonic/mode hypotheses as a ranked probability list (never a single answer);
  **note extraction** from a contour (ornaments, slides and vibrato kept as bend data
  inside notes); **SMF writer** with pitch bend, RPN range, project or technical tempo.
- **Pitch detection** (`audio_engine::pitch`, YIN): per-frame estimate with
  confidence, streaming tracker, offline contour; **live pitch** in `RecorderStatus`;
  **`InputMonitor`** = tuner/meter on the input without recording.
- **Project**: `analysis/<take>.{contour,notes,notes.edited}.json` (atomic writes);
  `project.json` stores the user-chosen scale and A4 (optional; old files load).
- **Desktop**: tuner (note, cents needle, Hz, confidence, in-scale indicator) with the
  first practice "Hit the note"; "Analyse" → contour + proposed notes + scale
  suggestions with percentages (the user chooses, or none); piano roll aligned to the
  waveform with editing (↑/↓, Delete, S split, M merge, Ctrl+Z undo) saved separately
  from the automatic notes; A4 per project; MIDI export (project tempo/meter or a
  labelled technical 60 BPM with a README next to the file).
- Device tests now wait up to 2 s for WASAPI streams to start (flaky under parallel
  stream opening).
- **2.7a "Свири по ноти"** (0.2.1): `.mid` import via midly (ADR-0019) → `NoteEvent`
  + tempo map (format 0/1, tempo changes, running status, open notes); a MIDI file is
  a take (`midi_file`) rendered by the built-in synth (flute/sine/triangle voices,
  envelope, pitch bend, soft limiter) so waveform, playback, loop and export work
  unchanged; tempo ×0.25–2, mute per track, voice change re-render the take, never the
  file; **falling-notes view** with a piano keyboard, key names and in-scale colouring;
  **"Play the notes"** renders the current notes (extracted, edited or MIDI) with the
  synth — the first musical translator. Spec: `docs/MIDI_PLAYER_TRAINER_SPEC.md`.

### Measurements (owner's laptop, release build, YIN hop 10 ms, window 1024)
| material | pitch time | voiced | f0 p05 / median / p95 | confidence p10 / median | octave-sized jumps |
|---|---|---|---|---|---|
| kaval take 18.7 s, 48 kHz stereo, RMS −30 dBFS | 286 ms (65× realtime) | 35 % | 214 / 338 / 566 Hz | 0.86 / 0.97 | 21 / 621 (3.4 %) |
| `re.mp3` 59.45 s | 1998 ms (30× realtime) | 71 % | 272 / 330 / 377 Hz | 0.93 / 1.00 | 16 / 4151 (0.4 %) |

Synthetic (tests): pure tones < 1 cent at 44.1/48 kHz up to 1760 Hz; harmonic tones
(5 partials) < 2 % frames off by ≥ 100 cents; vibrato ±50 c at 6 Hz tracked (mean
< 10 c); 200→800 Hz sweep < 5 % frames off; silence/noise unvoiced.

## 0.1.0 — Phase 1: core recording & project system (2026-10-16, owner's Windows laptop)

### Implemented
- **Streaming recorder** (ADR-0014): takes go straight to a float32 WAV with the header
  checkpointed every second; `repair_wav_header` recovers a take after a crash; a take
  is never overwritten; live waveform while recording (10 ms peak bins in RAM only);
  write failure and device loss surface in the status and stop the take cleanly;
  noise-floor estimate; requested sample rate validated against the device.
- **Project system** `crates/project` (ADR-0015/0016): `<name>.ethnoflow/` with
  `project.json` (atomic save; tempo map and meter start empty) and SQLite `project.db`
  (migrations v1→v2, BLAKE3 checksums, trim points, settings). Takes are registered
  before recording and repaired on open; originals are verified on every open; delete
  moves to `trash/`; relink accepts only identical bytes; imports keep original bytes;
  derived takes (cut from a selection) link to their parent.
- **Metronome / transport**: sample-accurate click as a pure function of the frame
  (no drift, seek/loop safe), odd meters with groupings (7/8 = 2+2+3 …), count-in
  trimmed off non-destructively, loop range and click mute adjustable while playing.
- **Import** (ADR-0017): MP3/FLAC/Ogg Vorbis/WAV via symphonia, decode only.
- **Desktop**: home screen with the seven areas (only "Play & record" active; others
  disabled with their planned phase); project new/open/recent/close; take list with
  rename, delete (confirmation), relink, verify; device/sample-rate selects, test tone;
  tempo & meter panel; drag selection on the waveform → trim to selection / save as new
  take / loop; export defaults to 32-bit float; keyboard Space/R/Esc/arrows; busy
  indicator; commands run off the main thread; window close finalizes a running take.

### Tested
- Rust: 59 unit + 3 integration (audio-engine, incl. 30-min streaming simulation
  `--ignored`), 8 unit + 10 integration (project: reopen bit-exact, crash recovery,
  orphan adoption, checksum mismatch, relink, trash, import, trim, derived, v1→v2),
  4 (desktop Timing). All green on the laptop with real WASAPI devices; clippy clean.
- TypeScript: tsc clean; 11 vitest.
- Manual on the laptop: project create → record kaval (18.7 s) → stop → file header
  consistent with file size → import `re.mp3` (48 kHz stereo, 59.45 s) → playback;
  release build + NSIS/MSI installers produced; owner installed the NSIS build.

### Known issues
- Phase 1 definition-of-done items that need the owner at the instrument are **not yet
  run**: ≥3-min take → close → reopen checksum; kill-mid-recording → reopen recovery
  (covered by automated tests, not yet by a real crash); 30-min continuous record +
  playback with overruns = 0; clean-machine installer smoke test.
- Hardware log for the manual tests (microphone, headphones, latency feel) not yet
  recorded by the owner.
- After stop, the take is loaded whole into RAM for playback/peaks (streaming player
  is Phase 5); trim reloads the file.
- Debug (`tauri dev`) builds are 10–30× slower in DSP paths than the installer; judge
  speed on the installed build.
- No E2E (tauri-driver) tests and no CI matrix update yet (planned in TEST_PLAN).
- Node.js v24 on the laptop (CLAUDE.md says 22 LTS); no problem observed.

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
