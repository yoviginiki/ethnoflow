# Smoke report — 2026-10-18T15:54:05.618Z

App driven through the assistant bridge. 29/29 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 315 |
| project: open or create the smoke project | PASS | opened _smoke | 449 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 1064 |
| screen: home | PASS |  | 682 |
| screen: play | PASS |  | 563 |
| screen: compose | PASS |  | 456 |
| screen: studio | PASS |  | 564 |
| screen: ambient | PASS |  | 448 |
| screen: soundscapes | PASS |  | 560 |
| screen: rhythm | PASS |  | 461 |
| screen: library | PASS |  | 452 |
| screen: instrument | PASS |  | 558 |
| screen: songmap | PASS |  | 448 |
| screen: samples | PASS |  | 572 |
| screen: references | PASS |  | 454 |
| screen: start | PASS |  | 456 |
| screen: help | PASS |  | 571 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 16 | 192 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12036 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.28 | 1252 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1154 |
| references: add a render, analyse, phrases | PASS | tempo 120.5 BPM, 1 segments, 1 phrases, voiced 94% | 2088 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2518 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1008 |
| presets: save, list, load | PASS | 1a14fb83e06-befc | 1494 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 5.0 s peak 0.14 | 1024 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.10 | 3478 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 9 | 1964 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 1008 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
