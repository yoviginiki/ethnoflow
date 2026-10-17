# Live Performance Director — Phase 0 architecture audit (2026-10-18)

Spec: `docs/ETHNOFLOW_LIVE_PERFORMANCE_DIRECTOR_SPEC.md` §14. Shared inventory:
`docs/audits/CURRENT_STATE.md`. Status: **Phase 0, no code; waiting for approval.**

**Резюме за собственика.** За „диригента на живо“ имаме здрави парчета: запис на
микрофона с непокътнат оригинал, анализ на височина/фрази/намерение с толерантност към
грешки, общ бийт-часовник в енджина, метроном с предброяване, маркери за секции. Липсват
точно нещата, които правят картата жива: модел на песента в тактове (общ с Композитора),
позиция на транспорта в тактове/удари с компенсирана латентност, опашка за реплики
(cues) по аудио часовника, модел на репликите и екранът „Сега / Следва / После“.

## 1. What exists and is reusable (with files)

| Need (spec) | Existing | File | Reuse |
|---|---|---|---|
| Transport clock for cues (§5) | `BeatClock` (anchor frame/beat/bpm) drives drums/bass; graph `frame` counter | `graph/sources/rhythm.rs`, `graph/mod.rs` | yes: cue positions in beats against the same anchor |
| Transport (play/stop/seek/loop) | arrangement transport in **seconds**; rhythm start/stop separately; metronome with count-in in the recorder | `arrangement_cmds.rs`, `rhythm_cmds.rs`, `metronome.rs`, `commands.rs` | partly: needs one beat-based position for the whole song |
| Status to UI | polling 150 ms (`ambient_status`, `rhythm_status` with bar/beat) | `layers.ts`, `RhythmPanel.tsx` | for display only; cue *triggering* must not rely on UI timers (spec §5) |
| Output latency compensation | input-latency calibration screen exists; **no measured output latency** | `Calibrate.tsx`, `commands.rs` | add output latency estimate (cpal buffer + device report) |
| Song sections | markers in seconds (`Intro/A/B/Build/Drop/Break/Outro`) | `project.json`, `Arrange.tsx` | migrate to `SongMap` (ticks) shared with P2 |
| Recording with raw preservation | `Recorder` streams to disk; originals immutable; count-in/click | `recorder.rs`, `take_writer.rs` | yes |
| Pitch/phrase/intent analysis | YIN, contour → notes, phrases, tonal tracker with hysteresis, intent classes, Stable/Assisted/Creative | `pitch/`, `phrase.rs`, `tracker.rs`, `intent.rs` | yes for take review (offline); live path exists only for the tuner (`InputMonitor`) |
| Guide tone / notation | MIDI player with falling notes and piano roll; live synth voices | `FallingNotes.tsx`, `PianoRoll.tsx`, `live_synth.rs` | yes for "listen then play" and guide melody |
| Trainer | evaluate vs notes (Listen/Wait/Rhythm) | `trainer.rs`, `TrainerPanel.tsx` | partial: cue-based exercises differ (actions, rests) |
| Harmony that does not chase notes | tracker hysteresis + locks | `tracker.rs`, `intent_cmds.rs` | yes (Phase 6) |
| Export | MIDI, ambient stems, arrangement mix (clips only), markers in project | various | needs cue sheet, aligned stems (P2 F) |
| Assistant bridge | state/commands folder | `assistant.ts` | cue plans could be requested from Claude Code later |

## 2. Gaps (missing APIs and models)

1. **SongMap in ticks with tempo/meter maps** — missing (shared with P2 C).
2. **Song transport**: one position (tick) for clips + rhythm + atmosphere program; today
   three clocks (seconds transport, beat clock, program lane time).
3. **Cue queue in the engine**: lookahead scheduling of `CueEvent{tick, id}` against the
   beat clock; the engine marks "fired at frame F"; the shell relays with the measured
   output latency; the UI never computes musical time from `Date.now()`.
4. **Cue data model and plans** (`Cue`, `CuePlan`, `Take` annotations) — missing.
5. **Cue planner** (deterministic, constraint-based, seeded) — missing; inputs partly
   exist (phrases of the owner's take, section roles from P2, instrument range from the
   instrument profile `instrument_cmds.rs`).
6. **Coach UI** (NOW/NEXT/LATER, countdown, practice loop by bars) — missing.
7. **Live activity detection** (is the player playing / resting) — `InputMonitor` gives
   pitch + peak live; phrase boundaries live are not implemented (F8.1).
8. **Monitor bus for speech/click cues** separate from the export mix — missing; today
   the metronome is a separate stream (`metronome.rs`), which is the right precedent.
9. **Footswitch/MIDI controller mapping** — `midir` input exists for notes; no mapping
   layer.

## 3. Architecture proposal (adapting the stack)

```
song_map_core      crates/project/src/songmap.rs  (ticks, tempo map, sections, cues, plans; migrations)
cue_planner        crates/music-core/src/cues.rs  (pure, seeded; policies per section role; scoring)
cue_scheduler      crates/audio-engine/src/graph/cues.rs (CueQueue on BeatClock; lookahead; seek invalidation)
song_transport     shell: one TransportState{tick, playing, loop} driving clips (seconds derived),
                   rhythm (beat anchor) and atmosphere program; status pushed as Tauri events ≤ 30 Hz
performance_capture existing recorder + calibration (+ output latency estimate)
performance_analyzer existing pitch/phrase/intent offline; live activity from InputMonitor
coach_ui           apps/desktop/src/Director.tsx (+ SongMapView shared with P2)
export_service     cue sheet JSON/HTML, markers CSV/MIDI, aligned stems (P2 F)
```

Threading: cue firing decided in the audio thread by frame comparison (no allocation:
fixed-capacity queue filled from the shell through `ringbuf`), published as atomics
(`last_fired_cue`, `frame`); the shell polls at 60 Hz or receives from the stream thread
and emits a Tauri event with `latency_frames` subtracted. UI shows countdown from the
last event + its own interpolation only for *display*, re-synced every event.

## 4. Risks and unknowns

- Output latency on WASAPI shared mode is reported by cpal only approximately; measure
  with a loopback (play click → record) in the calibration screen (owner's machine).
- One song transport means refactoring three existing clocks; do it as the first slice
  (Phase 1) with regression tests on clips, rhythm and program.
- Speech cues (TTS) need a dependency or Windows SAPI; defer; visual + soft click first.
