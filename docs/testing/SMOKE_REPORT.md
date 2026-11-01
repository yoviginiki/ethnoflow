# Smoke report — 2026-11-01T11:05:06.083Z

App driven through the assistant bridge. 48/48 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 626 |
| project: open or create the smoke project | PASS | opened _smoke | 456 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 961 |
| screen: home | PASS |  | 782 |
| screen: play | PASS |  | 562 |
| screen: tuner | PASS |  | 889 |
| screen: compose | PASS |  | 578 |
| screen: studio | PASS |  | 454 |
| screen: ambient | PASS |  | 566 |
| screen: scapes | PASS |  | 449 |
| screen: rhythm | PASS |  | 566 |
| screen: live | PASS |  | 449 |
| screen: library | PASS |  | 455 |
| screen: instrument | PASS |  | 560 |
| screen: songmap | PASS |  | 449 |
| screen: samples | PASS |  | 567 |
| screen: references | PASS |  | 454 |
| screen: academy | PASS |  | 562 |
| screen: create | PASS |  | 452 |
| screen: start | PASS |  | 453 |
| screen: help | PASS |  | 563 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 197 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12006 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1138 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1038 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1858 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2530 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 904 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3999 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 23, override round trip ok | 3555 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.00 s: 0 notes, 0 phrases, mode assisted | 3985 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 17038 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 41, 3.5 s, analysed true (7 notes) | 431 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3541 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 4 folder(s) kept, probe added and removed | 1500 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 2050 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 464 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 16 tabs in 5 areas, each under its own area | 16501 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1106 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1859 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6796 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 42 (midi_file) | 2570 |
| presets: save, list, load | PASS | 1a196c87894-fbe2 | 1602 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1017 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.13 | 15072 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS | 4521 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.54 | 721 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 772 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
