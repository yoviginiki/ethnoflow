# ETHNOFLOW STUDIO — LIVE PERFORMANCE DIRECTOR & INTERACTIVE SONG MAP

Received from the owner on 2026-10-18 (written with Claude on the web). Kept verbatim.
Owner's framing (translated): *"Think how it can help me while I play: when I see the drop
coming I stop, or I start something; show me what is expected of me as a musician, remind
me what and how to play so it gets better. For a beginner this will be great. Add it, as a
map of the song, but develop it and explain it well to Claude Code so it really gets
built."* Owner's instruction: integrate as a core module next to the Arrangement Composer;
first audit the existing code and a detailed UI blueprint; **no code before approval of
architecture and design**. Audit: `docs/performance-director/`.

---

Implementation specification for Claude Code — v1.0
Status: Proposed extension to Music DNA Analyzer, Arrangement Composer, Intelligent Harmonic Accompaniment Engine, Music Academy, and Rhythm Laboratory.
Product principle: The software should not merely play backing music. It should guide a real instrumentalist through a composition, make musical intentions understandable, anticipate upcoming transitions, and help beginners develop expressive timing, phrasing, restraint, and arrangement awareness. It must remain useful without cloud AI.

## 0. NON-NEGOTIABLE IMPLEMENTATION CONTRACT
- Inspect the existing EthnoFlow repository and specifications before modifying anything. Document actual architecture, supported audio interfaces, timebase, transport, current arrangement schema, note detection and UI conventions. Do not invent integration APIs.
- Deliver real Windows software, not a mockup. Every visible action must have a functioning handler, persisted state, and appropriate test. Unimplemented features must be explicitly disabled and labeled; never simulate success.
- Preserve current projects and existing functionality; implement migrations with backups and rollback.
- Separate audio callback from analysis, database, UI and generative work. No allocations, locks, filesystem access, or AI calls on the real-time audio thread.
- Preserve original microphone recording unmodified. Guidance and pitch correction are optional and non-destructive.
- No arbitrary claim of 100% note-intent recognition. Show uncertainty and offer manual overrides.
- First implement an end-to-end vertical slice with one song, one instrument, a section map, countdown, cue playback and recording; only then expand.
- Every delivered phase needs screenshots of the actual app, an audio demo, automated test results, known limitations, and a feature-by-feature acceptance matrix.

## 1. USER JOURNEYS
A. Prepare a performance before playing
1. Open a project or generate a song arrangement.
2. See a full-song map with Intro / Build / Drop / Breakdown / Drop 2 / Outro and editable sections.
3. Select the instrument (kaval, flute, other), proficiency (beginner/intermediate/advanced), style (ethno liquid DnB), and role (lead, response, atmospheric ornament, silence).
4. Generate 2–4 proposed performance plans. Each plan consists of time-bound cues, not necessarily fixed notes.
5. Preview suggested phrases with MIDI/synth guide tone, or play the underlying original audio; accept, move, delete or edit cues.
6. Select rehearsal, take recording, or free improvisation mode; set count-in and metronome.
7. During playback, follow the playhead and upcoming cue panel; see a countdown and clear instructions.
8. After recording, inspect where the musician played, rested, entered early/late, and what might improve.
9. Export the take, cue sheet, markers, and optional MIDI transcription to DAW.
B. Learn while playing
- Beginner guidance: plain-language actions ('play a long D', 'repeat the short phrase', 'leave 2 beats of silence'), with notation optional.
- Advanced guidance: pitch, duration, articulation, dynamics, harmonic function, section role, and alternative motifs.
- Show why a cue matters: space before drop, call-and-response, contrast, tension/release, avoiding masking bass/snare.
- Never shame users for mistakes. Score timing and pitch only against an explicitly chosen reference; improvisation is not automatically wrong.
C. Adaptive improvisation
- Listen to live audio and estimate activity, notes and phrase boundaries.
- Use confidence and hysteresis. If unsure, hold the existing accompaniment and show a neutral cue.
- Let the musician lock the harmony/arrangement or allow conservative adaptation at musical boundaries.
- Offer suggestions without interrupting performance; defer detailed feedback until playback ends.

