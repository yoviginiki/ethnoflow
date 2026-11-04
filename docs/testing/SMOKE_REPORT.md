# Smoke report — 2026-11-04T19:34:42.524Z

App driven through the assistant bridge. 63/63 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 221 |
| project: open or create the smoke project | PASS | opened _smoke | 1561 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 864 |
| screen: home | PASS |  | 795 |
| screen: play | PASS |  | 577 |
| screen: tuner | PASS |  | 1341 |
| screen: compose | PASS |  | 575 |
| screen: studio | PASS |  | 461 |
| screen: ambient | PASS |  | 561 |
| screen: scapes | PASS |  | 473 |
| screen: rhythm | PASS |  | 793 |
| screen: progression | PASS |  | 689 |
| screen: live | PASS |  | 480 |
| screen: library | PASS |  | 577 |
| screen: instrument | PASS |  | 467 |
| screen: songmap | PASS |  | 458 |
| screen: samples | PASS |  | 569 |
| screen: references | PASS |  | 461 |
| screen: academy | PASS |  | 463 |
| screen: create | PASS |  | 576 |
| screen: start | PASS |  | 465 |
| screen: help | PASS |  | 582 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 654 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12020 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1144 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1031 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1872 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2418 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1014 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4014 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 56, override round trip ok | 3435 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.12 s: 0 notes, 0 phrases, mode assisted | 4004 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 9 decisions, 172 BPM, sections 0 | 20111 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 144 smoke edit, 2.5 s, analysed true (6 notes) | 419 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3557 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 6 folder(s) kept, probe added and removed | 1493 |
| nyquist: every plug-in in the folder runs with its defaults (bundled + Documents\EthnoFlow\Plugins) | PASS | 43 plug-ins OK (22 generators); slowest Surf [LFO]… 2789 ms | 6109 |
| nyquist generator → WAV sample for the sample lists (bundled Tone, 1.5 s pluck at D4) | PASS | smoke tone.wav · 1.50 s · 48000 Hz · 1 ch | 910 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 1976 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2451 |
| follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode | PASS | 3 tonal layers on D F A, back to the mode without a chord | 3480 |
| the atmosphere knows the bar: with sync = every beat, Soundscapes bells land on the beat grid of the export | PASS | 7/7 on the grid at 172 BPM; free: 14/16 off | 3070 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 11 steps (асистент (мост) ← асистент (мост) ← асистент (мост)), undo → step 10, redo → step 11 | 1491 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 574 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 18 tabs in 5 areas, each under its own area | 18352 |
| keyboard: the dock switches on with the project's voice (live synth runs), points at the instrument, off | PASS | ports 1, synth on → instrument → off | 9281 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 905 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening true | 1889 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6711 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 145 (midi_file) | 2585 |
| presets: save, list, load | PASS | 1a1a80cf725-8947 | 1162 |
| CLAP: fixture scan + render | PASS | not run: build the fixture first (cargo test -p ethnoflow-plugin-host) | 50 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.45 | 18018 |
| atmosphere: a Pulse layer (S3) swells audibly in the export and its params round-trip | PASS | atmosphere.wav 480.0 s peak 0.59 | 10907 |
| atmosphere: bells with Orbit (S5) move between left and right in the export | PASS | L/R balance per second -0.03 0.09 0.01 0.34 -0.42 -0.75 -0.21 | 13044 |
| studio: track strip — pan, send, EQ / compressor and the loudness target are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS, comp peak 0.09 vs 0.26, envelope tail 0.1 %, -21.6 → -14.0 LUFS | 6963 |
| editor: cut / undo / redo / paste / silence / jump, then save as a new take | PASS | 3.50 s → cut → 2.50 s; 4 steps; saved smoke edit | 18561 |
| export: FLAC 24-bit mix and stems (magic bytes, smaller than WAV) | PASS | 79178 kB FLAC vs 135000 kB WAV 24, 1 stems | 16060 |
| export: one file per song-map section (A10 multiple export) | PASS | 3 files, first 480.00 s | 9404 |
| what next: the header hint follows the path (takes → background → export) | PASS | nextBackground → start | 526 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.79 | 757 |
| template: a new project from the liquid DnB style has the progression, the world and the layers set | PASS | genre liquid, world (journey built) | 1711 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 1015 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
