# ETHNOFLOW STUDIO — RHYTHM LABORATORY & INSTRUMENT FORGE

Received from the owner on 2026-10-18 (written with Claude on the web). Kept verbatim.
Owner's instruction with it: *"Read it and analyse the existing rhythm section. Preserve
everything that works. First do an audit and a plan, then implement one fully functional
Instrument Designer with real audio processing, preset saving and sequencer integration.
Work in stages, with automated tests and real listening. No fictitious functions. Do not
move to the next big stage without approval."* Audit and plan: `docs/rhythm/`.

---

Master implementation specification for Claude Code
Version: 1.0 | Target: Windows desktop | Language of UI: Bulgarian first, English optional
Status: Product requirements and engineering plan, not a claim of implemented features.
MISSION: Transform the existing rhythm section (currently approximately five basic instruments) into a professional, extensible rhythm-composition environment with a dedicated, fully editable designer page for every instrument. Enable users to record organic sounds, synthesize unprecedented timbres, save portable original presets, assemble personal kits, compose complex rhythms, and export production-ready stems for any DAW. Preserve everything that already works.

## 0. Instructions to the coding agent — mandatory
1. Read the existing repository, existing EthnoFlow master plan, ambient/harmony specifications, and relevant tests before touching code. Do not assume the current architecture or filenames.
2. Produce docs/rhythm/AUDIT.md with current implementation, five instruments, routing, playback, audio backend, UI, sample format, timing, preset persistence, and technical debt. Mark unknowns explicitly.
3. Produce docs/rhythm/IMPLEMENTATION_PLAN.md, a dependency graph, risks, proposed schema migrations, and an incremental compatibility strategy. Seek approval before major rewrites or dependencies.
4. Implement in vertical slices, each ending with a working Windows build and audible test. Do not replace working sound with mock controls. Do not implement the entire specification in a single uncontrolled pass.
5. Keep audio rendering deterministic where seeded, sample-accurate where required, non-blocking in the realtime audio callback, and independent of network connectivity.
6. Prioritize sound quality, reliable playback, musicality, ergonomics, and stability over quantity of superficial features.
7. Every UI parameter must connect to a real DSP or sequencer behavior. Disable or label planned functions rather than pretending they work.
8. Use appropriate open-source libraries where licensing permits. Track each dependency and sound asset license; do not bundle copyrighted sample packs without explicit redistribution rights.
9. Add unit, property-based, integration, performance, offline-render comparison, and human listening tests. Document limitations honestly.
10. Keep UI explanations accessible to beginners and advanced controls available via expandable panels.

## 1. Product vision and primary workflows
EthnoFlow is an instrument-making studio, not merely a drum machine. A user can:
- Start with a preset instrument, open its dedicated Instrument Designer page, change its synthesis, sample, tuning, envelopes, effects, modulation, velocity response, and expressive controls, and save it under a new name.
- Create a new instrument from silence, an oscillator, a noise source, a recorded organic sound, a layered multisample, or a hybrid graph.
- Design a whole rhythm kit from individually saved instruments; arrange patterns, fills, grooves, and scenes; morph between patterns; render audio and MIDI.
- Create basses, pads, plucks, textures, percussion, drones, and experimental instruments in the same system.
- Record original sounds through a professional microphone and convert them into playable instruments, including unusual percussive and granular timbres.
- Use an intelligent rhythm assistant that suggests musical options while preserving creative ownership and allowing complete editing.
- Use instruments from the rhythm section anywhere else in EthnoFlow: harmonic accompaniment, Ambient Playground, live play, and DAW tracks.

Product principles
Sound first; no hardcoded five-instrument ceiling; modular yet beginner-friendly; preset portability; undoable nondestructive editing; local-first; no forced AI; open interoperability; professional timing; clear provenance; no false claims of legal exclusivity.

## 2. Navigation and information architecture
Proposed top-level areas:
1. Rhythm Laboratory: sequencer, pattern editor, kit mixer, groove browser, scene arranger, performance controls.
2. Instrument Forge: searchable instrument library and a full designer page for each instrument.
3. Sample Lab: capture, trim, denoise, slice, transient detection, normalize, tagging, multisampling.
4. Sound Library: presets, sample assets, kits, grooves, favorite sounds, user collections, version history.
5. Performance & Export: live pads, MIDI mapping, automation, offline rendering, stems and MIDI export.