## 2. SONG MAP — DATA MODEL AND EDITING
SongMap has a canonical timebase in beats/ticks with tempo-map and time-signature support; derive seconds from tempo map, never use seconds as the only source of truth.
Entities:
- Project: id, title, version, tempoMap, meterMap, keyCandidates, assets, tracks, arrangement, cuePlans.
- Section: id, type (intro/build/drop/break/outro/custom), startTick, endTick, name, energyTarget [0,1], densityTarget [0,1], tensionTarget [0,1], instrumentRoles, transitionRules, notes.
- Transition: fromSectionId, toSectionId, anticipationBars, fillPolicy, riserPolicy, silenceWindow, expectedEnergyDelta.
- Cue: id, startTick, endTick, instrumentId, action (play/hold/rest/enter/exit/repeat/answer/improvise), priority, noteTargets (optional), phraseRef (optional), dynamic, articulation, instructionBg, instructionEn, rationale, source (user/generated/analyzed), confidence, approved.
- CuePlan: id, sourceArrangementVersion, skillLevel, style, lockedCues, generatedSeed, createdAt.
- Take: id, audioAssetId, timestamps, latencyCalibration, detectedEvents, notes, annotations, originalUnchanged.
- Analysis annotations and confidence must be distinct from verified ground truth.
Editing operations: drag section boundaries with snap, duplicate section, rename, change role, drag cue, trim cue, lock cue, multi-select, undo/redo, compare cue plans, save presets. Validate no negative durations or invalid references. Custom and asymmetric meters must not break rendering.

## 3. INTERFACE BLUEPRINT (DESKTOP WINDOWS)
Layout at 1440×900
- Top 64 px transport: Play/Stop/Record, BPM, meter, timecode, bar:beat, loop, metronome, latency indicator, mode switch.
- Left 240 px song navigator: sections, bookmarks, cue plan list, instrument and skill controls.
- Central timeline (flex): horizontally scrollable multi-lane song map with colored sections, bar grid, zoom, playhead, energy curve, drum/bass/pad/kaval activity lanes, cues and transitions.
- Right 300 px Performance Coach: NOW / NEXT / LATER cards, large action text, countdown, pitch target or 'free', dynamics, explanation and quick dismiss/lock.
- Bottom 180 px optional note staff/piano roll and live pitch trace; collapsible to maximize map.
- Keyboard shortcuts: Space play/pause; R record; M metronome; L loop; +/- zoom; Left/Right navigate bars; Enter accept cue; Esc dismiss. Must avoid conflicts with DAW shortcuts.
- Resizable panes, high-DPI scaling, keyboard navigation, accessible contrast and reduced-motion mode.
- Bulgarian-first UI with localization keys and English fallback. Do not put text into graphics.
Visual requirements: Distinct but subdued colors for sections; strong playhead; upcoming section preview; no illegible tiny badges; responsive panels at 1280×720 and 1920×1080; actual screenshots must be reviewed for overflow, clipping and visual hierarchy.

## 4. CUE ENGINE — REAL MUSICAL BEHAVIOR
The cue engine must generate musically purposeful actions rather than random events every 8 bars.
Inputs: section type, energy curve, track activity, available melodic motifs, harmonic plan, performer skill, instrument range, breath constraints, current phrase, and manual preferences.
Guidance policies:
- Intro: expose motif, long tones, sparse exploration.
- Build: shorten motifs, increase anticipation, reserve space for risers/fills.
- Pre-drop: propose a rest or sustained resolution; configurable 1/2/4-bar window, never mandatory.
- Drop: offer strong entry on downbeat or deliberate delayed entry; choose based on energy and arrangement density.
- Breakdown: slower, more lyrical or spacious phrasing.
- Drop 2: vary register, rhythm or motif, not necessarily volume.
- Outro: simplify and resolve, or leave an open ending.
Cue selection algorithm:
1. Create section-level role plan and target activity envelope.
2. Identify candidate motifs from user recordings or a constrained phrase generator.
3. Generate alternative cue sequences under hard constraints (instrument range, maximum phrase length, breathing/rest, locked harmony, protected silence, no overlap with conflicting role).
4. Score continuity, novelty, energy fit, density fit, phrase balance, breathing feasibility and beginner difficulty.
5. Penalize over-cueing, abrupt changes, repetitive identical motifs and needless high-register instructions.
6. Keep top 3–4 meaningfully different plans. Expose score explanations, not a misleading 'quality percentage'.
7. User approves a plan; generation never silently overwrites manual edits.
For free improvisation, cues can be non-pitch-specific: 'hold', 'leave space', 'answer the bass', 'repeat motif', 'reduce intensity'.

## 5. COUNTDOWN & ANTICIPATION
- Always display current section, next section, bars and beats until transition.
- Configurable pre-cues at 8/4/2/1 bars and 4/2/1 beats; support visual, optional soft click, optional speech, and MIDI cue output.
- Speech must never mask the musician or unexpectedly enter exported mix. Use separate monitor bus.
- Countdown must derive from the same transport clock as audio, compensated for calibrated output latency; no independent UI timer driving musical events.
- Cue scheduling should use a lookahead event queue and sample-accurate or buffer-accurate transport positions.
- When transport seeks, loops or tempo changes, reschedule pending cues; no stale cue triggers.
- Support optional footswitch/MIDI controller for next section, repeat section, dismiss cue, record take, and 'follow me' hold.

