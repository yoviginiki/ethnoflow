# Rhythm Laboratory & Instrument Forge — implementation plan R0–R2 (proposal, 2026-10-18)

Status: **waiting for the owner's approval** (spec §14 "Stop for approval"). No application
code for this program has been written. Companion: `AUDIT.md`,
`docs/audits/CURRENT_STATE.md`, `docs/traceability/FEATURE_MATRIX.md`.

**Резюме за собственика.** Планът прави от четирите твърдо вградени части *инструменти-
пресети* с регистър и библиотека (R1), после дава на един от тях истинска страница за
звука с обвивки, филтър, ефекти, прослушване и „Запази като“ (R2, доказателство на
концепцията), без да губи нищо работещо. Раздел 1 отговаря на въпроса ти „защо всичко
звучи еднакво“ и казва коя мярка в кой етап го решава.

## 1. Why everything sounds the same — and which step changes it

The sameness has five concrete causes in the code; none is a mystery and each has a fix
with a measurable acceptance.

| # | Cause (file) | Effect you hear | Fix | Step | Acceptance |
|---|---|---|---|---|---|
| 1 | **One timbre model per part**, 3 knobs on drums, 1 filter on bass, 3 fixed timbres on ostinato/arp; envelopes are internal constants (`rhythm.rs`, `bass.rs`, `patsynth.rs`) | every project's kick, snare, bass are the same instrument | instruments become **presets with their own parameters**: source (osc/noise/sample), amp + pitch envelopes, filter with envelope, drive/saturation, transient, room send; 10–15 curated, genuinely different factory presets (kicks from soft wooden to hard electronic; snares; three bass families) | R1 (registry, presets), R2 (designer exposes them) | two factory kicks differ in measured spectral centroid, decay time and transient energy by stated minimums; a listener in a blind A/B tells them apart |
| 2 | **Every hit identical** (no round-robin, no per-hit variation) | "machine-gun" drums, lifeless | **Microvariation** per hit: deterministic per-hit offsets of pitch, decay, brightness and velocity→timbre mapping, seeded (spec §7 "Microvariation Engine", pulled forward) | R2 | 16 consecutive kicks are not bit-identical; variance within stated bounds; still deterministic per seed |
| 3 | **No organic material**: drums cannot play samples; own recordings reach only the atmosphere (loop/cloud) | nothing sounds like *your* room, wood, skin, breath | **Sampler source** in the instrument graph + Sample Lab capture → instrument (spec §5 B); own tapan/darbuka/wood/breath hits become presets | R2 (sampler node, one-shot) and R4 (capture workflow) | record a hit → preset → plays in the pattern → reopened project still plays it |
| 4 | **Same groove rules** for all songs: 3 styles with fixed anchors, one loop forever, fills every N bars | all songs have the same feel and the same arc (none) | R3 sequencer with per-step data, variations A/B/C/D, Euclid, polymeter; R6 generators returning *different* candidates with explanations; song-level development is **Program P2** (Composer) | R3, R6, P2-D | 4 generated candidates differ in ≥ 2 of: anchors, density, syncopation, fill placement; P2: sections differ structurally |
| 5 | **Same mix**: one reverb for everything, no per-instrument effects or sends, no stereo treatment | every song has the same frequency balance and space | per-instrument FX chain (saturation, transient, EQ, delay/reverb sends) in the designer; kit mixer | R2 (one effect + sends), R3 (kit mixer), R5 (full chain) | stems of two kits show different spectral balance; A/B audible |

So: **the plan contains the resolution, but only if R2 includes items 1–3 and 5 together
for one instrument** (not a page with the existing three knobs). That is how the PoC is
scoped below. Item 4's song-level half is deliberately in Program P2; the two programs
meet at the sequencer's "section-aware pattern variants".

A note on "authentic": synthesis will never make a *real* tapan; the honest path is the
owner's own recordings (item 3) layered with synthesis (item 1), and ethno meters/accents
(already in `groove.rs` "ethno" style). We will not label synthesized approximations as
authentic instruments (spec §3).

## 2. Target architecture (adapting the existing stack, no new stack)

### 2.1 Engine: instrument graph inside one slot

> **Deviation recorded 2026-10-18 (R3 as built):** instead of a nested `InstrumentRack`
> source inside slot 12, the flat graph was widened to 24 slots and slots 16..23 are the
> rack's extra parts (one per instrument, each with the existing per-slot strip, bus and
> stem). Same guarantees (lock-free params, pattern swaps, offline stems), far less risk
> for the four legacy parts; a nested rack remains possible if more than 8 extra parts
> are ever needed. The sequencer/kit-mixer items of R3 are still open (see the matrix).

Keep the 16-slot flat graph; add an **`InstrumentRack` source** that owns N instruments
(voices + per-instrument strips + sends) and renders them into one slot (slot 12 for the
rhythm; the drone/pad/atmosphere stay where they are). This removes the 4-part ceiling
without touching every slot consumer, and gives per-instrument buses.

