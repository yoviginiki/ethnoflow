# Smoke report — 2026-11-01T16:40:30.622Z

App driven through the assistant bridge. 57/58 steps passed (silent run: sound steps skipped).

| Step | Result | Detail | ms |
|---|---|---|---|
| bridge: ping | PASS | true | 311 |
| project: open or create the smoke project | PASS | opened _smoke | 578 |
| project: scale and timing round trip | PASS | D dorian, 120 BPM 7/8 (2+2+3) | 973 |
| screen: home | PASS |  | 681 |
| screen: play | PASS |  | 569 |
| screen: tuner | PASS |  | 1020 |
| screen: compose | PASS |  | 461 |
| screen: studio | PASS |  | 458 |
| screen: ambient | PASS |  | 464 |
| screen: scapes | PASS |  | 567 |
| screen: rhythm | PASS |  | 447 |
| screen: progression | PASS |  | 562 |
| screen: live | PASS |  | 457 |
| screen: library | PASS |  | 567 |
| screen: instrument | PASS |  | 466 |
| screen: songmap | PASS |  | 672 |
| screen: samples | PASS |  | 794 |
| screen: references | PASS |  | 465 |
| screen: academy | PASS |  | 562 |
| screen: create | PASS |  | 454 |
| screen: start | PASS |  | 567 |
| screen: help | PASS |  | 580 |
| prefs: partial nested merge keeps the other fields | PASS | ostinato steps 8 | 645 |
| instruments: every factory preset renders audibly (offline) | PASS | 23 presets, all audible | 11958 |
| rhythm: offline render with the four parts | PASS | rhythm-four.wav 3.5 s peak 0.25 | 1152 |
| rhythm: Instrument Rack parts render their own stems | PASS | 7 stems | 1039 |
| references: add a render, analyse, phrases | PASS | tempo 80.1 BPM, 1 segments, 1 phrases, voiced 90% | 1884 |
| song map: sections written and read back, follower reports a section | PASS | bar 6 in "build" | 2414 |
| transport: loop in bars stored and cleared | PASS | 3–6 | 1015 |
| academy: lessons validate, instances are seeded, scoring and progress round trip | PASS | 45 lessons, context 57..81, rhythm score 1 | 4053 |
| trainer: history log round trip; fingering overrides saved, applied and restored | PASS | history 42, override round trip ok | 3430 |
| live accompanist: start on the microphone, tick, locks + bypass, stop (no sound is produced) | PASS | status after 0.82 s: 0 notes, 0 phrases, mode assisted | 3547 |
| make the song: one call turns the newest take into scale + tempo + background + map + track (silent) | PASS | 9 decisions, 172 BPM, sections 0 | 24453 |
| take: the ▶ path — load the latest take through load_take (no sound) and see it loaded | PASS | take 89 smoke edit, 2.5 s, analysed true (6 notes) | 528 |
| home: the five areas open their main screens, Continue opens Play with the latest take | PASS | play→play, background→compose, song→studio, learn→academy, library→library; continue → play with the latest take loaded | 3546 |
| samples: sample_folders adds and removes a folder in My samples (prefs.sampleFolders) | PASS | 5 folder(s) kept, probe added and removed | 1501 |
| soundscapes: a world builds a whole journey (4 scenes, breathing lanes) into its own rack, silently; the atmosphere is untouched; the rack is restored | PASS | 6 layers in the Soundscapes rack, scenes Начало 26s → Развитие 40s → Връх 30s → Отзвук 24s, atmosphere untouched | 1962 |
| progression: the generator fills the bars by genre and seed, steps are editable, follow is a switch (silent) | PASS | liquid seed 7: Iadd9 · iv7 · vii7 · VI7; edited to Isus2 · VImaj7, follow on, silence | 2486 |
| follow the chord: with follow_chord on, tonal layers re-pitch to the chord (D F A), off → back to the mode | PASS | 3 tonal layers on D F A, back to the mode without a chord | 3505 |
| the atmosphere knows the bar: with sync = every beat, Soundscapes bells land on the beat grid of the export | PASS | 7/7 on the grid at 172 BPM; free: 14/16 off | 2976 |
| history: the Soundscapes rack remembers steps; undo / redo through the bridge restore them | PASS | 11 steps (асистент (мост) ← асистент (мост) ← асистент (мост)), undo → step 10, redo → step 11 | 1496 |
| soundscapes: the old screen id opens Atmosphere in the modules view (alias) | PASS | screen soundscapes → ambient (modules view) | 582 |
| areas: every tab of Play / Background / Song / Learn / Library is a screen of its area | PASS | 18 tabs in 5 areas, each under its own area | 18468 |
| keyboard: the dock switches on with the project's voice (live synth runs), points at the instrument, off | PASS | ports 1, synth on → instrument → off | 9266 |
| now playing: the bar's snapshot has every source, stop_all leaves silence | PASS | before: any=false; after stop_all: silence | 892 |
| tuner: settings round trip (432 Hz, guitar, locked A2) and the reading shape | PASS | a4 432 Hz, guitar, lock 45, listening false | 1877 |
| tuner: the sound bed switches the microphone to the bass range (30 Hz) and back | PASS | kaval 70 Hz → bed 30 Hz (lock D1 = 36.7 Hz) → restored | 6147 |
| sheet: MusicXML import becomes a MIDI take (tie, chord, rest, 7/8, 60 BPM) | PASS | take 90 (midi_file) | 2572 |
| presets: save, list, load | PASS | 1a197fb0f50-6705 | 1170 |
| CLAP: fixture scan + render | PASS | clap-fixture.wav 3.8 s peak 0.13 | 1124 |
| atmosphere: choir + bloom layers render audibly through the export | PASS | atmosphere.wav 480.0 s peak 0.44 | 16935 |
| studio: track strip — pan, send, EQ / compressor and the loudness target are real in the export | **FAIL** | envelope: head 0.0763 tail 0.0873 | 6449 |
| editor: cut / undo / redo / paste / silence / jump, then save as a new take | PASS | 3.50 s → cut → 2.50 s; 4 steps; saved smoke edit | 7513 |
| export: FLAC 24-bit mix and stems (magic bytes, smaller than WAV) | PASS | 78926 kB FLAC vs 135000 kB WAV 24, 1 stems | 17850 |
| what next: the header hint follows the path (takes → background → export) | PASS | nextBackground → start | 217 |
| export: whole song offline with stems | PASS | song.wav 6.0 s, stems 4, drums peak 0.79 | 663 |
| template: a new project from the liquid DnB style has the progression, the world and the layers set | PASS | genre liquid, world (journey built) | 1512 |
| cleanup: your prefs, project and screen restored | PASS | _smoke.ethnoflow | 1294 |

Renders: `qa/renders/smoke/` (gitignored). A FAIL names the feature; fix it before anything else.
