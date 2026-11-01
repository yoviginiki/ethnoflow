# Smoke report — 2026-11-01T22:20:32.002Z

App driven through the assistant bridge. 60/60 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 531 |
| project: open or create the smoke project | PASS | already open | 91 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 859 |
| screen: home | PASS |  | 795 |
| screen: play | PASS |  | 575 |
| screen: tuner | PASS |  | 896 |
| screen: compose | PASS |  | 574 |
| screen: studio | PASS |  | 455 |
| screen: ambient | PASS |  | 452 |
| screen: scapes | PASS |  | 561 |
| screen: rhythm | PASS |  | 560 |
| screen: progression | PASS |  | 447 |
| screen: live | PASS |  | 453 |
| screen: library | PASS |  | 560 |
| screen: instrument | PASS |  | 457 |
| screen: songmap | PASS |  | 566 |
| screen: samples | PASS |  | 467 |
| screen: references | PASS |  | 447 |
| screen: academy | PASS |  | 565 |
| screen: create | PASS |  | 454 |
| screen: start | PASS |  | 564 |
| screen: help | PASS |  | 457 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 204 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12014 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1035 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1150 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1873 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2469 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1039 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3933 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 51, override round trip ok | 3517 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.92 s: 0 notes, 0 phrases, mode assisted | 3974 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 9 decisions, 172 BPM, sections 0 | 22510 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 124 smoke edit, 2.5 s, analysed true (6 notes) | 527 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3545 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 6 folder(s) kept, probe added and removed | 1390 |
| nyquist generator → WAV sample for the sample lists (bundled Tone, 1.5 s pluck at D4) | PASS | smoke tone.wav · 1.50 s · 48000 Hz · 1 ch | 1003 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 2069 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2452 |
| follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode | PASS | 3 tonal layers on D F A, back to the mode without a chord | 3484 |
| the atmosphere knows the bar: with sync = every beat, Soundscapes bells land on the beat grid of the export | PASS | 7/7 on the grid at 172 BPM; free: 14/16 off | 3064 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 11 steps (асистент (мост) ← асистент (мост) ← асистент (мост)), undo → step 10, redo → step 11 | 1504 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 582 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 18 tabs in 5 areas, each under its own area | 18419 |
| keyboard: the dock switches on with the project's voice (live synth runs), points at the instrument, off | PASS | ports 1, synth on → instrument → off | 9738 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 889 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1859 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6673 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 125 (midi_file) | 2566 |
| presets: save, list, load | PASS | 1a1993219c0-0d28 | 1278 |
| CLAP: fixture scan + render | PASS | not run: build the fixture first (cargo test -p ethnoflow-plugin-host) | 50 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.43 | 16450 |
| studio: track strip — pan, send, EQ / compressor and the loudness target are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS, comp peak 0.09 vs 0.26, envelope tail 0.1 %, -21.6 → -14.0 LUFS | 7026 |
| editor: cut / undo / redo / paste / silence / jump, then save as a new take | PASS | 3.50 s → cut → 2.50 s; 4 steps; saved smoke edit | 18392 |
| export: FLAC 24-bit mix and stems (magic bytes, smaller than WAV) | PASS | 80240 kB FLAC vs 135000 kB WAV 24, 1 stems | 15675 |
| export: one file per song-map section (A10 multiple export) | PASS | 3 files, first 480.00 s | 9606 |
| what next: the header hint follows the path (takes → background → export) | PASS | nextBackground → start | 309 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.79 | 653 |
| template: a new project from the liquid DnB style has the progression, the world and the layers set | PASS | genre liquid, world (journey built) | 1725 |
| cleanup: your prefs, project and screen restored | PASS | _smoke.ethnoflow | 1118 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