Dedicated instrument page — required layout
Each instrument opens at /instruments/:instrumentId (adapt to actual routing):
- Header: name, type, category, tags, preset version, author, Save As, Compare A/B, undo/redo, favorite.
- Large playable audition pad/keyboard with velocity sensitivity and optional MIDI input.
- Main waveform or spectral visualization (real measurements, not decoration).
- Source tab: sample/oscillator/noise/granular/resonator/physical-model sources.
- Shape tab: amplitude and pitch envelopes, transient, decay, body, tone, tuning.
- Filter & FX tab: filters, distortion, saturation, compressor, delay, reverb, stereo width.
- Modulation tab: LFOs, step modulators, envelopes, random, modulation matrix.
- Layers tab: multiple sources, key/velocity zones, round robin, layer mix, phase handling.
- Performance tab: velocity curve, aftertouch, MIDI CC mapping, choke group, voice limit, glide, macros.
- Advanced tab: oversampling, quality mode, anti-aliasing, polyphony, random seed, debug meter.
- Bottom strip: waveform history, peak/RMS, CPU estimate, audition transport, export, preset notes.
- Beginner Simple mode with 6–10 musically named macros, and Expert mode with complete parameter access.

Example macro controls
Kick: Punch, Weight, Sub, Click, Length, Dirt, Space.
Snare: Snap, Body, Noise, Tail, Width, Vintage.
Hat: Brightness, Sizzle, Decay, Metal, Motion.
Bass: Sub, Growl, Harmonics, Movement, Attack, Glide.
Ethno percussion: Material, Resonance, Strike, Room, Organic Variation.

## 3. Extensible instrument taxonomy
The existing five sounds become five presets, not five fixed engine types. Implement a registry of instrument definitions with categories and tags; do not encode a fixed array of instrument slots.
Initial factory instrument families:
- Drum percussion: acoustic/electronic kick, snare, clap, rimshot, closed/open/pedal hi-hat, ride, crash, tom, shaker, tambourine, cowbell, clave, woodblock, handclap, brush, clicks, foley hits.
- Ethno/world: tapan-inspired drums, darbuka-style goblet drums, frame drums, bells, wooden percussion, seeds/rattles, struck stones and resonant metal. Avoid claiming cultural authenticity for synthesized approximations.
- Bass: pure sine sub, analog mono bass, Reese, FM bass, wavetable bass, plucked bass, granular bass, sampled bass, resonator bass.
- Tonal: mallets, plucks, keys, bell-like instruments, synthetic strings, pads, drones, arpeggiated voices.
- Experimental: breath/noise instrument, metallic textures, granular clouds, spectral resonators, water/wood/stone transformations, evolving percussion.
- User-defined: arbitrary layered instruments with tags, icons, and a custom macro panel.
Factory library goal: 30–50 genuinely distinct and useful presets in a later phase, with 10–15 carefully curated initial presets. Do not mass-produce near-identical presets merely to hit a number.

## 4. Sound engine architecture
Use the existing project stack where practical. If Tauri/Rust is already used, favor a native Rust DSP core with clear stable interfaces; assess mature DSP crates and JUCE integration only where justified. Python is for offline analysis and asset generation, not in the realtime callback.

Graph model
Instrument -> Source Nodes -> Shapers -> Filters -> Modulation -> Effects -> Voice Mixer -> Instrument Bus -> Master Bus

Required node classes:
- SamplePlayer (one-shot, loop, crossfade loop, reverse, pitch shift where feasible).
- Oscillator (sine, triangle, saw, pulse, noise; band-limited at higher quality modes).
- FM operator (2-op initial, expandable), additive partial bank.
- Granular engine (grain size, density, position, spray, pitch spread, window shape, freeze).
- Resonator / modal synthesis (tuned partials and damping; experimental).
- Envelope generators (ADSR and multi-stage for pitch and amplitude).
- Filters (low/high/band-pass, resonant state-variable where appropriate).
- Distortion/saturation, transient shaper, bitcrush, EQ, compressor, delay, reverb sends.
- LFO, random/sample-and-hold, step modulation, modulation routing.
- Layer mixer, velocity zones, round robin, voice allocation, choke groups.

