# Music DNA · Arrangement Composer · Musical Director — Phase A (audit, architecture, blueprints, plan)

Spec: `docs/ETHNOFLOW_MUSIC_DNA_ARRANGEMENT_COMPOSER_MASTER_SPEC.md`. Shared inventory:
`docs/audits/CURRENT_STATE.md`. Status: **Phase A only, no code; waiting for approval.**
The owner asked for eight deliverables; they are the eight sections below.

**Резюме за собственика.** Днес EthnoFlow генерира цикъл от N такта и го повтаря; няма
понятие за песен (секции в тактове, енергия, влизания и излизания, преходи), няма анализ
на чужди песни (няма дори FFT в енджина), няма диригент. Предлагам: (1) Rust модул за
анализ на референтни песни (темпо кандидати с ½/2×, бийтове, кривa на новост, повторения,
граници с увереност; етикетите са хипотези, не истина), (2) песенна карта в тактове с
темпо карта като общ модел за всички програми, (3) композитор на три нива (форма →
секция → фраза → събития), който ползва днешните генератори като „речник“, но решава
кой свири кога и какво се променя, (4) диригент с обясними диагностики по тактове. Без
Python на машината на потребителя: анализът е на Rust.

## 1. Analysis of the existing limitations (why the DnB is cyclic)

Verified in code (`CURRENT_STATE.md` §3 has the file references):

1. **Loop, not song.** Generators emit `bars` bars (default 4) and sources loop them;
   humanize is frozen at generation → identical cycles.
2. **No time structure.** The only scheduled changes: a fill every N bars, ostinato
   variation every N bars. No sections, energy targets, entries/exits, breakdowns,
   transitions, automation on rhythm/bass (automation exists only in the atmosphere).
3. **Static harmony in the rhythm.** Bass/ostinato read one chord span per pattern
   (`static_spans`); harmony proposals are not placed on a timeline.
4. **Sections are decoration.** Markers (`Intro/A/B/Build/Drop/Break/Outro`) live in
   seconds in `project.json`; no generator, transport or mixer reads them.
5. **One timebase short.** One BPM + one meter per project; the arrangement runs in
   seconds, the rhythm in beats; no tempo map, no bar grid shared by all.
6. **No analysis of references.** Decoding exists (symphonia), but there are no spectral
   features, beat tracker, novelty or self-similarity — no FFT crate at all.
7. **Export omits the background.** `export_arrangement_mix` renders only the clips; the
   rhythm has no offline render; stems exist only for the atmosphere.
8. **The timbre side** of monotony is Program P1 (same instruments everywhere); both
   programs are needed for an audible change.

Real evidence attached: screenshots `docs/audits/screenshots/2026-10-18-rhythm.png`,
`…-studio.png`, `…-compose.png`. Audio: a baseline rhythm render will be produced by the
R0 `rhythm_render` command (P1) and kept in `qa/renders/baseline-loop.wav`; today the only
deterministic renders are the atmosphere's.

## 2. Architecture — Music DNA Analyzer

Language decision: **Rust, inside the workspace** (`crates/analysis`, new), not Python.
Reasons: CLAUDE.md rule "no toolchain on end-user machines", no Python on the owner's
machine, the engine already decodes files; librosa pitfalls listed in the spec §3.4 are
avoided by owning the pipeline. Python stays optional for *offline experimentation* on the
developer side (not shipped). Dependency to approve: an FFT crate (`realfft`/`rustfft`,
MIT/Apache) → ADR-0021.

Pipeline (all offline, worker thread, progress + cancel, cached by content hash +
analyzer version in `<project>/analysis/ref/<hash>/`):

