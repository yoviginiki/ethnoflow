# EthnoFlow Studio — Implementation & Design Fidelity Contract

Received from the owner on 2026-10-17 (written with Claude on the web). Status: **mandatory
engineering specification** for Claude Code in this repository, on top of the master plan,
FEATURE_SPEC and CLAUDE.md. How it is applied here is summarised at the end
(§ "Application in this repository"); the contract text follows verbatim.

---

Version: 1.0
Status: Mandatory engineering specification for Claude Code / Cursor
Scope: Entire Windows application; first enforcement target: Rhythm Laboratory + Instrument Forge.
This contract supplements the existing EthnoFlow master plan and module specifications. It does not replace them. The goal is to eliminate shallow UI mockups, disconnected controls, silent scope reductions and unverifiable completion claims.

## 0. Non-negotiable instructions to the coding agent

1. Audit first. Inspect repository structure, implemented audio graph, UI routes, persistence, dependencies, tests, and existing features. Produce docs/audits/CURRENT_STATE.md with exact file references and a feature inventory. Never assume a feature exists because a component has a matching name.
2. Build a traceability matrix from every requirement in the EthnoFlow specs to a component, audio implementation, persistence format, test and acceptance evidence. Save as docs/traceability/FEATURE_MATRIX.md.
3. Do not silently simplify. If a requested feature is too large, label it NOT IMPLEMENTED, explain its dependencies, propose a staged implementation, and ask for approval. Never claim a placeholder, a CSS knob, or a static waveform is a completed audio feature.
4. No cosmetic-only controls. Every interactive element must have a defined value range, default, unit, real effect on audio or application state, automation/persistence behavior, and test.
5. No fabricated screenshots or tests. Screenshots must come from the actual running Windows app; audio examples must come from the actual audio engine. Provide commands and file paths to reproduce results.
6. Never overwrite user recordings or presets. Use versioned migrations, backups and atomic save operations.
7. Finish vertical slices. A slice is complete only when UI → state → audio engine → save/reload → export → tests work together.
8. Preserve existing working features. Run baseline tests before editing; compare results after. Record intentional breaking changes.
9. Stop at review gates. Do not advance to the next major milestone without explicit user approval.
10. Be transparent. Each report must include completed, partially completed, blocked, unimplemented and known-defect items.

## 1. Definition of Done (DoD)

A feature is DONE only if all relevant conditions are met:
- Accessible from the actual Windows app through a discoverable navigation path.
- Visually implemented to the approved UI blueprint, including labels, units, hover/focus/disabled states, keyboard operation and useful tooltips.
- Audio effect is measurable or audible, and is not a fixed pre-rendered demonstration masquerading as synthesis.
- Controls have deterministic, bounded parameters; automation uses sample-accurate or block-accurate scheduling as appropriate.
- No audible clicks on ordinary parameter changes; smoothing and crossfades where required.
- Presets/projects survive save, restart, load and version upgrades.
- Undo/redo behavior is documented and tested where editing occurs.
- Stereo/mono and sample-rate behavior is defined; no NaNs, runaway amplitude, unbounded memory or UI-thread audio processing.
- Automated tests cover happy paths, boundary values, invalid data, serialization and regression.
- A human can verify it with a reproducible test scenario, an actual screenshot and an audio render.

Statuses: NOT STARTED, DESIGNED, IMPLEMENTING, FUNCTIONAL, TESTED, VISUALLY VERIFIED, DONE, BLOCKED. DONE cannot be assigned automatically based on code generation alone.

## 2. Required artifacts before implementation

For each major screen/module create:
1. docs/design/<module>/SCREEN_BLUEPRINT.md — grid, regions, widths, hierarchy, navigation, states, breakpoints, keyboard accessibility, text labels and design tokens.
2. docs/design/<module>/CONTROL_CATALOG.csv — control ID, label, tooltip, type, min/max/default/unit, mapping, audio target, smoothing, automation, persistence, test ID.
3. docs/engineering/<module>/SIGNAL_FLOW.md — actual DSP signal path and threading boundaries.
4. docs/engineering/<module>/DATA_MODEL.md — instrument/preset/project JSON schema, migration and compatibility policy.
5. docs/testing/<module>/ACCEPTANCE.md — scenario-by-scenario verification with expected results.
6. docs/design/<module>/REFERENCE_COMPARISON.md — approved visual reference vs actual app screenshots; list deviations and fixes.

