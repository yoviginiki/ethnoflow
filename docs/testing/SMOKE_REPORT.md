# Smoke report — 2026-11-01T12:18:54.498Z

App driven through the assistant bridge. 51/51 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 314 |
| project: open or create the smoke project | PASS | opened _smoke | 902 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 1062 |
| screen: home | PASS |  | 790 |
| screen: play | PASS |  | 569 |
| screen: tuner | PASS |  | 783 |
| screen: compose | PASS |  | 687 |
| screen: studio | PASS |  | 457 |
| screen: ambient | PASS |  | 561 |
| screen: scapes | PASS |  | 454 |
| screen: rhythm | PASS |  | 779 |
| screen: progression | PASS |  | 677 |
| screen: live | PASS |  | 575 |
| screen: library | PASS |  | 453 |
| screen: instrument | PASS |  | 449 |
| screen: songmap | PASS |  | 561 |
| screen: samples | PASS |  | 464 |
| screen: references | PASS |  | 572 |
| screen: academy | PASS |  | 454 |
| screen: create | PASS |  | 564 |
| screen: start | PASS |  | 452 |
| screen: help | PASS |  | 559 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 194 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11985 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1033 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1032 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1976 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2414 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1014 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4026 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 29, override round trip ok | 3437 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.14 s: 0 notes, 0 phrases, mode assisted | 4090 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 10 decisions, 172 BPM, sections 2 | 17942 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 53, 3.5 s, analysed true (7 notes) | 652 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3428 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 5 folder(s) kept, probe added and removed | 1511 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 1933 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2465 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 4 steps (пътуване: Киберпънк дъжд ← пътуване: Родопска нощ ← асистент (мост)), undo → step 3, redo → step 4 | 1493 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 572 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 17 tabs in 5 areas, each under its own area | 17491 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1106 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1860 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6820 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 54 (midi_file) | 2558 |
| presets: save, list, load | PASS | 1a1970c0d3d-fd93 | 1603 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1024 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.14 | 14536 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS | 4434 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.54 | 767 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 802 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
