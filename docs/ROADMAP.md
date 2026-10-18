# Roadmap and phase gates

Each phase ends with a real demo and an `Implemented / Tested / Known Issues / Next /
User Decisions` report, then stops for approval. Phases are from the master plan §14;
this file tracks status and the concrete Phase 1 plan.

| phase | name | status |
|---|---|---|
| 0 | Discovery & feasibility | done; verified on the owner's Windows laptop 2026-10-16 (CHANGELOG) |
| 1 | Core recording & project system | **implemented 2026-10-16 (0.1.0)**; DoD manual items (≥3-min reopen, real crash, 30-min, clean-machine install) pending owner — see CHANGELOG known issues |
| 2 | Pitch, notation & practice | **approved 2026-10-16; 2.1–2.5 implemented the same day; DoD (2.6) needs the owner's kaval test and a DAW re-import** |
| 3 | Harmony / drone MVP | not started |
| 4 | Liquid DnB engine | **0.4.0**: rhythm section (synthesized drums, bass, ostinato, arpeggio) on one beat clock in Compose First, sections, tempo from the take, countermelody; F4.4 CLAP spike deferred (needs owner's go for the dependency) |
| 5 | Mini DAW + export | **F5.1 pulled forward (0.2.9)**: tracks/clips editor on the audio graph, after the owner's feedback that the pieces were not connected; F5.2–F5.4 not started |
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

## Order of work after Phase 2 (owner decision, 2026-10-17)

The owner's priority is the **Intelligent Harmonic Accompaniment** (Phase 3). The
updated `FEATURE_SPEC.md` also adds Phase 1A (Ambient Playground) and F1.6 (audio
graph). Both Phase 3 and Phase 1A need the audio graph (drone and pad are live
sources). Order:

1. Phase 2 remainder that does not depend on the owner: Loop drill, latency
   calibration — small; the owner-dependent checks (kaval profile, fingering data,
   Reaper re-import) proceed in parallel.
2. **F1.6 audio graph** (layers, parameter smoothing, master limiter, offline render).
3. **Phase 3**: F3.1 explanations and fixtures → F3.2 drone → F3.3 pad → F3.5 Compose
   First ("Подготви фон": tonic, mode, drone + pad in under a minute) → **F2.9–F2.11
   error-tolerant performance intelligence** (phrases, tonal tracker with hysteresis,
   note classifier, Stable/Assisted/Creative with an on/off switch and a visible badge)
   → F3.4 harmony proposals consuming IntentEstimate (3 styles, A/B) → F3.6 controls.
4. Phase 1A Ambient Playground on the same graph (**done in 0.3.0**), then Phase 4.

**Adjustment after the owner's first hands-on test (2026-10-17, 0.2.7/0.2.8):** the
owner asked where the tracks are, how to hear two things at once, and for play/stop
on the background. F5.1 (tracks and clips) was therefore pulled forward onto the
audio graph — the clips, the drone and the pad share one transport, one master and
one limiter — together with an explicit background play/stop in every screen
(0.2.9). Phase 1A and Phase 4 follow as before; F5.2–F5.4 stay in Phase 5.

## Owner's consolidated backlog (sent back 2026-10-18 with "these too, don't forget")

| # | Item | Status / where |
|---|---|---|
| 1 | Cloud (granular pad from a take) | done 2026-10-17 (F1B.2) |
| 2 | Shimmer (transposed reverb) | done 2026-10-18 (macro on the master space) |
| 3 | Resonator (bells, bowls, strings from the mode) | done 2026-10-18 (layer kind "Резонатор") |
| 4 | Sound Designer for the rhythm section | **P1 R2 done as the Instrument Designer (bass/drums/synth presets)** |
| 5 | New rhythm instruments (pad synth, keys, percussion: tapan, darbuka, daire; lead) | **tapan, chord pad, lead shipped 2026-10-18** (Instrument Rack, up to 8 extra parts); darbuka, daire, keys pending |
| 6 | Sample folder browser (Soundly Audio etc.), search, audition, "+ to Atmosphere" | done 2026-10-18 (screen "Моите семпли"; notes/tags per sample pending) |
| 7 | Choir drone (vowel), Bloom (generative melody) | **done 2026-10-18** (Soundscapes kinds "Хор" and "Блум") |
| 8 | Tape loop with live mic, Spaces (convolution) | need live input / FFT (FFT approved via ADR for P2) |
| 9 | Genre recipes in the guide (ambient, liquid DnB, ambient DnB step by step) | first version 2026-10-18 (guide § "Рецепти по жанр"); grows with each module |
| 10 | Track mixer (level, pan, solo, sends) | partial: background buses done; track strips pending (F5.2) |
| 11 | Phrase & Vocal Chopper | pending (F5.3) |
| 12 | DAW export (stems, MIDI, tempo map in one package) | WAV stems + markers.csv + manifest.json done 2026-10-18 (song export); MIDI per track pending |
| 13 | Take fade in/out; shared transport across screens; kaval fingerings (owner's table); trainer to the end | transport = P3 Phase 1; others pending |
| 14 | Academy | Phase 6 |
| 15 | Intelligent melody variations | Phase 7 / P2 D |
| 16 | Live accompanist following the playing | Phase 8 / P3 6 |
| 17 | CLAP host | **spike done 2026-10-18** (ADR-0018 accepted: clap-sys + libloading; host crate + fixture instrument + tests; `clap_scan`/`clap_render` via bridge); next: tuning via note expressions, params/state, UI picker in the Instrument Rack, C++ instrument build |
| 18 | Release 0.5.0 (installer, clean machine, owner's manual tests) | pending |

## Programs received 2026-10-18 (owner specifications; gated, audits first)

Three master specifications arrived from the owner on 2026-10-18, each with its own
phase gates and the rule "no code before the audit/plan is approved":

| Program | Spec | First gate | Audit / plan |
|---|---|---|---|
| **P1 Rhythm Laboratory & Instrument Forge** (R0–R8) | `docs/ETHNOFLOW_RHYTHM_INSTRUMENT_FORGE_MASTER_SPEC.md` | R0 audit + plan → approval → R1 registry → R2 Instrument Designer MVP (PoC: one instrument in its own page, real DSP, Save As, back into the rhythm section, save/reopen, WAV) | `docs/rhythm/AUDIT.md`, `docs/rhythm/IMPLEMENTATION_PLAN.md` |
| **P2 Music DNA Analyzer, Arrangement Composer, Musical Director** (A–G) | `docs/ETHNOFLOW_MUSIC_DNA_ARRANGEMENT_COMPOSER_MASTER_SPEC.md` | A done; **B started 2026-10-18** (`crates/analysis`, References screen: tempo/key/structure on request, take into project); C slice = Song map; F slice = song export | `docs/music-dna/` |
| **P3 Live Performance Director & Interactive Song Map** (0–7) | `docs/ETHNOFLOW_LIVE_PERFORMANCE_DIRECTOR_SPEC.md` | Phase 0 audit + UI blueprint → approval | `docs/performance-director/` |

Shared artifacts required by the quality contract: `docs/audits/CURRENT_STATE.md`
(inventory with file references and real screenshots) and
`docs/traceability/FEATURE_MATRIX.md`. They absorb and supersede the "Sound Designer /
new instruments" items of Phase 1B (F4.8/F4.9 → P1) and extend Phases 5, 7 and 8 of the
FEATURE_SPEC (studio export → P2 F; intelligent variations → P2 D/E; live accompanist →
P3). The Soundscapes modules still pending (Shimmer, Resonator, Choir drone, Bloom, Tape
loop, Spaces) continue as small slices between the gates, because they do not depend on
the programs' architecture decisions.

**Owner approval 2026-10-18 ("da")** of the three plans and their decisions (P1 R1→R2 with
the bass PoC, synthesis + own samples; P2 Rust analysis with an FFT crate via ADR, SongMap
in ticks; P3 Phase 1 = shared SongMap + song transport, visual/soft-click cues first).

**Recorded order after the owner's messages of 2026-10-18:** background mixer (done) →
audits for P1/P2/P3 (this commit) → owner approval → P1 R1/R2 PoC → P2 B/C → P3 1/2 …,
interleaved as the owner decides at each gate.

## Phase 1B — Soundscapes & Sound Designer (approved 2026-10-17, in progress)

**Owner decision (2026-10-17 evening):** "davai napred" on the proposal below, in the
proposed order. Two additions from the owner: (1) **teach me** — each module and the
genre workflows (ambient, liquid DnB, "ambient drum & bass" as a new style) must come
with in-app guidance on how to use them to make good music, not only controls; so
every F1B module ships with a short "how to use it" text in its panel (BG+EN) and
`docs/GUIDE_SOUNDSCAPES.md` grows one recipe per module; (2) **presets everywhere,
per project, and whole-screen "session" saves** — shipped the same evening (preset
library in every panel, Session in the top bar, Library screen, per-project
`session.json`). Order now: Soundscapes screen + module framework → Cloud → Shimmer
→ Resonator → Sound Designer → new instruments → Choir drone → Bloom → Tape loop
(live input) → Spaces (ADR for FFT). The sample folder browser ("Семпли") is slotted
after the Soundscapes framework.

Original proposal:

The owner asked for modern soundscape modules ("like Native Instruments": a
Soundscapes button, a dropdown of modules, knobs per module) and a sound designer for
the rhythm instruments with the ability to add more. Analysis and proposal:
`docs/SOUNDSCAPES_CONCEPT.md`; items F1B.1–F1B.8 and F4.8–F4.9 in `FEATURE_SPEC.md`.
Proposed order: screen + module framework → Cloud → Shimmer → Resonator → Sound
Designer → new instruments → Choir drone → Bloom → Tape loop (live input, decision)
→ Spaces (FFT dependency, ADR). The preset library UI (all panels with sliders) and
the sample folder browser go first because the modules' presets depend on them.

## Phase 4 — Liquid DnB machine (0.4.0, 2026-10-17) — phase report

**Implemented.** A rhythm section in "Prepare background": synthesized drums (liquid
two-step, halftime, ethno by meter groups; complexity, humanize, swing, fills), a mono
bass (sub / pluck / Reese, glide, beat-synced wobble, sidechain from the kick) that
follows the armed proposal's chord or the mode, an ostinato with lockable motifs and
an arpeggiator — all on one beat clock from the project's tempo and meter, started and
stopped with the background like the drone and the pad; locks per part and
"Regenerate" for the unlocked ones; "→ MIDI takes" (drums on GM channel 10) so bass
and drums are editable, placeable as clips and exportable as .mid; the "Deep Ethno
DnB" style (172 BPM 4/4 when nothing is set, liquid drums + Reese + pad); section
markers in "My studio"; "Suggest tempo from the take" (onsets → bpm + offset, the
take is never moved); countermelody answers in the rests → new MIDI take.

**Tested.** Workspace tests green (music-core 61 + 2 integration, engine 126, project
24 + 12, shell 10), clippy clean, typecheck, vitest 21, vite build. Key tests: groove
backbone/fills/odd meters, bass roots on changes in scale and register, ostinato
locked motif unchanged under a new seed, arpeggio directions with no hanging notes,
zero countermelody overlaps at tension 0, tempo of a rubato phrase within 1 BPM,
beat clock continuity through a tempo change, drums on the grid, bass in tune with
glide and audible sidechain, pattern synth releases when the clock stops.

**Not run.** Listening by the owner (a real ethno liquid DnB track is the exit
criterion); the spectral "< 80 Hz drops ≥ 6 dB under the kick" measurement (the
sidechain is verified as an RMS ratio); a long session with all four parts plus the
atmosphere and the tracks on one device.

**Known issues / deferred.** Patterns are edited through their MIDI takes, not with a
step sequencer in the panel; breakbeat slicing of a user sample is not built; the
tempo offset from the estimate is shown, not stored as a tempo map; F4.4 (CLAP host
spike) needs a new dependency and a C++ toolchain → waits for the owner's explicit
yes (rule 10); no recorded drum samples by policy (everything synthesized).

**Next.** Owner's listening pass of the DnB style on a kaval take; then Phase 5
(F5.2–F5.4: chopper, stems, DAW export) or fixes first.

**User decisions.** Drums are synthesized, not sampled (consistent with F1A.8); the
rhythm section is one background switch (not four) so "Play/Stop background" covers
it; "Deep Ethno DnB" sets 172 BPM / 4/4 only when the project has no tempo yet.

## Phase 1A — Ambient Playground (0.3.0, 2026-10-17) — phase report

**Implemented.** "Atmosphere" screen: up to 8 synthesized layers (noise, wind,
texture, bells, strings, looped take) on the same audio graph as the drone, the pad
and the tracks; macros, master reverb, fade out, sleep timer, never auto-play; 9
factory presets + user presets, tag browser, favourites/recents, A/B, "Surprise me"
with locks and undo, snapshot history; automation lanes (LFO, seeded walk, keyframes,
humanize) and scene programs with crossfaded transitions, loop and offline bounce with
stems; Sample Lab recipe per take (normalise, fades, DC, high-pass, loop crossfade,
reverse), silence/zero-crossing tools, tags and licence, search; licensing docs with a
CI gate. Details per feature: FEATURE_SPEC status table (F1A.1–F1A.8).

**Tested.** `cargo test --workspace` green (engine 120 + 3 integration, project 24 +
12, music-core 52, shell 10); clippy `-D warnings` clean; `npm run typecheck`; vitest
21. Key acceptance tests: 8 layers 1 min / 10 min (release, ignored test) without a
silent block or click; every slider measurably changes the sound; automation value at
t = 37.25 s identical across runs and a JSON round-trip; scene transitions without
jumps; offline render chunk-independent (< 0.1 dB per second) and stems sum to the
mix; presets save → reopen identical; randomize never touches a locked layer; bells
and strings in tune across the register; db v1/v2 → v3 upgrade.

**Not run.** Listening by the owner (the DoD says "musically convincing, not only
tests"); the clean-Windows-11 install → Play → save Program → record → loop → export →
reopen walk-through (§14 of AMBIENT_HARMONY_SPEC); a realtime-vs-offline comparison
on a real device; the 20-minute program bounce timing on the owner's laptop (release
build renders 10 min × 8 layers in ≈ 19 s on the dev machine).

**Known issues.** Layer reorder is by buttons, not drag-and-drop; the sliders show the
base state while automation/scenes move the sound (no live value display); reverb is
master-only (no per-layer send); automation has no MIDI learn (F1.4); a take's recipe
applies on load, so changing it reloads the take (a short pause); recorded clips in
"My studio" still have no latency compensation (F1.4).

**Next.** Owner's listening pass on the 9 presets and the Mountain mist / Night
scenes; then Phase 4 (ostinato, bass, Liquid DnB) per the agreed order, or fixes from
the test first.

**User decisions.** Master stays at 0 dB with a > 0 dB warning (owner's 0.2.8
decision) instead of the spec's −18 dB start; preset names are Bulgarian-first with
English in the catalogue; no recorded audio is bundled — all sounds are synthesized.

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

### 2.7 "Свири по ноти" — MIDI player & trainer, instrument profiles (added 2026-10-17)
Full spec: `docs/MIDI_PLAYER_TRAINER_SPEC.md` (§7 has the phasing and decisions).
- **2.7a** (now): `.mid` import (midly, ADR-0019) → `NoteEvent` + tempo map in
  music-core; a MIDI file is a take in the project (source `midi_file`) rendered by a
  built-in simple synth so playback/loop/export work unchanged; falling-notes view with
  a piano keyboard + the existing piano roll; tempo 25–200 %, mute/solo per track;
  "Play the notes" for notes extracted from audio (first "musical translator").
- **2.7b**: SF2 synth + soundfont, MIDI keyboard input (midir), Wait and Rhythm trainer
  modes with MIDI **and microphone** (kaval) input, timing report.
- **2.7c**: notation view spike (VexFlow), left/right hand, transposition.
- **Instrument profile** per project (shakuhachi 1.8/D, kaval D, voice, piano/MIDI
  keyboard, other): range, transposition, shown next to the tuner; the Academy uses it.

### User decisions during Phase 2 (owner delegated, 2026-10-17)
1. A4 = 440 Hz by default; per-project override stays.
2. All 13 scale kinds stay; ranking by probability is enough for now.
3. Everything shows **sounding (concert) pitch**; transposed names come with the
   instrument profile (2.7). The owner's shakuhachi is 54 cm ≈ 1.8 shaku → key of D
   (lowest note ≈ D4); the kaval is in D.
