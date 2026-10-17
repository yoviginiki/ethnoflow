# ETHNOFLOW STUDIO — Ambient & Intelligent Harmonic Accompaniment
Product specification and implementation brief for Claude Code
Version 1.0 | Windows-first | Bulgarian-first UI | Local-first | Extension to EthnoFlow master plan
Автор: Ники (17.10.2026). Запазено дословно. Интеграцията с проекта: §15 най-отдолу и `FEATURE_SPEC.md`.

CLAUDE CODE INSTRUCTION: Read this document and the existing EthnoFlow master plan. Treat this as an additional module specification, not a replacement. First audit the repository and existing milestones. Produce an implementation plan, risks and a vertical-slice prototype. Do not claim completion until audio is audibly verified and automated tests pass. Do not copy proprietary assets or code from reference sites. Stop at each phase gate for approval.

## 1. Product vision
EthnoFlow is a Windows music-making and learning environment where someone can (A) build an evolving ambient accompaniment before playing an instrument, (B) record or play a melody and obtain an appropriate harmonic foundation, or (C) combine both workflows. It should be immediately enjoyable without knowing music theory, but capable of sophisticated editing and export to a DAW.
Primary example: a musician with a kaval or flute wants a beautiful drone, soft pads, subtle chord movement and organic textures, with optional Liquid Drum & Bass arrangement. The instrument's phrasing and microtonal ornaments must remain expressive; never force Western equal-tempered chords onto every folk melody.
The Ambient Playground is also a standalone instrument: users can listen, mix layers, move sliders, animate them over time, record field sounds, create soundscapes and export usable WAV files without recording a melody at all.

Success stories
1. Start with atmosphere: open app → select 'Mountain Mist' → audition eight layers → mute three → adjust sliders → animate a 5-minute evolving scene → save preset and export WAV + stems.
2. Play over it: select tonal center and mode or choose 'no harmony / free ambience' → enable metronome optionally → monitor microphone with low latency → record original kaval on its own track → export all parts separately.
3. Play first: record a 45-second kaval melody → analyze pitch contour, rests, ornaments and possible tonal centers → show uncertainty and alternatives → generate three compatible accompaniment proposals → compare, edit, export.
4. Build your own texture: record a stream or a wooden flute breath through a professional microphone → trim, de-noise non-destructively, loop, spatialize and save as a reusable sound layer.
5. Learn: hover on 'drone', 'fifth', 'add9', 'mode' or 'ostinato' to hear a short example and see a plain-language Bulgarian explanation.

## 2. Three first-class workflows
A. Prepare Background (Before Playing): manual tonic/mode, drone, chord pad, generative ambience, preset browser, 4–12 layered mixer, autoplay and scene automation. User can choose non-tonal ambience when unsure of key. A one-click 'Play & Record' starts the background and captures the acoustic instrument separately.
B. Analyze My Playing (After Recording): import/record, noise check, monophonic pitch estimation, note confidence, phrase segmentation, tonal-center hypotheses, mode candidates, harmonic suggestions, previews A/B/C. Explicitly distinguish detection from interpretation. Show a piano-roll and notation view; allow user correction of notes and tonic. Preserve the untouched source file.
C. Adaptive Live Accompaniment (Later): listen to the microphone and smoothly steer a prebuilt background. Start with slow changes at phrase boundaries, not instantaneous per-note chord changes. Manual 'lock tonic', 'freeze harmony' and emergency bypass. Strict low-latency requirements, no model calls or blocking operations on the audio callback thread.

