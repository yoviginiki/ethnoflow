# Assistant bridge — driving the running app from the terminal

Owner request (2026-10-17): *"every control, every preset, every setting should be
controllable by Claude Code from here; I play a melody, write a prompt, and Claude
Code makes the starting presets for the song; I can ask what is what and learn."*

The running app watches a folder. A coding assistant (Claude Code in this repo) reads
the app's state from one file and drops command files into an inbox; the app applies
them and answers in an outbox. Files only, no network, nothing leaves the machine.
The user can switch it off in the **Project** menu (🤖 "Bridge to Claude Code").

```
%APPDATA%\eu.ensodo.ethnoflow\assistant\
  state.json          snapshot the app rewrites ~1.5 s after anything changes
  inbox\<name>.json   one command per file; the app deletes it when taken
  outbox\<name>.json  the reply: {"ok":true,"result":…} or {"ok":false,"error":"…"}
```

Helper (Node, no dependencies): `scripts/assistant/ef.mjs`

```
node scripts/assistant/ef.mjs state
node scripts/assistant/ef.mjs send '{"op":"ping"}'
node scripts/assistant/ef.mjs send-file cmds.json      # one command or an array
```

In Claude Code's Bash tool the JSON quoting is easiest with a file: write `cmds.json`
to the scratchpad with the Write tool, then `send-file`.

## State (`state.json`)

`{ schema, at, lang, screen, project, loaded_take, prefs, layers, metronome, ambient,
rhythm_status, ambient_status }`

- `project`: `name`, `root`, `scale` (tonic + degrees in cents, or null = no key chosen),
  `a4_hz`, `timing` (bpm, numerator, unit, grouping), `markers`, `instrument`, `takes`
  (id, number, label, duration, source, …). Analysis files of a take (contour, notes)
  live under `<root>/analysis/<take>.*.json` and can be read directly.
- `prefs`: `drone`, `pad`, `intent`, `harmony`, `rhythm` (incl. `instruments`: preset ids per
  part, e.g. `"factory:bass.reese"` or a library entry id), `masterDb`, `stripControl`, `mixer`.
  Shapes = the TypeScript interfaces in `apps/desktop/src/store.ts`
  (`DronePrefs`, `PadPrefs`, `RhythmPrefs`, `HarmonyPrefs`, `IntentSettings`).
- `layers`: which background layers play (`drone`, `pad`, `ambient`, `rhythm`).
- `ambient`: the atmosphere rack (`Ambient` in `api.ts`: layers with `source.kind`
  noise/wind/texture/bells/pluck/loop/cloud, macros, automation, program). A `cloud`
  (granular) or `loop` layer needs `take_id` of a take in the project.

## Commands (`inbox`)

