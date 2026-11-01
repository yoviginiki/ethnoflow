# Smoke report — 2026-11-01T20:02:08.389Z

App driven through the assistant bridge. 59/59 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 649 |
| project: open or create the smoke project | PASS | opened _smoke | 1113 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 860 |
| screen: home | PASS |  | 681 |
| screen: play | PASS |  | 573 |
| screen: tuner | PASS |  | 999 |
| screen: compose | PASS |  | 458 |
| screen: studio | PASS |  | 449 |
| screen: ambient | PASS |  | 587 |
| screen: scapes | PASS |  | 463 |
| screen: rhythm | PASS |  | 909 |
| screen: progression | PASS |  | 562 |
| screen: live | PASS |  | 575 |
| screen: library | PASS |  | 451 |
| screen: instrument | PASS |  | 463 |
| screen: songmap | PASS |  | 575 |
| screen: samples | PASS |  | 478 |
| screen: references | PASS |  | 459 |
| screen: academy | PASS |  | 563 |
| screen: create | PASS |  | 487 |
| screen: start | PASS |  | 462 |
| screen: help | PASS |  | 565 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 196 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11964 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1130 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1037 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1864 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2527 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1013 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3920 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 46, override round trip ok | 3540 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.94 s: 0 notes, 0 phrases, mode assisted | 3992 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 10 decisions, 172 BPM, sections 5 | 19455 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 103 AEROJet_Blast Off Clean_344 Audio_Air Designed, 8.7 s, analysed true (2 notes) | 532 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3547 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 5 folder(s) kept, probe added and removed | 1507 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 1976 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: iadd9 · IV7 · vii7 · vi7; edited to isus2 · vimaj7, follow on, silence | 2461 |
| follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode | PASS | 3 tonal layers on D F A, back to the mode without a chord | 3482 |
| the atmosphere knows the bar: with sync = every beat, Soundscapes bells land on the beat grid of the export | PASS | 8/8 on the grid at 172 BPM; free: 15/17 off | 3065 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 11 steps (асистент (мост) ← асистент (мост) ← асистент (мост)), undo → step 10, redo → step 11 | 1504 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 562 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 18 tabs in 5 areas, each under its own area | 18383 |
| keyboard: the dock switches on with the project's voice (live synth runs), points at the instrument, off | PASS | ports 1, synth on → instrument → off | 9831 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 891 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1773 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6230 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 104 (midi_file) | 2568 |
| presets: save, list, load | PASS | 1a198b35e06-78d3 | 1292 |
| CLAP: fixture scan + render | PASS | not run: build the fixture first (cargo test -p ethnoflow-plugin-host) | 47 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.36 | 16911 |
| studio: track strip — pan, send, EQ / compressor and the loudness target are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS, comp peak 0.09 vs 0.26, envelope tail 0.1 %, -21.6 → -14.0 LUFS | 6925 |
| editor: cut / undo / redo / paste / silence / jump, then save as a new take | PASS | 3.50 s → cut → 2.50 s; 4 steps; saved smoke edit | 17510 |
| export: FLAC 24-bit mix and stems (magic bytes, smaller than WAV) | PASS | 78176 kB FLAC vs 135000 kB WAV 24, 1 stems | 15670 |
| export: one file per song-map section (A10 multiple export) | PASS | 4 files, first 480.00 s | 12063 |
| what next: the header hint follows the path (takes → background → export) | PASS | nextBackground → start | 304 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.78 | 773 |
| template: a new project from the liquid DnB style has the progression, the world and the layers set | PASS | genre liquid, world (journey built) | 1625 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 1113 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
