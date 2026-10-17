# CURRENT_STATE — what EthnoFlow Studio actually contains (2026-10-18)

Shared inventory for the three owner programs (Rhythm Laboratory & Instrument Forge,
Music DNA / Arrangement Composer / Musical Director, Live Performance Director). Every
claim below points at a file; "unknown" is written where it was not verified. Screenshots
are from the running app on the owner's machine (1536×864 logical = 1920×1080 at 125 %),
taken through the assistant bridge on 2026-10-18 (`docs/audits/screenshots/`).

**Резюме за собственика.** Имаме работещ аудио граф с 16 слота, четири ритъм инструмента
(барабани, бас, остинато, арпеджио) плюс контрамелодия към MIDI, дрон, пад, атмосфера с
7 вида слоеве, писти с клипове, анализ на височина/ноти/лад/фрази/темпо и библиотека с
пресети. Нямаме: инструменти като отделни пресети със собствена страница, секвенсер по
стъпки, песенна структура (секциите са само маркери, ритъмът не ги чете), темпо карта,
анализ на чужди песни (няма FFT), стемове от ритъма, транспорт в тактове за цялото
приложение. Експортът „Микс“ записва само клиповете, без фона. Това е честната основа за
трите програми.

## 1. Stack and process model

| Layer | What | Files |
|---|---|---|
| Shell | Tauri 2, one `AppState { Mutex<Inner> }`, commands per domain | `apps/desktop/src-tauri/src/*.rs` (22 modules, `lib.rs` registers ~150 commands) |
| UI | React 18 + TypeScript + zustand, Vite; strings via `i18n.ts` (BG + EN); polling (150 ms) for meters/status, no event push | `apps/desktop/src/*.tsx` |
| Engine | Rust crate `ethnoflow-audio-engine`: cpal streams, recorder, player, graph | `crates/audio-engine/src/` |
| Theory | `ethnoflow-music-core` (pure): scales, pitch, notes, phrases, tonal tracker, intent, harmony, groove, bassline, ostinato, counter, tempo | `crates/music-core/src/` |
| Project | `ethnoflow-project`: `.ethnoflow` folder, `project.json`, SQLite takes, `ambient.json`, `arrangement.json`, `session.json`, library | `crates/project/src/` |
| Deps | cpal, hound, symphonia (mp3/flac/ogg/wav decode only), rusqlite, blake3, midly, midir, serde, ringbuf; **no FFT crate, no Python** | `docs/DECISIONS.md` ADR-0001…0020 |

Audio callback rule (CLAUDE.md §1) is honoured: sources exchange state through
`SmoothedParam`/atomics and `ringbuf` (`PatternSwap`), no allocation in `render`.

## 2. Audio graph (the thing every program must build on)

`crates/audio-engine/src/graph/mod.rs`: fixed **16 slots** (`MAX_LAYERS = 16`), block of
64 frames, per-slot gain/pan/one-pole LP/HP/mute/solo/`fade_ms`, master gain + master
reverb ("space") + limiter, `render`/`render_with_stems`/`render_offline`, frame counter.
Slot map (shell constants):

