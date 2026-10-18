# Changelog

## Unreleased — Phase 2: pitch, notation & practice (in progress, 2026-10-16)

### Implemented
- **`crates/music-core`** (pure): Hz ↔ MIDI at a configurable A4; scales (7 modes,
  harmonic/melodic minor, hijaz, double harmonic, pentatonics, user-defined cents);
  tonic/mode hypotheses as a ranked probability list (never a single answer);
  **note extraction** from a contour (ornaments, slides and vibrato kept as bend data
  inside notes); **SMF writer** with pitch bend, RPN range, project or technical tempo.
- **Pitch detection** (`audio_engine::pitch`, YIN): per-frame estimate with
  confidence, streaming tracker, offline contour; **live pitch** in `RecorderStatus`;
  **`InputMonitor`** = tuner/meter on the input without recording.
- **Project**: `analysis/<take>.{contour,notes,notes.edited}.json` (atomic writes);
  `project.json` stores the user-chosen scale and A4 (optional; old files load).
- **Desktop**: tuner (note, cents needle, Hz, confidence, in-scale indicator) with the
  first practice "Hit the note"; "Analyse" → contour + proposed notes + scale
  suggestions with percentages (the user chooses, or none); piano roll aligned to the
  waveform with editing (↑/↓, Delete, S split, M merge, Ctrl+Z undo) saved separately
  from the automatic notes; A4 per project; MIDI export (project tempo/meter or a
  labelled technical 60 BPM with a README next to the file).
- Device tests now wait up to 2 s for WASAPI streams to start (flaky under parallel
  stream opening).
- **2.7a "Свири по ноти"** (0.2.1): `.mid` import via midly (ADR-0019) → `NoteEvent`
  + tempo map (format 0/1, tempo changes, running status, open notes); a MIDI file is
  a take (`midi_file`) rendered by the built-in synth (flute/sine/triangle voices,
  envelope, pitch bend, soft limiter) so waveform, playback, loop and export work
  unchanged; tempo ×0.25–2, mute per track, voice change re-render the take, never the
  file; **falling-notes view** with a piano keyboard, key names and in-scale colouring;
  **"Play the notes"** renders the current notes (extracted, edited or MIDI) with the
  synth — the first musical translator. Spec: `docs/MIDI_PLAYER_TRAINER_SPEC.md`.
- **"To the player"** (0.2.2): the current notes become a MIDI take of the project in
  one click (no export/import) and open in the falling-notes view.
- **F2.1 rest / F2.3 / F2.4 rest** (0.2.3): intervals, chord detection, transposition,
  grid quantisation in music-core; tuner "Hold the tone" (drift strip, seconds in
  tolerance); piano roll drag (move / resize), Alt+click to add, undo/redo;
  `docs/BENCHMARKS.md`.
- **F2.5 instrument profiles** (0.2.3): global profiles (`instruments/<id>/profile.json`
  + note samples), "Meet my instrument" wizard measuring each note from a held tone
  (≥ 1.5 s steady, median Hz, offset, stability), diagnosis (systematic vs random),
  sample playback with pitch shift, project attaches a profile; tuner shows the
  profile mark and the written note for transposing instruments.
- **F2.8 trainer** (0.2.4): `evaluate` in music-core (per-note verdicts, level
  tolerances, latency); "Trainer" panel driven by the microphone pitch — Listen, Wait
  (the lit note advances when held, wrong notes flash red, no timing), Rhythm (play
  along with the synth, per-note report at the end).
- **F2.7 rest** (0.2.5): MIDI take transposition (±24, re-render only); loop selection
  snaps to bars when the file has tempo and meter; **live synth** (`live_synth.rs`:
  8 voices, attack/release, oldest-voice stealing, commands through a lock-free ring,
  nothing allocated in the callback) played from the computer keyboard (two rows =
  two octaves, ← → octave, Scale Lock to the chosen scale).
- **F1.4.4 MIDI input** (0.2.6, ADR-0020 midir): choose a MIDI port in the device row;
  notes go to the live synth through a 2 ms forwarding thread and light the on-screen
  keys; the trainer takes a held MIDI key instead of the microphone when a port is open.
