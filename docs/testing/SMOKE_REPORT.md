# Smoke report — 2026-11-01T07:55:13.893Z

App driven through the assistant bridge. 45/45 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 640 |
| project: open or create the smoke project | PASS | opened _smoke | 463 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 956 |
| screen: home | PASS |  | 781 |
| screen: play | PASS |  | 445 |
| screen: tuner | PASS |  | 571 |
| screen: compose | PASS |  | 466 |
| screen: studio | PASS |  | 449 |
| screen: ambient | PASS |  | 566 |
| screen: soundscapes | PASS |  | 455 |
| screen: rhythm | PASS |  | 566 |
| screen: live | PASS |  | 454 |
| screen: library | PASS |  | 456 |
| screen: instrument | PASS |  | 566 |
| screen: songmap | PASS |  | 455 |
| screen: samples | PASS |  | 560 |
| screen: references | PASS |  | 451 |
| screen: academy | PASS |  | 562 |
| screen: create | PASS |  | 454 |
| screen: start | PASS |  | 454 |
| screen: help | PASS |  | 563 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 196 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12020 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.24 | 1027 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1141 |
| references: add a render, analyse, phrases | PASS | tempo 120 BPM, 1 segments, 1 phrases, voiced 90% | 1867 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2420 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1003 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4030 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 16, override round trip ok | 3550 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.84 s: 0 notes, 0 phrases, mode assisted | 3431 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 2082 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 27, 3.5 s, analysed true (6 notes) | 413 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3537 |
| background: the six tabs are screens of the Background area (compose, ambient, soundscapes, rhythm, instrument, live) | PASS | 6 tabs, all under "background" | 5957 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1222 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1876 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6645 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 28 (midi_file) | 2559 |
| presets: save, list, load | PASS | 1a1961ad307-1b95 | 1271 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1032 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.11 | 2972 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116692935×, reverb tail -50 dBFS | 4505 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.59 | 653 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 784 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