If no approved screenshot or pixel-perfect mockup is supplied, do not invent a claim of visual fidelity. Produce a detailed UI proposal and request approval before implementing a large screen.

## 3. Global Windows UI system

- Desktop-first design at 1920×1080 and 1366×768; verify both. Resizable and DPI-aware at 100%, 125%, 150% scaling.
- Clear three-level navigation: global workspace → instrument/module → detailed editor.
- Dark professional audio-workstation theme with readable contrast, consistent spacing and typography; no giant empty panels or oversized generic cards.
- Audio meters and waveform/spectrum views must display measured engine output, not decorative animation.
- All audio parameters show meaningful numerical readouts, units, reset/double-click behavior and contextual help in Bulgarian.
- Support pointer and keyboard; no functionality hidden exclusively behind drag-and-drop.
- Tooltips explain what a control changes acoustically, not just repeat its label.
- Undo/redo, preset dirty indicator, A/B compare, reset, copy/paste settings, favorite and search should be consistent across modules.
- Persist panel layout and selected tab per user. Avoid UI re-render patterns that interrupt live audio.

## 4. Instrument Forge — mandatory screen blueprint

Header: instrument name, category, author, unsaved marker, preset browser, previous/next preset, save as, version history, favorite, A/B compare, randomize-with-locks, undo/redo, MIDI learn (when supported), help.

Left browser (collapsible): categories (Kick, Snare, Clap, Hat, Percussion, Bass, Drone, Pad, Pluck, Texture, Hybrid, User), tags, searchable presets, favorites, recently used, factory/user distinction, sample source list.

Center workspace (tabbed):
- Overview: signal-chain diagram with active modules and meters, 8 assignable macro knobs, instrument keyboard/pads, audition buttons, quick ADSR/filter/effect overview.
- Sources: at least two independent layers; each layer can choose sampler or synthesis source. Gain, pan, pitch, coarse/fine tuning, key/velocity range, start offset, looping, reverse for samples, mute/solo. Show waveform for sample layer and oscillator/partial representation for synth layer.
- Amplitude: ADSR with editable graph and numerical values, velocity sensitivity, curve selection; live audition.
- Filters: filter type (initially LP/HP/BP), cutoff in Hz, resonance/Q, drive when implemented, envelope amount, key tracking; spectrum before/after where feasible.
- Modulation: at least two LFOs, envelope routing, modulation matrix rows with source → destination → amount → polarity; validate against feedback cycles where necessary.
- Effects: reorderable effect chain (initially distortion, delay, reverb, compressor/EQ as engine capabilities permit); bypass, wet/dry, output gain, clipping meter.
- Performance: mono/poly, voice limit, glide, pitch bend range, velocity curves, choke groups for percussion, MIDI note mapping.
- Advanced: oversampling options only when genuinely supported, CPU meter, voice diagnostics, sample-rate and latency information.

Bottom area: always-visible audition pads/keyboard, output peak/RMS meters, waveform/spectrum view, active-voice count, save/render controls. Panels must remain navigable without covering essential audio controls.

### 4.1 Mandatory control examples

| ID | Control | Behavior | Validation |
|---|---|---|---|
| amp.attack | Attack, ms | Smooth amplitude rise after note-on | 0–5000 ms; audio envelope measurement |
| amp.decay | Decay, ms | Time to sustain level | 0–10000 ms; render test |
| amp.sustain | Sustain, dB or normalized | Level after decay | documented mapping; save/load |
| amp.release | Release, ms | Tail after note-off | 0–20000 ms; no stuck voices |
| filter.cutoff | Cutoff, Hz | Audible spectral change | frequency sweep test |
| filter.resonance | Resonance/Q | Resonance near cutoff; bounded | stress test; no instability |
| lfo1.rate | LFO rate, Hz or synced | Modulates selected destination | deterministic render; tempo sync |
| sample.start | Sample start | Shifts playback start position | waveform edit + audible change |
| layer.mix | Layer balance | Changes audible blend | meter/render comparison |
| macro.1 | Assignable macro | Controls one or more mapped parameters | mappings persist and are testable |

