# Live Performance Director — feature contracts (Phase 0)

Each feature: inputs, outputs, errors, persistence, test. "Unimplemented" features are not
shown in the UI; they are listed here with their phase.

## F-PD-1 SongMap (shared with P2 Phase C)
- Data: `SongMap { ppq, tempo_map: [{tick, bpm}], meter_map: [{tick, num, den, grouping}],
  sections: [Section], cue_plans: [CuePlan] }` in `project.json` (schema bump + migration
  from markers at the single tempo; backup of the previous file).
- Operations: add/split/merge/rename/role/drag with snap; validation: no negative or
  overlapping sections, references valid, custom meters allowed.
- Errors: invalid edit → rejected with reason, state unchanged.
- Tests: round-trip, migration, invalid edits, odd meters, undo/redo.

## F-PD-2 Song transport
- One position in ticks drives clips (seconds via tempo map), rhythm (beat anchor) and
  the atmosphere program; play/stop/seek/loop; tempo change re-anchors without stopping.
- Output: Tauri event `transport` ≤ 30 Hz `{tick, bar, beat, playing, latency_ms}` plus
  the atomics for the engine.
- Tests: seek/loop/tempo/restart keep clips, rhythm and program aligned (offline render
  comparison); no stale state after seek.

## F-PD-3 Cue scheduler
- Engine `CueQueue` (fixed capacity, filled via ringbuf) fires `CueFired{id, frame}` by
  frame comparison on the beat clock; pre-cues are separate queue entries.
- Shell subtracts measured output latency and emits `cue` events; seeks/loops/tempo
  changes clear and refill the queue (generation counter).
- Tests: pre-cue offsets, seek invalidation, duplicate prevention, drift over 10 minutes,
  latency compensation applied.

## F-PD-4 Cue model and plans
- `Cue { id, start_tick, end_tick, instrument_id, action, priority, note_targets?,
  phrase_ref?, dynamic, articulation, instruction_bg, instruction_en, rationale, source,
  confidence, approved, locked }`, `CuePlan { id, arrangement_version, skill, style,
  seed, cues, locked_cues }`.
- Persistence in the project; edits never overwritten by regeneration.

## F-PD-5 Cue planner v1 (pure, seeded)
- Inputs: SongMap sections + roles, track activity proxies, motifs (analysed take
  phrases or constrained generator), harmonic plan, skill, instrument range (profile),
  breath constraint (max phrase length by skill), locks.
- Output: 3–4 plans that differ (not copies) with per-cue rationale; score explanations,
  no "quality %".
- Policies per role (intro/build/pre-drop/drop/break/drop 2/outro) as in spec §4.
- Tests: seed reproducibility, constraints (range, max phrase, protected rests, no overlap
  with conflicting role), plans differ by a defined metric, density limits per skill.

## F-PD-6 Coach UI
- NOW/NEXT/LATER, countdown, accept/dismiss/lock, pre-cue flashes, reduced motion,
  keyboard; strings via i18n.
- Tests: component tests for state transitions from cue events; screenshots.

## F-PD-7 Guided recording and take review
- Record with the existing recorder (raw preserved); align detected activity/notes
  (existing offline analysis) with cues; metrics early/late/rest with uncertainty.
- Tests: synthetic audio with known onsets; real microphone session by the owner.

## F-PD-8 Practice features (Phase 5)
Loop by bars with count-in, listen-then-play with guide tone, silence exercise, drop
timing trainer with latency tolerance, breath planner (user markers), notation views.

## F-PD-9 Adaptive assistant (Phase 6)
Live phrase tracking from `InputMonitor` + `tracker.rs` hysteresis; Stable/Assisted/
Creative; confidence gating; harmonic changes only at section/bar boundaries.

## F-PD-10 Export (Phase 7)
Cue sheet JSON/HTML, markers CSV/MIDI, guide MIDI, raw WAV, aligned stems (P2 F), manifest.
