# Smoke report — 2026-11-01T07:22:13.339Z

App driven through the assistant bridge. 43/43 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 317 |
| project: open or create the smoke project | PASS | opened _smoke | 578 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 858 |
| screen: home | PASS |  | 676 |
| screen: play | PASS |  | 564 |
| screen: tuner | PASS |  | 566 |
| screen: compose | PASS |  | 578 |
| screen: studio | PASS |  | 898 |
| screen: ambient | PASS |  | 459 |
| screen: soundscapes | PASS |  | 462 |
| screen: rhythm | PASS |  | 564 |
| screen: library | PASS |  | 458 |
| screen: instrument | PASS |  | 463 |
| screen: songmap | PASS |  | 562 |
| screen: samples | PASS |  | 456 |
| screen: references | PASS |  | 560 |
| screen: academy | PASS |  | 456 |
| screen: create | PASS |  | 469 |
| screen: start | PASS |  | 584 |
| screen: help | PASS |  | 571 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 646 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11949 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.24 | 1160 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1068 |
| references: add a render, analyse, phrases | PASS | tempo 120 BPM, 1 segments, 1 phrases, voiced 90% | 1795 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2530 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1019 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3946 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 14, override round trip ok | 3556 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.84 s: 0 notes, 0 phrases, mode assisted | 3467 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 1995 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 23, 3.5 s, analysed true (6 notes) | 526 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3456 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1140 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1878 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6683 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 24 (midi_file) | 2568 |
| presets: save, list, load | PASS | 1a195fc9a29-217e | 1275 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1029 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.10 | 2971 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116692935×, reverb tail -50 dBFS | 4445 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.59 | 773 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 822 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
