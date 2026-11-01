# Smoke report — 2026-11-01T12:52:35.269Z

App driven through the assistant bridge. 52/52 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 210 |
| project: open or create the smoke project | PASS | already open | 85 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 850 |
| screen: home | PASS |  | 678 |
| screen: play | PASS |  | 559 |
| screen: tuner | PASS |  | 785 |
| screen: compose | PASS |  | 777 |
| screen: studio | PASS |  | 451 |
| screen: ambient | PASS |  | 452 |
| screen: scapes | PASS |  | 553 |
| screen: rhythm | PASS |  | 455 |
| screen: progression | PASS |  | 560 |
| screen: live | PASS |  | 456 |
| screen: library | PASS |  | 450 |
| screen: instrument | PASS |  | 568 |
| screen: songmap | PASS |  | 453 |
| screen: samples | PASS |  | 568 |
| screen: references | PASS |  | 449 |
| screen: academy | PASS |  | 456 |
| screen: create | PASS |  | 568 |
| screen: start | PASS |  | 453 |
| screen: help | PASS |  | 561 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 194 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12014 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1139 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1040 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1863 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2418 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 994 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4020 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 33, override round trip ok | 3533 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.12 s: 0 notes, 0 phrases, mode assisted | 3974 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 10 decisions, 172 BPM, sections 2 | 3074 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 61, 3.5 s, analysed true (7 notes) | 422 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3544 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 5 folder(s) kept, probe added and removed | 1513 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 1962 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2467 |
| follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode | PASS | 3 tonal layers on D F A, back to the mode without a chord | 3485 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 13 steps (пътуване: Родопска нощ ← асистент (мост) ← асистент (мост)), undo → step 12, redo → step 13 | 1610 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 455 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 17 tabs in 5 areas, each under its own area | 17486 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1122 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1876 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 7312 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 62 (midi_file) | 2446 |
| presets: save, list, load | PASS | 1a1972b09fa-39f7 | 1273 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1004 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.12 | 4434 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS | 4504 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.53 | 762 |
| cleanup: your prefs, project and screen restored | PASS | _smoke.ethnoflow | 782 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