```
ReferenceIngest  decode (symphonia) → mono analysis copy @ 22.05/44.1 kHz, original untouched
FeatureExtraction  STFT (2048/512) → RMS, spectral centroid/rolloff/flux, low-band energy,
                   onset strength, chroma (12), MFCC-like timbre (20 bands log-mel → DCT)
BeatGrid  tempogram/autocorrelation of onset strength → tempo candidates with confidence,
          explicit half/double listing; beat tracking by dynamic programming; downbeats
          "unknown" unless a meter hypothesis is confident; meter estimate or unknown
Structure  novelty curve (checkerboard kernel on a self-similarity matrix of timbre+chroma),
           repeated-region search (diagonal stripes), boundary proposals at fine/medium/
           coarse resolution, snapped to beats only when grid confidence allows
SectionHypothesis  functional role proposals from features (energy level/slope, low-band
                   entry, density, novelty) with confidence and evidence strings; never
                   from position alone; "unknown" allowed; user labels stored separately
Events  kick/bass entry, thinning, HF sweep, gap/silence, loudness lift, harmonic reset,
        motif recurrence — each with time, confidence, evidence
BlueprintService  abstract form: sections (role, bars, energy range, density, active roles,
                  transition out), tempo/meter with confidence, change frequency — no audio,
                  no melody
```

Contracts: typed Rust structs serialized to JSON for the UI (versioned `schema_version`);
Tauri commands `ref_import`, `ref_analyze` (async, progress events), `ref_get_map`,
`ref_set_label/boundary` (user edits separate from computed), `ref_extract_blueprint`.
Test fixtures: synthetic 174 BPM beat (87/174 surfaced), gradual build, abrupt drop,
silence intro, odd meter, repeated drop; at least 10 human-labeled tracks later (owner's
own licensed music) with precision/recall reported, not claimed.

## 3. Architecture — Intelligent Arrangement Composer

Shared model first: **SongMap** (`crates/project/src/songmap.rs`): tempo map (segments
bpm/start tick), meter map, sections `{id, role, start_tick, end_tick, energy, density,
tension, roles, transition_in/out, locked}`, PPQ ticks as the canonical timebase;
seconds derived. This is also P3's model and replaces seconds-only markers (migration:
markers → sections at the project's single tempo).

Composer (`crates/music-core/src/arranger/`, pure, seeded):

1. **SongFormPlanner**: from a blueprint, a style template or a wizard → ordered sections
   with bar counts (defaults 8/16/32, any allowed incl. asymmetric and Bulgarian meters),
   target energy/density/tension curves, anchors to keep across sections.
2. **SectionOrchestrator**: per section which roles are active (kaval, drone, pad, chords,
   bass, kick, snare, hats, perc, fx, ambience, countermelody) and their variation budget;
   kaval priority reserves space (density caps, register avoidance) using existing phrase
   analysis of the owner's take.
3. **GrooveEvolutionEngine**: reuses today's generators as a vocabulary but drives their
   parameters per section (style family, density, ghost rate, fill policy with cooldown,
   half-time breakdowns), produces *variants* per phrase (micro/meso/macro) instead of one
   loop; P1's step sequencer becomes the representation when it exists (R3); until then,
   per-section `RhythmSettings` + event lists.
4. **MotifDevelopmentEngine**: takes the analysed kaval motif (or a recorded one) and
   produces transformations (displacement, augmentation, register, omission, call/
   response, mode-respecting) for countermelody/arp/ostinato, tracking motif identity.
5. **Harmony over time**: `harmony.rs` proposals placed per section with tension targets;
   bass strategies (sub, moving, Reese, call/response, dropout).
6. **TransitionEngine**: bar-accurate events — fill, bass cut, silence window, filter sweep
   (automation lanes on the buses/instruments), reverse swell (from the atmosphere),
   riser (synth), tape stop (rate ramp on a loop layer) — each with intensity, length,
   affected tracks; crossfades/zero-crossing safe.
7. **ArrangementRenderer**: converts the plan into real project objects: rhythm
   settings per section + pattern swaps scheduled on the `BeatClock` (lookahead queue in
   the shell, atomics in the engine), automation on buses, clips for kaval phrases,
   atmosphere scenes (the existing `program.rs` is the right place for ambience); preview
   live and bounce offline with stems through the same graph.

Candidates: seeded constrained search (beam of form plans × orchestration choices) scored
on structure consistency, novelty budget, motif continuity, role compatibility, harmonic
plausibility, kaval space and user weights; 3–8 candidates that differ in form, not only
notes; locks on sections/tracks/motifs; regenerate scope only.

## 4. Architecture — Musical Director

