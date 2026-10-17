# Live Performance Director — phase plan (Phase 0, for approval)

| Phase | Scope | Acceptance (evidence) | Depends on |
|---|---|---|---|
| **0** | this audit + blueprint + contracts + test matrix; real screenshot of the current arrangement UI (`docs/audits/screenshots/2026-10-18-studio.png`) | owner approval | — |
| **1** | SongMap in ticks (migration from markers) + song transport + `SongMapView` with 6 editable sections, zoom/scroll, playhead in bars, persistence | seek/loop/tempo/restart tests; screenshots 1280/1440/1920; short demo recording | shared with P2 C |
| **2** | Cue scheduler + hand-authored cues (play/hold/rest/enter) + countdown synced to the audio clock with latency compensation + Coach cards | no stale cues after seek (test + demo); scheduling accuracy measured | 1 |
| **3** | Cue planner v1: three plans for an existing arrangement, skill modes, edits/locks, compare, deterministic regeneration, rationale | plans distinct by metric; constraints tests | 2, P2 D (arrangement) or hand-made SongMap |
| **4** | Guided recording & take review: raw WAV, latency calibration, activity aligned to cues, early/late/rest with uncertainty | synthetic + microphone session | 2 |
| **5** | Beginner coach & notation: explanations, loop practice, staff/piano roll, drop trainer, guide tone, help | accessibility checks; owner session | 4 |
| **6** | Adaptive assistant: live phrase tracking, conservative harmonization, Stable/Assisted/Creative, confidence gating | ~20 % wrong-note synthetic cases, ornaments, tempo drift, noise | 5, F8.1 |
| **7** | DAW export & controllers: aligned stems, MIDI, markers, cue sheet, footswitch/MIDI mapping | interop with a DAW (owner) | P2 F |

Proposed interleaving with the other programs: P1 R0 render → P2 B → **P3 1 = P2 C**
(one SongMap) → P1 R1/R2 → P2 D → **P3 2–3** → P2 E → P3 4–5 → P2 F → P3 6–7.

Decisions requested: (1) approve Phase 1 scope as the shared SongMap + song transport;
(2) visual + soft-click cues first, speech later; (3) the interleaving above.
