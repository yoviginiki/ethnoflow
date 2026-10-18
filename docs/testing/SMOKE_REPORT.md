# Smoke report — 2026-10-18T16:07:14.816Z

App driven through the assistant bridge. 30/30 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 421 |
| project: open or create the smoke project | PASS | opened _smoke | 458 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 972 |
| screen: home | PASS |  | 787 |
| screen: play | PASS |  | 565 |
| screen: compose | PASS |  | 453 |
| screen: studio | PASS |  | 447 |
| screen: ambient | PASS |  | 457 |
| screen: soundscapes | PASS |  | 563 |
| screen: rhythm | PASS |  | 458 |
| screen: library | PASS |  | 562 |
| screen: instrument | PASS |  | 456 |
| screen: songmap | PASS |  | 567 |
| screen: samples | PASS |  | 456 |
| screen: references | PASS |  | 451 |
| screen: start | PASS |  | 562 |
| screen: help | PASS |  | 556 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 16 | 198 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12022 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.28 | 1138 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1152 |
| references: add a render, analyse, phrases | PASS | tempo 120.5 BPM, 1 segments, 1 phrases, voiced 94% | 2212 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2406 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 997 |
| presets: save, list, load | PASS | 1a14fc43b44-6e61 | 1496 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 5.0 s peak 0.14 | 1136 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.10 | 2945 |
| studio: track strip — pan and send are real in the export | PASS | L/R 127908475×, reverb tail -60 dBFS | 4410 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 9 | 1624 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 913 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
