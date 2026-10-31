# Smoke report — 2026-10-31T14:49:28.485Z

App driven through the assistant bridge. 39/39 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 414 |
| project: open or create the smoke project | PASS | already open | 90 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 856 |
| screen: home | PASS |  | 787 |
| screen: play | PASS |  | 561 |
| screen: tuner | PASS |  | 1004 |
| screen: compose | PASS |  | 897 |
| screen: studio | PASS |  | 457 |
| screen: ambient | PASS |  | 572 |
| screen: soundscapes | PASS |  | 467 |
| screen: rhythm | PASS |  | 477 |
| screen: library | PASS |  | 566 |
| screen: instrument | PASS |  | 464 |
| screen: songmap | PASS |  | 566 |
| screen: samples | PASS |  | 469 |
| screen: references | PASS |  | 463 |
| screen: academy | PASS |  | 450 |
| screen: create | PASS |  | 560 |
| screen: start | PASS |  | 458 |
| screen: help | PASS |  | 561 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 16 | 212 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12032 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.27 | 1043 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1047 |
| references: add a render, analyse, phrases | PASS | tempo 120.1 BPM, 1 segments, 1 phrases, voiced 92% | 1884 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2521 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1017 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3912 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 7, override round trip ok | 3557 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.84 s: 0 notes, 0 phrases, mode assisted | 3436 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 2086 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1978 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 12 (midi_file) | 2779 |
| presets: save, list, load | PASS | 1a1926fb758-4025 | 1181 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1012 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.12 | 2980 |
| studio: track strip — pan and send are real in the export | PASS | L/R 128435128×, reverb tail -56 dBFS | 4545 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.59 | 670 |
| cleanup: your prefs, project and screen restored | PASS | _smoke.ethnoflow | 795 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
