# Smoke report — 2026-10-18T18:21:17.369Z

App driven through the assistant bridge. 33/33 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 530 |
| project: open or create the smoke project | PASS | opened _smoke | 456 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 960 |
| screen: home | PASS |  | 802 |
| screen: play | PASS |  | 560 |
| screen: compose | PASS |  | 888 |
| screen: studio | PASS |  | 469 |
| screen: ambient | PASS |  | 563 |
| screen: soundscapes | PASS |  | 456 |
| screen: rhythm | PASS |  | 564 |
| screen: library | PASS |  | 454 |
| screen: instrument | PASS |  | 553 |
| screen: songmap | PASS |  | 453 |
| screen: samples | PASS |  | 458 |
| screen: references | PASS |  | 561 |
| screen: academy | PASS |  | 452 |
| screen: start | PASS |  | 563 |
| screen: help | PASS |  | 564 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 16 | 626 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12007 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.28 | 1370 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1703 |
| references: add a render, analyse, phrases | PASS | tempo 120.5 BPM, 1 segments, 1 phrases, voiced 94% | 1972 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2511 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 907 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 43 lessons, context 57..81, rhythm score 1 | 4025 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 1, override round trip ok | 3553 |
| presets: save, list, load | PASS | 1a1503eef7a-d900 | 1490 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 5.0 s peak 0.14 | 1127 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.12 | 3394 |
| studio: track strip — pan and send are real in the export | PASS | L/R 127908475×, reverb tail -60 dBFS | 4488 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 9 | 2071 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 904 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
