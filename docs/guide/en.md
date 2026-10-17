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
- **Macros** act on everything: intensity, brightness, motion, space and its size, tonal gravity, width.
- **Automation and scenes**: lanes over time and a program of scenes; export to WAV with stems.
- **Samples**: "Import samples…" or drag files onto the window (e.g. from Soundly). Each file becomes a take and a looped layer.
- A slider at its minimum means the layer is off (silence), not "very quiet".

## Soundscapes

The same layers as Atmosphere, one at a time, with big knobs:

- Pick a **Module** (layer) at the top or add a new one by kind. Under the title is "How to use it" for that module.
- **Cloud** (granular): pick a take (kaval, voice, rain) and it becomes an endless pad of small grains. Position = where in the take, Scan 0 = freezes the moment, Grain and Density go from whisper to choir, Pitch transposes (−12 = an octave down), Reverse and Octave up are per-grain probabilities. The strongest ambient module for your own recordings.
- Knobs: level with a meter, pan, the module's parameters, HP/LP. Drag up/down; Shift = fine; double-click = default; the wheel works too.
- **Shuffle** gives new values to this module only (the level stays); 🔒 protects it from "Surprise me".
- **Module presets** (💾/📚) with tags and notes, in the shared Library.
- On the right: a mini mixer and the macros as knobs. Whatever you do here shows in Atmosphere too.

## Rhythm section

- Four instruments on one shared clock: **drums** (liquid, halftime, ethno), **bass** (sub, pluck, Reese with wobble and sidechain), **ostinato** (a step motif, lockable), **arpeggio**.
- Tempo and meter are the project's ("Tempo & meter"). "Suggest tempo from the take" measures a take's pulse; you accept it.
- Bass and ostinato follow the project's mode and the pad's chord. Moving a slider never interrupts the rhythm.
- "→ MIDI takes" writes the parts as MIDI into the project (drums on channel 10). "WAV + stems" renders the section offline exactly as it plays: a mix plus a folder with a stem per part. Sections (intro, verse…) are markers in My studio.
- Level at the minimum = the part is off.

## My studio (tracks)

- Takes are arranged as clips on tracks, with section markers and one transport. Drone, pad, atmosphere and rhythm play together with the clips.
- **Import** adds a file as a new take and a clip at the cursor; **Export mix** writes everything to one WAV.
- Trimming, silence at the edges ("Silence analysis"), snapping to zero crossings.

## Library, presets and sessions

- Every panel with sliders has **💾 Save preset** (name, tags, note) and **📚 Presets** (search by name, tag and note; sort; "this project only"; overwrite ⟳; edit ✎; delete 🗑).
- **Session** (top, next to the project menu) = all settings at once: drone, pad, rhythm, harmony, intent, atmosphere and which layers play.
- **Library** (Home) shows everything from every project with filters by kind, project and tag. "Load" applies the preset and opens its screen.
- Write notes: "why" and "when it works". They are searched, and they are how you learn.

## Tempo, meter and metronome

- "Tempo & meter": BPM, numerator/unit, grouping for additive meters (e.g. 7/8 = 3+2+2).
- The metronome has its own ▶/■; recording has its own click and count-in.
- Nothing assumes 4/4 or 174 BPM: without a meter the rhythm does not start, and Start from a melody says explicitly when it suggests a value.

## Bridge to Claude Code

- The 📁 Project menu has a switch **🤖 Bridge to Claude Code**. When on, the app watches a folder in your profile (the path is shown there): it writes its state and reads commands.
- What it allows: from the terminal Claude Code reads the key, tempo, takes and all settings and can change them, add modules, save and load presets and sessions, start and stop layers.
- Typical flow: record a melody → in the terminal write "make me starting presets for ambient for this take and explain" → you get settings, presets with notes and an explanation of what to turn.
- Local files only; nothing leaves the machine. Switch it off with the same switch.

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