Realtime rules
- Never allocate memory, read files, wait for locks, or call Python/network services in the realtime audio callback.
- Preload or stream sample data through background workers and bounded queues; define behavior for missing assets.
- Timestamp events in sample frames; schedule ahead of the callback; parameter smoothing to avoid clicks.
- Safe voice stealing, bounded polyphony, denormal protection, clipping protection, gain staging.
- Support common 44.1/48/96 kHz project rates with resampling policy; 32-bit float internal signal path.
- Measure CPU, xruns, jitter, render determinism, and latency on representative Windows devices.
- Offline export must reuse the same DSP graph and automation as realtime playback, not a simplified substitute.
- Provide an audio-engine abstraction to support future VST3/CLAP hosting only after licensing, security and stability review.

## 5. Instrument creation workflows
A. Create from synthesis
New Instrument -> Choose category/template -> Select source -> Audition -> Shape transient/body -> Tune -> Modulate -> Add FX -> Map macros -> Name/tag -> Save preset -> Add to kit.
B. Create from organic recording
Choose input (WASAPI shared/exclusive or ASIO if supported) -> test level/latency -> record mono/stereo -> inspect waveform -> trim and fade -> remove clicks/rumble conservatively -> detect transient/loop -> slice or pitch-map -> add synthesis layers -> audition -> save immutable source asset and derived instrument.
Preserve original recording. Record sample rate, channels, capture date (optional), gain, device details (optional and privacy-sensitive), and processing history. No cloud upload by default.
C. Create from an existing instrument
Duplicate or fork preset -> edit -> A/B compare -> save as a new version. Never silently overwrite factory presets.
D. Experimental instrument recipes
- Breath-to-Bass: kaval breath recording + pitch-tracked low oscillator + filtered noise transient + saturation + envelope follower.
- Wooden Liquid Kick: wood strike transient + synthesized sub pitch sweep + transient shaping + short room.
- Granular River Pad: own water recording -> granular cloud -> resonant tonal partials -> slow modulation -> stereo ambience.
- Metallic Ghost Snare: short metallic tap -> filtered noise + modal resonators -> modulated tail.
- Evolving Ethno Pulse: organic hit + step modulation + probability + pitch-following drone.
All recipes must be user-editable and must not require generative AI.

## 6. Rhythm Laboratory — sequencer
Core sequencer
- Unlimited logical instrument tracks within CPU/memory limits, sensible UI virtualization.
- Step sequencer 8/16/32/64+ steps; pattern lengths independent per track.
- Piano-roll and event-list views; drag/drop notes, resize, duplicate, mute/solo.
- Tempo, meter, subdivision, triplets, swing, per-track phase/rotation.
- Per-step velocity, gate, microtiming, probability, ratchets, repeats, accents, flam, conditional triggers.
- Euclidean rhythms with pulses/steps/rotation and editable generated steps.
- Polyrhythm and polymeter (e.g. 3 against 4; 7/8 patterns over 4/4) with clear cycle visualization.
- Humanization with separately controlled timing/velocity/timbre; seed and amount; avoid drift.
- Groove templates with editable timing/velocity curves; groove extraction from user performance after robust onset detection.
- Pattern variations A/B/C/D, fills, break/roll, call-and-response, ghost notes.
- Pattern chaining, scene launch, song arrangement timeline and automation lanes.
- Step-locks for instrument parameters (e.g. filter on step 5) with smoothing and correct recall.
- Live record from MIDI keyboard, pads, computer keyboard and optionally mic-transient capture.
- Quantization: non-destructive, strength and swing; preserve original timestamps.

Liquid Drum & Bass specialized toolkit
- Tempo presets ~160–180 BPM, not hardcoded to 174.
- Two-step kick/snare foundations with syncopated kick variations, ghost snares, rolling hats, ride/shaker layers.
- Breakbeat slicing and rearrangement with transient-safe crossfades; original breaks only if appropriately licensed.
- Reese/sub bass rhythmic alignment, sidechain control, low-end management.
- Smooth transitions, intro/breakdown/drop/bridge/outro and restrained fills.
- Groove complexity slider with actual rule changes; density, energy, syncopation, ghost-note rate, swing, human feel.
- Make DnB musical and subtle enough for acoustic kaval; no default overpowering of lead instrument.

