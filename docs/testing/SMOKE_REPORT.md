# Smoke report — 2026-11-01T13:01:31.472Z

App driven through the assistant bridge. 52/52 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 634 |
| project: open or create the smoke project | PASS | opened _smoke | 454 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 968 |
| screen: home | PASS |  | 780 |
| screen: play | PASS |  | 463 |
| screen: tuner | PASS |  | 905 |
| screen: compose | PASS |  | 673 |
| screen: studio | PASS |  | 456 |
| screen: ambient | PASS |  | 456 |
| screen: scapes | PASS |  | 560 |
| screen: rhythm | PASS |  | 453 |
| screen: progression | PASS |  | 450 |
| screen: live | PASS |  | 566 |
| screen: library | PASS |  | 455 |
| screen: instrument | PASS |  | 567 |
| screen: songmap | PASS |  | 456 |
| screen: samples | PASS |  | 461 |
| screen: references | PASS |  | 559 |
| screen: academy | PASS |  | 455 |
| screen: create | PASS |  | 561 |
| screen: start | PASS |  | 457 |
| screen: help | PASS |  | 560 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 196 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 12001 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1034 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1053 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1872 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2535 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1011 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4028 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 34, override round trip ok | 3437 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 1.12 s: 0 notes, 0 phrases, mode assisted | 4013 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 10 decisions, 172 BPM, sections 2 | 3051 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 63, 3.5 s, analysed true (7 notes) | 424 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3540 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 5 folder(s) kept, probe added and removed | 1502 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 1971 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2577 |
| follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode | PASS | 3 tonal layers on D F A, back to the mode without a chord | 3482 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 7 steps (пътуване: Родопска нощ ← асистент (мост) ← асистент (мост)), undo → step 6, redo → step 7 | 1496 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 475 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 17 tabs in 5 areas, each under its own area | 17550 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 1123 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1885 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 7239 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 64 (midi_file) | 2443 |
| presets: save, list, load | PASS | 1a197333898-4a1b | 1284 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1018 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 6.0 s peak 0.12 | 4489 |
| studio: track strip — pan and send are real in the export | PASS | L/R 116662531×, reverb tail -51 dBFS | 4522 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.53 | 653 |
| cleanup: your prefs, project and screen restored | PASS | no project was open | 795 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
