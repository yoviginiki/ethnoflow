# Smoke report — 2026-10-31T19:23:36.712Z

App driven through the assistant bridge. 40/40 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 537 |
| project: open or create the smoke project | PASS | opened _smoke | 459 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 1072 |
| screen: home | PASS |  | 672 |
| screen: play | PASS |  | 568 |
| screen: tuner | PASS |  | 467 |
| screen: compose | PASS |  | 564 |
| screen: studio | PASS |  | 451 |
| screen: ambient | PASS |  | 456 |
| screen: soundscapes | PASS |  | 561 |
| screen: rhythm | PASS |  | 461 |
| screen: library | PASS |  | 560 |
| screen: instrument | PASS |  | 453 |
| screen: songmap | PASS |  | 463 |
| screen: samples | PASS |  | 559 |
| screen: references | PASS |  | 449 |
| screen: academy | PASS |  | 562 |
| screen: create | PASS |  | 456 |
| screen: start | PASS |  | 565 |
| screen: help | PASS |  | 455 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 206 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12084 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.24 | 1049 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1047 |
| references: add a render, analyse, phrases | PASS | tempo 120 BPM, 1 segments, 1 phrases, voiced 90% | 1879 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2418 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1015 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4021 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 10, override round trip ok | 3535 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.72 s: 1 notes, 0 phrases, mode assisted | 3459 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 2004 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1984 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6678 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 16 (midi_file) | 2676 |
| presets: save, list, load | PASS | 1a1936ab226-0306 | 1177 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1020 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.13 | 2957 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116692935×, reverb tail -50 dBFS | 4552 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.59 | 671 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 798 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