| op | fields | effect |
|----|--------|--------|
| `ping` | | `{pong:true, schema}` |
| `state` | | the snapshot, in the reply |
| `message` | `text` | shows a line in the app's status bar (say what you did) |
| `screen` | `screen`: home/play/tuner/compose/studio/ambient/soundscapes/rhythm/live/library/instrument/songmap/samples/references/academy/create/start/help (every area is a screen with tabs: play = play/tuner, background = compose/ambient/rhythm/instrument/live; `soundscapes` is an alias that opens Atmosphere in its modules view, song = studio/songmap/create/start, learn = academy/references, library = library/samples) | navigates |
| `area` | `area?`: play/background/song/learn/library | without `area`: lists the five Home areas, their tabs (`tabs`: area id → screen ids) and the current one; with it: opens the area's main screen (play → play, background → compose, song → studio, learn → academy, library → library), returns `{area, screen}` |
| `sample_folders` | `add?`, `remove?`: arrays of absolute folder paths | Library › My samples: the scanned folders (`prefs.sampleFolders`); without fields just lists them; returns `folders` |
| `scapes` | — | Soundscapes 2: the world ids, the journey in the rack now (`current`, `phase` {index, role, name, progress, remaining_s, end_s}, `playing`, `position_s`) |
| `scapes_build` | `world`, `minutes?` (8), `seed?`, `loop?`, `play?` | builds the world's journey into the Soundscapes rack (its own rack, slots, bus and stem; the atmosphere is untouched; returns `previous` for a restore) with 4 scenes and 3 breathing lanes; `play: true` **starts sound** (atmosphere on, program from 0) |
| `scapes_stop` | — | stops the program and the Soundscapes layer |
| `set_scapes` | `ambient`: a whole rack | replaces the Soundscapes rack (same shape as `set_ambient`; `scapes_build` returns `previous` for this) |
| `progression` | — | the chord progression (genre, seed, bars, steps, follow, edited), its labels (I, ii, V7…) in the project's scale and the chord of the current bar |
| `progression_set` | `generate?: true` + `genre?` (rnb/rock/pop/dnb/liquid/ambient/ethno/jazz), `seed?`, `bars?`, `bars_per_chord?`, `drums?: true`; `steps?`: [{degree 1..7, quality, bars}], `follow?` | generates or edits the progression; `follow` only acts while the song or the rhythm runs (no sound by itself); `drums` sets the rhythm's drum style to the genre's |
| `chord_pitches` | `midi`: [MIDI notes] or null | the chord the racks follow (normally set by the progression / live follower); racks with `follow_chord` re-pitch their tonal layers at once |
| `rack_pitches` | `rack?`: ambient / scapes | what every layer would play now (Hz per layer, empty for non-tonal), also while stopped; plus `follow_chord` |
| `rack_history` | `rack?`: ambient (default) / scapes, `undo?` / `redo?`: count, `jump?`: step index | the rack's edit history (steps with labels: knob, new layer, scene, journey, preset, assistant); undo / redo / jump apply that state without a new step; returns `index`, `count`, `labels`, `layers` |
| `scapes_seek` | `position_s?`, `playing?` | jumps the journey and/or runs / pauses its program (only while the Soundscapes layer is on: the program clock lives with the running rack); returns `position_s`, `playing`, `end_s` |
| `continue` | — | Home's "▶ Continue": opens Play & record with the latest take loaded (no takes: empty, ready to record), no sound; returns `screen`, `loaded_id`, `latest_id` |
| `set_prefs` | `prefs`: any of the pref keys, partial objects merge | drone/pad/rhythm/harmony/intent/master |
| `layers` | `drone`/`pad`/`ambient`/`rhythm`/`scapes`/`metronome`: bool | **starts or stops sound** — tell the user first |
| `set_ambient` | `ambient`: a whole rack | replaces the rack (loop layers of missing takes are dropped) |
| `set_macros` | `macros`: partial `Macros` | intensity, brightness, motion, space, space_size, tonal_gravity, width |
| `add_layer` | `kind`, `name?`, `source?` (partial), `patch?` (gain_db, pan, lowpass_hz, highpass_hz…), `take_id` for loop | adds a module (max 8) |
| `patch_layer` | `id` or `name`, `patch` (layer fields; `patch.source` merges) | changes one module |
| `remove_layer` | `id` or `name` | removes it and its automation |
| `save_preset` | `kind`, `name`, `tags?`, `notes?`, `payload?` (`"current"` or omitted = what is set now) | library entry; kinds: session, module, drone, pad, rhythm, harmony, intent, ambient |
| `list_presets` | `kind?`, `project?`, `query?` | entries (search covers name, tags, notes) |
| `load_preset` | `id`, `go?` (default true: open its screen) | applies it |
| `session` | `payload` (a session payload), `restore_layers?` | applies all settings at once |
| `rhythm_render` | `path`, `bars?`, `stems?` (default true) | offline WAV + stems of the current rhythm settings (audio evidence) |
| `clap_scan` | `folders?` (default: the CLAP standard folders) | `.clap` files and the instruments they export |
| `clap_render` | `path`, `plugin_id`, `out`, `midi?` | renders a two-bar demo phrase through the CLAP instrument into a WAV (proof it plays in the graph) |
| `transport_status` | | song position: playing, source, seconds, tick, bar, beat, bpm, meter, loop_bars |
| `now_playing` | — | what sounds now, as the bottom bar shows it: `recording`, `take` {id, position_s, duration_s}, `notes`, `background` {drone, pad, ambient, rhythm, any}, `song` {bar, beat, seconds, source}, `metronome`, `live`, `any` |
| `stop_all` | — | stops every engine except a running recording (take, notes, background layers, song transport, metronome, live follower, previews); returns the snapshot after |
| `load_take` | `id?` (default: the latest take) | loads a take the way the ▶ on the take list does (waveform, analysis, MIDI view), no sound; returns `loaded_id`, `name`, `duration_s`, `analysed`, `notes` |
| `play_take` | `id?`, `from_s?` (default: the latest take from its start) | **sound**: loads if needed and plays the take on the chosen output; `stop_take` stops it and returns the cursor |
| `transport_play` | `from_s?` | **starts the song timeline (sound)**; a running rhythm is re-anchored to the same bar |
| `transport_stop` | | pauses the timeline (background switches stay) |
| `transport_seek` | `seconds` | moves the song position (rhythm follows) |
| `transport_loop` | `start_bar`, `bars` (omit both = off) | loop in bars, stored in the arrangement |
| `reference_phrases` | `id` | extracts melodic phrases (pitch track → notes → phrases) and returns them with compact notes (s, d, m, c, v) |
| `reference_phrase_to_take` | `id`, `index` | puts phrase `index` into the open project as a MIDI take |
| `academy_lessons` | | all lessons (id, level, track, exercise type, passed) + the exercise context (tonic, kind, range) |
| `academy_instance` | `id`, `seed?` | a concrete, reproducible instance of the lesson's exercise (prompt notes, choices, grid…) + demo notes |
| `academy_score_rhythm` | `onsets`, `grid`, `level?` | scores claps against the grid (deviations in ms) |
| `academy_record` | `id`, `score`, `passed` | records an attempt in the local progress |
| `academy_reset` | `id?` | clears progress for one lesson (or all) |
| `make_song` | `style?`, `take_id?` | builds the song from a take: scale, tempo/meter + background by style, song map from phrases, melody track, measured balance; returns the decisions |
| `live_start` | | starts the live accompanist on the microphone with the current intent/harmony prefs (needs a project) |
| `live_tick` | | the follower's status (context, harmony, counts, recent notes) and new decisions |
| `live_set` | `locks?`, `bypass?` | locks tonic/mode/chord/drone; emergency bypass |
| `live_stop` | | stops following and releases the pad/drone |
| `trainer_history` | `limit?` | recent trainer results (rhythm reports, drill passes), newest first |
| `trainer_log` | `entry` | appends a trainer result (what the Studio trainer does after each report) |
| `fingering_overrides` | `instrument?`, `overrides?` | without `overrides`: the user's chart corrections; with: validates and saves them (holes = 8 of c/o/x/h) |
| `error_clear` | | clears the app's error banner |
| `create_project` | `name`, `style?` (one of the nine starter ids = a template) | closes the current project and creates a new one |
| `instrument_list` | | instrument presets (id, name, category, factory) |
| `instrument_render` | `id`, `out`, `seconds?`, `midi?` | renders a preset's audition pattern to a WAV (silent proof per instrument) |
| `instrument_audition` | `id`, `seconds?` | **plays** a preset through the designer's audition path for a few seconds |
| `reference_remove` | `id` | removes a reference's library copy |
| `import_audio` | `path` | imports a WAV/MP3/FLAC/OGG, a `.mid`, or a MusicXML sheet (`.musicxml`/`.xml`, uncompressed) as a take; reply: take_id |
| `tuner` | `a4?` (Hz 400–480 or null = the project's), `instrument?` (free/kaval-d/shakuhachi-d/guitar/monochord), `lock?` (MIDI or null) | sets the Tuner screen prefs and returns them plus the live reading (hz, confidence, target, cents, direction) while the tuner listens; `min_hz` = the microphone's current range floor (70, or 30 in the bass range the screen requests for the monochord bed / a locked note under 100 Hz) |
| `arrangement` | | the studio arrangement (tracks, clips, loop) |
| `set_arrangement` | `arrangement` | replaces it (validated); track strips: gain_db, pan, lowpass_hz, highpass_hz, send |
| `set_song_map` | `song_map` (whole SongMap) | replaces the project's song map (validated) |
| `what_next` | | the "What next" hint of the header: `{key, screen, text}` |
| `keyboard` | `on?`, `source?` (voice \| instrument \| pad \| drone \| module:ambient:N \| module:scapes:N), `octave?` (24..96), `lock?`, `velocity?`, `midi_input?`, `bend?` (semitones ±24), `mod?` (0..1) | the keyboard dock above the Now-playing bar: reads / sets it; returns `on, source, octave, lock, velocity, midi_input, midi_ports, synth_running, designer_voice, held` |
| `note` | `midi`, `on?` (default true), `velocity?` | **sound**: one key where the dock points (the project's voice or the instrument being edited) |
| `notes_off` | | releases everything the keyboard holds |
| `edit_open` | `id?` (default: the latest take) | opens a take in the audio editor (working copy + history); returns the editor status (`seconds`, `versions`, `index`, `clipboard_seconds`, `dirty`) |
| `edit` | `edit`: `{op, a, b, at, seconds, db, target_dbfs, bands, threshold_db, ratio, attack_ms, release_ms, makeup_db, ceiling_db}` with `op` = cut, copy, delete, paste, silence, trim, insert_silence, gain, normalize, fade_in, fade_out, reverse, **eq** (bands: `{kind: low_shelf\|peak\|high_shelf, hz, gain_db, q}`), **compress**, **limit**, **noise_profile** (a, b = noise only), **denoise** (reduction_db 0..40, sensitivity 0.5..4; needs a profile), **declip** (threshold 0.5..1), **deess** (threshold_db, amount_db), **declick** (sensitivity 2..40), **stretch** (factor 0.25..4, changes the length), **pitch** (semitones ±24, keeps the length), **echo** (delay_s, feedback, mix), **reverb** (size, damp, mix) |
| `edit_preview` | `edit` (an effect op), `seconds?` | **sound**: the effect on its region before applying it |
| `edit_macro` | `ops` (a list of `edit` ops), `a?`, `b?` | A12: applies the chain in order to [a, b) (default the whole take); returns the status |
| `ny_list` | — | A9: the Audacity Nyquist plug-ins: `folder` (Documents\EthnoFlow\Plugins) and `plugins` (id `bundled:<id>` or path, name, kind, controls with var/kind/initial/min/max/choices) |
| `edit` with `{ op: "nyquist", a, b, plugin, values: [{ var, value }] }` | — | A9: runs a process / generate plug-in on [a, b) (generate inserts when a = b); a message from the plug-in comes back as the error text |
| `ny_analyze` | `plugin`, `values?`, `a?`, `b?` | A9: runs a plug-in without changing the take; returns `message`, `labels` (start/end/text relative to a) and `printed` |
| `edit_spectrum` | `a`, `b`, `bands?` | averaged spectrum of the region: `{bands: [{hz, db}], peak_hz, peak_db}` |
| `edit_silences` | `threshold_db?` (−50), `min_s?` (0.3) | `{silences: [[start, end], …]}` |
| `edit_contrast` | `fg_a`, `fg_b`, `bg_a`, `bg_b` | RMS of the foreground vs. the background in dB and their difference | one operation on the working copy (seconds absolute); returns the status |
| `edit_undo` / `edit_redo` / `edit_jump` | `step` for jump | walk the history |
| `edit_snap` | `seconds` | the nearest zero crossing (±10 ms) |
| `edit_peaks` | `bins?`, `from_s?`, `to_s?` | reduced peaks of the working copy (or of a range): `{bins, duration_seconds, peak}` |
| `edit_play` | `from_s?`, `to_s?` | **sound**: plays the working copy; `stop_take` stops |
| `edit_save` | `label?` | writes the current version as a new take derived from the original; returns `{project, take_id}` |
| `edit_status` / `edit_close` | | the editor's status / close the session |
| `export_song` | `path`, `seconds?`, `drone?`, `pad?`, `ambient?`, `rhythm?`, `stems?`, `target_lufs?` (number = normalise to it, `null` = as rendered, absent = the studio's setting, default −14), `format?` (float32 \| pcm24 \| pcm16 \| flac24 \| flac16; FLAC writes `.flac` for the mix and the stems), `start_s?` (keep only from this second: the song is rendered from 0) |
| `export_sections` | `dir`, `seconds?` (end of the last section), the `export_song` options | A10: one file per song-map section (marker to marker) into `dir`; returns `{files}` | the studio's whole-song export (mix + stems + markers + manifest); the result carries `lufs`, `lufs_after`, `gain_db` |
| `reference_add` | `path` (WAV/MP3/FLAC/OGG) | copies the song into the references library (never into a project); reply: id, name, duration |
| `reference_list` | | the references library (id, name, duration, analyzed) |
| `reference_report` | `id` | the saved analysis report (tempo candidates, key hypotheses, segments) without the curves |
| `reference_analyze` | `id` | runs the analysis (on request only) and returns the report |
| `open_project` | `path` (the `.ethnoflow` folder) | closes the current project and opens that one; reply: name, root, takes |
| `recent_projects` | | the recent list (name, path) |
| `set_scale` | `scale` or null | project tonic/mode |
| `set_timing` | `timing` | project tempo/meter |

Replies to `set_*` include the resulting values, so a typo in a field name shows up as
"unchanged" rather than silently vanishing.

## How the assistant should use it (the owner's workflow)

1. The owner records a melody (a take) and asks, in the terminal, for "a starting
   preset for this song" (ambient, liquid DnB, ambient DnB…).
2. Read `state`: scale (if none, read the take's analysis / key hypotheses and
   **propose**, never assume D minor or 174 BPM), timing, instrument, which takes exist.
3. Build the background: `set_prefs` for drone/pad/rhythm/harmony, `set_ambient` or
   `add_layer` for the atmosphere, `set_macros`. Keep levels safe (nothing above 0 dB,
   atmosphere layers around −18 dB, noise lower).
4. `save_preset` each part **with notes that explain the choice** ("Pad: open fifths so
   the kaval's thirds stay free; dark because the melody sits low") and a `session`
   preset named after the song. The owner reads these notes to learn.
5. Say in the reply what was set and why, in the owner's language (Bulgarian), and
   what to turn to change the character. Start sound only when asked (`layers`).

Not in the bridge yet (add when the owner asks): recording/transport of takes,
arrangement clips, Sample Lab recipes, trainer settings.

## Safety

Local files under the user's own profile; the app applies only the vocabulary above,
validated by the same shell commands the UI uses. Audio starts only through `layers`.

## Smoke test

`node scripts/dev/smoke.mjs [--sound] [--quick] [--project <path>]` drives the running app
through these operations (every screen, renders, references, song map + follower, transport,
presets, CLAP, export) and writes `docs/testing/SMOKE_REPORT.md` with PASS/FAIL per step.
Silent by default; `--sound` adds short audible checks (rhythm, drone+pad, audition).
