# Smoke report — 2026-10-31T18:44:59.344Z

App driven through the assistant bridge. 40/40 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 329 |
| project: open or create the smoke project | PASS | opened _smoke | 1001 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 2601 |
| screen: home | PASS |  | 565 |
| screen: play | PASS |  | 564 |
| screen: tuner | PASS |  | 889 |
| screen: compose | PASS |  | 1130 |
| screen: studio | PASS |  | 457 |
| screen: ambient | PASS |  | 452 |
| screen: soundscapes | PASS |  | 566 |
| screen: rhythm | PASS |  | 451 |
| screen: library | PASS |  | 561 |
| screen: instrument | PASS |  | 461 |
| screen: songmap | PASS |  | 458 |
| screen: samples | PASS |  | 575 |
| screen: references | PASS |  | 454 |
| screen: academy | PASS |  | 456 |
| screen: create | PASS |  | 552 |
| screen: start | PASS |  | 565 |
| screen: help | PASS |  | 571 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 16 | 642 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11919 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.27 | 1135 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1047 |
| references: add a render, analyse, phrases | PASS | tempo 120.1 BPM, 1 segments, 1 phrases, voiced 92% | 1864 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2534 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1002 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3921 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 8, override round trip ok | 3539 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.36 s: 2 notes, 0 phrases, mode assisted | 3994 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 3483 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 2024 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6681 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 14 (midi_file) | 3230 |
| presets: save, list, load | PASS | 1a1934755ec-e683 | 1650 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 911 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.10 | 3049 |
| studio: track strip — pan and send are real in the export | PASS | L/R 128435128×, reverb tail -56 dBFS | 4511 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.59 | 774 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 696 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
