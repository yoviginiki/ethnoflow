# Live Performance Director — UI blueprint (Phase 0, for approval)

Screen "Диригент" (Home area "Диригент на живо"), Bulgarian first, English keys. Target
1440×900; verified at 1280×720 and 1920×1080; 125 % scaling. Dark theme as the rest of the
app. No text in graphics. Keyboard first-class.

```
┌─ Transport 64 px ───────────────────────────────────────────────────────────────┐
│ ▶ ■ ● | 174 BPM 4/4 | 0:48.2 | Такт 29.1 / 128 | 🔁 | 🎵 метроном | ⏱ лат. 12 ms | Режим: Репетиция ▾ │
├─ Navigator 240 px ─┬─ Song map (flex) ──────────────────────────┬─ Coach 300 px ─┤
│ Секции            │ ruler: тактове + mm:ss, zoom −/+          │ СЕГА            │
│  Intro 1–16       │ ┌Intro┐┌Build┐┌Drop 1┐┌Break┐┌Drop 2┐┌Outro┐│ Задръж дълъг Ре  │
│  Build 17–32      │ energy curve ───────────────────────────  │ меко · lead     │
│ ▸Drop 1 33–64     │ lanes: барабани ▮▮▮  бас ▮▮  пад ▬▬  кавал ▬ │ ───────────────  │
│  Break 65–80      │ cues: [▶ влез][⏸ пауза][↻ повтори][✎ отговори] │ СЛЕДВА в 4 т.   │
│  Drop 2 81–112    │ transitions: ⟋ fill  ⟍ cut  ~ sweep         │ Пауза преди дропа│
│  Outro 113–128    │ playhead ▏ (strong)  countdown badge 4·3·2·1│ „остави място“  │
│ Планове: A B C    │                                            │ ───────────────  │
│ Инструмент: кавал │                                            │ ПО-КЪСНО        │
│ Ниво: начинаещ    │                                            │ Влез на дропа   │
│ Роля: lead ▾      │                                            │ [Приеми][Скрий] │
├─ Bottom 180 px (collapsible) ────────────────────────────────────────────────────┤
│ нотен ред / piano roll на водещата мелодия · жива линия на височината · такт/удари │
└──────────────────────────────────────────────────────────────────────────────────┘
```

## Regions and behaviour

- **Transport**: play/stop/record; BPM and meter read from the SongMap (editable in
  Темпо и такт); timecode + bar:beat from the song transport events; loop toggle (region
  by bars); metronome; measured latency with "калибрирай"; mode switch Репетиция /
  Запис / Импровизация. Space, R, M, L shortcuts; Esc dismisses the current cue card.
- **Navigator**: sections list (click = seek, drag = reorder later), bookmarks, cue plans
  A/B/C with "сравни", instrument (from the instrument profile), skill level, style, role.
- **Song map**: horizontally scrollable lanes; section blocks with role colors (subdued,
  colorblind-safe pairs + labels); bar grid; zoom; energy curve (target from the plan,
  actual from offline proxies when available); activity lanes per role; cue blocks on the
  performer lane (drag to move, trim ends, lock 🔒, right-click edit); transition glyphs;
  a strong playhead; a countdown badge near the next transition (bars·beats).
- **Coach**: three cards NOW / NEXT / LATER with large action text (Play / Hold / Rest /
  Enter / Repeat / Answer / Improvise / Exit), pitch target or "свободно", dynamic
  (soft/medium/strong), one-line rationale ("пространство преди дропа"), buttons Приеми /
  Скрий / 🔒; pre-cue flashes at 8/4/2/1 bars (configurable), beats 4·3·2·1 before an
  Enter cue. Reduced-motion option disables flashes.
- **Bottom**: staff or piano roll of the guide melody/motif, live pitch trace from the
  tuner path, collapsible (▾).

## States
Empty (no song map → "Генерирай аранжимент" or "Създай секции"), rehearsal, recording
(red frame, raw take preserved), reviewing a take (markers of early/late/rest with
uncertainty), plan generation running, no microphone (cues still run from the timeline).

## Control inventory (v0; full CONTROL_CATALOG.csv before Phase 1 code)

| id | control | range/default | effect | persistence |
|---|---|---|---|---|
| transport.play/stop/record | buttons | — | song transport | — |
| transport.loop_bars | region in bars | 2/4/8/16, off | loop | project |
| transport.metronome | toggle | off | metronome stream | prefs |
| transport.mode | Репетиция/Запис/Импровизация | Репетиция | cue behaviour, recording | project |
| map.zoom | slider | bars per screen 8–256 | view | prefs |
| map.section.* | drag/split/merge/rename/role | — | SongMap | project |
| cue.action | enum | — | cue model | project (plan) |
| cue.start/end | ticks | snap to beat | cue model | project |
| cue.lock | toggle | off | planner respects | project |
| plan.generate | button + seed | — | planner | project |
| plan.skill | начинаещ/среден/напреднал | начинаещ | planner density | project |
| coach.precue_bars | 8/4/2/1 multi | 4,1 | pre-cue timing | prefs |
| coach.accept/dismiss | buttons | — | plan approval state | project |
| latency.calibrate | button | measured ms | countdown offset | prefs/device |

Visual verification: screenshots of the real app at 1280×720 / 1440×900 / 1920×1080 after
Phase 1 and each later phase, compared to this blueprint; deviations listed in
`docs/design/performance-director/REFERENCE_COMPARISON.md`.
