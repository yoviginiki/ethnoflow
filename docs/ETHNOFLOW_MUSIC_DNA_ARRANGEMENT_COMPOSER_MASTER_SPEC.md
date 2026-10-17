# ETHNOFLOW STUDIO — MUSIC DNA ANALYZER & INTELLIGENT ARRANGEMENT COMPOSER

Received from the owner on 2026-10-18 (written with Claude on the web). Kept verbatim.
Owner's framing: *"The problem is not that EthnoFlow cannot generate a Drum & Bass rhythm,
but that it generates a musical loop instead of composing and developing a whole song."*
Owner's instruction: do **Phase A only** (full audit of the existing code and audio
engine; why the current DnB sounds cyclic; architectures of the Analyzer, the Composer
and the Musical Director; UI blueprints; integration plan; feature/test matrix; staged
plan without simplification), **no code before approval**. Audit: `docs/music-dna/`.

---

Master implementation specification for Claude Code · v1.0 · 2026-10-17
Status: Product/engineering requirements, not an implementation claim. Integrate with the existing EthnoFlow codebase after auditing it. Preserve existing working features. This document extends the Master Plan, Ambient/Harmony Engine, Intelligent Harmonic Accompaniment Engine, Rhythm Laboratory/Instrument Forge, and Implementation Quality Contract.

## 0. Executive intent and non-negotiable outcomes
The current Drum & Bass generator produces loops, not engaging compositions. Build a full song-level composition system that listens to a user-owned/reference audio recording, creates an editable evidence-based structural map, and uses the map or original structural plans to arrange the user's OWN music. It must generate evolving arrangements, not merely repeat a 4/8-bar pattern with arbitrary fills. The user is a beginner learning music and plays expressive, sometimes imperfect Bulgarian kaval/flute. The system must be understandable in Bulgarian, but sophisticated enough for professional DAW export.
Deliver three tightly integrated products:
1. Music DNA Analyzer: offline reference track analysis and interactive map of song form, energy, rhythmic density, timbral novelty, spectral balance, bass activity, harmonic change, repetition, transition cues, and hypothesized sections.
2. Intelligent Arrangement Composer: timeline-based arrangement generator with meaningful musical arcs, motif transformation, sections, instrument entry/exit, micro/macro variation, transitions, and reusable arrangement blueprints.
3. Musical Director / Anti-Boredom Engine: transparent diagnostics, suggestions, and controlled re-composition that identifies excessive repetition and underdeveloped sections without destroying the musical identity.
Not allowed: hard-coded section 3 = drop, pretending ML labels are ground truth, copying melodies or stems from reference recordings, one random fill per 8 bars as the whole solution, fake sliders/buttons, fabricated audio QA, fake screenshots, or claiming feature completion without demonstrable output.

