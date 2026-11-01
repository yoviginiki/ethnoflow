# Smoke report — 2026-11-01T11:42:40.704Z

App driven through the assistant bridge. 51/51 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 636 |
| project: open or create the smoke project | PASS | opened _smoke | 565 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 864 |
| screen: home | PASS |  | 683 |
| screen: play | PASS |  | 563 |
| screen: tuner | PASS |  | 895 |
| screen: compose | PASS |  | 668 |
| screen: studio | PASS |  | 445 |
| screen: ambient | PASS |  | 452 |
| screen: scapes | PASS |  | 562 |
| screen: rhythm | PASS |  | 456 |
| screen: progression | PASS |  | 563 |
| screen: live | PASS |  | 455 |
| screen: library | PASS |  | 459 |
| screen: instrument | PASS |  | 558 |
| screen: songmap | PASS |  | 462 |
| screen: samples | PASS |  | 563 |
| screen: references | PASS |  | 448 |
| screen: academy | PASS |  | 459 |
| screen: create | PASS |  | 571 |
| screen: start | PASS |  | 460 |
| screen: help | PASS |  | 561 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 194 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12010 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1029 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1049 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1871 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2520 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1007 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4032 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 26, override round trip ok | 3525 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.02 s: 0 notes, 0 phrases, mode assisted | 3990 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 8 decisions, 172 BPM, sections 2 | 17433 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 47, 3.5 s, analysed true (7 notes) | 546 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3459 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 4 folder(s) kept, probe added and removed | 1500 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 2072 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2469 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 3 steps (Начало (както беше) ← пътуване: Родопска нощ ← асистент (мост)), undo → step 2, redo → step 3 | 1497 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 592 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 17 tabs in 5 areas, each under its own area | 17433 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1132 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1873 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6676 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 48 (midi_file) | 2569 |
| presets: save, list, load | PASS | 1a196eae7c0-60c4 | 1166 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1121 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.13 | 12951 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS | 4517 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.54 | 657 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 796 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
