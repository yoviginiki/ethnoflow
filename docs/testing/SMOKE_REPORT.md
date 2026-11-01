# Smoke report — 2026-11-01T11:18:21.174Z

App driven through the assistant bridge. 48/48 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 552 |
| project: open or create the smoke project | PASS | opened _smoke | 713 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 645 |
| screen: home | PASS |  | 822 |
| screen: play | PASS |  | 599 |
| screen: tuner | PASS |  | 1341 |
| screen: compose | PASS |  | 593 |
| screen: studio | PASS |  | 484 |
| screen: ambient | PASS |  | 611 |
| screen: scapes | PASS |  | 917 |
| screen: rhythm | PASS |  | 1019 |
| screen: live | PASS |  | 892 |
| screen: library | PASS |  | 471 |
| screen: instrument | PASS |  | 622 |
| screen: songmap | PASS |  | 942 |
| screen: samples | PASS |  | 496 |
| screen: references | PASS |  | 512 |
| screen: academy | PASS |  | 485 |
| screen: create | PASS |  | 492 |
| screen: start | PASS |  | 491 |
| screen: help | PASS |  | 598 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 681 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12021 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1088 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1226 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 2228 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2449 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 929 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4003 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 24, override round trip ok | 3512 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.24 s: 0 notes, 0 phrases, mode assisted | 4001 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 21484 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 43, 3.5 s, analysed true (7 notes) | 573 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3509 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 4 folder(s) kept, probe added and removed | 1475 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 2021 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 603 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 16 tabs in 5 areas, each under its own area | 16374 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1184 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1857 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 7236 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 44 (midi_file) | 2599 |
| presets: save, list, load | PASS | 1a196d47370-6641 | 1610 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1018 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.13 | 13911 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS | 15548 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.54 | 758 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 791 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