```
InstrumentRack (slot 12)              per instrument (preset-driven, lock-free params)
  events: PatternInbox<StepEvent>  →  VoiceAllocator (poly/mono, choke groups)
  clock: BeatClock (shared)             └ Voice = Source(osc | noise | sampler) → PitchEnv
  mix: strips (gain/pan/LP/HP/sends)         → AmpEnv → Filter(SVF, env amount) → Drive
  returns: delay, room (shared, cheap)        → Transient → strip → rack mix (+ sends)
```

- Node classes for R2 (minimum): `Oscillator` (sine/tri/saw/pulse, band-limited polyBLEP
  — exists in `pad.rs`), `Noise` (exists), `Sampler` (one-shot from an immutable take
  buffer; reuse `take_buffer` cache), `AdsrEnv` + `PitchEnv` (new, 2-stage), `Svf` (exists
  in `gen.rs`), `Drive` (`soft` exists), `TransientShaper` (new, simple), sends to the
  existing master reverb + a new tempo-synced `Delay` return (R3).
- Realtime rules: all parameters are `SmoothedParam`/atomics; patterns and preset swaps
  arrive via `ringbuf` like today (`PatternSwap`); sample buffers are `Arc<AudioBuffer>`
  loaded off-thread; no allocation in `render` (voices pre-allocated, bounded polyphony,
  voice stealing oldest-quietest).
- Offline render: the same `InstrumentRack` in `Graph::render_offline`/`render_with_stems`
  → rhythm WAV and per-instrument stems (R0 deliverable for the current four parts, R2 for
  the rack).

Existing `DrumSource`/`BassSource`/`PatternSynthSource` stay as the **legacy engines**
behind four factory presets until R3 moves their patterns to the step sequencer; a preset
can reference either the legacy voice or the new node graph (field `engine: "legacy_bass" |
"graph"`), so nothing regresses while the designer grows.

### 2.2 Data model (project crate)

`crates/project/src/instrument.rs` (new), versioned JSON, atomic writes, backups:

```json
{ "schema_version": 1, "id": "inst_…", "name": "Дървен кик", "category": "drum.kick",
  "tags": ["organic","liquid"], "author": "user", "parent_id": null, "engine_version": "0.5.0",
  "engine": "graph",
  "layers": [{ "source": {"kind":"sampler","asset":"blake3:…","start":0,"reverse":false},
               "gain_db": 0, "pan": 0, "pitch_semitones": 0, "vel_range": [0,127] },
             { "source": {"kind":"osc","wave":"sine"}, "pitch_env": {"start_st": 24, "decay_ms": 60},
               "gain_db": -6 }],
  "amp_env": { "attack_ms": 1, "decay_ms": 180, "sustain": 0, "release_ms": 40, "velocity": 0.6 },
  "filter": { "type": "lp", "cutoff_hz": 8000, "q": 0.7, "env_amount": 0, "key_track": 0 },
  "fx": [{ "kind": "drive", "amount": 0.2 }, { "kind": "transient", "attack": 0.3, "sustain": 0 }],
  "sends": { "room": 0.1, "delay": 0 }, "macros": [{ "id": "punch", "value": 0.7, "map": [...] }],
  "microvariation": { "pitch_cents": 8, "decay": 0.1, "brightness": 0.1, "seed": 1 },
  "voice": { "mode": "poly", "max": 4, "choke_group": "hats", "glide_ms": 0 },
  "created_at": "…", "updated_at": "…", "license": { "assets": ["blake3:…"] } }
```

- Stored in the library (`library_cmds.rs`) as kind `instrument` (payload = this document)
  **and** referenced from the project's rhythm settings by id; factory presets are
  read-only files in the app (`crates/project/src/instrument_presets.rs`), "Edit copy"
  forks with `parent_id`.
- Migration: `RhythmPrefs.drums/bass/ostinato/arp` keep working; on first run after R1 the
  shell creates four user presets from them ("Барабани (моите)", …) and the rhythm
  settings gain `instrument_ids`; old sessions without ids resolve to the legacy presets.
  Rollback: the new fields are additive; old builds ignore them.
- Kits (R3): `{ id, name, instruments: [{ instrument_id, track }] }`.

### 2.3 UI (designer page, Bulgarian first)

Route/screen `instrument` with the instrument id in the store (no router today; the
breadcrumb shows "Инструменти › <име>"). Layout follows the contract's blueprint, scaled
to what R2 implements (unimplemented tabs are not shown, not disabled-and-faked):

- Header: name (editable), category, tags, ★, A/B, undo/redo, **Save As**, "Update preset",
  dirty marker, help.
- Left: instrument browser (library kind `instrument`: search, tags, factory/user, recent).
- Center tabs in R2: **Overview** (macros 6, audition pad/keyboard, meters), **Sources**
  (layers list: osc/noise/sampler with waveform), **Shape** (amp ADSR + pitch env with
  graph and numbers), **Filter & FX** (SVF + env amount, drive, transient, sends), **Voice**
  (poly/mono, choke, glide, microvariation). Modulation/Layers-zones/Advanced come in R5.
- Bottom: audition transport (plays the instrument alone or inside the current pattern
  "Adaptive Audition"), peak/RMS, render button (WAV of 4 bars with the current pattern).