## 3. Sound-generation modules
### 3.1 Intelligent Harmonic Accompaniment Engine (IHAE)
Inputs: source melody MIDI/pitch curve or manually selected tonic, scale/mode, note density, phrase boundaries, desired mood, harmonic complexity, instrument style, user feedback.
Outputs: (1) drone, (2) chord progression/pad, (3) ostinato/arpeggio, (4) countermelody (optional), (5) bass roots/phrases, (6) annotated explanation, (7) MIDI and rendered audio.
Submodules:
* Tonal Center & Mode Hypotheses: chroma/pitch-class statistics, stable long notes, cadence cues, melodic contour; rank alternatives with confidence and user override. Support modal scales, custom scales and cents deviations; don't assume D minor merely because D is common.
* Drone Designer: root, fifth, octave, suspended tones; evolving harmonics, breath, detuning, stereo motion; configurable beating and tension. Allow continuous pitch and alternate tunings.
* Harmony Planner: constrained chord vocabulary by mode, phrase-aware change rate, dissonance budget, voice-leading penalties, melody-note compatibility, open voicings, non-chord-tone tolerance, drone-first mode. Never label a subjective choice 'objectively correct'.
* Pad Voice Engine: envelopes, filter, subtle modulation, stereo, reverb, note-velocity curves and smooth chord transitions.
* Ostinato & Arpeggiator: rhythmic motifs, density, register, random seed, phrase-aware regeneration, locked motifs, swing.
* Bass Foundation: root-note options, movement constrained by harmony and style; optionally Liquid DnB sub bass / Reese bass with sidechain-compatible rhythm.
* Candidate Generator & Evaluator: generate 3–10 proposals with deterministic seeds; score voice leading, pitch clashes, density, melody masking, phrase alignment, user taste; preserve top variants and allow mixing components from different proposals.
* Explainability: display 'Why this works' in beginner Bulgarian: 'Тук Ре остава опорен тон, а Ла е квинта.' Show warning when analysis is uncertain.
Three core harmonic presets: Traditional Drone, Emotional Liquid, Deep Ethno DnB. Also 'Ambient No Key' for pure environmental textures.

### 3.2 Ambient Generator / Soundscape Instrument
A playable ambient instrument inspired by the interaction pattern of multi-slider sound generators such as myNoise, but with independently designed UI, code and sounds.
Layer rack: initially 8 layers, expandable to 16+; each layer can be an audio loop, field recording, noise generator, wavetable synth, granular cloud, drone oscillator, pad, resonator or sequenced texture. Drag to reorder. Per-layer mute/solo, volume, pan, pitch, filter, width, density, envelope, send effects, output bus, loop region, reverse (offline), crossfade, random seed. Provide sensible controls per source type rather than exposing meaningless knobs.
Global controls: master level, scene intensity, warmth/brightness, motion, space, tonal gravity, stereo width, randomness, tempo/free-time, limiter, headphone safety. Distinguish musical 'harmonic layers' from atonal field ambience. Save/reload without sound discontinuities.
Dynamic sliders: sliders alter sound immediately with smoothed parameter ramps, no zipper noise. Automation modes: static, slow random walk, LFO, keyframes, user-drawn curves, preset morph A→B, scene timeline, MIDI controller mapping. Range/period/depth/phase and 'humanize' controls. Use bounded seeded randomness; never change unexpectedly after reopening a saved project.
Scenes & Program mode: arrange sound states on a timeline: intro → mist → distant bells → warm drone → finale. Set each scene duration, transitions and per-layer automation; support free-running and beat-synced clocks. Preview at any point, loop sections, save as a named Program, render full program to WAV. A 20-minute ambient program must not require recording in real time: implement deterministic offline bounce after real-time MVP.
Musical generation: optional generative note patterns using selected scale, tonal center, interval restrictions, probabilistic gates and rhythm. Avoid random notes with no musical structure. Support evolving drone/chord changes, sparse bells, plucks and breath textures; preserve silence and dynamic range.
Preset browser: searchable tags (Forest, River, Night, Fog, Mountain, Ethno, Meditation, Dark, Warm, Liquid, No-key); favorite, recent, compare A/B, randomize gently, lock favorite layers, undo randomize, snapshot history. Ship 8–12 small high-quality presets; quality over quantity.
Listening UX: One-click Play, Pause, Stop, fade out, timer, looping, waveform/level meters, optional spectrum, safe initial volume. Never auto-start sound on app launch. Make it useful as a listening tool even with no DAW knowledge.