Generative rhythm algorithms (non-AI MVP)
Use constraint-driven generators with explicit musical rules and seeded randomization:
1. Choose meter and grid.
2. Place structural anchors (e.g. backbeat) using genre profiles.
3. Add accents, hats, syncopations, ghosts under density constraints.
4. Enforce voice interaction, choke groups, min inter-hit intervals.
5. Evaluate repetition, contrast, groove, musical coherence and collision risks.
6. Return 4–12 genuinely different candidates with explanations.
7. Allow locking any track or events while regenerating the remainder.
8. Compare A/B and preserve reproducibility via seed and rule version.
Optional later ML can rank candidate grooves against user preferences; never make cloud inference a dependency.

## 7. Intelligent Instrument Designer — advanced capabilities
Modulation matrix
Sources: LFO, envelope, velocity, key tracking, note number, random, step sequencer, envelope follower, macro, aftertouch, MIDI CC, audio sidechain. Destinations: pitch, cutoff, amplitude, panning, grain density, distortion, effect sends, layer mix, resonator damping. Bipolar/unipolar depth, curves, smoothing, polarity, bypass.
Macro morphing
- Two or more snapshots A/B/C/D of instrument settings.
- Morph X/Y pad and timeline automation; interpolation appropriate to parameter type (logarithmic frequency, decibels, discrete switches with safe transitions).
- Lock protected parameters while morphing.
- Save morph movements as reusable automation.
Sound DNA / mutation
- Mutate selected parameters within safe bounded ranges; preserve user-locked traits.
- Generate 8 variants, audition blind, favorite and breed/mix two settings.
- Explain changed parameters; undo and recover original.
- Seed every random mutation for reproducibility.
Spectral and organic features
- Waveform, spectrum, spectrogram, transient map, optional harmonic/noise separation offline.
- Convolution with user-recorded impulse responses in a later phase.
- Audio-to-modulation: map input dynamics to envelopes/filters, with safety limiting and privacy controls.
- Freeze/resample: turn an evolving patch into a playable new sample, preserve parent preset provenance.
- Multi-sampling wizard: record several pitches/velocities, auto-map with editable zones and round-robin.
Novel but feasible performance ideas
- Rhythm-to-Timbre: sequencer accents control the instrument synthesis itself.
- Contextual Timbre: instrument subtly changes brightness/resonance according to arrangement density.
- Acoustic/Electronic Blend: morph from recorded organic sound to synthetic counterpart.
- Microvariation Engine: deterministic humanized round-robin to prevent machine-gun repetition.
- Gesture Recorder: record macro knob movements as editable automation clips.
- Instrument Lineage: visual ancestry of derived presets and samples, with version compare.
- Adaptive Audition: play preset against current kaval/harmony loop rather than in isolation.

## 8. Integration with other EthnoFlow modules
- Harmony Engine publishes tempo, harmonic context, root/mode candidates and confidence; Rhythm Lab may use these but must not chase uncertain notes.
- Stable/Assisted/Creative accompaniment modes apply to rhythm/bass too; do not change drum groove on every imperfect flute note.
- Ambient Playground can host Instrument Forge presets as layers and automate their macros.
- Music Academy offers contextual tutorials: note values, rhythm, syncopation, ADSR, filters, synthesis, sidechain, polymeter.
- Live Instrument uses the same transport, MIDI clock and audio graph.
- DAW Export provides MIDI note events, CC/automation where interoperable, and per-track rendered stems.
- Project format stores references to versioned presets/assets with dependency validation.

## 9. Factory library and licensing
- Start with procedurally synthesized drums and original patches, plus a small curated collection of freely redistributable audio assets whose exact licenses and attribution are verified.
- Candidate sources to investigate: Freesound (filter by CC0 and verify each file), official open sample libraries with explicit redistribution grants, recordings made by the user. Do not treat all 'free download' samples as embeddable.
- Asset manifest must include origin URL/creator, license, license text or reference, modification notes, checksum, redistribution status, and attribution requirements.
- A generated patch can be original as a configuration; do not claim exclusive copyright over basic waveforms, generic synthesis techniques, or unlicensed source recordings.
- User-owned preset export includes dependency manifests. Offer both portable package with eligible embedded samples and reference-only package when licenses prohibit redistribution.
- Ship offline with a small factory library; additional packs optional, with explicit user consent for downloads.

