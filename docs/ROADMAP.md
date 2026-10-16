# Roadmap and phase gates

Each phase ends with a real demo and an `Implemented / Tested / Known Issues / Next /
User Decisions` report, then stops for approval. Phases are from the master plan §14;
this file tracks status and the concrete Phase 1 plan.

| phase | name | status |
|---|---|---|
| 0 | Discovery & feasibility | done; verified on the owner's Windows laptop 2026-10-16 (CHANGELOG) |
| 1 | Core recording & project system | **implemented 2026-10-16 (0.1.0)**; DoD manual items (≥3-min reopen, real crash, 30-min, clean-machine install) pending owner — see CHANGELOG known issues |
| 2 | Pitch, notation & practice | **approved by owner 2026-10-16; plan below; in progress** |
| 3 | Harmony / drone MVP | not started |
| 4 | Liquid DnB engine | not started |
| 5 | Mini DAW + export | not started |
| 6 | Beginner Academy v1 | not started |
| 7 | Intelligent variations | not started |
| 8 | Live accompanist R&D | not started |
| 9 | Release readiness | not started |

## Phase 0 — exit criteria vs. reality

Master plan exit: "installable app that records microphone and plays back WAV reliably".

| criterion | state |
|---|---|
| repository, docs, ADRs, CLAUDE.md | done |
| Tauri skeleton builds and runs | done (Linux release binary, Xvfb screenshots in `docs/phase0/`) |
| Rust audio spike: device → record → waveform → play → WAV | done; 37 automated tests green; hardware path exercised only through an ALSA null device |
| Python packaging spike | deferred to Phase 2 start (ADR-0011) — not needed by P0/P1 |
| installable Windows build | **not produced here** (Linux sandbox). `npm run tauri build` on Windows is step 4 of the hand-off in CLAUDE.md |
| real microphone test | **pending** on the owner's laptop |

Phase 0 is therefore *technically complete, physically unverified on Windows*. The
first hour on the laptop closes that gap before Phase 1 starts.

## Phase 1 — Core recording & project system (proposed plan)

Exit (master plan): record and reopen an original kaval take with no data loss.

### 1.1 Streaming recorder (replaces in-memory cap)
- Drain thread writes `assets/originals/<take-id>.wav` (32-bit float) incrementally;
  header finalised on stop; recovery pass on startup repairs a header left open by a crash.
- Peak cache computed incrementally during recording (live waveform while recording,
  not just a time bar).
- Tests: long-recording simulation (synthetic source, 30+ min equivalent), crash-recovery
  fixture (truncated WAV), overrun injection.

### 1.2 Project system (`packages/project-schema` + SQLite)
- `.ethnoflow` directory layout from master plan §10; `project.json` v1 with schema
  version, sample rate, tempo map placeholder, markers, takes list.
- SQLite (`rusqlite`, bundled) for metadata, checksums (BLAKE3) of originals, autosave
  every N seconds + on every state change; migrations table from day one.
- Tests: save/load roundtrip, migration v0→v1, missing-file relink, checksum mismatch
  detection.

### 1.3 Transport + metronome
- `Transport` abstraction (position, play/stop/record, loop range, tempo, time signature
  incl. 5/8, 7/8, 9/8 groupings); metronome click rendered in the engine (sample-accurate),
  count-in, mute during playback.
- Tests: click placement sample-accurate at several tempos/meters; loop boundaries.

### 1.4 Device settings & robustness
- Sample-rate/channel selection where the device supports it; input gain meter with
  noise-floor estimate; device test tone; feedback warning when input monitoring is on.
- Device disconnect/reconnect: stream error → clean stop, take preserved, UI message.
- Latency calibration tool (loopback click test) — measurement only, used in Phase 2.

### 1.5 Import
- MP3/FLAC/OGG decode via `symphonia` (ADR-0004); owner's `re.mp3` used locally only.
- Non-destructive trim (in/out points stored in the project; original untouched).

### 1.6 UI
- Home screen stub with the seven areas from master plan §9 (only "Свири и записвай"
  active, the rest clearly marked as not yet available — no fake buttons).
- Project open/new/recent; take list; take rename; delete with confirmation.
- Keyboard: Space play/stop, R record, Esc stop; screen-reader labels on controls.

### 1.7 Definition of done
- Record a real kaval take ≥ 3 min → close app → reopen → identical bytes (checksum).
- Kill the app mid-recording → reopen → the take up to the crash is recoverable.
- 30-minute continuous record + playback on the owner's laptop without dropouts
  (overrun counter stays 0) — logged in CHANGELOG with hardware details.
- Windows NSIS installer built from CI/script; clean-machine smoke test.

Estimated effort: 2–3 weeks of focused work.

### Предложени допълнения от `INSTRUMENT_MACHINE_CONCEPT.md` §8 (чакат одобрение)

Не променят Phase 1. При одобрение се вписват в съответната фаза по-долу.

| модул | фаза |
|---|---|
| Live input monitor, Audio-to-MIDI с pitch bend, предложени (не автоматични) корекции | 2 |
| Scale Lock на компютърната клавиатура | 2 |
| Liquid Pad (емоции, X/Y), Liquid Resonator drone за монохорд | 3 |
| Groove Engine, Sub-Bass с auto-sidechain, X/Y Wobble & Glide | 4 |
| CLAP host spike + първи C++ инструмент (ADR-0018, proposed) | 4 (край) |
| Phrase & Vocal Chopper, formant shifter, Chop Randomizer | 5 (sampler) |
| Auto Studio Engineer (видима верига), Organic Space | 5 |
| Академия overlay, X-Ray, Coach, Урок №1 | 6 |
| Evolve Melody, Mutate Voice, умни зарове с locks | 7 |
| NKS, RØDECaster pads мапинг (MIDI вход = ново ADR) | 8 или 9, след лицензна проверка |
| Ambient инструмент (granular) | след 9 |

