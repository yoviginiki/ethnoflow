# Smoke report — 2026-11-01T08:33:17.632Z

App driven through the assistant bridge. 45/45 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 648 |
| project: open or create the smoke project | PASS | opened _smoke | 470 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 974 |
| screen: home | PASS |  | 777 |
| screen: play | PASS |  | 452 |
| screen: tuner | PASS |  | 562 |
| screen: compose | PASS |  | 453 |
| screen: studio | PASS |  | 565 |
| screen: ambient | PASS |  | 449 |
| screen: soundscapes | PASS |  | 451 |
| screen: rhythm | PASS |  | 562 |
| screen: live | PASS |  | 453 |
| screen: library | PASS |  | 567 |
| screen: instrument | PASS |  | 456 |
| screen: songmap | PASS |  | 558 |
| screen: samples | PASS |  | 456 |
| screen: references | PASS |  | 472 |
| screen: academy | PASS |  | 562 |
| screen: create | PASS |  | 455 |
| screen: start | PASS |  | 560 |
| screen: help | PASS |  | 453 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 199 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12018 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1142 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1049 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1876 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2423 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1017 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4028 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 18, override round trip ok | 3442 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.84 s: 3 notes, 0 phrases, mode assisted | 3568 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 1985 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 31, 3.5 s, analysed true (7 notes) | 533 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3439 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 16 tabs in 5 areas, each under its own area | 16562 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1112 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1870 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6648 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 32 (midi_file) | 2666 |
| presets: save, list, load | PASS | 1a1963daa15-18c0 | 1168 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1015 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.11 | 2954 |
| studio: track strip — pan and send are real in the export | PASS | L/R 117404984×, reverb tail -50 dBFS | 4533 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.52 | 655 |
| cleanup: your prefs, project and screen restored | PASS | _smoke.ethnoflow | 1290 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