### 3.3 Organic Field Recording / Sample Lab
* Select microphone and audio interface (WASAPI shared/exclusive when available; ASIO support only if licensing/implementation permits), sample rate, buffer, mono/stereo, input gain and monitoring.
* Record 24-bit PCM WAV when hardware/backend supports it; offer 48 kHz as default. Protect from clipping; meters, peak hold, pre-roll, recording timer, waveform display, monitoring on/off.
* Trim, split, normalize with headroom, fades, DC removal, high-pass, optional de-noise, silence detection, spectral display (later), zero-crossing cuts and loop crossfade.
* Tag recording with title, category, location as optional user-entered text (never automatic precise geolocation), notes, mic and license. Maintain original file immutable; processing is non-destructive with versioned recipes.
* Layer designer: map samples to keyboard notes, granular density, playback rate, random start offset, envelopes, filters and convolution/reverb; render as preset and stems.
* Audio sample metadata: SHA-256, source path, creator, provenance, license, allowed redistribution, sample rate, bit depth, duration, channels, BPM/key if estimated, loudness and peak.
* Sound ideas: water stream, leaves, rain on wood, wind, kaval breath, tapping stones, wooden clicks, metal resonance, soft bells. Include anti-feedback monitoring safeguards.

### 3.4 Rhythm and arrangement extensions
* Optional Liquid Drum & Bass mode with 160–180 BPM defaults, adjustable tempo; drum break patterns, ghost notes, hats, shuffle, fills, kick/snare, sub bass, pads.
* Harmonic pad should work without drums; do not force everything into DnB.
* Arrangement engine can transition from pure ambient intro to DnB groove and back; export MIDI drum notes and stems.

## 4. User interface: Windows app
Home: Start Ambient / Play & Record / Analyze Melody / Learn Music / Open Project.
Ambient Playground: left preset/library browser, center 8 vertical channel strips with clear names and meters, right global macros and sound detail, bottom scene timeline and transport. Drag-and-drop layers. Large readable labels in Bulgarian, accessible keyboard navigation and tooltips.
Harmony Lab: audio/pitch timeline, tonic and mode candidates, chord/drone preview cards, compare variations, lock melody and generate alternatives.
Sample Lab: record panel, waveform editor, sound tagging and layer mapping.
Learning panel: context-sensitive mini-lessons and interactive audio examples. Beginner/Advanced toggle.
Export: mixdown WAV, per-layer WAV stems aligned from 0:00, MIDI where applicable, JSON project/preset, metadata/attributions. 16/24-bit PCM, 44.1/48 kHz, optional 96 kHz after testing, stereo/mono options; prevent clipping and include tail rendering.

## 5. Proposed architecture
* Desktop shell: Tauri 2 + React/TypeScript, Windows 10/11 x64, SQLite for metadata and projects.
* Real-time DSP: Rust audio engine (e.g. CPAL for I/O plus purpose-built graph); callback must be allocation-free/lock-free where practical, avoid disk/network/ML in callback; parameter smoothing, ring buffers and background workers. Consider dedicated proven audio DSP crates after evaluation.
* Offline analysis: Python service/sidecar for librosa, pitch detection and optional Basic Pitch; no Python install required for end user. Design IPC as versioned local API with progress, cancellation and timeouts.
* Notation/MIDI: choose suitable open-source notation and MIDI libraries after licensing review; preserve original timing, pitch bends and custom tunings.
* Data: versioned project schema, source recordings in asset store, relative paths, checksum, deterministic random seeds, undo history, migration tests.
* Audio graph: sources → per-layer DSP → buses → master limiter/meter → audio device; parallel offline render uses same DSP behavior when possible.
* Local-first: no cloud account or AI API required for core features. Optional AI recommendations must be opt-in and gracefully degrade offline.
* Accessibility: keyboard operation, screen reader labels, color-independent meters, scalable UI, high contrast.

