# Smoke report — 2026-11-01T09:03:38.841Z

App driven through the assistant bridge. 46/46 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 424 |
| project: open or create the smoke project | PASS | opened _smoke | 577 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 970 |
| screen: home | PASS |  | 684 |
| screen: play | PASS |  | 567 |
| screen: tuner | PASS |  | 561 |
| screen: compose | PASS |  | 906 |
| screen: studio | PASS |  | 458 |
| screen: ambient | PASS |  | 581 |
| screen: rhythm | PASS |  | 452 |
| screen: live | PASS |  | 567 |
| screen: library | PASS |  | 451 |
| screen: instrument | PASS |  | 452 |
| screen: songmap | PASS |  | 565 |
| screen: samples | PASS |  | 455 |
| screen: references | PASS |  | 559 |
| screen: academy | PASS |  | 456 |
| screen: create | PASS |  | 454 |
| screen: start | PASS |  | 564 |
| screen: help | PASS |  | 566 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 194 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12037 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.26 | 1033 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1042 |
| references: add a render, analyse, phrases | PASS | tempo 80 BPM, 1 segments, 1 phrases, voiced 90% | 1866 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2533 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1008 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3908 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 20, override round trip ok | 3563 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.82 s: 0 notes, 0 phrases, mode assisted | 3459 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 1980 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 35, 3.5 s, analysed true (7 notes) | 528 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3562 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 2 folder(s) kept, probe added and removed | 1395 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 568 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 15 tabs in 5 areas, each under its own area | 15465 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1127 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1887 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6700 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 36 (midi_file) | 2570 |
| presets: save, list, load | PASS | 1a196597604-4069 | 1280 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1026 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.10 | 2942 |
| studio: track strip — pan and send are real in the export | PASS | L/R 117675072×, reverb tail -51 dBFS | 4519 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.53 | 642 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 796 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
