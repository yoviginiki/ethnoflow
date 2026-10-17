# Rhythm section audit (Phase R0) — 2026-10-18

Spec: `docs/ETHNOFLOW_RHYTHM_INSTRUMENT_FORGE_MASTER_SPEC.md` §0.2, §16.1. Shared inventory:
`docs/audits/CURRENT_STATE.md` (read it first; this file only goes deeper on the rhythm).

**Резюме за собственика.** Ритъм секцията е добра *машина за цикли*: четири синтезирани
инструмента на общ часовник, детерминистични по seed, с плавна смяна на патерни и MIDI
експорт. Не е *студио за инструменти*: инструментите не са пресети със собствен живот,
няма страница за звука им, няма стъпков секвенсер, няма повече от четири части, няма
WAV/стемове от ритъма. Нищо от работещото не трябва да се губи: планът (следващият файл)
ги превръща в първите четири пресета на новия регистър.

## 1. Component map (what the five instruments really are)

```
RhythmPanel.tsx / RhythmScreen.tsx (UI, prefs.rhythm)
        │ debounced 120 ms  invoke start_rhythm / set_rhythm / regenerate / rhythm_to_takes
        ▼
rhythm_cmds.rs (shell)            music-core generators (pure, seeded)
  timing() ─ one BPM + Meter      groove.rs      → DrumPattern (hits in beats)
  chord_pcs() ─ root/pcs from     bassline.rs    → BassLine over static_spans()
      armed harmony or scale      ostinato.rs    → Motif steps / arpeggio
  keys() ─ rebuild triggers       counter.rs     → countermelody (offline → MIDI take)
  sync() ─ pattern_channel swap   tempo.rs       → tempo candidates (notes-based)
  apply_live() ─ atomics
        ▼ GraphCmd::SetSource once per part; PatternSwap afterwards
audio-engine graph slots 12..15
  DrumSource (kick/snare/ghost/hats/shaker synth, kick env → bass sidechain)
  BassSource (sub | pluck | Reese; SVF cutoff/res, drive, glide, wobble, warmth)
  PatternSynthSource ×2 (8 voices, timbre 0..2, release)
  BeatClock shared (anchor frame/beat/bpm, generation), Cursor per source
```

Routing: each part is one graph slot with gain/pan/LP/HP; the background mixer bus
"rhythm" adds trim/pan/cuts/fade on top (`bus_cmds.rs`). Master: reverb send only from
the atmosphere macros (`master.space`); the rhythm has no send. Output: the one cpal
stream of the graph (`GraphPlayer`), 48 kHz by default (`OFFLINE_RATE`), stereo.

Playback timing: patterns are beat-indexed; `Cursor::seek` on swap keeps the beat;
humanize is applied at generation time (`groove.rs` jitter up to ±12 ms at 170 BPM).
Tempo change re-anchors the clock (`on_timing_changed`) without stopping.

Sample format: none — all four parts are synthesized; there is no sampler node in the
graph (the atmosphere's `LoopSource`/`GrainSource` read take buffers but are not note
triggered).

Preset persistence: `prefs.rhythm` (UI object, `RhythmPrefs`) in localStorage and per
project in `session.json`; whole-object presets in the library (kind `rhythm`). No
per-instrument preset, no id, no version, no author, no lineage.

## 2. Measured behaviour (reproduced from code and tests)

| Aspect | Fact | Where |
|---|---|---|
| Determinism | same seed → same pattern (test) | `groove.rs` tests, `rhythm.rs` tests |
| Loop length | `bars` 1..64, UI default 4; identical every cycle | `GrooveParams.bars`, `Cursor` |
| Variation over time | fill every N bars (snare roll last beat); ostinato variation every N bars; nothing else | `groove.rs::fill`, `ostinato.rs` |
| Odd meters | `Meter` groupings (7/8 = 2+2+3…) honoured by "ethno" style | `pattern.rs`, `groove.rs` |
| Swing | sixteenth swing | `swing_offset` |
| Chord following | bass/ostinato follow the pad chord or scale root; one span per pattern | `static_spans`, `chord_pcs` |
| Parameter changes while playing | no chop (lock-free swap + 120 ms debounce) | fixed 2026-10-17 |
| Levels | −40 dB floor = off; bus fades | `apply_live`, `bus_cmds.rs` |
| Export | MIDI takes only (drums GM ch. 10); no WAV | `rhythm_to_takes` |
| CPU | not measured for the rhythm alone (**unknown**) | — |
| Latency | output latency not measured (**unknown**) | — |

## 3. Technical debt relevant to the program

1. `rhythm_cmds.rs` is 1117 lines mixing generation, graph wiring, MIDI export and
   validation; the Instrument Designer needs a seam between *instrument sound* and
   *pattern*. Today they are coupled per part (e.g. `DrumParams` and `GrooveParams` live
   in one `RhythmSettings`).
2. Four hard-coded slots (12..15) and `MAX_LAYERS = 16`: more instruments need either
   more slots (cheap: raise the constant; the graph is a flat mixer) or a sub-mixer source
   that hosts N instruments in one slot (keeps slot count, adds a voice mixer; preferred
   for R1+ because it also gives per-instrument buses/sends).
3. `PatternSynthSource` is a generic note synth with 3 timbres; `BassSource` is the only
   part with a real filter; `DrumSource` exposes three knobs. None has user-facing
   envelopes; amplitude shapes are internal constants.
4. No offline render for the rhythm: tests of "before/after" sound and the spec's WAV
   acceptance need `render_offline` wiring with the same sources and clock.
5. Markers are not a data model for sections (seconds, no tempo map) — shared with P2/P3.
6. UI strings exist for everything, but tooltips rarely explain the *acoustic* effect
   (contract §3).

## 4. What must be preserved (regression list for R1/R2)

- Start/stop with the background transport, fades, locks, regenerate, seeds, MIDI export,
  tempo re-anchoring, chord following, odd meters, the two-step/halftime/ethno styles, the
  Reese wobble + sidechain, the ostinato motif lock, per-project session recall, the
  rhythm presets already saved in the library (kind `rhythm` must still load).
- Tests that must keep passing: `cargo test -p ethnoflow-music-core`, engine
  `rhythm::tests`, `bass::tests`, `patsynth::tests`, shell `rhythm_cmds` validation tests,
  vitest `starters.test.ts` (uses `DEFAULT_RHYTHM`).

## 5. Unknowns to resolve in R0 (before approval is final)

- CPU per instrument and headroom for > 8 instruments on the owner's laptop (measure with a
  release build and the offline renderer; `BENCHMARKS.md` has no rhythm numbers).
- Whether the owner wants sample-based drums in the first PoC or synthesis only (the
  spec allows both; synthesis has no licensing cost).
- Which instrument is the PoC: proposal in the plan (bass), alternative (kick).