### User decisions taken before Phase 1 (2026-10-16)
1. Plan approved as is.
2. `re.mp3` may be used locally for import tests (never uploaded, never committed).
3. Default export format: 32-bit float.
4. Project location default: `%USERPROFILE%\Documents\EthnoFlow\`.
5. Hardware: Intel i5-12450H, 16 GB, RTX 4050 Laptop, Realtek HD Audio (SST) + Nahimic;
   microphone/headphones used for the manual tests still to be reported.

## Phase 2 — Pitch, notation & practice (plan, approved 2026-10-16)

Exit (master plan): the user records a melody, sees plausible notes, edits mistakes,
exports MIDI. Everything here is a *proposal* to the user (confidence shown, manual
choice, original audio and raw contour untouched) — master plan §5.2/§5.3,
`INSTRUMENT_MACHINE_CONCEPT.md` §6.

### 2.1 `crates/music-core` (pure, no audio I/O)
- Pitch ↔ note: configurable reference (A4 = 440 by default, user-settable), MIDI number,
  cents offset, octave; transposing-instrument offset kept separate from sounding pitch.
- Scales/modes: diatonic modes, harmonic/melodic minor, phrygian dominant (hijaz),
  double harmonic, pentatonics, plus a user-defined scale (arbitrary cents). Scale
  membership and nearest-degree queries.
- Tonic/mode hypotheses from a pitch histogram: ranked list with confidence, never a
  single answer; no default key.
- Tests: conversions at several references, membership, hypotheses on synthetic pitch
  sets (clear major, clear hijaz, ambiguous).

### 2.2 Pitch detection in Rust (ADR-0011 path: DSP first)
- Monophonic f0 estimator (YIN/pYIN-style: difference function, CMNDF, threshold,
  parabolic interpolation) with a confidence value and RMS; hop ≈ 5–10 ms.
- Real-time: per-block estimate from the recorder's drain thread (never the audio
  callback), exposed in `RecorderStatus` for the tuner.
- Offline: contour over a whole take, saved to `analysis/<take>.contour.json`
  (time, f0, confidence, rms); raw contour is kept separate from notes.
- Tests: pure tones (error < 1 cent), sweeps, vibrato, breath noise → low confidence,
  octave-error rate measured on synthetic material; a benchmark on the owner's kaval
  take reports numbers (accuracy, cents error, latency), never a made-up percentage.
- New dependency only if measured necessary (e.g. `rustfft`, MIT) → ADR.

### 2.3 Note extraction (offline, proposal only)
- Contour → segments: stable-pitch regions, onsets, rests/breaths, confidence gating;
  ornaments and slides are kept as pitch-bend data inside a note, not deleted.
- Output `analysis/<take>.notes.json`: notes with start/end, MIDI number, cents
  deviation, confidence, bend curve. Quantisation to a grid only if the project has a
  tempo and only as a separate, reversible step.
- Tests: synthetic melodies with known notes (precision/recall), grace notes, a long
  tone with vibrato stays one note.

### 2.4 Desktop
- **Tuner** (live): note, octave, Hz, cents needle, confidence; in-scale indicator when
  the project has a chosen scale (otherwise neutral, no assumed key).
- **Pitch contour** overlay on the waveform; **piano-roll note view** with manual editing
  (move pitch, drag boundaries, split/merge, delete, restore) — edits stored in
  `analysis/`, audio untouched, undo.
- **Scale suggestion** panel: ranked hypotheses with percentages; the user picks one or
  none; the choice is saved in `project.json`.
- **Practice: "Hit the note"** (first ear lesson): a target note is shown, the tuner
  detects when the user holds it within ±N cents; no scoring beyond that in Phase 2.

### 2.5 MIDI export
- Standard MIDI File (format 0/1) from the edited notes, pitch bend for the bend curve
  (range declared via RPN), note velocities from RMS. Tempo: the project's tempo map if
  set; otherwise an explicitly labelled technical tempo so that ticks map to seconds
  (documented in a README next to the file) — no musical tempo is assumed.
- Own minimal SMF writer (no dependency) with tests that parse the bytes back;
  the owner re-imports into a DAW and confirms timing.

### 2.6 Definition of done
- A real kaval melody: contour and notes shown within 2 s of stopping (release build);
  the user corrects at least one note; MIDI exported and re-imported in a DAW with
  correct timing; the raw contour and the original WAV are byte-identical before/after.
- Measured and logged: tuner update latency, octave-error rate on synthetic set,
  note precision/recall on synthetic melodies, numbers on the kaval take.
- No key, tempo or meter assumed anywhere; all tests and clippy green.

### User decisions needed during Phase 2
1. Default tuning reference 440 Hz (changeable per project)?
2. Which scale families to show first for kaval material (suggest: diatonic modes,
   hijaz/phrygian dominant, harmonic minor, double harmonic, pentatonic)?
3. Is a kaval a transposing instrument in your practice (display in concert or written
   pitch)?