| Slot | Owner | Source |
|---|---|---|
| 0 | drone | `DroneSource` (`graph_cmds.rs`) |
| 1 | pad | `PadSource` (`pad_cmds.rs`) |
| 2 | arrangement clips | `ClipSource` (`arrangement_cmds.rs`) |
| 3 | reserved | — |
| 4..11 | atmosphere rack | noise/wind/texture/bells/pluck/loop/**cloud** (`ambient_cmds.rs`) |
| 12 | drums | `DrumSource` (`rhythm_cmds.rs`) |
| 13 | bass | `BassSource` |
| 14 | ostinato | `PatternSynthSource` |
| 15 | arpeggio | `PatternSynthSource` |

Background mixer buses (2026-10-18, `bus_cmds.rs`) fold trim/pan/cuts/fades into these
slots. The graph is a flat mixer: **no instrument-level graphs, no sends per slot, no
sidechain bus** (the bass reads the kick envelope through a shared atomic instead), no
per-slot effects beyond LP/HP.

Timebase: `BeatClock` (`sources/rhythm.rs`) = anchor frame/beat/bpm + generation; the
shell's `timing()` gives one BPM and one meter per project (`project.json` `timing`).
**No tempo map, no meter map.** The arrangement transport (`ClipsParams.transport`) runs in
seconds; the rhythm runs on beats; they share the graph frame counter but not a bar grid.

## 3. The rhythm section as it is (the "five instruments")

| # | Instrument | Pattern generator (music-core) | Sound (engine) | Live params | Persisted |
|---|---|---|---|---|---|
| 1 | Drums | `groove.rs::generate_groove` — styles liquid / halftime / ethno, complexity, humanize, swing, `fill_every_bars`, seed, `bars` 1..64 | `DrumSource`: synthesized kick/snare/ghost/closed+open hat/shaker; `kick_hz`, `decay`, `brightness`; publishes kick envelope | level, kick, decay, brightness | `prefs.rhythm.drums` (per project via `session.json`) |
| 2 | Bass | `bassline.rs` over `static_spans` (chord/root spans for `bars`) — density, syncopation, passing, register | `BassSource`: voices sub / pluck / Reese, cutoff, resonance, drive, glide, sidechain (from kick env), wobble div/depth, warmth | all of those | `prefs.rhythm.bass` |
| 3 | Ostinato | `ostinato.rs::generate_ostinato` — steps, step_beats, density, probability, `variation_every_bars`, lockable `Motif` | `PatternSynthSource` (8 voices, timbre 0..2, release) | level, timbre, release | `prefs.rhythm.ostinato` |
| 4 | Arpeggio | `ostinato.rs::generate_arpeggio` — direction, octaves, step, gate, density | `PatternSynthSource` | level, timbre, release | `prefs.rhythm.arp` |
| 5 | Countermelody | `counter.rs::generate_countermelody` from the analysed melody's phrases | **offline only** → MIDI take (`harmony_cmds::countermelody_to_take`); no live source | — | take in the project |

Mechanics (`rhythm_cmds.rs`, 1117 lines): `start_rhythm` builds the four sources on one
`BeatClock`; `sync` regenerates patterns when a *key* (style/complexity/seed/bars/meter/
chord…) changes and hands them over lock-free (`PatternSwap`, cursor re-seeks the beat);
`apply_live` pushes continuous params; `on_timing_changed` re-anchors the clock; UI
debounces 120 ms. `rhythm_to_takes` writes the four parts as MIDI takes (drums on GM 10).
Locks (`locked`) protect a part from "Регенерирай". The UI is `RhythmPanel.tsx` (438
lines: one row per part with level, lock, seed, expandable sliders) inside Compose and in
its own screen `RhythmScreen.tsx` (screenshot `2026-10-18-rhythm.png`).

What does **not** exist in the rhythm section: a step grid or event editor; per-step
velocity/probability/ratchet/microtiming/parameter locks; more than 4 live parts; sample
playback for drums; instrument presets with their own identity (the library saves the
whole `rhythm` prefs object, kind `rhythm`); an instrument designer page; envelopes
(ADSR) or filter envelopes as user controls; per-part effects or sends; pattern chaining,
scenes or variations A/B/C/D; Euclidean generator; polymeter (one `bars` length for all);
groove templates/extraction; live MIDI/pad recording into patterns; **offline render of
the rhythm** (no WAV, no stems; only MIDI).

### Why the DnB sounds like a loop (measured in code, not by ear)

1. Every generator produces exactly `bars` bars (UI default 4) and the sources loop that
   block forever (`Cursor` over `length_beats`). Humanize jitter is baked in at generation,
   so **every cycle is bit-identical**.
2. The only time-structured events are `fill_every_bars` (a snare roll in the last beat,
   `groove.rs::fill`) and the ostinato's `variation_every_bars`. There is no energy arc, no
   section plan, no instrument entries/exits, no transitions, no automation on rhythm or
   bass parameters (automation lanes exist only in the atmosphere rack, `program.rs`).
3. The bass follows `static_spans`: one chord (or the root) held for the whole pattern;
   harmony proposals (`harmony.rs`) are not placed on a timeline.
4. Section markers exist (`set_markers`, names Intro/A/B/Build/Drop/Break/Outro,
   `Arrange.tsx` ruler) but **nothing reads them**: `rhythm_cmds.rs` has no reference to
   markers; the arrangement transport does not expose "current section".
5. Kaval takes are placed as clips on tracks; nothing analyses *where* the melody is active
   to make room (the phrase analysis exists, `phrase.rs`, but is not connected to the
   rhythm or the mixer).

## 4. Harmony, analysis and performance intelligence (reusable by all programs)

- Pitch: YIN per frame with confidence, streaming tracker, offline contour → notes with
  bends (`pitch/`, `analysis_cmds.rs`; `analysis/<take>.{contour,notes,notes.edited}.json`).
- Key: ranked hypotheses over 13 scale kinds with evidence (`music-core/scale.rs`, shown
  with percentages; the user chooses; `set_scale`).
- Phrases: boundaries by breath/rest/long tone (`phrase.rs`), tonal tracker with
  hysteresis (`tracker.rs`), intent classifier stable/ornament/transition/noise/error
  (`intent.rs`), modes Stable/Assisted/Creative with benchmarks (`BENCHMARKS.md`).
- Harmony proposals: 3 strategies with score breakdowns, A/B, countermelody (`harmony.rs`,
  `counter.rs`, `HarmonyPanel.tsx`).
- Tempo: `tempo.rs::estimate_tempo` from note onsets (candidates with fit); **no audio beat
  tracker**, no downbeats, no spectral features of any kind (no FFT).
- Trainer: evaluates a performance against notes (`trainer.rs`, `TrainerPanel.tsx`).
- Input latency calibration screen exists (`Calibrate.tsx`); **output-latency compensation
  for visual cues: unknown / not implemented**.

## 5. Arrangement, transport and export

- `arrangement.json`: tracks (name, gain, pan, mute/solo) with clips (take, start, offset,
  length, fades) in **seconds**; loop region; markers in `project.json`. UI `Arrange.tsx`
  (1244 lines): ruler with markers, clips drag/trim, record into a track, import, undo/redo.
  Screenshot `2026-10-18-studio.png`.
- Transport: `arrangement_play/pause/seek/status`; the background layers run on the same
  graph and are heard, but **"Експорт микс" renders only `ClipSource`**
  (`export_arrangement_mix`: a fresh graph with the arrangement slot alone) — drone, pad,
  atmosphere and rhythm are not in the exported file. The atmosphere has its own offline
  export with stems (`export_ambient`). MIDI export: analysed notes, harmony, rhythm parts.
- No tempo map, no bar/beat positions on the timeline (ruler shows seconds and bar
  numbers derived from the single BPM).

## 6. Persistence and presets

- Project folder: `project.json` (name, timing, scale, a4, instrument, markers, takes
  index), `project.db` (takes, recipes, meta), `assets/originals/*` (immutable),
  `analysis/*.json`, `ambient.json`, `arrangement.json`, `session.json` (UI settings).
- Library (`library_cmds.rs`, app config `library/*.json`): kinds session, module, drone,
  pad, rhythm, harmony, intent, ambient, background; name/tags/notes/project/dates;
  search over notes. No versioning, no lineage, no asset references, no schema version
  per entry (the payload is the UI's JSON).
- Factory content: 9 atmosphere presets (`ambient_presets.rs`); licensing gate and
  manifest for shipped audio (`licensing.rs`, `assets/asset-manifest.json`); drums are
  synthesized, no shipped samples.

## 7. Tests and quality baseline (run 2026-10-18)

| Suite | Count | Command |
|---|---|---|
| Rust unit + integration | 190 `#[test]` functions (engine 131 incl. `tests/ambient_rack.rs`, `tests/pipeline.rs`; music-core 61 + `tests/modal_fixtures.rs`, `tests/performance_scenarios.rs`; project 24 + `tests/lifecycle.rs`; shell 13) | `cargo test --workspace` — green |
| Clippy `-D warnings` | clean | `cargo clippy --workspace --all-targets -- -D warnings` |
| TypeScript | clean | `npm run typecheck` |
| Vitest | 31 | `npm test` |

Audio evidence available today: deterministic offline renders exist for the atmosphere
(export with stems) and in engine tests; **no render path for the rhythm section** — the
first R0 deliverable must add one before any "before/after" listening can be recorded.

## 8. Known defects found while auditing

- Window title shows `EthnoFlow Studio вЂ" Phase 0`: the em dash in
  `tauri.conf.json` `"title"` is mis-encoded (visible in every screenshot). Fix: plain
  ASCII title or correct UTF-8.
- "Експорт микс" silently omits the background (see §5): users will expect what they hear.
- The screen title still says "Phase 0" in the window chrome.

## 9. Gaps against the three programs (summary; details in each audit)

| Need | State |
|---|---|
| Instrument = preset with identity, own page, registry > 5 | **missing** (fixed 4 slots + prefs blob) |
| Step sequencer, per-step data, polymeter, Euclid | **missing** |
| Envelopes/filters/FX per instrument as controls | partial (bass filter, drum kick/decay/brightness) |
| Rhythm offline render / stems | **missing** |
| Song structure data model (sections in beats, tempo map) | **missing** (markers in seconds only) |
| Reference-track analysis (beats, novelty, features) | **missing** (no FFT, no beat tracker) |
| Arrangement generator over sections, transitions, variation | **missing** (loops only) |
| Director diagnostics | **missing** |
| Transport in bars/beats with lookahead cue scheduling | partial (`BeatClock` for rhythm only; UI polls) |
| Cue model, countdown, coach UI | **missing** |
| Error-tolerant performance analysis | **present offline** (`intent.rs`, `tracker.rs`) |
| Take recording with raw preservation | **present** (`recorder.rs`, originals immutable) |