## 6. Reference research and licensing
Study features, not assets or UI duplication:
* myNoise.net: layered mixing, evolving ambience and sliders; study user workflow, independently implement visuals, sounds and code. Do not scrape or bundle recordings.
* Sonic Pi: algorithmic music patterns and sample organization; review licensing separately for code and individual CC0 samples. Do not embed its complete engine without legal review.
* Freesound: discover sounds with explicit license metadata. Only prebundle individually verified CC0 files with documented provenance, or author original recordings/synthesis. CC BY requires attribution; CC BY-NC is not suitable for commercial distribution. Respect platform API terms and uploader preferences, particularly for model training.
* Other inspiration to evaluate: generative ambient tools, granular synthesizers, DAW clip automation and soundscape mixers. Create a comparative research note with screenshots only where permitted, concrete features, complexity and original design decisions.
* Maintain `assets/ATTRIBUTION.md`, machine-readable `asset-manifest.json`, license tests and removal mechanism for questionable assets.

## 7. Build-in sample strategy
Phase 1: 12–20 original synthesized sounds (noise colors, wind-like filtered noise, sub drone, harmonic pad, bells, soft pluck, filtered textures) + a few verified CC0 loops if legally safe. This ensures the app works immediately and is distributable. Never assume 'free download' equals redistributable.
Phase 2: sample-pack importer with license checker and attribution prompts.
Phase 3: original field-recording packs and user-created instruments.
Phase 4: optional generative audio models (local or API), after validating licensing, compute cost and user control. Algorithmic ambient is a first-class solution; AI is not mandatory.

## 8. Staged implementation plan with acceptance gates
P0 — Repository audit & runnable Windows skeleton: inventory existing code; document ADRs; basic window, audio device selection, one oscillator/noise source, play/stop, safe volume. Gate: real audible output and no crash on device switching.
P1 — 8-layer Ambient Playground: eight independently adjustable layers, mute/solo, slider smoothing, meters, master bus, 8 presets, save/reopen. Gate: no clicks during normal slider movement; preset reload matches saved state; 10-minute playback stable.
P2 — Automation & Programs: per-layer keyframes, slow random walk, timeline scenes, preset morphing, deterministic seed, save and replay. Gate: reproducible automation values at timestamps; correct scene transitions; offline test against reference rendering.
P3 — Sample Lab: input-device selection, record, WAV import, trim, fade, loop crossfade, tagging, asset library. Gate: recordings reopen correctly, no destructive source overwrite, no missing assets after moving a project folder.
P4 — Harmony Lab Manual: user-selected tonic/mode, drone root/fifth, chord pad, ostinato, three previews and explainability. Gate: MIDI notes comply with mode unless tension explicitly enabled; no hanging notes or audible discontinuities.
P5 — Melody Analysis: import/record monophonic instrument, pitch curve, candidate modes and tonic, note editor, confidence, compare 3 accompaniment options. Gate: curated tests with vibrato, silence, ornaments, microtones and noise; report error rates and allow manual correction.
P6 — Arrangement & DAW Export: optional Liquid DnB drum/bass, sections, WAV mix/stems, MIDI, project manifests. Gate: all stems sample-aligned, import into a target DAW correctly, stems recombine close to reference mix where effects/routing permit.
P7 — Beginner Music Academy: progressive lessons on drone, intervals, rhythm, mode, chords, sound design and making a song; interactive examples linked to real project controls. Gate: novice can finish an ambient program and a simple melody backing without external help.
P8 — Advanced Audio: granular textures, sample mapping, MIDI controllers, additional effects, high-quality resampling, extended presets, offline rendering performance improvements.
P9 — Live Adaptive Accompanist (R&D): latency budget, pitch confidence hysteresis, phrase-aware harmonic transitions, fallback modes, dropout recovery; benchmark before shipping.
Every phase: unit tests, integration tests, manual Windows audio smoke test, documented performance, release notes, screenshot/video or audio proof, and user approval before next phase.