The table is illustrative, not exhaustive: generate the full catalog for every visible control before coding.

### 4.2 DSP contract

Propose and document an audio graph: note event → voice allocator → sample/oscillator layers → envelopes → filters → modulation → effects → master gain/meter → audio output. Real-time callback must not perform blocking disk IO, allocations on the critical path, locks with unbounded waits, Python calls or UI rendering. Heavy analysis and preset loading happen off the audio thread with safe handoff.

Use real sound synthesis and sample playback. Prefer a working narrow feature set over a broad decorative UI, but never mark omitted functions complete. Provide offline rendering for deterministic tests and exports.

## 5. Rhythm Laboratory — detailed requirements

### 5.1 Arrangement and editing
- Multiple instrument tracks with user-defined names, colors, icons, mute/solo, gain/pan, output routing and editable instrument assignment.
- Pattern grid with steps, measures, note events, velocity, note length, microtiming, probability, ratchets/retriggers, accent and per-step parameter locks.
- Variable pattern lengths per track and global transport sync; polymeter/polyrhythm without drift.
- Time signatures including 4/4, 3/4, 5/8, 7/8, 9/8, 11/8 and custom numerator/denominator, with correct metrical grouping options.
- Groove templates, swing, humanization with user-defined seed, undo/redo and A/B comparison.
- Euclidean rhythm generator with steps, pulses, rotation and accents; editable output.
- Fill generator and variation generator with locks so existing kick/snare patterns can be preserved.
- Liquid DnB templates with distinct kick/snare backbone, syncopated hats, ghost notes, breaks, fills, and bass sidechain hooks. Do not hardcode one pattern as the only result.
- Pattern chaining, scenes, arrangement timeline, loop ranges, transport, count-in and metronome.
- MIDI export with note/velocity/timing and tempo map where relevant; separate WAV stems rendered from the actual instruments.

### 5.2 Instrument integration
Each track opens its own full Instrument Forge page, not a simplified modal with five controls. Track instrument references are stable IDs pointing to immutable preset versions or editable copies; edits to one track do not silently overwrite shared factory presets. Implement explicit 'Edit copy' and 'Update user preset' actions.

### 5.3 Sound library
- Categories, tags, preview, favorites, custom collections, duplicate detection via content hash, source/author/license metadata.
- Import WAV/AIFF/FLAC where supported; resampling and channel conversion; preserve originals.
- Factory sounds must be legally redistributable and attributed where required. No unverified 'royalty-free' claims.
- Organic Sample Lab: record microphone, trim, fade, normalize with headroom, detect transients, slice, loop, pitch shift, spectral view, save as instrument source.
- Save user instrument preset and project references; include 'collect samples' / portable project export.

## 6. Avant-garde features — gated research roadmap

Implement only after the base audio graph, preset system and Rhythm Laboratory are stable:
1. Sound DNA / Mutation: parameter-space variation with seed, lockable genes, audition 8 candidates, reproducible saves, perceptual diversity checks.
2. Morphing: interpolate compatible synthesis parameters; crossfade incompatible sample sources; prevent clicks and extreme loudness spikes.
3. Granular engine: grain size, density, position, spray, pitch, window, stereo spread, freeze; bounded CPU and voice limits.
4. Physical/resonator synthesis: modal resonators, sympathetic vibration, struck/plucked/bowed approximations, useful for experimental ethno percussion.
5. Rhythm-to-sound modulation: route sequencer accents and per-step locks into synth/filter/effect parameters.
6. Generative percussion: grammar-based patterns, Euclidean variation, probabilistic fills and optional learned style suggestions; always deterministic with seed.
7. Audio-to-instrument: extract transient/body/tail from recorded sound; propose layers; require user confirmation.
8. Performance macros: map one control to several parameters with ranges and curves; record automation in timeline.
9. Preset lineage: track parent preset, changes, creator, license, source samples, revision history and reproducible mutation seed.
10. Accessibility/learning: each advanced parameter links to a short Bulgarian explanation and audio before/after example.

