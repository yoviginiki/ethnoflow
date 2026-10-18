# Smoke report — 2026-10-18T18:38:54.457Z

App driven through the assistant bridge. 34/34 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 527 |
| project: open or create the smoke project | PASS | opened _smoke | 551 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 853 |
| screen: home | PASS |  | 784 |
| screen: play | PASS |  | 561 |
| screen: compose | PASS |  | 457 |
| screen: studio | PASS |  | 451 |
| screen: ambient | PASS |  | 566 |
| screen: soundscapes | PASS |  | 457 |
| screen: rhythm | PASS |  | 560 |
| screen: library | PASS |  | 456 |
| screen: instrument | PASS |  | 447 |
| screen: songmap | PASS |  | 564 |
| screen: samples | PASS |  | 455 |
| screen: references | PASS |  | 559 |
| screen: academy | PASS |  | 453 |
| screen: start | PASS |  | 449 |
| screen: help | PASS |  | 564 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 16 | 192 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12113 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.28 | 1360 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1707 |
| references: add a render, analyse, phrases | PASS | tempo 120.5 BPM, 1 segments, 1 phrases, voiced 94% | 1866 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2512 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1002 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 43 lessons, context 57..81, rhythm score 1 | 4014 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 2, override round trip ok | 3422 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.20 s: 4 notes, 0 phrases, mode assisted | 3988 |
| presets: save, list, load | PASS | 1a1504f12b8-46dd | 1601 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 5.0 s peak 0.14 | 1011 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.11 | 3487 |
| studio: track strip — pan and send are real in the export | PASS | L/R 127908475×, reverb tail -60 dBFS | 4420 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 9 | 1620 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 901 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
