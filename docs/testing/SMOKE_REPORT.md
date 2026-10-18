# Smoke report — 2026-10-18T21:36:40.166Z

App driven through the assistant bridge. 36/36 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 328 |
| project: open or create the smoke project | PASS | opened _smoke | 461 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 1103 |
| screen: home | PASS |  | 685 |
| screen: play | PASS |  | 468 |
| screen: compose | PASS |  | 480 |
| screen: studio | PASS |  | 490 |
| screen: ambient | PASS |  | 478 |
| screen: soundscapes | PASS |  | 566 |
| screen: rhythm | PASS |  | 469 |
| screen: library | PASS |  | 469 |
| screen: instrument | PASS |  | 555 |
| screen: songmap | PASS |  | 480 |
| screen: samples | PASS |  | 467 |
| screen: references | PASS |  | 578 |
| screen: academy | PASS |  | 470 |
| screen: create | PASS |  | 474 |
| screen: start | PASS |  | 469 |
| screen: help | PASS |  | 575 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 214 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11991 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.28 | 1353 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1634 |
| references: add a render, analyse, phrases | PASS | tempo 80.2 BPM, 1 segments, 2 phrases, voiced 92% | 2070 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2395 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1765 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3807 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 5, override round trip ok | 4010 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.00 s: 0 notes, 0 phrases, mode assisted | 4008 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 4927 |
| presets: save, list, load | PASS | 1a150f1d1aa-6fb6 | 1603 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1034 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.12 | 3343 |
| studio: track strip — pan and send are real in the export | PASS | L/R 123413073×, reverb tail -61 dBFS | 4542 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.59 | 1296 |
| cleanup: your prefs, project and screen restored | PASS | _smoke.ethnoflow | 1200 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
