# Smoke report — 2026-10-18T14:54:24.648Z

App driven through the assistant bridge. 31/31 steps passed (with sound steps).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 313 |
| project: open or create the smoke project | PASS | opened _smoke | 564 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 969 |
| screen: home | PASS |  | 669 |
| screen: play | PASS |  | 562 |
| screen: compose | PASS |  | 460 |
| screen: studio | PASS |  | 564 |
| screen: ambient | PASS |  | 456 |
| screen: soundscapes | PASS |  | 562 |
| screen: rhythm | PASS |  | 456 |
| screen: library | PASS |  | 460 |
| screen: instrument | PASS |  | 563 |
| screen: songmap | PASS |  | 452 |
| screen: samples | PASS |  | 561 |
| screen: references | PASS |  | 454 |
| screen: start | PASS |  | 566 |
| screen: help | PASS |  | 467 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 16 | 197 |
| instruments: every factory preset renders audibly (offline) | PASS | 6 presets, all audible | 3580 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.28 | 1256 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1152 |
| references: add a render, analyse, phrases | PASS | tempo 120.5 BPM, 1 segments, 1 phrases, voiced 94% | 2091 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2428 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1014 |
| presets: save, list, load | PASS | 1a14f818c6a-c691 | 1490 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 5.0 s peak 0.14 | 1137 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 9 | 641 |
| sound: rhythm live for 2 s (meters move) | PASS | bar 2, peaks 0.00/0.00/0.02/0.00, section intro | 3771 |
| sound: drone + pad for 1.5 s | PASS |  | 2518 |
| sound: instrument audition 1 s | PASS | Pluck | 1508 |
| cleanup: your prefs, project and screen restored | PASS | test.ethnoflow | 1385 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
