# Test plan

Principle (master plan §13/§15): report failures as failures; a test that cannot run is
"not run: reason", never PASS.

## What exists after Phase 1 (2026-10-16)

| layer | location | count | runs where |
|---|---|---|---|
| Rust unit | `crates/audio-engine/src/*` | 59 (+1 `--ignored` 30-min simulation) | anywhere; device tests self-skip without a device |
| Rust integration | `crates/audio-engine/tests/pipeline.rs` | 3 | anywhere; 1 self-skips without a device |
| Rust unit | `crates/project/src/*` | 8 | anywhere |
| Rust integration | `crates/project/tests/lifecycle.rs` | 10 | anywhere |
| Rust unit | `apps/desktop/src-tauri/src/commands.rs` (Timing) | 4 | anywhere |
| TS unit | `apps/desktop/src/*.test.ts` (vitest) | 11 | anywhere |
| Type/lint | `tsc --noEmit`, `cargo clippy --workspace --all-targets -D warnings` | – | anywhere |
| Local-only | `decode::decodes_owner_reference_mp3_if_present` (`re.mp3`) | 1 | owner's laptop; skips elsewhere |

Covered: buffer maths; WAV roundtrips and corrupt files; **header repair after crash**;
**incremental take writer** (checkpoint validity, never overwrite, size cap, 30-min
stream); live peaks vs offline peaks; ring buffer overrun accounting and metering;
device enumeration and **rate/channel selection**; noise-floor estimate; click
sample-accuracy at several tempos and odd meters, seek/loop equivalence, mixing; player
loop wrap and click mute; symphonia decode (WAV identical to hound, garbage rejected);
project manifest roundtrip and schema refusal; **SQLite migrations v1→v2**; take rows;
**reopen bit-exact, crash recovery, orphan adoption, checksum mismatch, relink, trash,
import bytes, trim validation, derived takes**; Timing validation; UI formatting,
grouping parser, keyboard-target guard.

Not covered by automation (explicitly): real crash of the process (only simulated on
disk), device unplug (flag set from cpal's error callback, not exercised), native
dialogs, installer, window-close finalisation, UI flows.

## Phase 1 verification on the Windows laptop

| test | status |
|---|---|
| `cargo test --workspace -- --nocapture` with real WASAPI | pass (recorded/played lines printed, overruns 0) |
| `npm run typecheck`, `npm test` | pass |
| `npm run tauri build` → NSIS + MSI present | pass |
| record 18.7 s kaval → header matches file size | pass |
| import `re.mp3` → 48 kHz stereo 59.45 s | pass |
| ≥3-min take → close → reopen → checksum identical | **not run: owner at the instrument** |
| kill process mid-recording → reopen → recovered take | **not run: owner at the instrument** |
| 30-min record + playback, overruns = 0 | **not run: owner at the instrument** |
| installer on a clean machine | **not run: no clean machine yet** |

## Phase 1 additions planned vs done

- Streaming writer + header recovery — done.
- Project roundtrip, migration, checksum mismatch, relink — done.
- Metronome sample-accuracy at 60/120/174 BPM, 4/4, 7/8 (2+2+3), 9/8 — done
  (60/97.3/174/233 BPM, 3/4, 7/8, 9/8, 5/8).
- Device disconnect simulation — **not done** (needs a fake cpal host; flagged path only).
- symphonia fixtures — done for WAV; MP3/FLAC fixtures **not done** (no licensed fixture
  files yet; `re.mp3` local only).
- E2E via `tauri-driver` — **not done**.
- CI matrix with Windows `tauri build` artifact — **not done** (`.github/workflows/ci.yml`
  still Phase 0).

## Phase 2 (2026-10-16)

| layer | location | count |
|---|---|---|
| Rust unit | `crates/music-core/src/*` (pitch, scale, key, notes, midi) | 24 |
| Rust unit | `crates/audio-engine/src/pitch.rs`, `monitor.rs`, recorder tuner path | +8 |
| Rust unit | `apps/desktop/src-tauri/src/analysis_cmds.rs` | 2 |
| Rust integration | `crates/project/tests/lifecycle.rs` (analysis docs) | +1 |
| TS unit | `format.test.ts` (pitch helpers) | +3 |

Done: conversions at several references; membership incl. non-tempered custom
scales; hypotheses (clear major > 50 %, hijaz vs harmonic minor, ambiguous < 10 %);
pure tones < 1 cent to 1760 Hz at 44.1/48 kHz; harmonic tones < 2 % octave errors;
vibrato; sweep; silence/noise unvoiced; streaming = offline; note extraction (exact
melody, vibrato = one note, grace note kept, repeated pitch split by rest and onset,
slide leads into its target, low confidence → none); SMF bytes parsed back (VLQ, ticks,
tempo/meter metas, RPN, bend clamp and reset); analysis documents roundtrip; monitor and
recorder publish the mono-mix pitch.

Measured on the kaval take and `re.mp3`: see CHANGELOG "Measurements".

Not done: tuner update latency on the laptop (needs a loopback or the owner's
judgement); note precision/recall on *real* kaval material (owner's check in the piano
roll); DAW re-import of the exported MIDI (owner).
