# Risks

Likelihood/impact: L/M/H. Updated at each phase gate.

| # | risk | L | I | mitigation / status |
|---|---|---|---|---|
| R1 | WASAPI shared-mode latency or device quirks on the owner's hardware make cpal insufficient | M | H | Measure first (hand-off step 2–3). Fallbacks in ADR-0003 (cpal ASIO feature, JUCE sidecar). UI unaffected. |
| R2 | Unbounded memory during long recordings | was H | H | Found in P0 via null-device test; capped at 30 min (ADR-0007); streaming writer in P1 removes it. |
| R3 | Data loss on crash mid-recording | H | H | P1: incremental WAV writer + header recovery + autosave. Non-negotiable for the P1 exit. |
| R4 | Tonic/mode detection forced into Western major/minor | M | H | Multi-hypothesis posterior with confidence, user choice, no D-minor defaults; regression fixtures with modal material (P3). |
| R5 | Pitch tracking on breathy kaval tone (noise, ornaments, slides) | M | H | Keep raw pitch contour separately from quantised MIDI; confidence gating; benchmark on real recordings before quoting accuracy (P2). |
| R6 | Python/ML packaging on Windows (size, antivirus, startup) | M | M | Prefer Rust DSP/ONNX; sidecar only if unavoidable; spike at P2 start (ADR-0011). |
| R7 | Three WebViews behave differently (canvas, audio APIs) if macOS/Linux added | M | M | Canvas-only rendering, no Web Audio in UI; CI on all three OSes before promising them (ADR-0002). |
| R8 | Dependency/licence surprises (ASIO SDK, sample libraries, notation engines) | M | M | Every new dependency gets an ADR line; samples only with explicit licences; VexFlow (MIT) vs Verovio (LGPL) spike before P2 notation. |
| R9 | Scope creep: building Phase 4–7 features before P1 persistence is solid | H | M | Phase gates with owner approval; CLAUDE.md rule 12. |
| R10 | Unsigned Windows installer triggers SmartScreen warnings | H | M | Decide on a code-signing certificate before P9; document "clearly unsigned" path meanwhile. |
| R11 | Status polling adds UI-side jitter on slow laptops | L | L | Switch to Tauri events if measured (ADR-0008). |
| R12 | Sandbox verification ≠ Windows verification | H | M | Explicit hand-off checklist in CLAUDE.md; nothing in this repo claims a Windows installer exists. |