`crates/music-core/src/director.rs` (pure) + panel:
- Inputs: the arrangement plan + rendered proxies (per-track event density per bar,
  automation shapes, instrument roster per bar; later RMS/centroid from offline stems).
- Heuristics with exact bar ranges and evidence: repetition burden (identical 4/8-bar
  windows in rhythm/harmony/roster, tolerant for intentional ambient repetition), energy
  shape vs target, contrast between adjacent sections (roster, density, register, harmony,
  dynamics), motif continuity, space for lead (competing midrange when kaval active),
  transition quality (continuity + intended contrast), masking risk (heuristic).
- Output: findings `{bars, severity, explanation_bg, evidence, suggestions[]}`; each
  suggestion is a scoped regeneration or edit that can be auditioned, applied, rejected,
  pinned or muted per project. No single "quality %". All thresholds visible and editable.

## 5. UI blueprints (desktop 1920×1080, usable at 1366×768; BG first)

### 5.1 Music DNA Analyzer (screen "Анализ на песен")
- Top bar (56 px): file name, duration, sample rate/channels; transport (play/stop/loop,
  scrub); tempo candidates as chips (e.g. "174 · 78 %", "87 · 71 %", "unknown") with ½/2×
  toggles; analysis state (idle/running with progress/cancel/failed/partial/complete);
  rights reminder ("референцията служи само за структура").
- Center timeline (flex): bars **and** mm:ss ruler; waveform; beat grid; section
  rectangles with label + confidence badge ("Drop? 62 %", "unverified"); draggable
  boundaries with snap-to-beat toggle; split/merge; feature lanes toggled individually
  (RMS, onsets, low band, centroid, flux/novelty, chroma, similarity, activity).
- Bottom inspector (200 px): selected section start/end (bar + time), role select,
  confidence + evidence list, detected events, similar sections, notes.
- Right panel (300 px): Blueprint extraction — traits to borrow (form, dynamics,
  density, transitions), exclude audio/melody (always on), name/tags/notes → save to the
  library (kind `blueprint`).
- States: skeleton, empty (drop a file / import), loading, failed (why), partial (lanes
  computed, no grid), complete. Keyboard: space, arrows by bar/beat, +/- zoom, S split,
  M merge, L label.

### 5.2 Arrangement Composer (screen "Композитор")
- Top: style (liquid / atmospheric / jungle / deep / minimal / experimental / ethno
  fusion / hybrid), target duration, BPM (editable, blueprint default), meter, key/mode
  or undecided, energy arc preset, density, variation strength, surprise, motif retention,
  kaval priority, seed, **Generate candidates**.
- Overview lane: target energy arc vs computed; section blocks colored by role.
- Timeline: tracks Kaval, Drone, Pads, Chords, Bass, Kick, Snare, Hats, Perc, FX,
  Ambience, Countermelody (+ user tracks) with event/clip blocks per section; locks per
  section and per track; transitions as markers between sections.
- Section inspector (right): length in bars, energy/density/tension targets, primary
  motif, dominant instrument, transition in/out (type, length, intensity), variation
  budget, locked.
- Variation drawer (bottom-right): candidates A/B/C… with "what changed" diff, audition,
  accept/reject; regenerate selection.
- Bottom: clip detail / piano roll / drum grid (P1 R3) / automation / transition editor /
  kaval phrase placement; persistent undo/redo, save, render range, export stems/MIDI,
  panic.

### 5.3 Musical Director (panel, dockable in Composer and Studio)
- "Scan" → list of findings grouped by severity; each: bars (clickable → timeline), plain
  Bulgarian text, evidence row, buttons Listen before/after, Explain, Apply, Ignore,
  "Не предлагай повече в този проект".
- Health strip: section contrast, motif continuity, variation density, transition
  coherence, repetition burden, masking risk, dynamic arc — each a labeled bar with the
  threshold shown, with the caveat text "евристика, не художествена истина".

Visual verification: real screenshots at both resolutions after each slice, compared to
these blueprints; deviations listed.

## 6. Integration plan with existing modules