- **F1.6 audio graph** (0.2.7): `audio_engine::graph` — 16 pre-allocated layer slots
  with a `Source` trait, smoothed gain/pan/low-pass/high-pass (10 ms linear ramps,
  kept linear across blocks), constant-power pan, mute/solo, master gain and a
  look-ahead-free peak limiter (ceiling −0.3 dBFS), per-layer and master meters via
  atomics, `render_offline` on the same code path. `GraphPlayer` puts the graph on an
  output stream; sources are swapped through a lock-free ring and freed off the audio
  thread. Tests: sum/mute/solo/pan, no sample jumps on a gain ramp, NaN-free and
  deterministic offline render, limiter ceiling, and one hour of chunked rendering
  with the exact sample count and sine phase (f64 phase accumulator in `SineSource`).
- **F3.2 drone** (0.2.7): `DroneSource` — root/fifth/octave/sub voices of three
  detuned oscillators each, warmth (harmonics), movement (slow filter LFO), breath
  (slow swell), 100 ms root glide. The root is any frequency; the desktop "Drone"
  panel intonates it to the project's instrument profile (root and fifth take the
  measured cents of those notes) or to equal temperament at the project A4, and says
  which one it actually used. Root suggestion: scale tonic, else lowest calibrated
  note, else the user chooses (no built-in key). Presets Traditional / Deep Ethno /
  Cinematic; parameter changes never restart the stream. Not yet: "Liquid Resonator"
  (spectral freeze over a monochord), stereo motion, reverb ("space").