## 1. User stories and workflows
### 1.1 Analyze favorite track
- User imports MP3/WAV/FLAC/OGG where supported. Show track name, length, sample rate, channels, analysis status and error handling.
- Compute tempo candidates (including half/double-time ambiguity), beats, downbeats if available, time-signature estimate or 'unknown', section boundaries, feature curves, repeated regions, transition points.
- Display a multi-lane song map aligned in bars AND mm:ss, with zoom, scrubbing, loop playback and marker editing.
- Sections can be labeled manually: Intro, Establish, Build, Drop A, Variation, Breakdown, Bridge, Drop B, Outro, Custom. Machine-proposed labels show confidence and 'unverified'.
- User listens and corrects boundaries/labels; all edits persist. No destructive editing of the reference file.
- 'Extract blueprint' saves abstract structure: durations, intensity contour, rhythmic density, instrumentation roles, transition placement and change frequency; excludes copyrighted melodies/audio samples.
### 1.2 Compose original song from blueprint
- Start from analyzed blueprint, genre template, blank timeline, or guided wizard.
- Select DnB substyle: liquid, atmospheric, jungle, deep, neurofunk, minimal, experimental, Bulgarian ethno fusion; allow hybrid styles.
- Set target duration, BPM (default 174 as editable, not forced), key/mode or undecided, energy arc, complexity, kaval prominence, and desired density.
- Choose original instruments/presets from Instrument Forge, existing organic sample library and Harmony Engine; choose motifs or record one.
- Generate 3–8 arrangement candidates with different macro-forms, not just randomized notes; audition A/B and compare.
- Each result consists of real editable clips, notes, automation and event decisions. Changes to sections/length regenerate affected regions, not the entire song unless requested.
- User can lock sections, tracks, motifs, bass, chords, drums, samples, automation; regenerate only selected scope.
### 1.3 Record and build around imperfect kaval
- Preserve raw performance and expressive ornaments. Analysis must tolerate uncertain notes and variable timing.
- Offer free-time intro with drone/pad, then an explicit musical transition into grid-aligned DnB. Do not forcibly quantize the kaval.
- Analyze phrases and suggest entrance points, rests, response phrases, and sections that feature the kaval without continuously covering it.
- User can choose Follow kaval, Fixed arrangement, or Hybrid. A single wrong note never automatically changes whole-song harmony.
### 1.4 Beginner learning path
- Explain 'what is an intro/drop/breakdown/fill/automation/phrase/8 bars' in plain Bulgarian with clickable auditory examples.
- Every suggestion has 'Why?' explanation: 'This drop feels static because drum density, bass register and pad voicing have stayed nearly unchanged for 32 bars.'
- Guided Build my first DnB track: motif → foundation → groove → contrast → sections → transitions → mix → export. Offer a pro mode that hides coaching.

## 2. UX and visual contract — build actual product screens
### 2.1 Music DNA Analyzer screen
- Top: source metadata, transport, tempo candidates, analysis confidence, reanalyze controls, reference rights reminder.
- Center: horizontally zoomable bar/seconds timeline with labeled section rectangles, editable boundaries, waveforms, beat grid, detected downbeats and transitions.
- Feature lanes, toggle individually: RMS/loudness, onset density, bass energy, spectral centroid, spectral flux/novelty, harmonic chroma, rhythmic activity, section similarity, instrumentation estimates when supported.
- Bottom: selected-section inspector: time/bar start/end, confidence, notes, suggested role, audible change events, similarity to other sections, edit/merge/split.
- Right: Blueprint Extraction panel: map reference sections to abstract intent, select traits to borrow (form, dynamics, contrast), exclude audio/melody, save preset.
- Provide skeleton, empty, loading, failed, partial-confidence, and complete states. Every control must have behavior, persistence and tests.
### 2.2 Arrangement Composer screen
- Full-width multitrack timeline with colored blocks for song sections; track lanes for Kaval, Drone, Pads, Chords, Bass, Kick, Snare, Hats, Percussion, FX, Ambience, Countermelody and user-added tracks.
- Section inspector: length bars, target energy, rhythm density, tonal tension, primary motif, dominant instrument, transition in/out, variation budget, locked state.
- Variation drawer: A/B/C candidate preview, 'what changed', compare energy and instrument events, audition, accept/reject.
- Overview lane: target energy arc and actual computed energy; meaningful visual difference between build, drop, breakdown, reprise.
- Bottom: clip detail, piano roll, drum pattern editor, automation editor, transition editor, kaval phrase placement.
- Persistent controls: Undo/redo, save project, render selection, export stems/MIDI, seed and reproducibility, audio panic/stop.
### 2.3 Musical Director screen/panel
- Arrangement Health: section contrast, motif continuity, variation density, transition coherence, repetition burden, mix masking risk (heuristic), dynamic arc.
- Plain-language critiques with exact bar ranges and evidence; suggestion can be auditioned, applied, rejected or pinned.
- Never show unsupported 'professional quality 97%' pseudo-metrics. Explain that diagnostics are heuristic, not artistic truth.
### 2.4 Design quality
- Desktop-first at 1920×1080, usable at 1366×768, keyboard navigation, tooltips, high-DPI, dark theme, colorblind-accessible semantics.
- Use real screenshots from running Windows application and compare against UI blueprint; no stock image is an implementation reference by itself.
- No 'five controls pretending to be a DAW'. Make advanced controls discoverable in inspectors without overwhelming Beginner mode.

