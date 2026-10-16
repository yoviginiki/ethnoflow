# Test plan

Principle (master plan §13/§15): report failures as failures; a test that cannot run is
"not run: reason", never PASS.

## What exists after Phase 0

| layer | location | count | runs where |
|---|---|---|---|
| Rust unit | `crates/audio-engine/src/*` (`#[cfg(test)]`) | 34 | anywhere; 3 of them self-skip without a device |
| Rust integration | `crates/audio-engine/tests/pipeline.rs` | 3 | anywhere; 1 self-skips without a device |
| TS unit | `apps/desktop/src/*.test.ts` (vitest) | 6 | anywhere |
| Type check | `tsc --noEmit`, `cargo clippy -D warnings` | – | anywhere |
| UI smoke | manual/xdotool under Xvfb, screenshots in `docs/phase0/` | 3 flows | Linux sandbox so far |

Covered: buffer maths (peak/RMS/clip/mono/slice), WAV float32/pcm24/pcm16 roundtrips
with tolerances, corrupt/missing files, waveform peak reduction (bin counts, amplitude,
exact small cases), linear resampler (length, frequency, channel independence),
ring-buffer push/pop + overrun accounting, atomic float peak hold, in-memory cap,
device enumeration never panicking, record→stop shape, play→finish, mono/stereo
adaptation, full export→import pipeline incl. stereo channel identity, UI formatting.

Not covered yet (explicitly): real microphone capture on Windows, WASAPI device names
and switching, real-time playback timing (null device has no clock), native dialogs,
installer.

## Phase 0 verification matrix on the Windows laptop

| test | command | expected |
|---|---|---|
| engine tests with real devices | `cargo test --workspace -- --nocapture` | 37 pass; "recorded N frames @ 48000 Hz" and "played ..." lines printed, overruns 0 |
| playback clock | `cargo run -p ethnoflow-audio-engine --example play_timing` | ≈ 3.0 s wall time |
| dev app | `npm run tauri dev` | devices listed by WASAPI name; meter moves; waveform after stop; playback audible with playhead; export file opens in a DAW |
| export fidelity | export float32 → import → compare `TakeInfo` peak/RMS/frames | identical |
| release + installer | `npm run tauri build` | `.exe` under `target/release/bundle/nsis/` installs without admin and repeats the dev-app checks |

## Phase 1 additions (planned)

- Streaming writer: synthetic 30-min source, header recovery from truncated file.
- Project roundtrip, schema migration, checksum mismatch, missing-file relink.
- Transport/metronome sample-accuracy at 60/120/174 BPM, 4/4, 7/8 (2+2+3), 9/8.
- Device disconnect simulation (error callback injection) → clean stop, take preserved.
- symphonia MP3/FLAC decode fixtures; `re.mp3` local only.
- E2E (WebDriver via `tauri-driver`): first run, record, stop, play, export, reopen.
- CI: GitHub Actions matrix windows/macos/ubuntu for `cargo test` + `npm test`;
  Windows job also runs `tauri build` and uploads the installer as an artifact.
