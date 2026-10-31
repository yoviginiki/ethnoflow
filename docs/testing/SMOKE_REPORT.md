# Smoke report — 2026-10-31T20:08:31.937Z

App driven through the assistant bridge. 41/41 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 206 |
| project: open or create the smoke project | PASS | opened _smoke | 454 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 965 |
| screen: home | PASS |  | 792 |
| screen: play | PASS |  | 452 |
| screen: tuner | PASS |  | 567 |
| screen: compose | PASS |  | 474 |
| screen: studio | PASS |  | 460 |
| screen: ambient | PASS |  | 454 |
| screen: soundscapes | PASS |  | 568 |
| screen: rhythm | PASS |  | 453 |
| screen: library | PASS |  | 567 |
| screen: instrument | PASS |  | 463 |
| screen: songmap | PASS |  | 467 |
| screen: samples | PASS |  | 564 |
| screen: references | PASS |  | 472 |
| screen: academy | PASS |  | 453 |
| screen: create | PASS |  | 566 |
| screen: start | PASS |  | 450 |
| screen: help | PASS |  | 568 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 190 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11967 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.24 | 1161 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1067 |
| references: add a render, analyse, phrases | PASS | tempo 120 BPM, 1 segments, 1 phrases, voiced 90% | 1871 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2418 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1009 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4042 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 12, override round trip ok | 3431 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.82 s: 1 notes, 0 phrases, mode assisted | 3577 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 1981 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 19, 3.5 s, analysed true (6 notes) | 528 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1988 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6707 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 20 (midi_file) | 2561 |
| presets: save, list, load | PASS | 1a19393d216-2749 | 1165 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1021 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.12 | 2983 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116692935×, reverb tail -50 dBFS | 4517 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.59 | 670 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 910 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