| Existing | Role in P2 | Change needed |
|---|---|---|
| `project.json` markers | migrate to `SongMap` sections (ticks) | migration + both views for one release |
| `BeatClock`, `PatternSwap` | section-scheduled pattern swaps | shell lookahead queue; engine unchanged |
| `groove/bassline/ostinato/counter` | vocabulary for GrooveEvolution/Motif engines | parameterize per section; expose per-phrase variants |
| `harmony.rs`, `tracker.rs`, `intent.rs` | harmony over time; kaval-space analysis; Follow/Fixed/Hybrid modes | timeline placement; no live chasing |
| Atmosphere `program.rs` (scenes/automation) | ambience per section, swells, scenes | sections drive scene boundaries |
| Buses (`bus_cmds.rs`) + `fade_ms` | transitions on buses (cuts, sweeps, fades) | automation lanes on buses |
| Arrangement clips | kaval phrases placed per section | bar-based clip positions (ticks) |
| Export | full mix with background + aligned stems + MIDI + markers | new `export_song` using `render_with_stems` for all slots |
| Library | blueprints, arrangements, candidates as kinds | new kinds, payload schemas |
| Assistant bridge | the owner can ask Claude Code for arrangements through ops | new ops after D |

## 7. Feature and test matrix (summary; rows live in `docs/traceability/FEATURE_MATRIX.md` §C)

| Feature | Tests (automated) | Human evidence |
|---|---|---|
| Tempo candidates ½/2× | synthetic 174/87; rubato; silence intro | — |
| Boundaries with confidence | fixtures with tolerance; precision/recall reported | owner corrects 3 reference songs |
| No position-based labels | unit: same features at different indices → same role | — |
| Blueprint round-trip | serde + migration | — |
| Candidates distinct | ≥ 3 differ in section lengths/roles/arc (metrics) | A/B listening |
| 32 bars ≠ 8 × 4 | meso/macro event count per 32 bars > threshold | listening |
| Locks respected | regenerate bass leaves kaval/drums byte-identical | — |
| Odd meters | 7/8, 9/8, 11/8 bar mapping | — |
| Transitions click-free | max sample step test across boundaries | listening |
| Stems aligned | same length/start; MIDI timing vs events | DAW import check (owner) |
| Director findings | fixtures with known repetition → finding at right bars | owner reads critiques |
| Three demo tracks (A melodic liquid, B ethno kaval, C deep) | rendered WAV + JSON + change log | listening panel, no fabricated scores |

## 8. Staged plan without simplification (approval gates)

| Phase | Scope | Acceptance | Depends on |
|---|---|---|---|
| **A** | this document; baseline renders (needs P1 R0 `rhythm_render`); ADR for FFT | owner approval | — |
| **B** | `crates/analysis`: import, tempo/beat grid, features, novelty/recurrence, map editor UI with states, persistence, labels with confidence | demo on 5 varied files; correction workflow; fixtures pass; screenshots | ADR FFT |
| **C** | `SongMap` + blueprint schema, section editor, energy arc, bar/time sync, markers migration, export markers | save/load/round-trip; UI at both resolutions | B (for blueprint extraction), P3 shares C |
| **D** | first full-song arrangement: form planner, orchestrator, groove evolution over today's generators, motif engine v1, transitions v1, renderer (live + bounce) | one 2–4 min render with ≥ 3 structurally different sections; distinct candidates; locks | C, P1 R1 (instrument ids), P1 R0 render |
| **E** | Musical Director + scoped regeneration + A/B + audition-before-apply | fixtures; owner reads/accepts critiques | D |
| **F** | DAW export (background in mix, aligned stems, MIDI, markers, headroom/tails), beginner tutorial, visual QA, installer | DAW import verified by the owner | D |
| **G** | optional local models (role classification, downbeats, ranking) | benchmarks first | F |

Order relative to the other programs (proposal): P1 R0 render → P2 A/B start → P1 R1/R2
PoC → P2 C (shared SongMap with P3 Phase 1) → P2 D → P3 2/3 → P2 E/F. The owner decides at
each gate.

Decisions requested: (1) Rust analysis with an FFT crate (ADR) instead of Python; (2)
SongMap in ticks as the shared model, markers migrated; (3) the order above.