## 6. PERFORMANCE ANALYSIS AND ERROR TOLERANCE
- Keep raw audio and analysis separate. Extract pitch contours, onset candidates, RMS envelope, silence, phrase boundaries and confidence.
- Treat glissandi, breath, ornaments and unstable attacks as possible expressive events, not automatic wrong notes.
- In rehearsal against a known motif, align performance to reference with constrained dynamic time warping or equivalent, handling tempo drift and skipped notes.
- In improvisation mode, do not show 'wrong note' feedback without a user-selected tonal/harmonic constraint.
- Track early/late entry, phrase length, dynamics and rests; offer concise improvement notes after the take.
- Harmonic adaptation should be conservative and occur at section/bar boundaries, not at every detected note.
- Stable mode ignores live pitches for accompaniment decisions; Assisted mode uses high-confidence phrase evidence; Creative mode allows broader changes with explicit control.
- If pitch detection fails, cues still function from song timeline.

## 7. PRACTICE & EDUCATION FEATURES
- Guided rehearsal by section; loop 2/4/8/16 bars with optional count-in.
- Slow practice with time stretch that preserves pitch where feasible; clearly flag artifacts.
- 'Listen then play': guide melody and call-and-response.
- 'Silence exercise': learn when not to play; detect microphone activity during designated rests.
- 'Drop timing trainer': enter on bar 1, beat 1 after countdown, with timing tolerance and measured input latency.
- 'Breath planner': adjustable breath markers based on user feedback, not unsupported physiological inference.
- Notation: staff, note names, scale degrees and piano roll; allow user to hide theory or show explanatory tooltips.
- Explain arrangement vocabulary: intro, build, drop, breakdown, fill, riser, transition, motif, hook, call-and-response.
- Beginner difficulty scales number of cues, note range, rhythmic complexity, visual density and feedback frequency.

## 8. ARRANGEMENT INTEGRATION
- Music DNA Analyzer may propose section boundaries and energy curves from imported reference tracks. All labels must be editable and accompanied by confidence; do not assign Drop based solely on section index.
- Reference tracks may inform high-level form, density and transition placement; never reproduce copyrighted melodies, stems or recognizable audio in generated output without rights.
- Arrangement Composer supplies tempo map, section grid, stems, harmony and transitions; Performance Director supplies cue plan and live take.
- Rhythm Laboratory supplies fills and pre-drop events, but performer cues can reserve a silent window.
- Intelligent Harmonic Accompaniment Engine respects cue plan locks, fixed harmonic center and error-tolerance mode.
- Instrument Forge supplies guide instrument presets and user-created timbres.
- Ambient Playground can maintain a drone/pad independent of uncertain microphone notes.

## 9. EXPORT / INTEROPERABILITY
- Export cue sheet as PDF/HTML and JSON; arrangement markers as CSV and, where supported, MIDI markers; guide melody MIDI; recorded original WAV; optional processed monitor mix WAV; stems and project tempo map.
- Provide DAW-neutral folder export with manifest: sample rate, bit depth, BPM/tempo map, meter, bar offset, section markers, track names, source provenance.
- Export cue events as MIDI note/marker events only when semantically meaningful; avoid pretending a textual 'rest' cue is a playable MIDI note.
- All WAV stems must share the same start offset and duration for DAW alignment.

## 10. AUDIO & SOFTWARE ARCHITECTURE
Suggested module boundaries (adapt to existing stack):
- song_map_core: beat/tick data model, tempo mapping, serialization, migrations.
- arrangement_bridge: import arrangement analysis and stem activity.
- cue_planner: deterministic constraint-based cue generation with seed.
- cue_scheduler: sample-clock transport event scheduling.
- performance_capture: microphone input, recording and device configuration.
- performance_analyzer: offline pitch/onset/phrase analysis, confidence and comparison.
- coach_ui: map, cues, countdown, practice panels.
- export_service: WAV/MIDI/JSON/CSV/cue sheet exports.
Use Python for offline MIR and experimentation; real-time transport and audio scheduling should live in the existing native engine (Rust/C++ if available). React/Tauri UI receives throttled telemetry and does not control sample timing. IPC messages must be versioned and validated.

