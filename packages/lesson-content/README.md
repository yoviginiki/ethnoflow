# lesson-content

Academy lessons as data (FEATURE_SPEC F6.1; plan in `docs/academy/CURRICULUM.md`).

- `lessons/level<N>.json` — an array of lessons for level N (0–6). The schema and the
  validator live in `crates/lessons` (`Lesson`), which embeds these files at build time
  so the app works offline. `cargo test -p ethnoflow-lessons` validates every file.
- Texts are BG + EN (`{"bg": …, "en": …}`); explanations stay within three sentences.
- Exercise types and their parameters: CURRICULUM §3. Choice keys in ear exercises are
  translated by the UI (`interval:7`, `chord:major`, `mode:dorian`, `meter:7/8:2+2+3`).
- `fingerings/` (F2.6) is planned here too, after the owner's kaval table arrives.

License: the lesson texts are © EthnoFlow Studio, CC BY 4.0.