## 3. Music Information Retrieval pipeline
### 3.1 Ingestion and preprocessing
- Decode with supported libraries (e.g. soundfile/FFmpeg with explicit packaging and licensing checks). Normalize only analysis copy; preserve original bitstream.
- Analyze long files asynchronously, progress updates, cancellation, chunking, cache by content hash + analyzer version.
- Stereo-aware features where helpful; separate harmonic/percussive components as optional preprocessing. Detect silence and clipping.
- Track sample-rate conversions, feature hop length and all timing offsets precisely; no accidental drift.
### 3.2 Core features
- Tempo/beat candidates with confidence, explicit 87 vs 174 BPM ambiguity, beat and optional downbeat grid.
- RMS or perceptual loudness contour, spectral centroid, spectral rolloff, flux/onset strength, low-frequency energy, chroma, MFCC/timbre embedding, novelty curve.
- Self-similarity matrix and repeated-section search; detect repeated drops/refrains without presuming exact labels.
- Boundary detection using multiple complementary methods: novelty peak detection, recurrence/similarity, spectral/timbral segmentation, and optional pretrained models if licenses/performance permit.
- Optional source separation as a future capability: never imply perfect isolated drums/bass/vocals; artifacts and licenses matter.
### 3.3 Segmentation strategy
- Produce boundary proposals with confidence, then snap to plausible bar boundaries only when beat grid confidence supports it.
- Compare alternative segmentations (fine/medium/coarse) and let user choose resolution.
- Classify functional sections using transparent feature heuristics or trained classifier if available; allow 'unknown'. Position alone is NOT classification.
- Derive event annotations: kick/bass entry, rhythmic thinning, high-frequency sweep, silence/gap, loudness lift, harmonic reset, motif recurrence (with uncertainty).
- Keep labels and manual corrections separate from raw computed features. User corrections override predictions in project view.
### 3.4 Implementation guidance and code pitfalls
- Python libraries: numpy, scipy, librosa, soundfile; optional essentia if licensing/build works. Heavy models isolated in workers; Rust/Tauri UI/audio engine never blocks.
- librosa.segment.agglomerative(mfcc, k=8) does not magically label eight musical sections. Need feature standardization, correct axis/time alignment, endpoint insertion, tempo validation, and evidence-based labeling.
- Tempo return types differ by librosa versions; normalize scalar/array. Correct bar duration in 4/4 is 4*60/bpm, but bar alignment and meter are not guaranteed by a raw BPM estimate.
- Do not use exact fixed section count; auto-estimate with manual override and sensible bounds.
- Do not ship untested hard-coded Python snippets as production audio logic.

## 4. Blueprint data model and persistence
Define versioned, documented schemas, for example:
```json
{
  "schema_version": "1.0",
  "name": "Atmospheric liquid arc",
  "source": {"type": "reference_analysis", "reference_hash": "optional"},
  "tempo": {"bpm": 174, "meter": [4, 4], "confidence": 0.82},
  "sections": [
    {"id": "intro", "role": "intro", "bars": 16, "energy": [0.15, 0.3], "density": 0.2, "active_roles": ["drone", "ambience", "kaval"], "transition_out": "filtered_riser"},
    {"id": "build", "role": "build", "bars": 16, "energy": [0.3, 0.72], "density": 0.55, "active_roles": ["pads", "percussion", "bass_hint"], "transition_out": "one_beat_gap"},
    {"id": "drop_a", "role": "drop", "bars": 32, "energy": [0.82, 0.87], "density": 0.83, "active_roles": ["drums", "sub", "pads", "kaval"], "variation_events": [8, 16, 24]}
  ],
  "random_seed": 4312
}
```
This is a minimal example, not a complete rigid template. Store sections, event timelines, instrument references, automation, locks, seeds, rendered artifacts and provenance. Define migrations. References to samples and presets must remain valid across project reloads or show missing-asset repair UI.

