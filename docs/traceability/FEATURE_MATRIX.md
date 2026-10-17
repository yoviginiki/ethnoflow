# FEATURE_MATRIX — requirements → implementation → tests → evidence (v1, 2026-10-18)

Statuses per the fidelity contract: NOT STARTED · DESIGNED · IMPLEMENTING · FUNCTIONAL ·
TESTED · VISUALLY VERIFIED · DONE · BLOCKED. "DONE" is never set by code alone; the
owner's listening/visual check moves TESTED → VISUALLY VERIFIED/DONE. This v1 covers the
requirement *groups*; each program phase adds its rows when it starts.

## A. Existing product (FEATURE_SPEC Phases 0–4, 1A, 1B so far)

| Req | Component | Persistence | Tests | Evidence | Status |
|---|---|---|---|---|---|
| F1 recording, takes, project | `recorder.rs`, `project/` | project.json + SQLite | engine `pipeline.rs`, project `lifecycle.rs` | phase0 screenshots | TESTED (owner manual DoD items pending) |
| F1.6 audio graph | `graph/mod.rs` | — | graph tests (sum/mute/solo/pan/filters/fade) | — | TESTED |
| F2 pitch, notes, key, trainer | `pitch/`, `scale.rs`, `trainer.rs` | `analysis/*.json` | music-core 61, scenarios | BENCHMARKS.md | TESTED |
| F2.9–2.11 phrases, tracker, intent | `phrase.rs`, `tracker.rs`, `intent.rs` | annotations json | `performance_scenarios.rs` | BENCHMARKS.md | TESTED (offline) |
| F3 drone, pad, Compose First, harmony | `drone.rs`, `pad.rs`, `harmony.rs`, `Compose.tsx` | prefs/session | engine + music-core | — | TESTED |
| F4.1–4.7 rhythm section | `groove/bassline/ostinato/counter.rs`, `rhythm/bass/patsynth.rs`, `rhythm_cmds.rs` | prefs/session, MIDI takes | music-core + engine + shell | screenshot 2026-10-18-rhythm.png | TESTED (loop-only by design, see audits) |
| F4.3 sections | markers in `project.json`, `Arrange.tsx` | project.json | shell | screenshot studio | FUNCTIONAL (not consumed by rhythm) |
| F5.1 tracks & clips | `arrangement.rs`, `clips.rs`, `Arrange.tsx` | arrangement.json | engine + shell | screenshot studio | TESTED |
| F1A atmosphere rack, presets, automation, scenes, export | `ambient*.rs`, `Ambient.tsx` | ambient.json, presets | `ambient_rack.rs` | — | TESTED |
| F1B.0 Start from a melody | `Start.tsx`, `starters.ts` | library sessions | vitest 6 | screenshot start | TESTED |
| F1B.1 Soundscapes | `Soundscapes.tsx` | library kind module | — | screenshot soundscapes | FUNCTIONAL (needs owner visual check) |
| F1B.2 Cloud | `grain.rs`, `LayerSource::Cloud` | ambient.json | 4 engine tests | — | TESTED (listening pending) |
| Background mixer | `bus_cmds.rs`, `BackgroundMixer.tsx` | prefs/session | graph fade test, shell test | — | TESTED (listening pending) |
| Preset library, sessions, Library screen | `library_cmds.rs`, `PresetLibrary.tsx`, `Library.tsx` | config/library | shell test, vitest | — | TESTED |
| Assistant bridge | `assistant_cmds.rs`, `assistant.ts` | config/assistant | shell test; live ping | state read 2026-10-17 | TESTED |
| Help / guide | `Help.tsx`, `docs/guide/*.md` | — | vitest md | — | FUNCTIONAL |

## B. Program P1 — Rhythm Laboratory & Instrument Forge (spec §14)