## 10. Persistence, schema and portability
Define stable versioned formats, preferably JSON metadata plus binary audio assets:
```json
{
  "schemaVersion": 1,
  "id": "uuid",
  "name": "Wooden Liquid Kick",
  "category": "drum.kick",
  "author": "user",
  "engineVersion": "semver",
  "sources": [{"type": "sample", "assetId": "sha256:..."}, {"type": "oscillator", "waveform": "sine"}],
  "graph": {"nodes": [], "edges": []},
  "macros": [{"id": "punch", "value": 0.7}],
  "modulation": [],
  "midiMappings": [],
  "randomSeed": 12345,
  "dependencies": [],
  "tags": ["organic", "liquid-dnb"],
  "createdAt": "ISO-8601",
  "updatedAt": "ISO-8601"
}
```
This is illustrative, not a complete validated schema. Implement schema validation, migration, compatibility tests, robust error reporting, atomic writes, backups and recovery. Use content hashes to deduplicate samples; do not delete shared assets while referenced.
Preset entities: InstrumentPreset, PresetVersion, AudioAsset, SampleZone, Kit, Pattern, Scene, GrooveTemplate, AutomationClip, ExportJob, ProjectDependency. Preserve stable IDs across rename and duplication.
Library experience
Search by type, tags, mood, source, creator, favorites, date, acoustic/electronic, CPU cost. Audition in isolation or current project; compare presets; mark favorites; create custom collections; batch import/export; missing-file repair; history and backup. Default naming avoids collisions. Autosave drafts without corrupting approved versions.

## 11. Windows UX and accessibility
- Responsive high-DPI Windows UI, keyboard shortcuts, scalable panels, accessible labels and focus order.
- Real metering and readable parameter values/units; contextual Bulgarian tooltips with a beginner explanation and advanced technical description.
- First-run audio setup wizard: select device, test output, test microphone, buffer size, sample rate, MIDI device.
- Clear transport: play/stop/record/loop, tempo, time signature, metronome, count-in, panic/all-notes-off.
- Guard against hearing damage: output limiter and volume defaults, avoid sudden jumps when switching presets.
- Undo/redo for all destructive-looking edits; confirm overwrites; auto-recovery after crash.
- Performance modes: Eco, Balanced, Studio, with measured quality/cost tradeoffs.

## 12. Exports and interoperability
- WAV 24-bit PCM and 32-bit float, selected sample rates, mono/stereo; optional FLAC if library permits.
- Separate stems for each track, dry/wet options, effects returns, master mix, all aligned to bar 1/time zero and identical duration.
- MIDI Type 1 per track or project with tempo map, meter, notes, velocity, CC when supported.
- One-shots and loops with correct loop points, tempo/key metadata where meaningful.
- Render in realtime or offline, with progress and cancel; verify no clipping, truncated tails, missing assets, or drift.
- Preset package export/import with dependency manifest and license compliance.
- Later: plugin hosting/export only after explicit architectural and licensing review; don't promise VST3 creation in MVP.

## 13. Testing strategy and quality gates
Unit/property tests
- Every oscillator, envelope, filter, and effect produces finite samples and handles extreme inputs.
- Parameter bounds, smoothing, no NaNs/Inf, voice limits, deterministic seeded patterns.
- Sequencer correctness across odd meters, polymeter, ratchets, microtiming, probability and tempo changes.
- Serialization round-trip and migrations preserve audible settings.
- Missing sample and invalid preset produce clear errors, not crashes.
Audio integration tests
- Render reference patterns offline and compare timing/event counts; use perceptual/tolerance metrics rather than brittle exact floating-point comparisons across hardware.
- Measure note onset error in samples, timing jitter, dropouts/xruns, CPU, memory, peak level, DC offset and tail truncation.
- Compare realtime and offline renders for equivalent automation behavior.
- Verify choke groups, polyphony, preset swaps, sample rate changes, audio device disconnect/reconnect.
Usability and listening tests
- Beginner can create a new kick, edit it, save it, reopen it, and use it in a beat without documentation.
- User can record a wooden tap and create a playable original percussion preset in <= 10 guided actions (design target, not guaranteed result).
- Test on headphones and speakers, low and high buffer settings; subjective sound quality judged by listening panels and reference comparisons.
- User can create a 16-bar Liquid DnB groove, swap any instrument, save kit, reopen project and export aligned stems/MIDI.
CI and Windows builds
- Format, lint, static checks, unit/integration tests, signed or unsigned dev installer as appropriate, artifact checksums.
- Keep a small offline smoke test that records (with user permission), plays a test tone, and exports a short WAV.
- Never report a feature as complete without actual sound and passing acceptance tests.