For each research feature write a feasibility note, DSP method, CPU budget, measurable test, UI control catalog and fallback if infeasible. Do not ship placeholders labeled 'AI'.

## 7. Audio quality and safety gates

- Confirm host/device sample rate, channel layout, buffer size and latency; provide diagnostics.
- Test at 44.1 kHz and 48 kHz; 128/256/512 sample buffers where supported.
- No clipping by default: calibrated gain staging, peak metering and optional limiter (not a substitute for correct mix levels).
- Detect NaN/Inf, runaway feedback, denormals, stuck notes and sample underruns; recover without crash.
- Reproducible offline render for tests with fixed seeds; compare envelope timing, spectral properties and event placement numerically.
- Manual listening tests with headphones and speakers; attach actual renders and log observations.
- Record 10-minute continuous playback and rapid preset switching stress tests; report dropouts and CPU usage rather than inventing a pass.

## 8. Preset / project data contracts

Every user instrument preset must include stable ID, schema version, human-readable name, category, tags, creator, creation/update timestamps, engine version, layer definitions, synthesis parameters, modulation routes, effect chain, macro mappings, MIDI mappings, referenced sample hashes, license/source metadata and optional preview audio. Keep factory presets immutable. Use safe relative asset paths, validation, migration, version history and atomic writes. Define a portable preset bundle format containing required owned or redistributable samples; warn if external samples cannot legally be bundled.

Every project must save tempo map, time signatures, pattern data, instrument references, arrangement, automation, mixer state, sample references and app version. Test save → close → reopen → render equality for deterministic content.

## 9. Visual fidelity process

Before code: create a real wireframe/mockup for each major screen and obtain approval. Include precise layout hierarchy, control density and navigation. Reference imagery is inspiration only, not a literal spec or permission to copy third-party UI.

After each slice: launch the Windows app, capture full-screen screenshots at 1920×1080 and 1366×768; compare against the approved blueprint. Check: missing panels, incorrect control count, wasted space, illegible labels, poor contrast, horizontal scrolling, clipped controls, inconsistent spacing, inactive-looking controls and visual hierarchy. Document deviations and fix before declaring visual completion.

Do not invent a numeric '95% visual match' without a defined rubric and approved reference. Use a checklist with evidence and explicit user review.

## 10. Test matrix (minimum)

| Test | Action | Expected result |
|---|---|---|
| INS-001 | Load factory kick, play pad | Audible kick, meters react |
| INS-002 | Change attack 0→500 ms | Measurable envelope change |
| INS-003 | Sweep filter cutoff | Spectral response changes audibly and numerically |
| INS-004 | Add LFO to pitch | Pitch modulation heard and measured |
| INS-005 | Save preset, restart app, reload | Identical parameters and deterministic render |
| INS-006 | Duplicate preset, edit duplicate | Original remains unchanged |
| INS-007 | Import user WAV | Waveform, preview and instrument source work |
| INS-008 | Record organic sample | Recorded audio persists and can be played in sampler |
| RHY-001 | Create 16-step 4/4 pattern | Correct kick/snare events and transport |
| RHY-002 | Create 7/8 grouped pattern | Correct metrical timeline and playback |
| RHY-003 | Different lengths per track | Stable repeating polymeter |
| RHY-004 | Set probability with fixed seed | Repeatable output |
| RHY-005 | Set per-step filter lock | Audible per-step timbre changes |
| RHY-006 | Export stems and MIDI | Files align in time; notes and events preserved |
| RHY-007 | Open track's Instrument Forge | Full editor; changes affect only intended instrument |
| QA-001 | Switch presets rapidly while playing | No crash/stuck notes/unacceptable pops |
| QA-002 | Resize app / change DPI | All essential controls accessible |
| QA-003 | Save/reopen complete project | Arrangement, instruments and sample links preserved |

