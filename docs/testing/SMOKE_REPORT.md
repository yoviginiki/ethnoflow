# Smoke report — 2026-11-01T08:54:01.777Z

App driven through the assistant bridge. 45/45 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 645 |
| project: open or create the smoke project | PASS | opened _smoke | 564 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 859 |
| screen: home | PASS |  | 788 |
| screen: play | PASS |  | 571 |
| screen: tuner | PASS |  | 1003 |
| screen: compose | PASS |  | 895 |
| screen: studio | PASS |  | 562 |
| screen: ambient | PASS |  | 459 |
| screen: rhythm | PASS |  | 456 |
| screen: live | PASS |  | 571 |
| screen: library | PASS |  | 461 |
| screen: instrument | PASS |  | 456 |
| screen: songmap | PASS |  | 557 |
| screen: samples | PASS |  | 452 |
| screen: references | PASS |  | 562 |
| screen: academy | PASS |  | 454 |
| screen: create | PASS |  | 456 |
| screen: start | PASS |  | 558 |
| screen: help | PASS |  | 563 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 197 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11938 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1162 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 964 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1885 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2531 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1017 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 3929 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 19, override round trip ok | 3549 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.84 s: 1 notes, 0 phrases, mode assisted | 3550 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 1985 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 33, 3.5 s, analysed true (7 notes) | 426 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3547 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 580 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 15 tabs in 5 areas, each under its own area | 15378 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1230 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1768 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6714 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 34 (midi_file) | 2672 |
| presets: save, list, load | PASS | 1a19650a798-1a60 | 1597 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1139 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.12 | 2860 |
| studio: track strip — pan and send are real in the export | PASS | L/R 117404984×, reverb tail -50 dBFS | 4536 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.52 | 761 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 797 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
