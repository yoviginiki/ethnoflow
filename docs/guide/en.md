# How to use EthnoFlow Studio

This guide is also shown inside the app (Home → Help, with search). It lives in one place: `docs/guide/bg.md` (Bulgarian) and `docs/guide/en.md` (English). Every new feature adds its paragraph here.

## Home and projects

- Top right is the **📁 Project** menu: new, open, recent, close. Everything in a project saves itself; there is no Save button.
- A project is a folder `Documents\EthnoFlow\<name>.ethnoflow` with the takes (originals are never modified), the analysis, the atmosphere and the remembered settings (`session.json`).
- Every project **remembers its own settings**: drone, pad, rhythm, harmony, atmosphere. Open another project and its settings come back.
- The floating bar at the bottom appears when something plays in the background: play/stop, switches for drone, pad, atmosphere and rhythm, the level, and quick buttons to the screens.

## Background mixer

- The **🎚** button (in the floating bar and wherever "Background: play and level" is) opens the mixer: one strip each for drone, pad, atmosphere and rhythm, plus the master.
- Each strip has **Playing/Stopped**, a meter, **Level** (a trim on top of the modules' own levels), **Pan**, **HP/LP** cuts and **Fade in/out** in seconds. Switching on is no longer a click: the layer comes in over its fade-in and goes out over its fade-out (defaults 2 s / 3 s, rhythm 0.5 s / 1 s).
- The strip's cuts add to the modules' own (the stricter wins). Level at the minimum = the layer is off.
- Mixer settings are part of the session and are remembered per project.

## Start from a melody

The quickest path from an idea to a background:

1. **Melody**: record with the microphone (🎙 Record), import a file (📂 Import: WAV, MP3, FLAC, OGG, MIDI) or pick an existing take.
2. **🔍 Analyse**. Nothing changes by itself; the analysis only proposes: a key with percentages (pick another or "no key"), a tempo with percentages (or the project's), and the melody's register (lowest, highest and median note).
3. **Variants**: five complete starting points tuned to the melody: *Quiet ambient*, *Dark drone*, *Liquid DnB*, *Ambient DnB*, *Ethno pulse*. Each card says what it assumes for key, tempo and meter and where that comes from (analysis, project, or suggested). **▶ Try** writes the key and tempo into the project and starts everything; **💾 Save as session** stores it in the Library with notes; **↶ Undo** restores what was there before the first try.
4. Then go to Compose, Soundscapes or the Rhythm section and shape it.

The pad and the textures always sit **below** the melody (about two octaves under its median note) so they never fight it.

## Play & record

- Pick the microphone and headphones in the device settings. The metronome has its own ▶/■ next to "Tempo & meter".
- **Record** makes a new take in the project. The background (drone, pad, atmosphere, rhythm) is heard while you play but **is not recorded**: only the microphone is. For a mixed result use My studio → Export mix.
- The tuner shows the note, the cents and whether it is in the project's mode.
- "Analyse" here does the same as in Start from a melody but without the variants: contour, notes (editable), key proposals.

## Compose (accompanist)

- **Mode**: choose a tonic and mode or accept a proposal from the analysis. The drone and the pad follow the tonal centre.
- **Drone**: a root tone with fifth, octave, sub, warmth, movement and breath. The lowest layer.
- **Pad**: a chord from the mode with a voicing, an emotion and an X/Y macro. Its register is best kept below the melody.
- **Harmony**: proposals for chord movement in three styles with A/B. "Intent" follows what you play and suggests tonic and chord.
- Styles ("Deep Ethno DnB", "Ambient no key"…) are quick starting settings.

## Atmosphere

- Up to 8 layers: noise, wind, texture, bells, plucks, looped take. Each has level, pan, filters, its own parameters, M/S and a lock.
- **Presets** on the left (factory and yours), "Surprise me" changes the unlocked layers, the history goes back.
- **Macros** act on everything: intensity, brightness, motion, space and its size, tonal gravity, width, **shimmer** (the space's tail comes back transposed by an octave, a fifth or octave+fifth: an "angelic choir" behind every tone; 20–40 % for glitter, 60 %+ for a cloud of overtones).
- **Automation and scenes**: lanes over time and a program of scenes; export to WAV with stems.
- **Samples**: "Import samples…" or drag files onto the window (e.g. from Soundly). Each file becomes a take and a looped layer.
- A slider at its minimum means the layer is off (silence), not "very quiet".

## My samples

- From Home → **My samples**. Add folders ("Soundly Audio" is offered on first open if it exists): the app scans them recursively for WAV, MP3, FLAC, OGG without copying anything.
- Search by name and path, sort by name, date or length; ▶ plays the file through the shared output (stops by itself, or with ■).
- "+ Take" imports it into the project; "+ Loop" and "+ Cloud" import it and immediately make a layer in the atmosphere. From there it is a take like any other.
- Soundly workflow: pick and download sounds in Soundly → "Rescan" here → listen → "+".

## References (whole songs to learn from)

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

## Soundscapes

The same layers as Atmosphere, one at a time, with big knobs:

- Pick a **Module** (layer) at the top or add a new one by kind. Under the title is "How to use it" for that module.
- **Resonator** (modal synthesis): a struck body in the mode, from string/tube (Structure 0) to bowl/bell (1); Strike, Brightness, Decay, Density. Good for sparse bell hits over the pad or a wooden pulse under the melody.
- **Cloud** (granular): pick a take (kaval, voice, rain) and it becomes an endless pad of small grains. Position = where in the take, Scan 0 = freezes the moment, Grain and Density go from whisper to choir, Pitch transposes (−12 = an octave down), Reverse and Octave up are per-grain probabilities. The strongest ambient module for your own recordings.
- Knobs: level with a meter, pan, the module's parameters, HP/LP. Drag up/down; Shift = fine; double-click = default; the wheel works too.
- **Shuffle** gives new values to this module only (the level stays); 🔒 protects it from "Surprise me".
- **Module presets** (💾/📚) with tags and notes, in the shared Library.
- On the right: a mini mixer and the macros as knobs. Whatever you do here shows in Atmosphere too.

## Song map

- From Home → **Song map** or from 🗺 in My studio. Sections (intro, establish, build, drop, variation, break, bridge, outro) sit on a bar ruler; edges drag by whole bars, "Split at position" cuts on a bar line, ⇥ merges with the next, 🔒 locks.
- Every section has **energy** and **density** (0..1): the targets the Composer and the Director will read. The map is stored in the project in ticks (with tempo and meter maps); the studio's markers are updated from it.
- Clicking the ruler moves the clip transport; the current bar and the bars until the next section are shown. One beat transport for all layers is the next step.

## Instruments (designer)

- From Home → **Instruments** or from ✎ next to each part in the rhythm section. Left: factory and your presets by category (drums, bass, synth) with search. Right: the chosen instrument with knobs.
- **Audition** plays a short pattern through exactly this sound at the project tempo; knobs are heard at once. **WAV** renders the same pattern to a file for comparison.
- The knobs are real sound parameters: for the bass voice (sub/pluck/Reese), attack/decay/sustain/release, pitch sweep, microvariation, cutoff/resonance/filter envelope, wobble, drive, warmth, glide, sidechain; for the drums kick fundamental, decay, brightness and a level per drum; for the synth timbre, attack, release, tone, dynamics. Every knob has a tooltip saying what it changes in the sound.
- **Save as…** makes your preset (factory ones never change; the copy remembers its parent). **Update** overwrites your preset. 🗑 deletes yours.
- **Use in** sends the preset to the rhythm section; each part there has a 🎛 picker ("own sound" = the part's fields). The choice is kept in the project's session.
- Why it matters: the four parts are no longer the same instruments in every project. Two kits or two basses must sound different, not just be named differently.

## Rhythm section

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

## My studio (tracks)

- Takes are arranged as clips on tracks, with section markers and one transport. Drone, pad, atmosphere and rhythm play together with the clips.
- **Import** adds a file as a new take and a clip at the cursor; **Export mix** writes the whole song: the clips plus the switched-on layers (drone, pad, atmosphere with its scenes, rhythm) through the same graph and limiter. Next to the WAV a folder holds one stem per group (same start and length), markers.csv with the song map's sections and manifest.json (tempo, meter, sections) for a DAW.
- Trimming, silence at the edges ("Silence analysis"), snapping to zero crossings.

## Library, presets and sessions

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
