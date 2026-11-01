# Smoke report — 2026-11-01T20:16:00.137Z

App driven through the assistant bridge. 59/59 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 417 |
| project: open or create the smoke project | PASS | opened _smoke | 475 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 963 |
| screen: home | PASS |  | 792 |
| screen: play | PASS |  | 564 |
| screen: tuner | PASS |  | 1438 |
| screen: compose | PASS |  | 562 |
| screen: studio | PASS |  | 892 |
| screen: ambient | PASS |  | 564 |
| screen: scapes | PASS |  | 451 |
| screen: rhythm | PASS |  | 461 |
| screen: progression | PASS |  | 565 |
| screen: live | PASS |  | 452 |
| screen: library | PASS |  | 573 |
| screen: instrument | PASS |  | 450 |
| screen: songmap | PASS |  | 453 |
| screen: samples | PASS |  | 569 |
| screen: references | PASS |  | 460 |
| screen: academy | PASS |  | 560 |
| screen: create | PASS |  | 453 |
| screen: start | PASS |  | 456 |
| screen: help | PASS |  | 568 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 194 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12015 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.27 | 1055 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1149 |
| references: add a render, analyse, phrases | PASS | tempo 80.2 BPM, 1 segments, 1 phrases, voiced 98% | 1768 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2527 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1001 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4029 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 47, override round trip ok | 3434 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.92 s: 1 notes, 0 phrases, mode assisted | 4003 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 9 decisions, 172 BPM, sections 0 | 24070 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 107 smoke edit, 2.5 s, analysed true (6 notes) | 534 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3431 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 5 folder(s) kept, probe added and removed | 1502 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 1969 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2565 |
| follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode | PASS | 3 tonal layers on D F A, back to the mode without a chord | 3496 |
| the atmosphere knows the bar: with sync = every beat, Soundscapes bells land on the beat grid of the export | PASS | 7/7 on the grid at 172 BPM; free: 14/16 off | 2986 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 11 steps (асистент (мост) ← асистент (мост) ← асистент (мост)), undo → step 10, redo → step 11 | 1502 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 579 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 18 tabs in 5 areas, each under its own area | 18405 |
| keyboard: the dock switches on with the project's voice (live synth runs), points at the instrument, off | PASS | ports 1, synth on → instrument → off | 9749 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 889 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1879 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6679 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 108 (midi_file) | 2571 |
| presets: save, list, load | PASS | 1a198c00d45-96d0 | 1267 |
| CLAP: fixture scan + render | PASS | not run: build the fixture first (cargo test -p ethnoflow-plugin-host) | 52 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.40 | 17464 |
| studio: track strip — pan, send, EQ / compressor and the loudness target are real in the export | PASS | L/R 118563757×, reverb tail -61 dBFS, comp peak 0.09 vs 0.28, envelope tail 0.1 %, -21.6 → -14.0 LUFS | 7449 |
| editor: cut / undo / redo / paste / silence / jump, then save as a new take | PASS | 3.50 s → cut → 2.50 s; 4 steps; saved smoke edit | 19045 |
| export: FLAC 24-bit mix and stems (magic bytes, smaller than WAV) | PASS | 80953 kB FLAC vs 135000 kB WAV 24, 1 stems | 16076 |
| export: one file per song-map section (A10 multiple export) | PASS | 3 files, first 480.00 s | 9343 |
| what next: the header hint follows the path (takes → background → export) | PASS | nextBackground → start | 525 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.79 | 767 |
| template: a new project from the liquid DnB style has the progression, the world and the layers set | PASS | genre liquid, world (journey built) | 1600 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 1115 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
