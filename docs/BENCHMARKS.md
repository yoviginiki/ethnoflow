# Benchmarks (measured, never estimated)

Rule (master plan §12, FEATURE_SPEC F2.2): every number here comes from a run on real
hardware or a reproducible synthetic test. Guesses are not allowed in this file.

Reproduce: `cargo run --release -p ethnoflow-audio-engine --example analyze_take -- <file>`
for real material; `cargo test -p ethnoflow-audio-engine --lib pitch` for the synthetic
set. Hardware: owner's laptop (i5-12450H, 16 GB, Realtek HD Audio (SST), Windows 11).

## Pitch detection (YIN, window 1024, hop 10 ms, release build)

| material | pitch time | voiced | f0 p05 / median / p95 | confidence p10 / median | octave-sized jumps |
|---|---|---|---|---|---|
| kaval take 18.7 s, 48 kHz stereo, RMS −30 dBFS (2026-10-16) | 286 ms (65× realtime) | 35 % | 214 / 338 / 566 Hz | 0.86 / 0.97 | 21 / 621 (3.4 %) |
| `re.mp3` 59.45 s, 48 kHz stereo | 1998 ms (30× realtime) | 71 % | 272 / 330 / 377 Hz | 0.93 / 1.00 | 16 / 4151 (0.4 %) |

Synthetic (unit tests, pass/fail thresholds are the measurements' bounds):

| test | result |
|---|---|
| pure tones 110–1760 Hz at 44.1 and 48 kHz | < 1 cent error, confidence > 0.9 |
| harmonic tones (5 partials), 8 pitches 150–784 Hz | < 2 % of voiced frames ≥ 100 cents off |
| vibrato ±50 cents at 6 Hz on 440 Hz | mean error < 10 cents, max deviation < 120 cents |
| exponential sweep 200→800 Hz over 2 s | < 5 % of frames > 60 cents from expected |
| silence; full-scale LCG noise | unvoiced (hz = None or confidence < 0.5) |
| streaming tracker vs offline contour | identical time stamps and values |

Live tuner path: estimate published every 20 ms from the drain thread (recorder and
input monitor). End-to-end latency on the laptop (sound → needle): **not measured yet**
(needs a loopback test or the owner's judgement; FEATURE_SPEC F2.2 asks for < 40 ms).

## Note extraction (synthetic, unit tests)

| case | result |
|---|---|
| 4-note melody with ±5 cent jitter and a rest | 4 notes, boundaries within 20 ms, cents ≤ 5 |
| 1 s note with ±50 cent vibrato | 1 note, bend data spans 40–55 cents |
| 30 ms grace note before a 0.5 s note | 1 note; grace kept as bend at the start |
| repeated pitch split by a 60 ms rest; by a 12 dB re-articulation | 2 notes each |
| 700-cent slide over 300 ms into a held note | 1 note on the target, bend starts ≤ −600 cents |
| frames at confidence 0.3 | no notes |

Real kaval material: note precision/recall **not measured yet** (needs a hand-annotated
reference; the owner reported the extracted notes as plausible on 2026-10-17).

## Synth (built-in voices)

| test | result |
|---|---|
| A4 rendered with sine / triangle / flute, read back by YIN | < 2 cents; peak within (0.2, 1.0] |
| −100 → 0 cent bend over 0.3 s | early frames ≈ 415 Hz (±40 c), late frames 440 Hz (±2 c) |
| 5-note chord | peak ≤ 1.0 (soft limiter) |
