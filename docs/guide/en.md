# How to use EthnoFlow Studio

This guide is also shown inside the app (the **? Help** button top right, with search). It lives in one place: `docs/guide/bg.md` (Bulgarian) and `docs/guide/en.md` (English). Every new feature adds its paragraph here.

## Home and projects

- **Home has five areas**, one verb each: **Play** (I play and record: microphone, tuner, metronome, takes), **Background** (I sound behind: drone, pad, atmosphere, rhythm, instruments, live accompanist), **Song** (I arrange: tracks, song map, guide, export), **Learn** (academy, trainer, gate, references), **Library** (presets, sessions, samples). Every area is one screen with tabs (a tab is remembered as a screen: a preset's "Load", the assistant and the bridge open the right tab directly): Play = Play & record · Tuner; Background = Drone & pad · Atmosphere · Rhythm section · Instruments · Live accompanist; Song = My studio · Song map · Create a song · Start from a melody; Learn = Academy · References; Library = Library · My samples. The big card opens the first tab, the "Tabs:" buttons under it open the others. Nothing was removed, only arranged.
- At the top of Home is **▶ Continue**: it opens Play with the latest take loaded (with no takes the button reads "● Record the first" and opens Play ready to record). Next to it, **I have a melody →** opens Start from a melody.
- The top bar says where you are: "Home › Background › Atmosphere". **? Help** is top right, next to the assistant.
- Top right is the **📁 Project** menu: new, open, recent, close. Everything in a project saves itself; there is no Save button.
- A project is a folder `Documents\EthnoFlow\<name>.ethnoflow` with the takes (originals are never modified), the analysis, the atmosphere and the remembered settings (`session.json`).
- Every project **remembers its own settings**: drone, pad, rhythm, harmony, atmosphere. Open another project and its settings come back.
- At the bottom of every screen is the "Now playing" bar (see its section below): what sounds, and "Stop all".

## Background mixer

- The **🎚** button (in the floating bar and wherever "Background: play and level" is) opens the mixer: one strip each for drone, pad, atmosphere and rhythm, plus the master.
- Each strip has **Playing/Stopped**, a meter, **Level** (a trim on top of the modules' own levels), **Pan**, **HP/LP** cuts and **Fade in/out** in seconds. Switching on is no longer a click: the layer comes in over its fade-in and goes out over its fade-out (defaults 2 s / 3 s, rhythm 0.5 s / 1 s).
- The strip's cuts add to the modules' own (the stricter wins). Level at the minimum = the layer is off.
- Mixer settings are part of the session and are remembered per project.

## Song › Start from a melody (styles)

The quickest path from an idea to a background. One **style** sets the drone, the pad, the rhythm, the atmosphere, the **chord progression** (follow on) and a **Soundscapes journey** in its world at once. Nine styles: Quiet ambient, Dark drone, Liquid DnB, Ambient DnB, Ethno pulse, RnB / soul, Rock, Pop, Jazz night. If the project already has a mode, the styles show without an analysed melody (from the project's mode and tempo); "Seed" + 🎲 gives other chords and another journey.

1. **Melody**: record with the microphone (🎙 Record), import a file (📂 Import: WAV, MP3, FLAC, OGG, MIDI) or pick an existing take.
2. **🔍 Analyse**. Nothing changes by itself; the analysis only proposes: a key with percentages (pick another or "no key"), a tempo with percentages (or the project's), and the melody's register (lowest, highest and median note).
3. **Variants**: five complete starting points tuned to the melody: *Quiet ambient*, *Dark drone*, *Liquid DnB*, *Ambient DnB*, *Ethno pulse*. Each card says what it assumes for key, tempo and meter and where that comes from (analysis, project, or suggested). **▶ Try** writes the key and tempo into the project and starts everything; **💾 Save as session** stores it in the Library with notes; **↶ Undo** restores what was there before the first try.
4. Then go to Drone & pad, Atmosphere or the Rhythm section and shape it.

The pad and the textures always sit **below** the melody (about two octaves under its median note) so they never fight it.

## "Now playing" bar (bottom of every screen)

An always-visible bar with everything that can sound, as chips: **Take** (the loaded
take: ▶ plays, ■ stops, shows the position; while recording the chip is red and leads to
"Play & record"), **Notes** (♪ plays the analysed take's notes), **Background** (▶/■
drone, pad, atmosphere, rhythm, the level 🔊 and the mixer 🎚), **Song** (the song
transport, shows bar.beat), **Metronome**. A lit chip is sounding. **■ Stop all** stops
all of them at once (not a running recording, which is stopped from its screen). When
nothing sounds it says "Silence". From the terminal: `now_playing` and `stop_all`.

## Play › Play & record

- **The transport is on top and stays there while you scroll**: ● Record, ▶ Play, loop,
  clock, metronome and the background. The take's waveform sits under it. With nothing
  loaded but takes present, there is a "Load the latest" button and the waveform says
  "Pick a take on the left or press ● Record".
- **Every row of "Takes" has a ▶**: it loads the take and plays it from the start; the
  same button becomes ■ while it plays. The row's name only loads (waveform, notes,
  analysis), without sound.
- **Devices, Keyboard, Drone and Pad are folded** into collapsible rows at the bottom with
  a short summary ("Drone: playing · D3", "Devices: <microphone>"); what you open stays open.
- Pick the microphone and headphones under "Devices". The metronome has its own ▶/■ next to "Tempo & meter".
- **Record** makes a new take in the project. The background (drone, pad, atmosphere, rhythm) is heard while you play but **is not recorded**: only the microphone is. For a mixed result use My studio → Export mix.
- The tuner shows the note, the cents and whether it is in the project's mode. The full
  tuner with a needle, instruments and 440/432 Hz is its own screen: Home › Play › Also here: "Tuner".
- "Import…" accepts WAV, MP3, FLAC, OGG, MIDI and a MusicXML sheet (see "Gate").
- "Analyse" here does the same as in Start from a melody but without the variants: contour, notes (editable), key proposals.

## Play › Tuner

- **Needle**: the deviation from the target note in cents, ±50 on the dial; the green
  zone is ±5 c ("In tune"). Below the needle it says what to do: "▲ Too low: go up" or
  "▼ Too high: come down", and by how many cents.
- **Instrument**: *Free* (nearest chromatic note), *Kaval in D* (every tone D4–A6, with
  the fingering next to the needle), *Shakuhachi 1.8 (D)* (holes ロ Ro, ツ Tsu, レ Re, チ Chi,
  リ Ri and the upper kan octave, named), *Guitar* (the six strings E A D G B E, with the
  string number), *Monochord / sound bed* (all strings on one tone: pick the note under
  "Target note", then tune string by string). The bed is a bass instrument (34–40 Hz,
  about C♯1–E1): for it, and for any locked note under 100 Hz, the tuner switches the
  microphone to the **bass range** by itself (listening from 30 Hz; the screen says so).
  The window is longer, so the needle reacts a little slower: let the string ring steadily
  and read then. The other instruments listen from 70 Hz.
- **Target note**: "nearest" or a locked note. Lock it when you tune one string or hole
  and do not want the needle to jump to the neighbouring tone.
- **A4**: 440 (standard), 432, or a typed value 400–480 Hz. The default is the project's
  reference. "Save to the project" makes the chosen reference the project's (analysis,
  falling notes and the trainer use it); "from the project" goes back to it.
- The microphone can be chosen here too. The tuner pauses while you record or play.
  Settings are remembered. From the terminal: `tuner` (also returns the live reading:
  note, cents, direction).
- **No blinking**: the last tone heard is held for about 0.7 s after it stops (shown
  dimmed), the note and the needle are smoothed, and the blocks keep their size, so the
  screen does not jump between "silent" and a note on every breath or weak frame.

## Gate: a lesson game that waits for the right note

In "Play & record", with a loaded and analysed take, the **Gate (game)** view (next to
"Piano roll" and "Falling notes") shows the song as a corridor: time runs right, pitch
runs up. Every note is a **wall with an opening** at its pitch; the ball is your tone
from the microphone.

- Pick the **Wait** mode in the "Trainer" panel and press ▶ Start. The song stops in
  front of every gate and **waits**: too high or too low and the ball is blocked (red)
  with an arrow showing which way, "▲ Too low: go up (−23 c)". Hold the right note for
  ~150 ms and you pass; the song continues to the next gate. There is no tempo: as slow
  as you need.
- The **Level** in "Trainer" sets how wide the opening is: beginner ±30 c, intermediate
  ±20 c, advanced ±10 c.
- Academy lessons with "Play along" open straight into this view. Notes for a song come
  from a MIDI file, from a take (Analyse), from a reference (phrase) or from a sheet
  (below).
- **Sheet music**: "Import…" also accepts MusicXML (`.musicxml`, `.xml`, uncompressed).
  MuseScore exports it directly; from a PDF or a photo the free Audiveris makes it
  (export → MusicXML, uncompressed). Pitches, durations, rests, ties, chords, tempo
  (including changes), time signature and transposing parts are read; without a tempo in
  the sheet the project's tempo is used, and without one, 120. Compressed `.mxl` is not
  read: unzip it (it is a ZIP).

## Trainer to the end: latency, drill, history, fingerings

In Play & record › **Trainer** (the panel under the falling notes):

- **Latency**: "Measure latency" plays 4 clicks through the headphones/speakers and measures
  when the microphone hears them (≈ 4 s; shows ms ± spread and how many clicks were heard).
  The value is kept in the settings and subtracted automatically from timing in the "In rhythm"
  and "Loop drill" modes and from the clapping exercise in the Academy. "✕" forgets it. If the
  microphone does not hear the clicks, raise the speakers a little or hold the headphones to it.
- **Loop drill**: two bars from the cursor (or 4 s without a tempo) play at 50 % tempo; you
  play along, every pass is scored; after 3 clean passes (≥ 90 % hits) the tempo steps up by
  10 %, until 100 %. The dots show the passes (green = clean). "Stop" interrupts.
- **Recent results**: every "In rhythm" report and every drill pass is logged locally (date,
  mode, tempo, level, hits/expected, mean deviation in cents and ms); the last five are shown
  under the panel. From the terminal: `trainer_history`.
- **Fingerings with the falling notes** (kaval profile): next to the falling-notes view the
  fingering of the lit (trainer) or next note is shown, with the one after it dimmed.
- **Editing the chart**: Academy › Fingerings › pick a note › click the holes (● → ○ → ◌ → ◐) ›
  "Save as mine". The correction lives in your settings folder and applies everywhere (tuner,
  falling notes, lessons); "Restore the original" removes it. The app's table never changes.

## Background: one screen with tabs

Home › **Background**. Everything that sounds behind the melody is one screen with five
tabs: **Drone & pad** (mode, tempo, styles, drone, pad, intent), **Atmosphere** (with a
"Layers" or "Modules" view, the former Soundscapes), **Rhythm section**, **Instruments**
and **Live accompanist**. A tab is
remembered as a screen: a preset's "Load", the assistant and the bridge open the right tab
directly. Play, stop and the level of the background are not in the tabs but in the "Now
playing" bar at the bottom (🎚 opens the mixer).

## Background › Drone & pad (accompanist)

- **Mode**: choose a tonic and mode or accept a proposal from the analysis. The drone and the pad follow the tonal centre.
- **Drone**: a root tone with fifth, octave, sub, warmth, movement and breath. The lowest layer.
- **Pad**: a chord from the mode with a voicing, an emotion and an X/Y macro. Its register is best kept below the melody.
- **Harmony**: proposals for chord movement in three styles with A/B. "Intent" follows what you play and suggests tonic and chord.
- Styles ("Deep Ethno DnB", "Ambient no key"…) are quick starting settings.

## Background › Live accompanist (follows your playing)

Background › **Live accompanist** tab (the same panel is also in Play & record). Start a drone and/or pad from the
background, press **Follow** and play: the follower listens to the microphone, cuts the
playing into notes and phrases, classifies the notes (stable / ornament / transition / noise /
probable error), tracks the tonal centre with hysteresis and, at the **end of every phrase**,
decides the pad chord and the drone root. Rules: it never changes anything mid-phrase or
because of an ornament; the mode switches only after several phrases of evidence (the mode
from the “Intent” badge: Stable slowest, Creative fastest); at low confidence it **holds** the
previous harmony and shows “Why the harmony stayed”; with no harmony yet it falls back to a
safe drone on the tonic. Audio never waits for the analysis.

- **Strategy**: drone only / modal pads / liquid chords (as in the proposals).
- **Lock**: tonic, mode, chord, drone — locked things are not touched.
- **Emergency bypass**: the follower keeps showing but does not act; the background stays.
- The panel shows the context and its confidence, the current harmony, phrase/change/hold
  counts, the recent notes with their class (coloured) and the alternative modes. “Stop
  following” releases the pad and drone to their own settings.
- Measured: the decision comes at the phrase boundary (after a ≈ 0.35 s breath for winds); a
  test with an ornamented phrase = 0 changes inside the phrase. From the terminal:
  `live_start`, `live_tick`, `live_set`, `live_stop`.

## Background › Atmosphere

- Up to 8 layers: noise, wind, texture, bells, plucks, looped take. Each has level, pan, filters, its own parameters, M/S and a lock.
- **Presets** on the left (factory and yours), "Surprise me" changes the unlocked layers, the history goes back.
- **Macros** act on everything: intensity, brightness, motion, space and its size, tonal gravity, width, **shimmer** (the space's tail comes back transposed by an octave, a fifth or octave+fifth: an "angelic choir" behind every tone; 20–40 % for glitter, 60 %+ for a cloud of overtones).
- **Automation and scenes**: lanes over time and a program of scenes; export to WAV with stems.
- **Samples**: "Import samples…" or drag files onto the window (e.g. from Soundly). Each file becomes a take and a looped layer.
- A slider at its minimum means the layer is off (silence), not "very quiet".

## Library › My samples

- Home › Library › **My samples** tab. Add folders ("Soundly Audio" is offered on first open if it exists): the app scans them recursively for WAV, MP3, FLAC, OGG without copying anything.
- Downloaded libraries (e.g. ELPHNT FIELD, CC0) live in `DocumentsEthnoFlowSamples<name>` and are added as a folder here; the assistant can add them through the bridge (`sample_folders`). The folder is not copied into the project: only the file you press "+" on enters it.
- Search by name and path, sort by name, date or length; ▶ plays the file through the shared output (stops by itself, or with ■).
- "+ Take" imports it into the project; "+ Loop" and "+ Cloud" import it and immediately make a layer in the atmosphere. From there it is a take like any other.
- Soundly workflow: pick and download sounds in Soundly → "Rescan" here → listen → "+".

## Learn › References (whole songs to learn from)

A separate, isolated place for songs you want to take something from — **outside the
projects**. "Add song…" (WAV, MP3, FLAC, OGG) copies the file into the references library;
the project is untouched. Nothing is analysed by itself: press "Analyse" and you get three
things, each with a confidence and a "why":

- **Tempo — candidates.** BPM chips; the half and the double are listed too, because the
  ear decides between 174 and 87. Pick the one you count, choose a meter and "Take the
  tempo into the project".
- **Key — hypotheses.** From the song's mean chroma. "Take the key" sets the tonic and
  mode in the project (drone, pad and rhythm follow).
- **Structure.** A lane of sections over the waveform (click one to hear it) and a table:
  start–end, role (drop/build/break/unclear) with confidence, length in bars at the chosen
  tempo, energy, density, bass and evidence ("repeats #1"). Boundaries come from novelty
  (self-similarity of timbre, chroma, energy) at three resolutions; roles come only from
  the sound, never from the position in the song. "Copy the structure into the song map"
  carries the same bar count per section into your project — then move and rename them
  in the Song map.

The notes under the song are for you ("I like how the bass enters in section 2").
**Melodic phrases**: "Extract phrases" pitch-tracks the melody band (150 Hz – 3 kHz), turns it
into notes and cuts them into phrases between rests; each shows time, section, note count,
span, "in key" share and confidence, with a mini piano-roll. ▶ plays it with a synthesized
voice at its original timing; "→ MIDI take" puts it into the project as a MIDI take (player,
track, trainer). These are proposals: on a dense mix the tracker follows the most periodic
voice, so it works best on solo passages and an up-front vocal or kaval.
From the terminal: `reference_add`, `reference_analyze`, `reference_report` (see the bridge).

## Assistant (on every screen)

The rail on the right (the 🧭 "Assistant" button at the top shows/hides it) knows the app's
state and says what to do: the take is in another key than the project ("Take the take's
key"), no tempo, the background is off, the melody track lets the lows through ("Apply" cuts
below 140 Hz), the map has no sections, latency is not measured, no instrument profile, the
background is running and the Live accompanist could follow. Every tip has a button that
**does** the thing or takes you to the screen and **lights up** the control (a pulsing frame).
While a guided lesson or the Create-a-song guide runs, its current step is on top. At the
bottom: the quick "Make the song" and the "What I did and why" list of the last build.

## Song › Create a song (step-by-step guide)

**Make the song** (button at the top of the screen and in the assistant): from the latest
take, in the chosen style (Liquid DnB, Ambient DnB, Quiet ambient, Dark drone, Ethno pulse),
the app itself:
1. analyses the take — notes, most frequent tones, **key** (the strongest hypothesis becomes
   the project scale), the take's **pulse**;
2. picks a background and tempo/meter by style (the same "starters" as Start from a melody),
   in the melody's register; if the take's pulse does not divide the style tempo it says the
   melody is a free phrase over the groove;
3. builds a **song map from the phrases** (intro = the first phrase, then build / drop / break /
   outro on bar lines) — the map follower changes density by itself;
4. puts the take on a clean **"Melody" track**, aligned to the first note, with a cut below
   140 Hz and a little space;
5. **balances by measured levels**: renders the first 16 s with stems through the real graph,
   measures each group's RMS and moves the levels towards the targets (melody −18 dBFS, drums
   −21, bass −23, pad −28, drone −30, extras −30); the before→after numbers are in the list.
Then start the background and listen; change the style or key from the assistant; export from
the studio. Nothing is hidden: every step is the same command the screens use.


Home › Song › **Create a song** tab. Seven steps in the real screens: melody (a take with ≥ 3 notes) →
scale (a hypothesis) → tempo and meter → song map (≥ 2 sections) → background (drone/pad/
rhythm) → studio (a clip on a track) → export of the whole song. "Start the guide" and each
step ticks itself when the app sees it done; "Go" takes you to the screen; the order is not
mandatory. While the guide runs, a bar on every screen shows the current step. The same
guided engine is lesson 5.6 in the Academy (Level 5), so passing it is recorded in the progress.

## Learn › Academy

Home › **Learn** › Academy tab. Lessons of 5–10 minutes: a short explanation (three sentences at
most), a demonstration the app plays itself, an exercise with a real check, and a quiz.
Progress is a local file (no points, streaks or notifications); "passed" means a threshold
was met. The whole programme is planned in `docs/academy/CURRICULUM.md`.

- **Levels 0–3** (28 lessons): First sound, Notes and rhythm, Melody, Harmony. Order: top to
  bottom, nothing is locked. The last lesson of level 3 is the guided **Lesson No. 1 "The ancient
  kaval meets 174 BPM"** (tone → rhythm 174 → take → scale → Space → preset).
- **Kaval (Thracian style)** (14 lessons) from the owner's knowledge base: sound, fingers and
  grip, registers (with the fingering chart), sitnene and tongue-less articulation, klepane,
  kaba; then Dosev's ornamentation (1994): vibrato, cutting, substitution, glissando, accents
  (AT), short trill, the song appendix. Every lesson has a "Read in the knowledge base" button
  to its exact section.
- **Fingerings**: the whole chart for the kaval in D by register (kaba, 1–4); click a note to
  hear it and see the diagram (T 2 3 4 | 5 6 7 8; closed ● open ○ optional ◌ half ◐). The same
  diagram appears next to the tuner when the project profile is a kaval, and in the exercises.
- **Knowledge base**: the full `docs/kaval-baza-znania.md` with a table of contents by part
  and chapter and search (tables and quotes render).

Exercises: **quiz**; **hold the tone** (the tuner counts seconds in tolerance; with a stability
σ target it is measured too); **read the note** (a note is shown, you play it, reaction time is
measured; on a kaval the fingering and the nearest one to what you play are shown); **hear and
repeat** (the synth plays the phrase, then you; timing is aligned to your first note; the
trainer's evaluation scores it); **ear** for interval / chord / mode / meter (rounds with
choices); **clapping** (one count-in bar, then the microphone counts hits against the grid;
result in ms); **play along** (the phrase becomes a MIDI take and opens in Play & record ›
Trainer; the Wait / Rhythm result is recorded in the lesson); **guided steps** (they tick
themselves from the real state: screen, metronome, tuner steady N s, rhythm running at X BPM,
new take with ≥ N notes, scale chosen, Space ≥ X, preset saved). While a guided lesson runs, a
bar on every screen shows the current step.

Exercises are **in the project scale and the instrument range** (profile or type). Without a
scale they use C major and a middle range and say so; pick a scale in the Scale panel.
Level: beginner ±30 c / ±80 ms, intermediate ±20 / ±50, advanced ±10 / ±30. "Once more" gives a
new variant (seed) of the same exercise.
From the terminal: `academy_lessons`, `academy_instance`, `academy_score_rhythm`,
`academy_record`, `academy_reset` (see the bridge).

## Background › Soundscapes (a journey: a whole composition)

Home › Background › **Soundscapes** tab. Not just a background but a composition with a shape:

- Pick a **world** (Ancient forest, Deep space, Cyberpunk rain, Rhodope night, Mountain morning, Thracian steppe), a **length** in minutes, a **seed** (🎲 Shuffle gives another journey, the same seed = the same journey) and whether it **loops**.
- **Soundscapes is its own module**, not a view of Atmosphere: its own rack (`scapes.json` in the project), its own **Scapes** switch in the "Now playing" bar and its own strip in the mixer 🎚, its own "soundscapes" stem in the export. The atmosphere stays as it is and both can play at the same time.
- **▶ Play the journey** builds the world's layers and four scenes into the Soundscapes rack: **Intro** (only the bed, quiet), **Development** (textures come in, the first events), **Climax** (everything plays, brighter), **Outro** (everything withdraws). Transitions between scenes are smooth, and three "random walk" lanes make brightness, space and motion breathe. The bar under the worlds shows which phase runs and how much is left.
- The **Journey** bar is live: it shows where you are, **click** it to jump to another moment, **❚❚ Pause / ▶ Resume** stops and runs the program without stopping the sound, **⏮ From the start** rewinds.
- **History of changes** (above the editor, and in Atmosphere): every change is a step with a name (a knob, a new layer, a scene, a journey, a preset, an assistant command). ↶ Undo / ↷ Redo (Ctrl+Z / Ctrl+Y) or "Restore here" on any step of the list when it gets messed up after your intervention. Kept while the app is open; the project holds the current state.
- Under the journey is the **Rack editor**: the same editor as Atmosphere, on the Soundscapes rack: layers (sliders or modules with big knobs), a new layer by kind, samples as "Loop" or "Cloud" (from Library › My samples with "+"), scenes and program, macros, mixer, presets. The journey only builds the starting state; from here on you change whatever you like.
- **Save as preset** keeps the journey in the Library (atmosphere presets, tag scapes). Play and level are also in the "Now playing" bar ("Scapes" switch); ■ Stop stops the program and the layer.
- Next stages (per `docs/design/SOUNDSCAPES_GENERATIVE_PLAN.md`): random events with tension and resolution, a pulse, orbital panning, reaction to the microphone.

## Background › Atmosphere › "Modules" view (the former Soundscapes)

Atmosphere's top row has a **Layers / Modules** switch. "Modules" shows the same rack one module at a time with big knobs. Until 1.11 this was a separate "Soundscapes" screen; now it is one screen and one "Atm." switch in the "Now playing" bar, because the sound is the same. Old presets and links that led to Soundscapes open Atmosphere in this view.

- Pick a **Module** (layer) at the top or add a new one by kind. Under the title is "How to use it" for that module.
- **Resonator** (modal synthesis): a struck body in the mode, from string/tube (Structure 0) to bowl/bell (1); Strike, Brightness, Decay, Density. Good for sparse bell hits over the pad or a wooden pulse under the melody.
- **Choir** (vocal drone): vowels a–e–i–o–u through formants on the mode, Breath for slow swells, Spread for width, Voices = how many tones sing; a low register = a men's choir. Under the kaval with a big Space.
- **Bloom** (generative melody): notes from the mode that appear by themselves — Density per minute, Length/Attack from drops to long tones, Phrasing from random to stepwise phrases with breaths. Same seed = same melody.
- **Follow the chord** (a switch in the top row of Atmosphere and Soundscapes): tonal layers (texture, bells, plucks, resonator, choir, bloom, sampler) take their pitches from the chord the pad / progression plays right now (and from the live accompanist while it follows) instead of the whole mode. That is how the sampler "plays" the progression. With no chord they play the mode. The offline export does not follow the progression bar by bar yet (it plays the mode).
- **Soundscape studio** (the Soundscapes tab): the worlds on top — the six built-in ones and **My worlds** (★ "Save as my world" keeps the current soundscape with its layers, scenes and macros; "Load" brings it back). Under the journey bar sits the studio: **samples on the left** from your folders (▶ previews through the output, "+" adds the file as a loop / cloud / sampler into this soundscape), **its modules on the right** with knobs (a click on a mixer row opens the layer's controls: level, pan, the kind's parameters, cuts, take, register). **Scenes of the journey** is a table layers × scenes: the slider is the layer's level in the scene (−60 = silent), the numbers above are the scene's length and the transition seconds; that is how you say when what comes in and how it develops. The full rack editor is folded at the bottom. (Fix 1.11: the Modules view on Soundscapes now writes the Soundscapes rack, not the atmosphere.)
- **On the bar** (a select in the top row of Atmosphere and Soundscapes: free / every beat / half beats / quarter beats): the event layers (bells, plucks, resonator, bloom, sampler) wait for the next grid line while the rhythm section or the song plays; the density decides *how many* strikes, the grid decides *when* (at most one per line). With the clock stopped they run free as before. The song export has the grid too (its clock starts at 0 at the project tempo). **Tension** (a bells slider, 0–50 %): that share of strikes lands a semitone or a tritone outside the mode and the next strike resolves it to the mode note; this is the Soundscapes events (S2): bells with 10 % tension in the worlds, on the grid whenever a rhythm plays.
- **Instrument from a folder (multisample)**: in Library › My samples every folder has a **🎹 Instrument from the folder (N)** button: it imports the files with a note in the name (Piano_C4.wav, A#3, Bb2, 060…) as takes and makes one Sampler layer with a zone per note; the nearest sample plays every note with a small speed change. "Into:" picks the rack (Soundscapes or Atmosphere). The layer editor lists the zones with each root (editable) and ✕.
- **Sampler** (a take as notes): pick a take of the project or a sample from Library › My samples ("+ Sampler") and it plays as notes of the mode: every note reads the sample faster or slower relative to **Root note** (the note the recording itself sounds at). Density per minute, Note length, Attack / Release, **Filter with Resonance** (a low-pass that "sings" around the cutoff), Start in the take, register. Brightness moves the filter, Motion scales the density; everything automates (LFO / random walk / keyframes) and goes into scenes. This is the answer to "samples with notes, pitch and filters".
- **Cloud** (granular): pick a take (kaval, voice, rain) and it becomes an endless pad of small grains. Position = where in the take, Scan 0 = freezes the moment, Grain and Density go from whisper to choir, Pitch transposes (−12 = an octave down), Reverse and Octave up are per-grain probabilities. The strongest ambient module for your own recordings.
- Knobs: level with a meter, pan, the module's parameters, HP/LP. Drag up/down; Shift = fine; double-click = default; the wheel works too.
- **Shuffle** gives new values to this module only (the level stays); 🔒 protects it from "Surprise me".
- **Module presets** (💾/📚) with tags and notes, in the shared Library.
- On the right: a mini mixer and the macros as knobs. Whatever you do here shows in Atmosphere too.

## Song › Song map

- Home › Song › **Song map** tab or from 🗺 in My studio. Sections (intro, establish, build, drop, variation, break, bridge, outro) sit on a bar ruler; edges drag by whole bars, "Split at position" cuts on a bar line, ⇥ merges with the next, 🔒 locks.
- Every section has **energy** and **density** (0..1): the targets the Composer and the Director will read. The map is stored in the project in ticks (with tempo and meter maps); the studio's markers are updated from it.
- Clicking the ruler moves the song transport; the current bar and the bars until the next section are shown.
- **Follow the map** (on by default): the sections drive the background. While the rhythm or the song runs, the current section decides which rhythm parts play, how dense the figures are and how loud the buses are (drone/pad/atmosphere/rhythm). Your settings are the base — a section only removes and softens, it never switches on what you turned off. The badge next to the switch says what it does right now (e.g. "3. Build 2 (build, 40%) · drums ✓ bass ✓ ostinato ✓ arpeggio ✗ · rhythm −3 dB, pad −2 dB").

  | Role | What it does |
  |---|---|
  | Intro | no drums or bass (nor tapan/lead), sparser ostinato, rhythm −6 dB, pad forward |
  | Establish | no arpeggio or lead, simpler drums, −3 dB |
  | Build | starts thin and brings everything back by the end; level rises from −6 to 0 dB |
  | Drop | everything plays, 0 dB; drone −3 dB |
  | Variation | everything plays, ostinato/arpeggio/lead with a new figure (other seed) |
  | Break | pad only (and lead), −4 dB |
  | Bridge | no ostinato or tapan, drums at half complexity |
  | Outro | lead stops halfway, drums in the last quarter; level falls to −12 dB |
  | Custom | all as you set it; energy moves the level (±3 dB) |

  The section's energy adds ±3 dB to the rhythm and lifts the pad in quiet parts; density scales the figures. Step by step: 1) tempo/meter and key; 2) sections (by hand or from References); 3) switch the background on; 4) ▶ or start the rhythm. The offline render and the export do not follow the map yet (next step).

## Background › Instruments (designer)

- Home › Background › **Instruments** tab or from ✎ next to each part in the rhythm section. Left: factory and your presets by category (drums, bass, synth) with search. Right: the chosen instrument with knobs.
- **Audition** plays a short pattern through exactly this sound at the project tempo; knobs are heard at once. **WAV** renders the same pattern to a file for comparison.
- The knobs are real sound parameters: for the bass voice (sub/pluck/Reese), attack/decay/sustain/release, pitch sweep, microvariation, cutoff/resonance/filter envelope, wobble, drive, warmth, glide, sidechain; for the drums kick fundamental, decay, brightness and a level per drum; for the synth timbre, attack, release, tone, dynamics. Every knob has a tooltip saying what it changes in the sound.
- **Save as…** makes your preset (factory ones never change; the copy remembers its parent). **Update** overwrites your preset. 🗑 deletes yours.
- **Use in** sends the preset to the rhythm section; each part there has a 🎛 picker ("own sound" = the part's fields). The choice is kept in the project's session.
- Why it matters: the four parts are no longer the same instruments in every project. Two kits or two basses must sound different, not just be named differently.

## Background › Progression (chords by genre)

Home › Background › **Progression** tab. A chord progression that the pad, the drone and the rhythm section follow:

- **Genre** (RnB, Rock, Pop, Drum & bass, Liquid, Ambient, Ethno, Jazz): each card says which steps and chords it likes, how many bars per chord and which drums fit. **Generate** lays the progression out from the project's scale (degrees I–VII, never an assumed key) for the chosen bars and seed; 🎲 Shuffle gives another, the same seed = the same. "Also set the drums to the genre" switches the drum style in the Rhythm section.
- **Steps**: each is a degree (1–7), a quality (triad, power, sus2, sus4, seventh, maj7, ninth, add9) and bars; edit them in place, + Step / ✕. The label shows the roman numeral in the mode (i, VII, V7…) and the notes.
- **Follow the progression**: while ▶ Song or the rhythm runs, the chord of the current bar goes into the pad, the drone root and the rhythm section (bass, ostinato, arpeggio). The live accompanist wins while it follows. The switch starts no sound by itself.
- The progression is part of the project's session (saved, and part of session presets).

## Background › Rhythm section

- Four instruments on one shared clock: **drums** (liquid, halftime, ethno), **bass** (sub, pluck, Reese with wobble and sidechain), **ostinato** (a step motif, lockable), **arpeggio**.
- Tempo and meter are the project's ("Tempo & meter"). "Suggest tempo from the take" measures a take's pulse; you accept it.
- Bass and ostinato follow the project's mode and the pad's chord. Moving a slider never interrupts the rhythm.
- "→ MIDI takes" writes the parts as MIDI into the project (drums on channel 10). "WAV + stems" renders the section offline exactly as it plays: a mix plus a folder with a stem per part. Sections (intro, verse…) are markers in My studio.
- Level at the minimum = the part is off.

### More instruments (Instrument Rack)

Under the four parts there is a row **"More instruments"** with **+ Tapan**, **+ Chord pad**,
**+ Lead** (up to 8 parts). Each new part has on/off, a level with a meter, 🔒, 🎲 seed, a
preset picker 🎛 (only presets of its kind) and ✎ to the designer; ✕ removes it.

- **Tapan**: a boom on every accent-group start of the meter (in 7/8 = 2+2+3 on the 1st,
  3rd and 5th eighth), a rim on the group's last eighth, ghosts by **Density**; **Fill** = a
  rim roll every N bars; **Humanize** shakes the velocities.
- **Chord pad**: plays the section's chord (or the mode's triad) in the chosen **Register**,
  one chord per **Bars per chord**, with a **Gap** before the next one (breathing).
- **Lead**: monophonic phrases from the mode's tones in the register — **Density**,
  **Step**, **Phrase (bars)** (repeats with a variation), **Leaps**. The preset (kaval-like,
  saw, hollow) is the sound; the seed is the phrase.

The parts go into the offline render and the song export as separate stems
(`tapan-1.wav`, `chordpad-2.wav`, `lead-3.wav`). In the instrument designer the new kinds
have their own knobs and a button **"Add to the rhythm as a new part"**.

## Song › My studio (tracks)

- **Track strip** (under the name): Pan (left–right), Cut below (HP), Cut above (LP) and **Send to the space** – how much of the track goes to the master reverb regardless of the global "Space". Level, Mute and Solo are on the top row. Everything is stored in the arrangement and goes into the export (the clips stem is stereo with the pans).

- Takes are arranged as clips on tracks, with section markers and one transport. Drone, pad, atmosphere and rhythm play together with the clips.
- **Import** adds a file as a new take and a clip at the cursor; **Export mix** writes the whole song: the clips plus the switched-on layers (drone, pad, atmosphere with its scenes, rhythm) through the same graph and limiter. Next to the WAV a folder holds one stem per group (same start and length), markers.csv with the song map's sections and manifest.json (tempo, meter, sections) for a DAW. **Format** (next to the button; the same choice sits in Play & record for a single take): WAV 32-bit float / 24 / 16 or **FLAC 24 / 16** (lossless, about half the size); the mix and the stems are written in the chosen format. MP3 is not available yet (needs LAME; an owner decision, ADR-0023). **Loudness** (next to the button): "as rendered" leaves the mix alone; **−14 LUFS** (default; the phone / streaming level), −16, −18 or −23 (broadcast) measures the whole song (BS.1770) and lifts or lowers it to the target, mix and stems by the same gain so they still sum, plus a limiter at −1 dBFS on the mix only. The message after the export states the measured loudness and the gain applied. The choice is remembered.
- **Template for a new project** (the "Template (style)" select in the new-project form): the project starts with the style's background ready (drone, pad, rhythm, atmosphere, progression and Soundscapes world), the same thing "Try" does in Start from a melody, but without a take or an analysis (the mode is the project's). Silent until you press play.
- **What next** (in the header of every screen): the next step on the path project → take → mode → background → track → export; click to go there. After an export it suggests playing along or saving the session.
- **Keyboard everywhere** (the "⌨ Keyboard" button in the Now-playing bar on every screen) opens a strip of keys above the bar. Play with the mouse (click a key), the computer keyboard (Z–M lower octave, Q–P upper, S D G H J / 2 3 5 6 7 black keys, ← → octaves) or a **MIDI keyboard** (pick the port in the strip; velocity, the sustain pedal, the **pitch-bend** wheel (±2 semitones) and the **mod wheel** are honoured: the mod wheel opens the filter (brightness) of bass, lead, synth and pad and adds vibrato to the project's voice). **Sound**: "The project's voice" (the voice of Play & record) or "The instrument being edited": in Background › Instruments the strip switches to the instrument you are editing by itself, so a key plays exactly that bass / synth / pad / lead at that note, and for drums and tapan every key is a hit (36 kick, 38 snare, 42 hat, 46 open hat, 70 shaker; other keys cycle through the six). **Pad** and **Drone** as the sound: while a key is held the pad plays its voicing on that note (or the drone moves to it) and both return to where they were on release; a layer that was off is switched on only for the press. **Module: …** (every bells / pluck / resonator / sampler layer of Atmosphere or Soundscapes while it sounds): the key is one strike of that module on that note, so you hear the module without waiting for its chance. "Scale lock" snaps the computer keys to the project's mode. The keyboard only sounds, it never records notes (MIDI into a track stays in My studio); "Stop all" releases its notes too.
- **Edit the take** (the "Edit" tab in Play; a button of the same name in Play & record and on a selected clip in My studio): a working copy of the take with a waveform, drag to select, **Ctrl+wheel to zoom**. Operations: Cut / Copy / Paste at cursor / Delete / Keep only the selection / Silence / Insert 1 s of silence; ±3 dB, Normalise (−1 dBFS), Fade in/out, Reverse, Snap to zero (Z). Without a selection the processing acts on the whole take. The **history** below lists every step: Ctrl+Z/Y or a click on a step walks back and forth (up to 200 steps, about 1 GB of memory; the oldest go first). "▶ Selection / From cursor" plays the working copy. The original file never changes: **"Save as a new take"** writes the current version as a new take derived from the original, labelled by the operations (e.g. "Take 3 · cut, normalise").
- **Track processing** (the "EQ·C" button in the strip; a "●" when something is on) opens a panel under the arrangement with an equaliser and a compressor for the selected track. **Low / Mid / High** are ±24 dB (shelves at 200 Hz and 4 kHz, a peak at "Mid Hz" 100 Hz–8 kHz; 0 dB = off). **Compressor**: Threshold (−60…0 dBFS), Ratio (1:1 = off … 20:1), Attack (0.1–200 ms), Release (1–2000 ms), Make-up (0–24 dB). Processing sits after the cuts and before the level, is heard live, is saved with the arrangement and goes into the export and the clips stem. A kaval start: a gentle low cut, +2 dB around 2 kHz for clarity, a compressor at −24 dB and 3:1 so the level does not jump between phrases.
- Trimming, silence at the edges ("Silence analysis"), snapping to zero crossings.
- **Takes** (recording over the background like in a studio): ● Record records at the cursor onto the selected track while the background and the other tracks play; **● One more take** records again from the same spot. Older takes at the same spot stay on the track, muted; in the clip panel "Takes" lists every recording at that spot with ▶ to audition and "Use this take" (only that one sounds). Nothing is deleted by itself.
- **Latency**: the "Latency" button in the record row measures how late the microphone hears the output (plays clicks; the same measurement as in the trainer). The measured time is cut from the start of every new recording so it sits exactly on the background; the message after the take says by how much.

## Library › Presets and sessions

- Every panel with sliders has **💾 Save preset** (name, tags, note) and **📚 Presets** (search by name, tag and note; sort; "this project only"; overwrite ⟳; edit ✎; delete 🗑).
- **Session** (top, next to the project menu) = all settings at once: drone, pad, rhythm, harmony, intent, atmosphere and which layers play.
- **Library** (Home) shows everything from every project with filters by kind, project and tag. "Load" applies the preset and opens its screen.
- Write notes: "why" and "when it works". They are searched, and they are how you learn.

## Song transport

The bar **⏮ ▶/⏸ bar.beat time 🔁** (in the Song map and the Rhythm section; My studio has
the same transport with more tools) is **one position for the whole song**: the clips on
the tracks, the rhythm section and the map line up on the bar. The rhythm joins where the
song is (when you switch it on while the song runs) and re-aligns on ⏮ or a click on the
ruler. **🔁 Loop** repeats N bars from the current bar (the region is stored in the
arrangement). The "bar.beat" position follows the meter map (in 7/8 the beats are eighths).
The background layers (drone, pad, atmosphere, rhythm) keep their own switches — the
transport does not stop them. Limitation: the rhythm clock runs at the project tempo (one
value); a tempo change inside the map re-aligns at the next play/seek.

## Tempo, meter and metronome

- "Tempo & meter": BPM, numerator/unit, grouping for additive meters (e.g. 7/8 = 3+2+2).
- The metronome has its own ▶/■; recording has its own click and count-in.
- Nothing assumes 4/4 or 174 BPM: without a meter the rhythm does not start, and Start from a melody says explicitly when it suggests a value.

## CLAP instruments (experimental)

EthnoFlow can load instruments in the **CLAP** format (`.clap`, as in Bitwig/Reaper): for
now only through the Claude Code bridge — `clap_scan` lists the instruments in the standard
folders, `clap_render` plays a two-bar phrase through the chosen instrument and writes it
to a WAV. That is how you check a plugin works in our graph before it gets a button in the
Instrument Rack (next step). Limitations: the plugin runs inside the app (a crash in it
stops EthnoFlow too), no parameters and no fine intonation yet (notes only).

## Bridge to Claude Code

- The 📁 Project menu has a switch **🤖 Bridge to Claude Code**. When on, the app watches a folder in your profile (the path is shown there): it writes its state and reads commands.
- What it allows: from the terminal Claude Code reads the key, tempo, takes and all settings and can change them, add modules, save and load presets and sessions, start and stop layers.
- Typical flow: record a melody → in the terminal write "make me starting presets for ambient for this take and explain" → you get settings, presets with notes and an explanation of what to turn.
- Local files only; nothing leaves the machine. Switch it off with the same switch.

## Genre recipes (step by step)

Three ways to reach something that sounds like a piece with today's knobs. Each starts
from "Start from a melody", because that is where key and tempo come from your take.

### Ambient (quiet, breathing bed for the kaval)
1. Record 20–30 s of kaval → **Start from a melody → Analyse** → accept the key → **Try "Quiet ambient"**.
2. **Soundscapes**: add a **Cloud** of the same take (Grain 150–300 ms, Density 15–25, Scan 0 = frozen, Pitch −12, Octave up 20 %). Level −20 dB. This is "the pad made of you".
3. Macros: Space 0.5–0.7, Size 0.8, **Shimmer 25–40 %** at the octave, Motion 0.4. If it turns to mush, lower Brightness to 0.4 and cut the Cloud on top (LP 6 kHz).
4. **Background mixer** 🎚: Drone −6 dB trim, Pad 0, Atmosphere 0; Fade in 4 s / out 6 s on all, so switching breathes.
5. Save a **Session** named after the piece with a note on what you liked.

### Liquid DnB (rolling, bright)
1. **Start from a melody → Try "Liquid DnB"** (160–180 BPM; if the analysis found 85–90, the variant doubles it and says so).
2. **Rhythm section**: Drums 🎛 "Liquid kit", Complexity 0.4–0.5, Humanize 0.3, Fill every 8. Bass 🎛 "Reese" (or "Pluck" for softer), Sidechain 0.6–0.7, Wobble 1/8 depth 0.4.
3. **Instruments**: open the bass with ✎, Attack 2 ms, Decay 250 ms, Sustain 40 %, Filter envelope 0.5; **Microvariation 25 %** so it is not a machine. "Save as" → "my liquid bass" → Use in Bass.
4. Pad: scale_triad, emotion "light", level −16; atmosphere: light Texture −22 and high Wind −28. Shimmer 15 %.
5. Song map: Intro 16 · Build 16 · Drop 32 · Break 16 · Drop 32 · Outro 16 bars (a guide, not a law). For now section changes are by hand: at the Break stop the drums from the mixer with Fade out 2 s, at the Drop bring them in with Fade in 0.5 s. The Composer will do this itself.

### Ambient drum & bass (the new style: rhythm under the melody)
1. **Start from a melody → Try "Ambient DnB"**.
2. Drums 🎛 "Soft wooden kit" or "Dark halftime kit", style **Halftime**, Complexity 0.3, level −12 to −14. Bass 🎛 "Deep sine" (Attack 60 ms, no drive). Sidechain 0.6.
3. Atmosphere: Bells (Density 3, Decay 15 s) −22, Texture −20, Wind −26, **Cloud** of the kaval −22 with Scan 0.2; macros Space 0.7, Size 0.9, Width 0.8, Shimmer 30 % at the fifth.
4. Mixer: Rhythm trim −3 dB and LP 9 kHz, Atmosphere trim 0, Pad trim 0; Rhythm fade out 4 s so it leaves like a wave.
5. While playing: let the drums enter after 16 bars and leave before the end. The song map tells you how many bars until the next section.

## Glossary

- **Drone**: a long continuous root tone; the foundation of ethno and ambient backgrounds.
- **Pad**: a soft, slow chord that "holds" the harmony.
- **Mode**: the set of notes the melody lives in; the tonic is its centre.
- **BPM**: beats per minute, the tempo. **Meter**: how beats group (4/4, 7/8 = 3+2+2).
- **dB**: sound level; 0 dB is the ceiling before the limiter, −18 dB is a good level for a layer.
- **Pan**: left–right. **HP/LP**: cut below/above (remove bass/treble). **Crossfade**: a smooth join at a loop's seam.
- **Macro**: one knob that moves many parameters at once.
- **Preset**: saved settings of one panel. **Session**: saved settings of everything.
- **Sidechain**: the bass "breathes" with every kick. **Wobble**: a slow, tempo-synced sweep of the bass filter.
- **Stem**: a separate track on export (drums only, pad only…).

## If a screen crashes

If a screen shows "This screen crashed" with the error text, the rest of the app (the
background, the assistant bridge, the transport) keeps working. "Try again" renders the
screen again; "← Home" takes you back to Home. Copy the error text and send it so we can fix it.
- If the error is in the shared part of the app (the bottom bar, the background), the same panel fills the window instead of a blank one; "Try again" reloads.