Every test must record PASS/FAIL/SKIP, environment, command, evidence file and issue link. SKIP is not PASS.

## 11. Milestone gates

- Gate A — Audit and design: inventory + traceability + approved detailed UI blueprint + DSP plan. No implementation claims.
- Gate B — Core audio: stable playback, sample/synth layer, note events, meters, offline renderer, device diagnostics and tests.
- Gate C — Instrument Forge vertical slice: complete single instrument editor with actual functional ADSR, filter, layer mixer, one modulation path, one effect, audition, save/load and render. Every included control tested; future controls clearly marked unavailable, not faked.
- Gate D — Rhythm Laboratory: sequencer + track assignment + multiple patterns + timing + WAV/MIDI export + preset integration.
- Gate E — Expanded sound design: full modulation matrix, effects, macros, organic recording and browser.
- Gate F — Advanced generative features: mutation, morphing, granular, resonators and complex generative rhythm after explicit approval.
- Gate G — Release readiness: packaged Windows installer, clean-machine test, signed binaries if available, crash reporting policy, licensing manifest, documentation and regression suite.

## 12. Mandatory agent reporting template

At the end of each milestone provide:
- Changed files: actual paths and brief explanation.
- Implemented controls: link each to catalog ID and real audio/state effect.
- Missing or simplified: explicit list and rationale; request approval for changes.
- Tests: commands, counts, PASS/FAIL/SKIP and saved artifacts.
- Screenshots: actual running app at required resolutions.
- Audio evidence: actual WAV renders demonstrating at least three different settings.
- Known issues: severity and reproducible steps.
- Next milestone: dependencies, estimated risk, user decisions required.

Do not state 'production ready', 'fully implemented', 'professional quality' or 'pixel-perfect' without the corresponding acceptance evidence.

## 13. First instruction to Claude Code

Read all EthnoFlow specifications and this contract. Inspect the repository and produce CURRENT_STATE.md, FEATURE_MATRIX.md, SCREEN_BLUEPRINT.md, CONTROL_CATALOG.csv, SIGNAL_FLOW.md and ACCEPTANCE.md for Rhythm Laboratory + Instrument Forge. Identify every simplification or missing feature in the current implementation. Propose a visual layout and an ordered series of fully functional vertical slices. Do not change application code yet. Wait for my approval of the design and acceptance matrix. After approval, implement one slice at a time, with real audio evidence, screenshots and automated tests. Never replace DSP with decorative UI or claim unfinished features are done.

---

## Application in this repository (agreed with the owner, 2026-10-17)

- The owner's instruction on receiving the contract: *"take this into account and we continue
  in the order you gave"* (Cloud → background mixer with fades → Shimmer → Resonator →
  Sound Designer / new instruments → sample browser → Choir drone → Bloom → Tape loop →
  Spaces; then Phase 5 studio).
- **Standards apply from now on to every slice**: real audio effect per control with a
  measurable test, bounded ranges with units and double-click reset, no clicks on
  changes, persistence (presets/session/project) with validation, a paragraph in
  `docs/guide/*.md`, the status vocabulary of §1 in reports, and the §12 template at
  milestones (including "missing or simplified").
- **First enforcement target (§13: Rhythm Laboratory + Instrument Forge)** = the Sound
  Designer and new-instrument work (FEATURE_SPEC F4.8/F4.9 and Phase 5 mixer). Before any
  code for it: Gate A artifacts (`docs/audits/CURRENT_STATE.md`,
  `docs/traceability/FEATURE_MATRIX.md`, `docs/design/<module>/SCREEN_BLUEPRINT.md`,
  `CONTROL_CATALOG.csv`, `docs/engineering/<module>/SIGNAL_FLOW.md`, `DATA_MODEL.md`,
  `docs/testing/<module>/ACCEPTANCE.md`) and the owner's approval.
- Screenshots: taken from the running app on the owner's machine (PowerShell capture) at the
  two required resolutions when a screen is declared VISUALLY VERIFIED; never described
  from imagination.
- Audio evidence: offline renders from the engine (`render_offline`, ambient export) saved
  under `qa/renders/` (gitignored) with the command that produced them.