## 5. Intelligent Arrangement Composer — actual music-level decisions
### 5.1 Hierarchical planning
- Song level: 2–6 minute shape (configurable), narrative/energy arc, section contrast, motif recurrences, primary instrument spotlight.
- Section level: role, instrumentation, rhythmic density, harmonic tension, bass strategy, variation schedule, transitions.
- Phrase level: 2/4/8-bar gestures, call/response, motif transformations, silence, fill preparation, pickup notes.
- Event level: MIDI notes, sample triggers, velocity, microtiming, automation curves, FX sends, clip placements.
### 5.2 Music grammar, not fixed rules
- Model meaningful transitions: establish → build → release → contrast → return; allow unconventional orders and different section lengths.
- 8/16/32 bars are defaults and templates, not mandatory; support 4, 12, 24, asymmetric forms and Bulgarian meters.
- Give musical functions to events: introduce, foreshadow, intensify, suspend, answer, surprise, resolve, reprise.
- Prevent overproduction: every section needs an audible purpose. Silence and negative space are legitimate composition tools.
### 5.3 DnB-specific groove intelligence
- 160–180 BPM editable, half-time perception, breakbeat ghost notes, kick/snare displacement, shuffle/swing, hats/shakers, rolling percussion, amen-inspired break techniques only with licensed/original source material.
- Groove families: liquid two-step, rolling, halftime breakdown, jungle syncopation, minimal deep, hybrid ethno. Each family has parameterized pattern grammar and variation strategies.
- Avoid unbounded random fills. Fills reflect phrase endings, tension, next section, density and motif; use fills sparingly with cooldown.
- Bass roles: sub foundation, moving liquid bass, Reese, call-response bass, bass dropout; sidechain interaction and mono low-frequency checks.
- Cross-section continuity: retain one anchor (motif, pad, bass note, percussion texture) while changing others to avoid incoherent jumps.
### 5.4 Motif transformation engine
- Operations: rhythmic displacement, augmentation/diminution, register shifts, inversion where musically appropriate, note omission, call-response, harmonic recontextualization, ornament reduction, timbral substitution, echo/resampling.
- Never automatically 'fix' folk ornamentation to Western scale; transformations respect selected mode and allowed microtonal settings.
- Track motif identity and recurrence so song sounds related but not identical. Offer transformation strength knob.
### 5.5 Variation planner
- Distinguish micro (ghost note, small velocity/filter change), meso (8–16-bar bass answer, alternate drum fill, pad voicing), macro (breakdown, new drop instrumentation, reharmonization, motif return).
- Use novelty budget, repetition threshold, phrase-aware placement, tension curve and probability with reproducible seeds.
- Do not change every parameter constantly; support sparse, meaningful evolution.
- Quantify repeated identical 4/8-bar windows, repeated automation shapes and unchanged track roster; produce suggestions with exact bar indices.
### 5.6 Transition designer
- Editable transitions: reverse cymbal, filter sweep, riser, downlifter, drum fill, bass cut, tape stop, silence, delay throw, atmospheric swell, reverb tail, organic recorded accent.
- Each transition has in/out bar positions, intensity, curve, length, affected tracks, FX preset, audio preview.
- Transition cannot clip, produce click, or drown kaval. Respect reverb tails and zero-crossings/crossfades.
### 5.7 Musical Director heuristics
- Repetition burden: identical rhythm/harmony/instrument state over too many bars, adjusted for ambient/minimal intentional repetition.
- Energy shape: compare target and actual approximate RMS, density and spectral activity, not only loudness.
- Contrast: difference in instrumentation, rhythm, register, harmony, ambience and dynamics between sections.
- Motif continuity: avoid complete randomness and disconnected sections.
- Space for lead: reduce competing midrange/lead lines when kaval active; suggest rests/call-response.
- Transition quality: audible continuity and intentional contrast, no unexplained cut unless desired.
- All scores configurable, inspectable, and dismissible; 'good' is not a single numerical truth.

