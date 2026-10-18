# Smoke report — 2026-10-18T17:26:40.054Z

App driven through the assistant bridge. 32/32 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 211 |
| project: open or create the smoke project | PASS | opened _smoke | 569 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 862 |
| screen: home | PASS |  | 790 |
| screen: play | PASS |  | 570 |
| screen: compose | PASS |  | 457 |
| screen: studio | PASS |  | 454 |
| screen: ambient | PASS |  | 565 |
| screen: soundscapes | PASS |  | 450 |
| screen: rhythm | PASS |  | 464 |
| screen: library | PASS |  | 565 |
| screen: instrument | PASS |  | 462 |
| screen: songmap | PASS |  | 566 |
| screen: samples | PASS |  | 464 |
| screen: references | PASS |  | 458 |
| screen: academy | PASS |  | 571 |
| screen: start | PASS |  | 459 |
| screen: help | PASS |  | 563 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 16 | 197 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12033 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.28 | 1365 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1697 |
| references: add a render, analyse, phrases | PASS | tempo 120.5 BPM, 1 segments, 1 phrases, voiced 94% | 1880 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2529 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1024 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 43 lessons, context 57..81, rhythm score 1 | 3915 |
| presets: save, list, load | PASS | 1a1500ceb74-fedf | 1489 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 5.0 s peak 0.14 | 1236 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.12 | 3287 |
| studio: track strip — pan and send are real in the export | PASS | L/R 127908475×, reverb tail -60 dBFS | 5070 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 9 | 1965 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 1010 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