## 14. Phased delivery — approval gates
Phase R0 — audit and audio baseline
Inventory the five existing instruments, reproduce current behavior, write architecture decision records, add automated smoke tests and benchmark baseline. Deliverable: audit + plan + known-working Windows build. Stop for approval.
Phase R1 — scalable instrument registry and library
Replace fixed instrument ceiling with dynamic registry, searchable instrument browser, preset storage and migration. Keep existing five sounds intact. Acceptance: add > 5 tracks/instruments, save/reopen, no regressions.
Phase R2 — Instrument Designer MVP
Dedicated page per instrument; sample/oscillator/noise sources, ADSR, filter, pitch envelope, gain/pan, audition, Save As, undo/redo, basic waveform. Acceptance: create two clearly different sounds, save and use in rhythm pattern.
Phase R3 — professional rhythm sequencer
Variable step lengths, per-step velocity, swing, microtiming, probability, ratchets, polymeter, Euclidean generation, kit mixer and basic pattern arrangement. Acceptance: 16-bar groove with fills; deterministic export.
Phase R4 — Sample Lab and organic capture
Mic/audio-interface capture, trimming, fades, transient detection, basic denoise, sample layering, dependency-safe asset management. Acceptance: record original hit -> make preset -> use in track -> reopen project.
Phase R5 — advanced synthesis and experimental instruments
FM, granular, resonator, modulation matrix, macro morph, multi-layer presets, round-robin, expressive MIDI. Acceptance: build an organic hybrid bass and a granular ambient percussion preset; pass CPU tests.
Phase R6 — generative rhythm and intelligent suggestions
Seeded pattern generation, groove extraction, style templates, lock/regenerate, ranking, A/B comparison and explanations. Acceptance: distinct musical variations with unchanged locked tracks.
Phase R7 — arrangement, automation and production export
Scenes, timeline, automation clips, aligned stems, MIDI, wet/dry rendering, preset packaging. Acceptance: import exports into an external DAW and verify alignment.
Phase R8 — polish, academy, accessibility, performance
Beginner tutorials, instrument-building wizard, Bulgarian help, preset lineage, audio stability, profiling, Windows packaging. Acceptance: novice completes full workflow; documented regression suite passes.
Each phase: (1) plan; (2) code; (3) tests; (4) real listening; (5) screenshots/logs; (6) changelog; (7) approval before next phase.

## 15. Non-goals and safeguards
- Do not train a large generative music model locally as a prerequisite.
- Do not copy proprietary plugin GUIs or copyrighted preset packs.
- Do not claim all user-created sounds are legally exclusive or automatically copyrightable.
- Do not invent instrument realism from a single crude oscillator and call it complete.
- Do not allow AI to silently replace or overwrite a user's preset.
- Do not introduce latency-heavy analysis into the audio callback.
- Do not make live accompaniment change key or chord because of a single mistaken kaval note.
- Do not make a visually elaborate interface with nonfunctional controls.

## 16. Initial delivery request to Claude Code
Read this file and the actual EthnoFlow repository. Do not start with Phase R5 or generate dozens of dummy presets. First deliver:
1. Current rhythm section audit, including screenshots or component map of the five instruments.
2. Proposed native audio engine and preset schema, with tradeoffs against the existing stack.
3. Detailed R0–R2 implementation plan, file-by-file estimates, migrations and rollback plan.
4. A working proof of concept: one existing instrument opens in its own designer page; edit its sound in realtime; Save As a new preset; add it to the rhythm section; save/reopen; export an audible WAV.
5. Automated tests and objective audio evidence (rendered file, measurements), plus a short listening checklist.
6. Stop and request approval before broadening to all instrument types.
Definition of success: The musician can gradually build a unique, durable, personally curated ecosystem of instruments and grooves, with production-quality control and no dependence on black-box AI.