## 6. Algorithms, optional AI, and control
Start deterministic/rule-based and signal-analysis-first. AI is optional for richer semantic section classification, timbre tagging, arrangement suggestions and personalized preferences; no mandatory cloud API. Model outputs are proposals with uncertainty and editable evidence.
Use seeded constrained search / beam search / evolutionary variants for arrangements: candidate score includes structure consistency, novelty, motif continuity, role compatibility, harmonic plausibility, instrument space and user preferences. Expose weights and don't train on user's references without explicit opt-in.
Add preference learning from accepted/rejected variations, stored locally and reversible. Distinguish 'reference influence on structure' from 'copying source musical content'.

## 7. Live audio, scheduling, and export
- Low-latency Rust audio thread schedules events sample-accurately or with documented precision; no blocking Python inference on callback.
- Analysis worker returns time-stamped results; composer converts plan into editable events/clips; audio engine renders deterministically from project state.
- Offline bounce full mix and individual synchronized WAV stems (same start, same length, sample rate, bit depth), optional MIDI per track, tempo/time-signature map, section markers as supported, project manifest.
- Allow render ranges, tail handling, silence at beginning, correct peak/headroom, clipping warnings, cancellation and progress.
- Exports to generic DAWs (Ableton, REAPER, FL Studio, Cubase, etc.) via standard audio/MIDI; don't promise native proprietary session export.

## 8. Proposed internal modules and API contracts
- ReferenceIngestService: import, decode, cache, provenance, metadata.
- BeatGridAnalyzer: candidate tempos, beat/downbeat events and confidence.
- FeatureExtractionService: curves with units/timebase.
- StructuralSegmentationService: candidate boundaries, repetition and novelty.
- SectionHypothesisService: editable labels, evidence and confidence.
- BlueprintService: source-independent arrangement abstraction.
- SongFormPlanner: macro section plan, bar counts, energy narrative.
- SectionOrchestrator: instrumentation and motif roles per section.
- GrooveEvolutionEngine: section-specific patterns and meaningful variations.
- MotifDevelopmentEngine: controlled transformations.
- TransitionEngine: bar-accurate musical transitions.
- MusicalDirector: diagnostics, explainable recommendations, user approval.
- ArrangementRenderer: event scheduling, preview and offline bounce.
- DAWExportService: stems, MIDI, markers, manifest.
For each public operation define typed input/output, errors, cancellation, persistence, CPU budget and test fixture. Build versioned contracts between Python analysis and Rust/TypeScript UI. Never couple an analysis label directly to an audio-thread callback.

## 9. Detailed control contracts (minimum)
For each UI control provide id, label BG/EN, units/range/default, input behavior, impact on audible result, automation support, preset serialization, undo/redo, disabled/error states, keyboard accessibility, test case.
Mandatory controls:
- Analysis: Analyze, Cancel, Tempo x0.5/x2, Correct beat grid, Add/Split/Merge section, Set label, Confidence overlay, Extract blueprint, Compare sections.
- Composer: Style, Target duration, BPM, Meter, Key/mode, Energy curve, Density, Variation strength, Surprise, Motif retention, Kaval priority, Generate candidates, Lock track, Lock section, Regenerate selection, Audition A/B, Apply candidate, Undo, Save preset.
- Musical Director: Scan, Show issue, Listen before/after, Explain, Apply fix, Ignore, Don't suggest again for this project.
- Export: Full mix, Stems, MIDI, Markers, Render range, Sample rate, Bit depth, Include tails, Open output folder.
No placeholder controls. If a feature is deferred, show explicit roadmap status outside production UI rather than dead buttons.