## 9. Required automated tests
* Audio graph: sum/mute/solo, pan, gain, no NaNs/denormals, peak control, sample-rate changes, device reconnect.
* Sliders: parameter bounds, smoothing, automation reproducibility, morph interpolation, undo/redo.
* Looping: seamless crossfade, zero-length rejection, resampling, no drift over 1 hour.
* Projects: save/load exact preset state, asset hashes, schema migration, missing-file handling.
* Recordings: correct duration/channels/sample rate, clipping warnings, cancel-safe file writes, immutable originals.
* Harmony: tonic/mode consistency, user override, voice-leading, microtonal handling, drone-only mode, no forced chords.
* Export: duration, bit depth, alignment, tail length, silence handling, no clipping, MIDI timing.
* Security/privacy: local files by default, no unsolicited uploads, path traversal defense on sample imports.
* Performance: measure CPU, memory, glitches on target Windows hardware; avoid unsupported blanket claims of 'zero latency'.

## 10. Suggested data models
`Project(id, name, sample_rate, tempo?, tonal_center?, mode?, scenes[], tracks[], assets[], random_seed, version)`
`Layer(id, source_id, kind, gain_db, pan, filters, sends, automation[], mute, solo, loop, license_ref)`
`AutomationLane(parameter_id, interpolation, keyframes[], generator?, seed)`
`Scene(id, name, start, duration, snapshots[], transitions[])`
`AudioAsset(id, sha256, origin, creator, license, path, channels, sample_rate, duration, tags[])`
`HarmonyProposal(id, tonic, mode, drone_notes[], chord_events[], ostinato_events[], bass_events[], confidence, explanation, seed)`
`Recording(id, source_asset_id, edits[], mic_settings, take_number, timestamp)`

## 11. UX detail: sliders as an instrument
* Slider movement should be musically meaningful: e.g. 'Wind' controls amplitude, 'Distance' low-pass + reverb, 'Motion' modulates filter slowly, 'Harmony' mixes tonal pad; advanced controls reveal exact DSP parameters.
* Every slider has double-click reset, numerical readout, optional automation arm, MIDI learn, right-click help, accessibility label.
* A macro can control multiple parameters with calibrated curves and safe bounds; support per-layer overrides.
* 'Surprise me' randomizes within compatible musical ranges; 'Keep what I like' locks selected channels.
* A 'Sound DNA' preset stores layer sources, parameters, automation, scene structure and license references; never depend on hidden online services to reopen it.

## 12. Out-of-scope for the first release
Do not implement a full commercial DAW, generative AI audio training, collaborative cloud accounts, live adaptive harmonization, VST hosting, 3D visualizers or marketplace before P0–P6 are stable. These may be future modules, not blockers for the first playable version.

## 13. Claude Code execution prompt
1. Read existing master plan and this spec. Inspect repository and detect actual implementation state; never assume files or dependencies exist.
2. Create `docs/ETHNOFLOW-AMBIENT-ARCHITECTURE.md`, `docs/DECISIONS.md`, `docs/PHASES.md`, `docs/AUDIO-LICENSING.md`.
3. Compare 2–3 real-time audio engine options for Windows and explain the tradeoffs; choose one and justify licensing.
4. Implement only P0, then P1 after approval. Deliver a working audible vertical slice before adding AI.
5. For every phase include: files changed, setup/build commands, automated test results, manual listening checklist, known limitations and next steps.
6. Prefer deterministic procedural synthesis and original sounds to questionable third-party sample packs.
7. Keep the UI in Bulgarian with English localization-ready identifiers. Make novice-friendly controls the default, with advanced panels available.
8. Ask before destructive repository operations, paid APIs, external downloads, or any asset with uncertain redistribution rights.
9. No mock buttons or fake meters presented as completed functionality. If a feature is not implemented, disable it and label it 'Планирано'.
10. When the phase is complete, stop and wait for explicit approval.