## 11. IMPLEMENTATION PHASES WITH ACCEPTANCE GATES
Phase 0 — Repository audit and UX blueprint
Deliver architecture map, dependency graph, screenshot of current arrangement UI, list of missing APIs, detailed design mockup and control inventory. No code changes until approved.
Phase 1 — Interactive Song Map vertical slice
Real song project with editable 6-section timeline, zoom/scroll/playhead, bar-based positions, section markers and persistence. Test seek, loop, tempo change and restart. Screenshot and demonstration video.
Phase 2 — Deterministic cue scheduler
Hand-authored play/hold/rest/enter cues with lookahead countdown synchronized to actual playback. Record a demo with no stale events after seeking. Test latency and scheduling accuracy.
Phase 3 — Cue planner v1
Three generated cue plans for an existing arrangement, skill modes, user edits/locks, plan comparison, deterministic regeneration, rationale. Tests ensure not all plans are trivial copies.
Phase 4 — Guided recording & take review
Record microphone WAV, monitor latency calibration, align activity to cues, display early/late/rest metrics with uncertainty, preserve raw take. Integration test with synthetic audio and real microphone.
Phase 5 — Beginner coach & notation
Interactive explanations, looped practice, note/staff view, drop trainer, optional guide tone, localized help. Accessibility checks.
Phase 6 — Adaptive performer assistant
Phrase tracking, conservative harmonization, configurable Stable/Assisted/Creative, confidence gating, failure-safe behavior. Tests with ~20% wrong-note synthetic cases, ornaments, variable tempo and noise.
Phase 7 — DAW export and advanced controllers
Aligned stems, MIDI, markers, cue sheet, footswitch/MIDI mapping, tempo map support and interoperability tests.

## 12. AUTOMATED TEST MATRIX
- Transport: start, pause, seek, loop, tempo changes, non-4/4, long session.
- SongMap: invalid boundaries, overlapping sections, migrations, undo/redo, project reload.
- Cue scheduler: pre-cue offsets, seek invalidation, duplicate prevention, clock drift, output latency.
- Generator: seed reproducibility, constraint satisfaction, protected rests, cue density limits, section transitions.
- Audio: mic permission failure, device disconnect, clipping, silence, breath noise, varying sample rates.
- Analysis: short ornament vs stable pitch, skipped notes, noisy input, timing offset, low confidence.
- UI: keyboard-only use, 125–200% scaling, 1280×720/1440×900/1920×1080 screenshots, no clipped text.
- Export: WAV sample alignment, MIDI note timing, markers, project roundtrip, no destructive alteration of source.
Set measurable performance budgets from actual baseline hardware and document them. Do not fabricate measured latency, accuracy or musical quality scores. Human listening evaluation with at least several contrasting examples is required.

## 13. DEFINITION OF DONE
A feature is not done until (a) real audio behavior works, (b) UI is implemented according to approved blueprint, (c) state persists, (d) errors are handled, (e) automated tests pass, (f) a real demonstration exists, and (g) known limitations are listed. No decorative buttons or fake waveforms.

## 14. INITIAL CLAUDE CODE INSTRUCTION
Read this specification and all current EthnoFlow project documents. Audit the repository. Produce docs/performance-director/ARCHITECTURE_AUDIT.md, UI_BLUEPRINT.md, FEATURE_CONTRACTS.md, PHASE_PLAN.md, and TEST_MATRIX.md. Include a screenshot of the current app and a concrete gap analysis. Do not modify application code until the user approves Phase 0. Then implement the phases in order; demonstrate each phase using the actual Windows app and audio output. Never silently simplify or replace a specified feature with a placeholder.

---

## Owner's concept notes (2026-10-18, verbatim summary)

Module name: **Live Performance Director & Interactive Song Map**. Three ways of working:
rehearsal with cues, take recording, free improvisation with an intelligent assistant.
Illustrative interface: transport (style, BPM, meter), mode (Rehearsal), current position
(bar 29 / 128), next transition (Drop 1), section strip Intro → Build → Drop 1 → Break →
Drop 2 → Outro with bar numbers, a "your next task" card ("finish the phrase and leave a
pause; about 4 bars to the drop; play a short closing motif, then free space for drums and
bass"), action chips Play / Hold / Rest / Enter on Drop, Next (4 bars: rest), Dynamics
(soft), Role (lead, kaval). Every section carries editable musical instructions that can
be auditioned, practised and recorded. Highlighted features: Now / Next / Later, Drop
Countdown by bars and beats, Phrase Suggestions, Practice Loop, Take Review (early / late /
rest), Adaptive Coach (does not treat every inexact note as an error). Cues must sync to
the real audio clock, not a UI timer.