Every control: id (catalog), range/unit/default, double-click reset, tooltip explaining
the acoustic effect (BG), persistence, test id. The catalog is produced before coding R2
(`docs/design/instrument-forge/CONTROL_CATALOG.csv`, contract §2).

## 3. Phases, deliverables, acceptance

### R0 — audit and baseline (this gate)
Deliverables: `AUDIT.md`, this plan, `CURRENT_STATE.md`, `FEATURE_MATRIX.md`, real
screenshots. Still to add inside R0 (small, no architecture change): **`rhythm_render`**
command = offline WAV + 4 stems of the current rhythm (same sources, same clock) so that
every later step has before/after audio; a benchmark line for rhythm CPU in
`BENCHMARKS.md`. Acceptance: WAV of the current four parts renders deterministically (same
seed → identical samples), listed in `qa/renders/`.

### R1 — registry, presets, browser (keep the four sounds intact)
1. `instrument.rs` schema v1 + validation + migration + tests (round-trip, invalid, old
   session without ids).
2. Four legacy presets generated from current defaults; library kind `instrument`;
   browser UI (search/tags/factory/user/favorites), "Edit copy", "Update user preset".
3. Rhythm settings reference instrument ids; `rhythm_cmds` resolves presets → existing
   engines (no sound change yet).
4. `InstrumentRack` skeleton in the engine with the legacy voices wrapped, so > 4 parts
   can exist (acceptance: 6 instruments playing, save/reopen, no regressions).
Risks: `rhythm_cmds.rs` refactor; mitigated by keeping `RhythmSettings` shape and adding
ids alongside. Files: `crates/project/src/instrument.rs`, `instrument_presets.rs`,
`apps/desktop/src-tauri/src/instrument_cmds.rs` (exists for *profiles*; new module
`forge_cmds.rs`), `apps/desktop/src/InstrumentBrowser.tsx`, `RhythmPanel.tsx`.

### R2 — Instrument Designer MVP (proof of concept)
PoC instrument: **the bass** (richest real DSP today: voices, SVF, drive, glide, wobble,
sidechain), with the drum kick as the second instrument if time allows. Steps:
1. Engine: `AdsrEnv`, `PitchEnv`, filter env amount, `TransientShaper`, one-shot `Sampler`
   layer, per-instrument strip + room send, microvariation; tests per node (finite,
   bounded, measured attack/decay times, cutoff sweep changes centroid).
2. Audition path: a `NoteInbox` (ringbuf) into the rack so the pad/keyboard triggers notes
   sample-accurately without a pattern; MIDI input reuse (`midir` exists).
3. Designer screen with the tabs above; realtime edits through atomics (no rebuild);
   Save As / Update / A/B / undo-redo (zustand history).
4. Use in the rhythm section: the bass track plays the new preset; save/reopen; offline
   WAV render (4 bars) + stems; before/after renders of two clearly different presets.
5. Docs: guide paragraph, control catalog, acceptance file with PASS/FAIL table,
   screenshots at 1920×1080 and 1366×768, listening checklist for the owner.
Acceptance (spec §14 R2 + §16.4): two clearly different sounds created, saved, used in a
pattern, survive reopen, rendered to WAV; every shown control measured; no clicks on
changes; polyphony bounded; CPU recorded.

### After R2 (for the record; each needs its own approval)
R3 step sequencer + kit mixer + pattern variants; R4 Sample Lab capture → instrument;
R5 FM/granular (Cloud node exists as a layer)/resonator/mod matrix/morph/round-robin;
R6 generative candidates with locks and explanations; R7 scenes/automation/stems/MIDI/
packaging; R8 polish. P2 (Composer) consumes R3's section-aware variants.

## 4. Dependency graph

```
R0 render ──► R1 schema ──► R1 registry/browser ──► R2 nodes ──► R2 designer ──► R2 PoC
                │                 │                       │
                └── migration     └── InstrumentRack ─────┴──► R3 sequencer ──► R6 generators
                                                                      │
                                                     P2 Composer ◄────┘ (section variants)
```

## 5. Risks

| Risk | Mitigation |
|---|---|
| Refactoring `rhythm_cmds.rs` breaks the live swap/no-chop behaviour | keep `PatternSwap` path untouched; add tests that run `sync` 50× while rendering offline and assert no discontinuity |
| CPU with many voices on the owner's laptop | bounded polyphony per preset, Eco/Balanced modes later; measure in R0 |
| Preset schema churn | `schema_version` + migration tests from day one; payloads validated in the shell |
| Scope creep in the designer (contract §0.3) | tabs not implemented are absent; the control catalog is the contract for R2 |
| Dependencies | none planned for R1/R2 (polyBLEP, SVF, ringbuf exist). FFT only for P2 (ADR). |

## 6. Decisions requested from the owner

1. Approve R1 → R2 as scoped (bass PoC first; kick second).
2. Confirm synthesis-first with sampler layers from *your* recordings (no third-party
   sample packs until licenses are verified).
3. Agree that the song-level sameness (arcs, sections) is solved in Program P2 and not
   duplicated here.