## 14. First release definition of done
On a clean Windows 11 machine the user installs EthnoFlow, selects an audio output, presses Play, hears a beautiful evolving 8-layer soundscape, adjusts sliders without pops, saves a Program, records a microphone sound, adds it as a loop, exports a stereo WAV, and can reopen the project offline. The next milestone adds manually selected drone/harmony and melody-driven proposals. The result must sound musically convincing, not merely satisfy UI tests.

---

## 15. Как се вписва в EthnoFlow (интеграционен план)

### 15.1 Най-важната промяна: Ambient Playground влиза рано
Този спец поставя 8-слойния Ambient Playground като **първия звучащ продукт** (P1–P2),
преди анализ на мелодия. Това е добро решение: не изисква pitch detection, само
Transport и аудио граф, и дава „музикално убедителен" резултат най-рано. Затова във
`FEATURE_SPEC.md` се добавя **Phase 1A — Ambient Playground** между Phase 1 и Phase 2.
Първият release (§14) = Phase 0 + 1 + 1A.

### 15.2 Съответствие на фазите
| този спец | EthnoFlow | бележка |
|---|---|---|
| P0 skeleton, device select, oscillator, play/stop | Phase 0 (готов) + F1A.1 | един осцилатор/шум през аудио графа е първата стъпка на 1A |
| P1 8-layer playground | **Phase 1A** (F1A.1–F1A.3) | |
| P2 automation & programs | **Phase 1A** (F1A.4–F1A.5) | |
| P3 Sample Lab | Phase 1 (F1.1, F1.4, F1.5) + F1A.6 (tagging, asset library, loop crossfade) | записът вече е Phase 1 |
| P4 Harmony Lab Manual | Phase 3 (F3.5 Compose First, F3.2, F3.3, F4.5 ostinato се тегли в Phase 3) | |
| P5 Melody Analysis | Phase 2 (F2.2, F2.4) + F3.1, F3.4 | |
| P6 Arrangement & DAW export | Phase 4 + F5.4 | |
| P7 Academy | Phase 6 | |
| P8 Advanced audio (granular, MIDI controllers, SRC) | Phase 5 + F1A.7 (granular) | |
| P9 Live adaptive | Phase 8 | |

### 15.3 Къде проектът решава различно (и защо)
1. **Python sidecar за анализ**: ADR-0011, първо Rust pYIN, после ONNX, Python само като
   fallback с отделен ADR. Спецът иска „no Python install for end user", което е същата
   цел; разликата е само в реда на опитите.
2. **Исканите документи**: `ETHNOFLOW-AMBIENT-ARCHITECTURE.md` → секция в
   `ARCHITECTURE.md` (аудио граф, слоеве, автоматизация); `PHASES.md` → `ROADMAP.md` +
   `FEATURE_SPEC.md`; `DECISIONS.md` вече съществува; **`AUDIO-LICENSING.md` се създава**
   (нов, задача F1A.8) заедно с `assets/ATTRIBUTION.md` и `asset-manifest.json`.
3. **Сравнение на 2–3 аудио engine-а**: направено в ADR-0003 (cpal срещу JUCE sidecar
   срещу raw WASAPI); ще се допълни с DSP crate оценка (`fundsp`, `dasp`) в F1A.1.
4. **Само myNoise като модел за взаимодействие, не за код/звуци/UI**: записано като
   правило в F1A; семплите са синтезирани оригинални + проверени CC0.
5. **Ambient No Key preset** и „non-tonal ambience when unsure of key" влизат като
   first-class режим, което съвпада с мастър плана (без насилствена тоналност).