- **F3.1 explanations** (0.2.7): `music_core::explain` gives the facts behind each
  hypothesis (tonic and fifth share, coverage, the decisive pitch class against the
  nearest rival with a different note set, loudest off-scale classes); the analysis
  panel words them under every suggestion ("D holds 38 % of the time; B♭ is 60 % more
  frequent than B (vs D dorian); 96 % in the scale"). 11 modal fixtures in
  `qa/fixtures/modal/` (JSON note lists with the known tonic/mode) are a regression
  test: the known mode must be in the top 2 — currently all are #1.
- **F3.3 Liquid pad** (0.2.7): `PadSource` — up to six chord voices (two detuned
  polyBLEP saws + a sub sine each, smooth attack/release so chord changes crossfade and
  retunes glide), Chamberlin SVF with a slow LFO, small Schroeder reverb (pre-allocated),
  four emotion presets (calm / melancholy / light / dark = brightness, movement, warmth,
  space, slowness) and an X/Y macro on top (X brightness·movement, Y depth·space); a
  seed fixes the oscillator and LFO phases so a render is reproducible. The "Pad" panel
  builds the chord from a root and a voicing — mode-neutral ones (1-5-8, sus2, sus4,
  1-5-9) or 1-3-5(-7) from the chosen scale with its own cents — intonated like the
  drone. Drone and pad share one output stream (slots 0 and 1); the stream stops when
  both are off. Not yet: tempo-synced LFO, voicings from F3.4, stereo width.
- **F3.5 Compose First** (0.2.7): the "Accompanist" area opens "Prepare background":
  tonal centre and mode (saved as the project scale, so the tuner, keyboard Scale Lock
  and pad voicings follow), tempo and meter (incl. 5/8, 7/8, 9/8), three working styles
  (Traditional Drone, Emotional Liquid, Cinematic Ambient) that set the drone and pad at
  once — Deep Ethno DnB and Ambient No Key are shown as planned (phase 4 / 1A), not
  faked — "allow tension" (otherwise voicings with a note outside the scale cannot be
  chosen), and "Play & record", which keeps the background running and starts a
  microphone take in the studio right away. Drone and pad are now driven by
  `useBackgroundLayers` at the app level: they keep playing across screens, with a
  floating badge and a "Stop background" button.
- **F2.9–F2.11 error-tolerant performance intelligence** (0.2.7, offline on the loaded
  take): `music_core::phrase` splits notes into phrases (breath gap, held tone + energy
  drop, leap after a held tone, bar-line bonus; stable tones weighted by duration ×
  contour stability × metric position); `music_core::tracker` keeps a (tonic, mode)
  posterior over recent phrases with forgetting, hysteresis (consecutive phrases + margin)
  and locks that keep measuring without acting; `music_core::intent` classifies every
  note (stable / ornament / transition / noise / probable error, with confidence and
  reasons — an out-of-scale note is an ornament when short next to a held tone, a mode-
  change candidate when it comes back, a probable error only when single and held),
  estimates the intent per phrase and feeds the tracker intended pitch classes only.
  Modes: Off (literal), Stable, Assisted, Creative, with "tolerance to errors" and
  "sensitivity to changes" sliders; prepared-melody input (deviation against the expected
  note) and free improvisation. The desktop has the on/off switch with a visible mode
  badge (studio, trainer, Compose First), class colours in the piano roll and falling
  notes, a per-phrase "why the harmony stayed / moved" list, and marks on notes (correct /
  ornament / error) saved with the take that calibrate the classifier per user. The
  scenario benchmark (`crates/music-core/tests/performance_scenarios.rs`, numbers in
  `BENCHMARKS.md`): 0 false context changes in every intelligent mode on 100 % correct,
  80 % correct, blips, ornaments, ±30 ¢ intonation, slides, variable tempo, breath noise
  and new phrases; a deliberate mode change is accepted after 2–4 phrases; literal mode
  switches 3–16 times on the same material.
- **F3.4 harmonic strategies** (0.2.7): `music_core::harmony` turns the per-phrase
  intent and the tracked tonal context into three proposals — drone only, modal pads
  (one chord per phrase from the mode's own degrees, cents included), liquid chords
  (7ths, add9, sus, quartal) — each voiced in a register with voice leading, scored on
  melodic compatibility, modal fidelity, voice leading, tension target, rhythmic fit,
  register separation and variety, with avoid notes as soft penalties, a hold rule
  ("harmonic movement"), chord/mode/tonic locks and a low-confidence rule that keeps the
  chord and says so. Every chord carries its reasons ("the melody sits on F and A",
  "stays because the analysis is unsure"); seeded choices reproduce; wrong notes in the
  performance do not move the chords (test). The studio's "Harmony — proposals" panel
  shows the three cards with score bars, "why it works", "Play as A/B" (the pad and the
  drone follow the armed proposal during playback, changing only at phrase boundaries),
  seed and MIDI export of chords + bass.
- **Background level and linkage** (0.2.8, after the owner's first test of 0.2.7: "the
  pieces are not connected, no volume up/down"): one master level for drone + pad
  (−40…+6 dB, limiter still at −0.3 dBFS) in the floating background bar on every
  screen and in Compose First; the bar also switches drone/pad on and off and jumps
  between the accompanist and the studio; master default raised from −6 to 0 dB and
  layer levels up to +6 dB.
- **"My studio" — tracks and clips (F5.1, 0.2.9)**, pulled forward after the owner's
  next questions ("how do I hear two things at once", "where are the tracks", "no
  play/stop for the background"): `arrangement.json` next to `project.json` (tracks →
  clips, a clip = {start, offset, length, gain, fades, mute} window into an immutable
  take, up to 8 tracks, validated, saved atomically); `ClipSource` on graph slot 2 with
  a shared lock-free transport (play/seek/loop/end), smoothed track gain/mute/solo, no
  allocation in the callback — so the clips, the drone and the pad sound together
  through one master level and one limiter; per-take mono cache at the stream rate;
  offline mix bounce through the same graph code. The screen: ruler in seconds and
  bars (when the project has tempo and meter), up to 8 tracks (name, level, M/S),
  clips with waveforms, move / trim edges / split (S) / duplicate (Ctrl+D) / delete,
  fades and clip gain, snap to grid and edges, loop region dragged in the ruler, zoom
  (Ctrl+wheel) and scroll, undo/redo (Ctrl+Z/Y), **Record** adds a new take at the
  cursor onto the selected (or a new) track while the clips and the background play,
  export mix (WAV, studio format). Deleting a take removes its clips. **Background
  transport**: an explicit "Play background / Stop background" with drone/pad switches
  and level in Compose First, the studio transport, the floating bar and the track
  editor; "Play background" brings back whichever layers were on last. Not yet: track
  pan (stored, not audible — the clip source is mono per slot), MIDI clips as editable
  notes (a MIDI take is a rendered clip), automation, recording-latency compensation
  (F1.4), the background inside the exported mix (stems, F5.4).
- **Phase 1A — "Atmosphere" ambient playground (0.3.0)**, the first-release product of
  AMBIENT_HARMONY_SPEC: a rack of up to 8 synthesized layers on graph slots 4..12
  (noise colours, wind, resonator texture, FM/modal bells, Karplus–Strong strings,
  looped take) next to the drone, the pad and the tracks — one master, one limiter;
  controls per source type, level/pan/LP/HP/M/S/lock per layer, meters, tonal layers
  on the project's mode (profile intonation) or a seeded "no key" set; macros
  (intensity, brightness, motion, space = new master reverb, tonal gravity, width);
  Play/Stop (never auto-play), fade out, sleep timer, > 0 dB warning. **Presets**: 9
  factory racks, user presets as JSON, tag browser, search, favourites, recents, A/B,
  "Surprise me" within musical bounds with per-layer locks and undo, snapshot history.
  **Automation**: lanes as pure functions of time (LFO, seeded walk, keyframes,
  humanize) on layer and macro parameters, 25 ms clock while playing. **Scenes**:
  snapshots on a timeline with crossfaded transitions, loop, duration in bars when the
  project has a tempo, preview from any point, **offline bounce** of the program with
  optional stems per layer (10 min × 8 layers ≈ 19 s in release). **Sample Lab**:
  non-destructive recipe per take (DC, high-pass, gain, normalise, fades, loop
  crossfade, reverse) applied on load and in clips/loops, silence detection → trim to
  the sound, snap trim to zero crossings, tags (category, place, notes, mic, licence)
  in project.db v3, search in the take list, "→ Layer in the Atmosphere". Licensing:
  `docs/AUDIO-LICENSING.md`, `assets/ATTRIBUTION.md`, manifest + CI gate (no bundled
  audio: everything is synthesized). Not run: the owner's listening pass and the
  clean-machine walk-through (see ROADMAP "Phase 1A — phase report").
- **Phase 4 — rhythm section (0.4.0)**: `music-core` generators (groove, bass line on
  chord spans, ostinato with locked motifs, arpeggiator, countermelody in the rests,
  tempo estimate from onsets — all pure and seeded) + engine `BeatClock` (shared
  frame/beat/bpm anchor; `Source::render_at` carries the graph frame), synthesized
  `DrumSource` (kick/snare/ghost/hats/shaker, kick envelope for sidechain), mono
  `BassSource` (sub/pluck/Reese, glide, beat-synced wobble, sidechain ducking),
  `PatternSynthSource` (8 voices with cents). Shell: slots 12..16, one rhythm switch
  in the background transport, patterns rebuilt in place on the same clock, tempo
  changes keep the beat, "→ MIDI takes" (drums on GM channel 10), locks + regenerate,
  section markers, "Suggest tempo from the take", countermelody → MIDI take. UI: the
  "Rhythm section" panel in Compose First replaces the Phase-4 placeholders; the
  "Deep Ethno DnB" style is live. Deferred: CLAP spike (F4.4) pending the owner's
  approval of the dependency.
- **After 0.4.0, owner feedback (2026-10-17)**: breadcrumb with the screen name; the
  Atmosphere panels no longer overlap (collapsible side panels); import button in My
  studio; standalone metronome start/stop; the rhythm section as its own screen; the
  limiter badge keeps its place; rhythm patterns swap in place while playing (no chop
  when a slider moves); level floors are silence (layer off); per-strip rotary knobs
  with a "View: sliders / knobs" switch; factory layer names localized; **sample
  import in Atmosphere**: "Import samples…" (multi-file) or drag-and-drop from
  Explorer / a sample manager such as Soundly — each file becomes a take in the
  project (named after the file) and a looped layer.
- **Song transport (P3 Phase 1 slice, 2026-10-18)**: `transport_*` commands on top of the
  arrangement timeline with one position converted through the song map (ticks ↔ seconds ↔
  beats); a running rhythm section is re-anchored on play/seek and joins at the song's bar
  when started while the timeline plays; loop in bars; `TransportBar` (bar.beat, time,
  BPM/meter, loop) in the Song map and Rhythm screens, polled status in the store.
  Limitation: the rhythm clock holds one tempo; map tempo changes re-align at the next
  play/seek. The atmosphere program lane keeps its own time (next step).
- **Instrument Rack (P1 R3, 2026-10-18 morning)**: three new instruments — **Tapan**
  (boom/rim/ghost synth; strokes on the meter's accent groups, fills), **Chord pad** (chord
  pattern through two detuned polyBLEP saws, SVF with movement) and **Lead** (mono, glide,
  delayed vibrato, filter envelope, drive; phrase generator over the mode) — as extra
  parts of the rhythm section: up to 8 on graph slots 16..23 (graph widened to 24 slots),
  each with preset, level, seed, lock and generator knobs; live swaps in place; stems in
  the rhythm render and the song export; designer knobs, audition and WAV for the three
  kinds; nine factory presets. Deviation from the plan recorded in
  `docs/rhythm/IMPLEMENTATION_PLAN.md` (dedicated slots instead of a nested rack source).
- **References + analyzer (P2 Phase B, 2026-10-18 morning)**: new crate
  `crates/analysis` (rustfft, ADR-0021): STFT features, tempo candidates with ½/2× listed,
  DP beat tracking, novelty boundaries on a 0.25 s self-similarity grid (coarse/medium/
  fine), segment statistics and role *hypotheses* with evidence. New "Референции" screen:
  whole songs copied into their own library outside the projects, analysed only on request
  (worker thread), waveform + structure lane + curves, tempo/key chips, "take tempo",
  "take key", "copy structure into the song map" (bar counts at the chosen tempo);
  playback of any section through the audition slot. Bridge ops `reference_*`,
  `open_project`, `recent_projects`.
- **Fixes (2026-10-18 morning)**: offline rhythm render was silent for parts that use an
  instrument preset (level never set) — levels now follow the part's slider whatever the
  voice, test added; bridge `set_prefs` merges nested objects at every depth (a partial
  `rhythm.ostinato` no longer wipes the other fields), test added; stale Vite on port
  5173 after a killed dev run documented.
- **Song export (P2 Phase F start, 2026-10-18, night)**: "Експорт на микса" now renders the
  whole song through one offline graph (clips + drone + pad + atmosphere with its program
  + rhythm, buses, master space/shimmer, limiter) with aligned stems per group,
  `markers.csv` from the song map and `manifest.json` (tempo/meter maps, sections);
  fixes the audit defect that the export omitted the background. Window title fixed
  (mis-encoded dash).
- **Sample browser (backlog #6, 2026-10-18, night)**: screen "Моите семпли": folders
  (Soundly Audio suggested), recursive scan with WAV facts, search/sort, preview through
  the graph's audition slot (auto-stop), import as take / looped layer / cloud; scan test.
- **Resonator (F1B.4, 2026-10-18, night)**: modal resonator layer (6 voices × 4 two-pole
  partials, harmonic ↔ bell ratios, noise-burst exciter with hardness, brightness tilt and
  damping, fundamental decay), tuned to the pitch set like the bells, automatable
  (density, decay, structure, brightness, strike); tests: in tune, structure/brightness/
  decay measurable, silence kept, seed + block-size independence. In Atmosphere,
  Soundscapes (how-to) and the automation editor.
- **Song map (P2 Phase C / P3 Phase 1 slice, 2026-10-18, night)**: the song's form in
  ticks (`crates/project/src/songmap.rs`: tempo map, meter map, sections with role /
  energy / density / tension / roles / lock, validation, split/merge, migration from the
  seconds markers with bar snapping; tests across tempo and meter changes); stored in
  `project.json`, derived from markers until first edited, markers kept in sync; screen
  "Карта на песента" with a bar ruler, draggable bar-snapped edges, add/split/merge/lock,
  energy/density, transport position and "next section in N bars". Not yet: one beat
  transport for clips + rhythm + atmosphere (next P3 step).
- **Shimmer (F1B.3, 2026-10-18, night)**: the master space gained a transposed feedback
  path (delay-line pitch shifter with two windowed taps, soft-clipped loop): macro
  "Shimmer" 0..1 and a transposition choice (octave / fifth / octave+fifth / fourth) in
  Atmosphere and Soundscapes, automatable, saved with the rack; engine tests: the shifter
  doubles a sine's frequency; the reverb tail with shimmer is an octave brighter and
  still decays.
- **Program P1, R1 + R2 PoC (2026-10-18, night)**: instruments as presets with identity
  (`crates/project/src/forge.rs`: schema v1, validation, migration, fork lineage, 14
  curated factory presets that differ measurably); user presets in the library (kind
  `instrument`); rhythm parts reference presets (`instruments` in the rhythm settings,
  voices applied live and offline, legacy fields kept); **Instrument Designer** screen
  with real knobs on the engine voices, live audition on graph slot 3 (knobs heard at once),
  Save As / Update / delete, "use in rhythm", WAV render of the audition pattern; the bass
  engine gained attack/decay/sustain/release, filter envelope, pitch sweep and seeded
  microvariation with five measured tests; shell tests: every factory preset renders
  audibly and presets of one category render differently.
- **Program P1, R0 (2026-10-18)**: offline render of the rhythm section ("WAV + stems"
  in the panel, `rhythm_render` command and bridge op): same generators, same sources on
  an offline clock, deterministic per seed; test proves determinism, audibility, stems
  summing to the mix and seed sensitivity. `apply_live` split into `apply_params` (shared
  by live and offline). Owner approved the three program plans ("da").
- **Background mixer (2026-10-18)**: the on/off switches of the background became fades.
  Four buses (drone, pad, atmosphere, rhythm) in the shell (`bus_cmds.rs`), folded into
  every member slot: trim (−40..+12 dB), pan, low/high cut, fade in/out in seconds; the
  engine's slots gained `fade_ms` so a gain change ramps over the chosen time (test:
  1 s fade vs 20 ms default). UI: 🎚 popover from the floating bar with knobs, meters per
  bus and the master; part of the session, per project. Owner's words: "not on/off, that
  is crude — like a real mix".
- **Cloud module (F1B.2, 2026-10-17, night)**: granular cloud of a take (grain, density,
  position, spray, scan/freeze, pitch, jitter, reverse and octave probabilities) as a new
  atmosphere layer kind, automatable, with engine tests (continuity, octave = 2× frequency,
  density vs gaps, block-size independence); "Cloud from a take…" in Atmosphere, module +
  how-to in Soundscapes; loop/cloud layers share the take guards (delete, relink, presets).
- **Start from a melody, Help, user guide (2026-10-17, night)**: Home opens with "Start
  from a melody": import/record/pick a take → Analyse (on request only) → key, tempo
  and register proposals → five complete starters (quiet ambient, dark drone, liquid
  DnB, ambient DnB, ethno pulse) tuned to the melody, each with its facts (what was
  assumed and where it came from), Try / Save as session / Undo. **User guide**
  `docs/guide/bg.md` + `en.md`, rendered in the app (Home → Help) with contents and
  search; rule 12 in CLAUDE.md keeps it current.
- **Soundscapes screen (F1B.1) and assistant bridge (2026-10-17, night)**: the
  atmosphere rack one module at a time with big knobs, a how-to text per module, module
  presets, shuffle/lock, mini mixer and macro knobs; **assistant bridge**
  (`docs/ASSISTANT_BRIDGE.md`): the app watches `%APPDATA%\eu.ensodo.ethnoflow\assistant`
  — `state.json` snapshot, `inbox/` commands (set prefs, rack, layers, macros, presets,
  scale, timing, screen), `outbox/` replies; helper `scripts/assistant/ef.mjs`; switch
  in the Project menu.
- **Preset library everywhere (2026-10-17, later)**: every panel with sliders (drone,
  pad, rhythm section, harmony, intent, atmosphere) has "Save preset" (name, tags,
  note) and "Presets" (search by name/tag/note, sort by date/name, this project
  only, overwrite, edit, delete); a **"Session"** preset in the top bar saves all
  screen settings + the rack + which layers play; the **Library** screen (Home)
  lists everything across projects with kind/project/tag filters and loads an entry
  into its screen. **Every project remembers its own session** (`session.json` in
  the project folder, written 1 s after a change, restored on open; layers are
  remembered but never auto-started). **"Project" menu** in the top bar on every
  screen (new, open, recent, close; autosave note) and the project picker on Home.

### Measurements (owner's laptop, release build, YIN hop 10 ms, window 1024)
| material | pitch time | voiced | f0 p05 / median / p95 | confidence p10 / median | octave-sized jumps |
|---|---|---|---|---|---|
| kaval take 18.7 s, 48 kHz stereo, RMS −30 dBFS | 286 ms (65× realtime) | 35 % | 214 / 338 / 566 Hz | 0.86 / 0.97 | 21 / 621 (3.4 %) |
| `re.mp3` 59.45 s | 1998 ms (30× realtime) | 71 % | 272 / 330 / 377 Hz | 0.93 / 1.00 | 16 / 4151 (0.4 %) |

Synthetic (tests): pure tones < 1 cent at 44.1/48 kHz up to 1760 Hz; harmonic tones
(5 partials) < 2 % frames off by ≥ 100 cents; vibrato ±50 c at 6 Hz tracked (mean
< 10 c); 200→800 Hz sweep < 5 % frames off; silence/noise unvoiced.

## 0.1.0 — Phase 1: core recording & project system (2026-10-16, owner's Windows laptop)

### Implemented
- **Streaming recorder** (ADR-0014): takes go straight to a float32 WAV with the header
  checkpointed every second; `repair_wav_header` recovers a take after a crash; a take
  is never overwritten; live waveform while recording (10 ms peak bins in RAM only);
  write failure and device loss surface in the status and stop the take cleanly;
  noise-floor estimate; requested sample rate validated against the device.
- **Project system** `crates/project` (ADR-0015/0016): `<name>.ethnoflow/` with
  `project.json` (atomic save; tempo map and meter start empty) and SQLite `project.db`
  (migrations v1→v2, BLAKE3 checksums, trim points, settings). Takes are registered
  before recording and repaired on open; originals are verified on every open; delete
  moves to `trash/`; relink accepts only identical bytes; imports keep original bytes;
  derived takes (cut from a selection) link to their parent.
- **Metronome / transport**: sample-accurate click as a pure function of the frame
  (no drift, seek/loop safe), odd meters with groupings (7/8 = 2+2+3 …), count-in
  trimmed off non-destructively, loop range and click mute adjustable while playing.
- **Import** (ADR-0017): MP3/FLAC/Ogg Vorbis/WAV via symphonia, decode only.
- **Desktop**: home screen with the seven areas (only "Play & record" active; others
  disabled with their planned phase); project new/open/recent/close; take list with
  rename, delete (confirmation), relink, verify; device/sample-rate selects, test tone;
  tempo & meter panel; drag selection on the waveform → trim to selection / save as new
  take / loop; export defaults to 32-bit float; keyboard Space/R/Esc/arrows; busy
  indicator; commands run off the main thread; window close finalizes a running take.

### Tested
- Rust: 59 unit + 3 integration (audio-engine, incl. 30-min streaming simulation
  `--ignored`), 8 unit + 10 integration (project: reopen bit-exact, crash recovery,
  orphan adoption, checksum mismatch, relink, trash, import, trim, derived, v1→v2),
  4 (desktop Timing). All green on the laptop with real WASAPI devices; clippy clean.
- TypeScript: tsc clean; 11 vitest.
- Manual on the laptop: project create → record kaval (18.7 s) → stop → file header
  consistent with file size → import `re.mp3` (48 kHz stereo, 59.45 s) → playback;
  release build + NSIS/MSI installers produced; owner installed the NSIS build.

### Known issues
- Phase 1 definition-of-done items that need the owner at the instrument are **not yet
  run**: ≥3-min take → close → reopen checksum; kill-mid-recording → reopen recovery
  (covered by automated tests, not yet by a real crash); 30-min continuous record +
  playback with overruns = 0; clean-machine installer smoke test.
- Hardware log for the manual tests (microphone, headphones, latency feel) not yet
  recorded by the owner.
- After stop, the take is loaded whole into RAM for playback/peaks (streaming player
  is Phase 5); trim reloads the file.
- Debug (`tauri dev`) builds are 10–30× slower in DSP paths than the installer; judge
  speed on the installed build.
- No E2E (tauri-driver) tests and no CI matrix update yet (planned in TEST_PLAN).
- Node.js v24 on the laptop (CLAUDE.md says 22 LTS); no problem observed.

## 0.0.1 — Phase 0 (2026-10-16, built and tested in a Linux sandbox)

### Implemented
- Cargo workspace; `crates/audio-engine` (cpal 0.16, hound, ringbuf): device
  enumeration, recorder (RT callback → ring buffer → drain thread, peak/clip/overrun
  counters, 30-min in-memory cap), player (channel mapping + resampling once, atomic
  playhead), WAV float32/pcm24/pcm16 read/write, waveform peak reduction, linear resampler,
  `!Send` stream-thread wrapper.
- `apps/desktop`: Tauri 2 shell with 14 typed commands; React 19 + TS + Vite 8 UI
  (BG/EN), device selects, dBFS meter with peak hold, canvas waveform with time ticks and
  playhead, click-to-seek, record/stop/play, native import/export dialogs, export format
  choice, status/error banners, light/dark theme, keyboard focus styles.
- Fixtures generator (`gen_fixtures`), playback clock diagnostic (`play_timing`).
- Docs: vision, architecture, 13 ADRs, roadmap with Phase 1 plan, risks, test plan;
  CLAUDE.md with Windows commands and hand-off checklist.

### Tested
- 37 automated tests green (34 unit + 3 integration); 6 vitest; tsc and clippy clean.
- Real cpal stream path exercised through an ALSA `null` device: record→stop→buffer,
  play→finish with correct position; Tauri app launched under Xvfb, driven with xdotool:
  idle → record → stop; import fixture → waveform rendered → play (screenshots in
  `docs/phase0/`).

### Known issues
- Not verified on Windows / real microphone / real output clock (sandbox has no audio
  hardware). The ALSA null device has no clock, so timing, latency and overrun behaviour
  with real devices are unmeasured.
- No Windows installer produced here; `npm run tauri build` on the laptop is required.
- Take is held in RAM (30-min cap) until the Phase 1 streaming writer.
- Live waveform during recording shows only an elapsed-time bar; peaks appear after stop.
- Device list showed no named devices in the sandbox (ALSA null only) — verify WASAPI
  names appear on Windows.

### Phase 0 Windows verification
Run on the owner's laptop, 2026-10-16.

**Machine:** Windows 11 Pro 10.0.26200; Intel Core i5-12450H; 16 GB RAM; NVIDIA GeForce
RTX 4050 Laptop GPU + Intel UHD Graphics. Audio: Realtek High Definition Audio (SST) with
Nahimic APO components; active endpoints `Microphone Array (Realtek(R) Audio)` and
`Speakers (Realtek(R) Audio)`. No external audio interface. Microphone and headphones
used for the manual test: not reported.

**Toolchain:** Rust 1.99.1 (stable-x86_64-pc-windows-msvc, rustup 1.29.1), VS 2022
Build Tools 17.14 (VC workload), Node.js v24.20.0 / npm 11.19.0 (CLAUDE.md asks for 22 LTS;
no problem observed).

| step | result |
|---|---|
| `cargo test --workspace -- --nocapture` | 37/37 pass (34 unit + 3 integration); device tests ran against real WASAPI: `recorded 480 frames @ 48000 Hz x2 (overruns 0)`, `played … position_frames: 9600, finished: true, stream_errors: 0`, `device test: 9120 frames @ 48000 Hz x2 → 73028 bytes` |
| `play_timing` (3.0 s tone) | 3.35 s (first, cold), then 3.27 / 3.05 / 3.06 s. Device clock is correct; the extra ~0.3 s on cold runs is WASAPI stream open, which the example's timer includes. No code change. |
| `npm install` / `typecheck` / `npm test` | 0 vulnerabilities; tsc clean; 6/6 vitest pass |
| `npm run tauri dev` | app launched; owner reports recording works. Device names in the selects, meter behaviour, overrun count, clipping, latency feel, headphone playback, 24-bit export → re-import: **not reported in detail** |
| `npm run tauri build` | success (release compile 3 m 14 s). Present: `target/release/bundle/nsis/EthnoFlow Studio_0.0.1_x64-setup.exe` (1.76 MiB), `target/release/bundle/msi/EthnoFlow Studio_0.0.1_x64_en-US.msi` (2.45 MiB) |
| install + repeat manual test from installed app | owner installed the NSIS build and reports it works (no detailed measurements) |

**Issues found:** none in code. Open: manual-test details above not yet recorded; Nahimic
APO on this Realtek path may add latency (unmeasured).
