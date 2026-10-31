# Smoke report — 2026-10-31T20:50:24.449Z

App driven through the assistant bridge. 42/42 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 193 |
| project: open or create the smoke project | PASS | opened _smoke | 451 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 965 |
| screen: home | PASS |  | 788 |
| screen: play | PASS |  | 559 |
| screen: tuner | PASS |  | 562 |
| screen: compose | PASS |  | 890 |
| screen: studio | PASS |  | 455 |
| screen: ambient | PASS |  | 563 |
| screen: soundscapes | PASS |  | 449 |
| screen: rhythm | PASS |  | 563 |
| screen: library | PASS |  | 453 |
| screen: instrument | PASS |  | 451 |
| screen: songmap | PASS |  | 563 |
| screen: samples | PASS |  | 453 |
| screen: references | PASS |  | 564 |
| screen: academy | PASS |  | 445 |
| screen: create | PASS |  | 563 |
| screen: start | PASS |  | 452 |
| screen: help | PASS |  | 561 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 195 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11999 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.24 | 1030 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1139 |
| references: add a render, analyse, phrases | PASS | tempo 120 BPM, 1 segments, 1 phrases, voiced 90% | 1866 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2420 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1017 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4012 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 13, override round trip ok | 3550 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.82 s: 0 notes, 0 phrases, mode assisted | 3434 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 2009 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 21, 3.5 s, analysed true (6 notes) | 526 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1459 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1992 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6231 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 22 (midi_file) | 2558 |
| presets: save, list, load | PASS | 1a193ba28d4-d497 | 1278 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1025 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.13 | 2970 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116692935×, reverb tail -50 dBFS | 4520 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.59 | 672 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 798 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
