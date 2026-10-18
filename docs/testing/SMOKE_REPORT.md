# Smoke report — 2026-10-18T19:19:37.311Z

App driven through the assistant bridge. 35/35 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 698 |
| project: open or create the smoke project | PASS | opened _smoke | 478 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 1000 |
| screen: home | PASS |  | 691 |
| screen: play | PASS |  | 588 |
| screen: compose | PASS |  | 994 |
| screen: studio | PASS |  | 892 |
| screen: ambient | PASS |  | 500 |
| screen: soundscapes | PASS |  | 460 |
| screen: rhythm | PASS |  | 572 |
| screen: library | PASS |  | 466 |
| screen: instrument | PASS |  | 479 |
| screen: songmap | PASS |  | 480 |
| screen: samples | PASS |  | 601 |
| screen: references | PASS |  | 896 |
| screen: academy | PASS |  | 753 |
| screen: create | PASS |  | 1044 |
| screen: start | PASS |  | 785 |
| screen: help | PASS |  | 593 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 16 | 656 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12041 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.28 | 1243 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1668 |
| references: add a render, analyse, phrases | PASS | tempo 120.5 BPM, 1 segments, 1 phrases, voiced 94% | 2137 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2309 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 995 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4066 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 3, override round trip ok | 3589 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.02 s: 0 notes, 0 phrases, mode assisted | 4402 |
| presets: save, list, load | PASS | 1a1507458cc-722d | 1521 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 5.0 s peak 0.14 | 1129 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.10 | 3374 |
| studio: track strip — pan and send are real in the export | PASS | L/R 127908475×, reverb tail -60 dBFS | 4488 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 9 | 1774 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 784 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
