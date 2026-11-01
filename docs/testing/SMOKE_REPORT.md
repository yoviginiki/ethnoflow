# Smoke report — 2026-11-01T11:33:46.938Z

App driven through the assistant bridge. 49/49 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 300 |
| project: open or create the smoke project | PASS | opened _smoke | 560 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 852 |
| screen: home | PASS |  | 778 |
| screen: play | PASS |  | 569 |
| screen: tuner | PASS |  | 669 |
| screen: compose | PASS |  | 888 |
| screen: studio | PASS |  | 889 |
| screen: ambient | PASS |  | 559 |
| screen: scapes | PASS |  | 448 |
| screen: rhythm | PASS |  | 455 |
| screen: live | PASS |  | 566 |
| screen: library | PASS |  | 458 |
| screen: instrument | PASS |  | 566 |
| screen: songmap | PASS |  | 460 |
| screen: samples | PASS |  | 461 |
| screen: references | PASS |  | 571 |
| screen: academy | PASS |  | 456 |
| screen: create | PASS |  | 561 |
| screen: start | PASS |  | 452 |
| screen: help | PASS |  | 559 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 197 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12026 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1030 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1037 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1862 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2519 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1014 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4028 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 25, override round trip ok | 3445 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.98 s: 0 notes, 0 phrases, mode assisted | 3548 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 16945 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 45, 3.5 s, analysed true (7 notes) | 527 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3520 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 4 folder(s) kept, probe added and removed | 1489 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 1957 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 3 steps (Начало (както беше) ← пътуване: Родопска нощ ← асистент (мост)), undo → step 2, redo → step 3 | 1590 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 467 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 16 tabs in 5 areas, each under its own area | 16486 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1111 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 2039 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6586 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 46 (midi_file) | 2562 |
| presets: save, list, load | PASS | 1a196e2beb7-4494 | 1204 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1031 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.13 | 13509 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS | 4513 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.54 | 668 |
| cleanup: your prefs, project and screen restored | PASS | test2.ethnoflow | 1305 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