## 10. Quality assurance and reproducible test fixtures
### 10.1 Analysis tests
- 174 BPM synthetic beat; 87/174 ambiguity correctly surfaced.
- Songs with silence, rubato, abrupt drop, gradual build, repeated chorus/drop, asymmetric sections, odd meter, quiet ambient intro.
- Boundaries within reasonable tolerance for curated annotated fixtures; report precision/recall and timing errors rather than claiming perfection.
- Human-labeled ground truth for at least 10 varied tracks where licensing permits; never claim universal accuracy from synthetic samples alone.
- Check no arbitrary intro/drop naming solely by ordinal section number.
### 10.2 Composition tests
- For same seed and preset, identical MIDI/events and reproducible offline render (allow documented floating-point tolerances).
- 3 candidate forms genuinely distinct in section lengths, role events or arc; not cosmetic changes.
- At least 3 different sections have audibly and structurally different arrangement states.
- 32-bar loop is not just repeated 8 times: verify meaningful meso/macro events and intentional contrast.
- Locks are respected; regeneration of bass does not modify kaval audio.
- Odd meter and non-4/4 do not crash, bar mapping remains correct.
- Silence/space does not count as broken or empty by default.
### 10.3 Audio and integration tests
- No audio clicks at transitions (crossfades), no uncontrolled clipping, valid WAV/MIDI export, all stems sample-aligned.
- Load-save-load preserves timeline, automation, instrument references, seeds and user edits.
- User audio stays local unless explicit cloud action; no upload without consent.
- Profile long song analysis for CPU/memory and UI responsiveness; tests on Windows target.
### 10.4 Human listening acceptance
- Produce 3 fully rendered original 2–4-minute DnB demo tracks: (A) melodic liquid, (B) ethno kaval fusion, (C) deep atmospheric. Include WAV, MIDI/stems, arrangement JSON and a change-log of actual structural events.
- A/B compare baseline loop-only generator with new arrangement. Have listeners mark perceived monotony, transitions, identity and musical flow; report findings without fabricated scores.
- Provide screenshots from actual running app of DNA map, Composer timeline, Musical Director panel, export UI.

## 11. Delivery phases with hard acceptance gates
Phase A — Existing system audit, no code changes
Inventory current tracks, audio engine, rhythm patterns, instrument presets, persistence, automation, export. Capture real app screenshots and audio render. Map gap against requirements. Propose integration architecture. Stop for approval.
Phase B — Music DNA Analyzer foundation
Implement import, robust tempo candidates, feature extraction, novelty/recurrence segmentation, editable map and project persistence. Demonstrate on varied audio, confidence, correction workflow. No fake section semantics.
Phase C — Blueprint extraction and song form timeline
Versioned schema, source-independent abstract blueprints, section editor, energy arc, bar/time synchronization. End-to-end save/load.
Phase D — First genuine full-song arrangement
Generate structured multi-section track with existing Instrument Forge, Rhythm Lab, Ambient and Harmony Engine. Build actual clips/events, transitions, motif variation and preview. At least one complete 2–4-minute render with distinct sections.
Phase E — Musical Director and iterative variation
Explainable monotony diagnostics, scoped regeneration, A/B candidates, lock semantics, audition-before-apply and reversible edits.
Phase F — DAW-grade export and polish
WAV stems, MIDI, markers, headroom/tail validation, preset library, beginner tutorial, accessibility and visual QA. Regression suite and Windows installer.
Phase G — Advanced research (optional)
Optional local models for section role classification, beat/downbeat enhancement, learned arrangement ranking, preference adaptation, separation-assisted instrumentation estimation. No cloud requirement, benchmarks first.
At every phase: code + unit/integration tests + real audio artifacts + real screenshots + updated docs + known limitations + explicit pass/fail. Never mark phase done on UI alone.

