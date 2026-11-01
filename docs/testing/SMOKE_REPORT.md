# Smoke report — 2026-11-01T13:41:19.884Z

App driven through the assistant bridge. 53/53 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 650 |
| project: open or create the smoke project | PASS | opened _smoke | 703 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 748 |
| screen: home | PASS |  | 795 |
| screen: play | PASS |  | 469 |
| screen: tuner | PASS |  | 585 |
| screen: compose | PASS |  | 919 |
| screen: studio | PASS |  | 576 |
| screen: ambient | PASS |  | 466 |
| screen: scapes | PASS |  | 469 |
| screen: rhythm | PASS |  | 474 |
| screen: progression | PASS |  | 475 |
| screen: live | PASS |  | 581 |
| screen: library | PASS |  | 464 |
| screen: instrument | PASS |  | 470 |
| screen: songmap | PASS |  | 463 |
| screen: samples | PASS |  | 594 |
| screen: references | PASS |  | 470 |
| screen: academy | PASS |  | 464 |
| screen: create | PASS |  | 577 |
| screen: start | PASS |  | 474 |
| screen: help | PASS |  | 588 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 647 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12049 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1061 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1086 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1795 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2543 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 924 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4079 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 37, override round trip ok | 3447 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.82 s: 0 notes, 0 phrases, mode assisted | 3459 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 10 decisions, 172 BPM, sections 2 | 3560 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 69, 3.5 s, analysed true (7 notes) | 534 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3488 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 5 folder(s) kept, probe added and removed | 1534 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 1900 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2517 |
| follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode | PASS | 3 tonal layers on D F A, back to the mode without a chord | 3544 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 7 steps (пътуване: Родопска нощ ← асистент (мост) ← асистент (мост)), undo → step 6, redo → step 7 | 1536 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 491 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 18 tabs in 5 areas, each under its own area | 18446 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1133 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1877 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6735 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 70 (midi_file) | 2577 |
| presets: save, list, load | PASS | 1a197578cba-ade0 | 1202 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1063 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.11 | 4379 |
| studio: track strip — pan, send, EQ / compressor and the loudness target are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS, comp peak 0.09 vs 0.26, -40.2 → -14.1 LUFS | 6047 |
| editor: cut / undo / redo / paste / silence / jump, then save as a new take | PASS | 3.50 s → cut → 2.50 s; 4 steps; saved smoke edit | 6037 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.77 | 680 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 836 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
