# Live Performance Director — test matrix (Phase 0)

Every row records PASS/FAIL/SKIP with environment, command and evidence file when run.
SKIP is not PASS. Budgets are measured on the owner's laptop first, then written here.

| ID | Area | Test | Phase |
|---|---|---|---|
| PD-T-01 | Transport | start, pause, seek, loop, tempo change keep clips/rhythm/program aligned (offline render diff) | 1 |
| PD-T-02 | Transport | non-4/4 and additive meters: bar:beat display and bar grid correct | 1 |
| PD-T-03 | SongMap | invalid boundaries, overlaps, references rejected; migration from markers; reload | 1 |
| PD-T-04 | SongMap | undo/redo of section edits | 1 |
| PD-T-05 | Scheduler | pre-cue offsets fire at the right frame (±1 block) | 2 |
| PD-T-06 | Scheduler | seek/loop invalidates pending cues; no duplicates | 2 |
| PD-T-07 | Scheduler | 10-minute run: no drift vs beat clock | 2 |
| PD-T-08 | Scheduler | output latency compensation applied to UI events (measured ms) | 2 |
| PD-T-09 | Planner | same seed → same plan; different seeds → plans differ by metric | 3 |
| PD-T-10 | Planner | constraints: range, max phrase, protected rests, density per skill | 3 |
| PD-T-11 | Planner | section policies: pre-drop rest window honoured when enabled, never mandatory | 3 |
| PD-T-12 | Audio | mic permission failure, device disconnect, clipping, silence, breath noise, sample rates | 4 |
| PD-T-13 | Analysis | ornament vs stable pitch, skipped notes, noisy input, timing offset, low confidence | 4/6 |
| PD-T-14 | UI | keyboard-only operation; 125–200 % scaling; screenshots 1280/1440/1920 without clipping | 1+ |
| PD-T-15 | Export | WAV alignment, MIDI timing, markers, project round-trip, raw take untouched | 7 |
| PD-T-16 | Human | rehearsal of one song by the owner with cues; notes on timing feel | 2+ |
| PD-T-17 | Human | listening: cues audible/visible without masking the kaval | 2+ |