## 12. Anti-simplification rules for Claude Code
1. Read existing repo and all EthnoFlow specifications before changing code.
2. Never silently reduce the scope. If time/token budget is insufficient, deliver a fully working vertical slice, document all pending requirements, and stop.
3. No fake 'AI analysis' generated from track position, no hardcoded 8 sections, no dummy waveforms, no static energy curves presented as measured.
4. Every audible parameter must connect to DSP/MIDI/audio state; every saved preset must roundtrip.
5. No code-only completion: require screenshots, audio examples, reproducible tests and traceability table.
6. Separate reference analysis from composition; abstract form may inspire a new original song, but don't reproduce source audio, melody, bassline or signature sound without proper rights.
7. User retains full control and authorship over their original recordings and created sounds; license of bundled samples must be documented.
8. Ask for approval after Phase A, and before major changes to audio architecture.

## 13. Implementation kickoff prompt
Read ETHNOFLOW_MUSIC_DNA_ARRANGEMENT_COMPOSER_MASTER_SPEC.md, ETHNOFLOW_IMPLEMENTATION_QUALITY_CONTRACT.md, and all current EthnoFlow specs. The current generator produces monotonous loops. We need a real Music DNA Analyzer, Intelligent Arrangement Composer and Musical Director. First do Phase A only: inspect actual code, map present features, record real audio and screenshots, identify gaps and architectural dependencies, create detailed screen blueprints and a requirements traceability matrix. Do not code yet. Do not simplify or substitute mocked features. Show an actionable implementation plan and wait for approval.

## 14. Suggested future expansions
- Arrangement fingerprint browser: compare energy/density/section shape of multiple references without copying musical content.
- Narrative prompt composer: 'start as lonely kaval in mist, grow into euphoric liquid drop, dissolve into forest ambience' maps to a transparent editable plan.
- Performance-aware breathing: allow kaval pauses to create responses, not constant backing saturation.
- Scene morphing: interpolate between A and B instrument/arrangement states over 4–32 bars, with control over which dimensions morph.
- Generative rehearsal: endlessly evolving but musically constrained accompaniment, distinct from finite exported song mode.
- Reference-informed teaching: click a transition and hear isolated original examples synthesized by EthnoFlow, with explanation in Bulgarian.
- Arranger conductor gestures: 'hold tension', 'one more phrase', 'drop now', 'thin drums', 'bring back motif' as safe quantized commands.
- Comparative timeline: overlay reference's abstract energy and section contour with original track, never raw source clips.

Definition of done: The user can load a reference song, see and correct a credible structure map, extract a non-copying arrangement blueprint, compose an original 2–4-minute evolving DnB song using own kaval/instruments, inspect meaningful variations, export synchronized stems/MIDI, and hear a substantial difference from a static loop. All of this works in the Windows application with verified sound, not merely mock UI.

---

## Owner's accompanying notes (2026-10-18, verbatim summary)

- Two corrections to an earlier Python approach: librosa can find structural change points but cannot reliably *name* each section Intro/Drop by its order; DnB arrangement is not strictly mathematical — 8/16/32-bar blocks are useful landmarks, not limits.
- Illustrative (not mandatory) target form of an original liquid DnB piece: Intro → Build → Drop A → Break → Drop B → Outro.
- Most important new capabilities: analysis of favourite songs into an editable structure map; several different arrangements from one musical idea; intelligent development of drums, bass, chords and motifs; professional transitions, automation, pauses, contrasts and climaxes; detection of excessive repetition with suggestions; locking favourite parts and regenerating only selected sections; integration with the kaval, Ambient Playground, Instrument Forge and Rhythm Laboratory; export of WAV stems and MIDI.
- Recommendation: build the Music DNA Analyzer and the Arrangement Composer as separate but connected modules, so the owner can study *why* favourite compositions work and use their structural principles without copying the works.
- Later idea: record several improvisations over the same composition, compare phrases and propose the best moments for a final arrangement (a "virtual producer").