| Req (spec) | Target component | Tests | Status |
|---|---|---|---|
| R0 audit + plan + baseline render | `docs/rhythm/*`, `rhythm_render` (offline WAV) | render determinism test | IMPLEMENTING (docs) |
| R1 instrument registry, browser, preset schema v1, migration | `crates/project/src/instrument.rs` (new), `library_cmds` kind `instrument`, `InstrumentBrowser.tsx` | schema round-trip, migration | DESIGNED |
| R2 Instrument Designer MVP (one instrument, real DSP, Save As, in rhythm, save/reopen, WAV) | `InstrumentDesigner.tsx`, engine envelopes/filter on `BassSource`/`DrumSource`, audition path | envelope/filter measurement, round-trip, render | DESIGNED |
| R3 sequencer (steps, velocity, swing, microtiming, probability, ratchets, polymeter, Euclid) | new `music-core/sequencer.rs`, engine `StepPatternData`, `StepGrid.tsx` | pattern tests odd meters/polymeter | NOT STARTED |
| R4 Sample Lab capture → preset | existing `SampleLab.tsx` + sampler source | integration | NOT STARTED |
| R5 FM/granular/resonator/mod matrix | engine nodes (`grain.rs` exists) | per-node tests | NOT STARTED (Cloud exists as a layer, not as an instrument node) |
| R6 generative rhythm with locks/explanations | generators + ranking | distinctness tests | NOT STARTED (partial: seeds, locks, regenerate) |
| R7 scenes, automation, stems, MIDI, packaging | export service | alignment tests | NOT STARTED (MIDI parts exist) |
| R8 polish, academy, installer | — | regression suite | NOT STARTED |

## C. Program P2 — Music DNA / Composer / Director (spec §11)

| Req | Target component | Tests | Status |
|---|---|---|---|
| A audit | `docs/music-dna/*` | — | IMPLEMENTING (docs) |
| B analyzer foundation (import, tempo candidates with ½/2×, features, novelty/recurrence, editable map) | `crates/analysis` (new, Rust FFT — ADR needed), `DnaAnalyzer.tsx` | synthetic 174/87 fixture, boundary tolerance fixtures | NOT STARTED |
| C blueprint schema + song form timeline (beats, tempo map) | `crates/project/src/songmap.rs` (new), `SongMap.tsx` | migration, round-trip | NOT STARTED |
| D first full-song arrangement | `crates/music-core/arranger/` (new): form planner, orchestrator, groove evolution, motif engine, transitions; renderer on the graph | distinctness, locks, determinism, 2–4 min render | NOT STARTED |
| E Musical Director | `director.rs` heuristics + panel | fixture critiques | NOT STARTED |
| F DAW export (aligned stems, MIDI, markers) | export service | alignment tests | NOT STARTED (ambient stems exist) |

## D. Program P3 — Live Performance Director (spec §11)

| Req | Target component | Tests | Status |
|---|---|---|---|
| 0 audit + blueprint | `docs/performance-director/*` | — | IMPLEMENTING (docs) |
| 1 interactive song map slice (6 sections, zoom, playhead in bars, persistence) | `songmap.rs` (shared with P2 C), `SongMapView.tsx`, beat transport status | seek/loop/tempo tests | NOT STARTED |
| 2 cue scheduler (lookahead, latency-compensated countdown) | engine `CueQueue` on `BeatClock`, shell events | scheduling accuracy, seek invalidation | NOT STARTED |
| 3 cue planner v1 | `music-core/cues.rs` | plans distinct, constraints, seeds | NOT STARTED |
| 4 guided recording & take review | recorder + `intent.rs` alignment | synthetic + mic | NOT STARTED (recording and analysis exist) |
| 5 beginner coach & notation | UI | accessibility | NOT STARTED |
| 6 adaptive assistant | `tracker.rs` live | wrong-note cases | NOT STARTED (offline tracker exists) |
| 7 DAW export & controllers | export, MIDI in | interop | NOT STARTED |
