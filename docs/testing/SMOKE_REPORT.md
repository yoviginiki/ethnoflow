# Smoke report — 2026-11-01T13:23:14.273Z

App driven through the assistant bridge. 52/52 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 417 |
| project: open or create the smoke project | PASS | already open | 87 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 851 |
| screen: home | PASS |  | 783 |
| screen: play | PASS |  | 563 |
| screen: tuner | PASS |  | 784 |
| screen: compose | PASS |  | 696 |
| screen: studio | PASS |  | 517 |
| screen: ambient | PASS |  | 488 |
| screen: scapes | PASS |  | 473 |
| screen: rhythm | PASS |  | 458 |
| screen: progression | PASS |  | 633 |
| screen: live | PASS |  | 906 |
| screen: library | PASS |  | 457 |
| screen: instrument | PASS |  | 565 |
| screen: songmap | PASS |  | 453 |
| screen: samples | PASS |  | 566 |
| screen: references | PASS |  | 464 |
| screen: academy | PASS |  | 455 |
| screen: create | PASS |  | 563 |
| screen: start | PASS |  | 458 |
| screen: help | PASS |  | 461 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 315 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11973 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1153 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1073 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1767 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2529 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1027 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3960 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 36, override round trip ok | 3431 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.12 s: 1 notes, 0 phrases, mode assisted | 4101 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 10 decisions, 172 BPM, sections 2 | 3435 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 67, 3.5 s, analysed true (7 notes) | 539 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3462 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 5 folder(s) kept, probe added and removed | 1502 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 2082 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2484 |
| follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode | PASS | 3 tonal layers on D F A, back to the mode without a chord | 3406 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 13 steps (пътуване: Родопска нощ ← асистент (мост) ← асистент (мост)), undo → step 12, redo → step 13 | 1516 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 581 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 17 tabs in 5 areas, each under its own area | 17439 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1133 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1877 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6721 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 68 (midi_file) | 2568 |
| presets: save, list, load | PASS | 1a1974713c5-9568 | 1175 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1032 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.11 | 4482 |
| studio: track strip — pan, send, EQ / compressor and the loudness target are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS, comp peak 0.09 vs 0.26, -40.2 → -14.1 LUFS | 6055 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.77 | 659 |
| cleanup: your prefs, project and screen restored | PASS | _smoke.ethnoflow | 793 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
