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
- `prefs`: `drone`, `pad`, `intent`, `harmony`, `rhythm`, `masterDb`, `stripControl`.
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
| `screen` | `screen`: home/play/compose/studio/ambient/soundscapes/rhythm/library | navigates |
| `set_prefs` | `prefs`: any of the pref keys, partial objects merge | drone/pad/rhythm/harmony/intent/master |
| `layers` | `drone`/`pad`/`ambient`/`rhythm`/`metronome`: bool | **starts or stops sound** — tell the user first |
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
