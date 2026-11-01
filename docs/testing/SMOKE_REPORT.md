# Smoke report — 2026-11-01T09:28:37.883Z

App driven through the assistant bridge. 48/48 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 429 |
| project: open or create the smoke project | PASS | opened _smoke | 689 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 753 |
| screen: home | PASS |  | 783 |
| screen: play | PASS |  | 452 |
| screen: tuner | PASS |  | 470 |
| screen: compose | PASS |  | 905 |
| screen: studio | PASS |  | 684 |
| screen: ambient | PASS |  | 468 |
| screen: scapes | PASS |  | 467 |
| screen: rhythm | PASS |  | 803 |
| screen: live | PASS |  | 688 |
| screen: library | PASS |  | 469 |
| screen: instrument | PASS |  | 576 |
| screen: songmap | PASS |  | 458 |
| screen: samples | PASS |  | 469 |
| screen: references | PASS |  | 580 |
| screen: academy | PASS |  | 462 |
| screen: create | PASS |  | 472 |
| screen: start | PASS |  | 570 |
| screen: help | PASS |  | 582 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 653 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11930 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.26 | 1176 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1073 |
| references: add a render, analyse, phrases | PASS | tempo 80 BPM, 1 segments, 1 phrases, voiced 90% | 1771 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2523 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1018 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3936 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 21, override round trip ok | 3557 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.80 s: 0 notes, 0 phrases, mode assisted | 3491 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 3007 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 37, 3.5 s, analysed true (7 notes) | 540 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3438 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 4 folder(s) kept, probe added and removed | 1512 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into the rack, silently, and the rack is restored | PASS | 6 layers, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, phase now: Начало | 1993 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 585 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 16 tabs in 5 areas, each under its own area | 16359 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1239 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1779 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6248 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 38 (midi_file) | 2574 |
| presets: save, list, load | PASS | 1a19670536d-01b3 | 1173 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1039 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 33.5 s peak 0.13 | 3442 |
| studio: track strip — pan and send are real in the export | PASS | L/R 117675072×, reverb tail -51 dBFS | 4537 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.54 | 769 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 791 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
