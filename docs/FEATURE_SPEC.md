# EthnoFlow Studio — обединена функционална спецификация за изпълнение

Този файл е **единственият** работен документ за функциите след Phase 0. Той обединява и
замества като източник на задачи: `INSTRUMENT_MACHINE_CONCEPT.md`,
`MIDI_PLAYER_TRAINER_SPEC.md`, `ACADEMY_INSPIRATION.md` (те остават като обяснение „защо").
Мастър планът (`MASTER_PLAN.md`) и `CLAUDE.md` са с предимство при конфликт.

Как се чете: всяка функция има ID, фаза, зависимости, какво точно се прави (стъпки),
къде в кода, и **Готово когато** (проверим критерий). Claude Code работи функция по
функция, в реда на фазите, и спира на края на всяка фаза за одобрение.

Принципи, които важат за всяка функция (не се повтарят по-долу):
- Асистентът **предлага**, човекът решава. Корекции се прилагат на копие, с преглед.
- Никакви hardcoded тоналност, темпо или размер. Default-ите живеят в preset-и.
- Микрофонът е първокласен вход навсякъде, където има MIDI клавиатура.
- Raw аудио не минава към UI; само редуцирани данни. RT callback без алокации.
- Всяка функция има тестове; тест, който не може да се пусне, се докладва като not run.
- UI текстове през i18n (BG + EN). Нищо фалшиво в UI; незавършено = feature flag + надпис.

---

## Статус спрямо репото (вписано 17.10.2026, версия 0.2.2)

Спецификацията е получена, след като Phase 1 и голяма част от Phase 2 вече са
реализирани. Където кодът живее другаде, отколкото спецификацията очаква, важи
реалното място (с ADR). Таблицата се поддържа при всяка завършена функция.

| F | статус | къде е реално / какво остава |
|---|---|---|
| F1.1 | **готово** (0.1.0) | `crates/audio-engine/src/{take_writer,recorder,peaks}.rs`; header checkpoint на всяка секунда, `repair_wav_header`, `LivePeaks`, команда `recording_waveform`. Remaining: тест „35 min под 50 MB RAM“ е покрит от 30-min симулация (`--ignored`), не с измерване на RAM. |
| F1.2 | **готово** | `crates/project` (ADR-0015: вместо `packages/project-schema`), SQLite миграции v1→v2, checksums, trash, relink, recovery при отваряне; начален екран със седемте области. Autosave е на всяка промяна (транзакция), без 30 s таймер. Remaining: реален kill -9 тест от собственика (ROADMAP DoD). |
| F1.3 | **частично** | `metronome.rs` (sample-accurate, групи 2+2+3…), count-in, loop и клик в `player.rs`. Няма общ `Transport` обект, tap tempo и punch-in; recorder/player не са закачени към обща позиция. |
| F1.4 | **частично** | sample rate/канали (`choose_config`), noise floor, тест тон, device_lost → чисто спиране. Remaining: latency calibration (loopback), MIDI вход (`midir`, ADR), hot-plug relist. |
| F1.5 | **частично** | symphonia декодиране (ADR-0017), trim in/out в проекта. Remaining: fade in/out параметри. |
| F2.1 | **готово** | `crates/music-core`: pitch/note, 13 лада вкл. хиджаз и двоен хармоничен, custom cents, хипотези, NoteEvent + tempo map (`events.rs`), SMF reader/writer, интервали, акорд detection, транспониране (`theory.rs`), квантизация към grid (`quantize.rs`). Имената на нотите (латиница/солфеж) са в UI i18n. |
| F2.2 | **готово** | YIN в `audio-engine/src/pitch.rs` (не в `services/analysis`): live в `RecorderStatus` и `InputMonitor`, офлайн `contour`, ноти в `music-core/src/notes.rs` (сегментация, орнаменти като bend). Числа: `docs/BENCHMARKS.md`. Remaining: onset чрез spectral flux (сега onset по RMS скок), latency на live pitch измерена на лаптопа (в BENCHMARKS като „not measured“). |
| F2.3 | **частично** | `Tuner.tsx`: нота, цента, Hz, увереност, в/извън скалата, „Уцели нотата“, режим „Дръж тона“ (лента с дрейфа за 10 s, секунди в ±15 цента). Remaining: транспонираща и профилна маркировка (с F2.5). |
| F2.4 | **частично** | piano roll с редакция (↑/↓, Delete, split, merge, влачене на начало и дължина, Alt+клик добавя, undo/redo), падащи ноти (`FallingNotes.tsx`), SMF export/import. Remaining: нотен лист (spike VexFlow, решение в MIDI_PLAYER_TRAINER_SPEC §7). Критерият „Reaper показва същите ноти“ чака собственика. |
| F2.5 | **реализирано, чака проверка** | `crates/project/src/instrument.rs` (профил, хранилище в `%APPDATA%\…\EthnoFlow Studio\instruments\<id>\`, диагностика), `audio_engine::held_tone`, desktop `instrument_cmds.rs`; UI: панел „Инструмент“ (избор/нов/откачи/изтрий), wizard „Запознай ме с инструмента“ (запис 3.5 s по нота → стабилен тон ≥ 1.5 s → семпъл + отклонение + стабилност, „чух друга нота“), пускане на семпъл (pitch shift по честота), тунерът показва профилната маркировка и написаната нота при транспозиция. Критерият „пълен профил на кавал за 2 минути“ чака Ники. |
| F2.6 | **не е започнато** | апликатури (данни + диаграма + редактор). |
| F2.7 | **частично** | `.mid` като запис, вграден синтезатор (без SF2), tempo ×0.25–2, mute по писта, глас, транспониране ±24, loop със snap към такт (при известни темпо и размер), „Пусни нотите“, „Към плейъра“, **жив синтезатор** (`live_synth.rs`, 8 гласа, lock-free команди) с компютърната клавиатура (два реда = две октави, ← → октава, Scale Lock). WAV render = обикновеният експорт на MIDI запис. Remaining: SF2 + soundfont (ADR), MIDI клавиатура (F1.4.4, `midir`), лява/дясна ръка. |
| F2.8 | **частично** | `music-core/src/trainer.rs`: `evaluate` (hit / wrong pitch / early / late / missed / extra, толеранси по ниво, latency) с тестове; UI панел „Трениране“ с вход от микрофона: **Listen**, **Wait** (свети нотата, чака я ≥150 ms, грешна = червено, без време), **Rhythm** (нотите свирят, микрофонът се сегментира в изсвирени ноти, отчет по ноти). Remaining: Loop drill (нужен е tempo factor и за аудио записи или само за MIDI), MIDI клавиатура като вход (F1.4.4), латентност от калибрация (сега 0), запис в прогрес (F6.3). Критерият „Wait с кавал на народна мелодия“ чака Ники. |
| F3–F9 | не са започнати | |

Липсващ документ: `ACADEMY_INSPIRATION.md` още не е в репото.

---

## Phase 1 — Запис, проекти, транспорт

### F1.1 Streaming recorder
Зависи от: Phase 0.
Стъпки:
1. Drain thread пише `assets/originals/<take-id>.wav` (float32) инкрементално; header се
   финализира при stop. Махни 30-min cap.
2. При старт на приложението: сканирай за незатворени WAV (header с размер 0), поправи
   header по реалния размер, маркирай take-а като „възстановен".
3. Инкрементални peaks по време на запис (нов `PeakAccumulator` в `peaks.rs`), команда
   `recording_peaks()` връща досегашните бинове; UI рисува вълната на живо.
Код: `crates/audio-engine/src/recorder.rs`, `peaks.rs`; `commands.rs`.
Готово когато: тест със синтетичен източник от 35 минути не расте в RAM над 50 MB;
тест с truncated WAV се възстановява с точния брой кадри до прекъсването; UI показва
растяща вълна при запис.

### F1.2 Project schema + SQLite
Зависи от: F1.1.
Стъпки:
1. Нов `packages/project-schema` (Rust crate `ethnoflow-project`): `project.json` v1 с
   `schema_version, name, sample_rate, created, tempo_map[], time_signatures[],
   markers[], takes[]`; `Take {id, path, checksum_blake3, duration, sample_rate,
   channels, source, created, trim_in, trim_out}`.
2. Директория `.ethnoflow/` по мастър план §10. SQLite (`rusqlite` bundled) за
   метаданни + таблица `migrations`. Autosave на всяка промяна + на 30 s.
3. Команди: `project_new/open/save/recent`, `take_list/rename/delete(confirm)/relink`.
4. UI: начален екран със седемте области от §9; активна само „Свири и записвай",
   останалите с надпис „в разработка" и без бутони, които симулират.
Код: `packages/project-schema/`, `apps/desktop/src-tauri/src/project.rs`,
`apps/desktop/src/screens/Home.tsx`, `Project.tsx`.
Готово когато: roundtrip save→close→open дава идентичен checksum на всеки take;
migration v0→v1 тест; relink на липсващ файл работи; kill -9 по време на запис →
reopen показва take до прекъсването.

### F1.3 Transport + метроном
Зависи от: F1.1.
Стъпки:
1. `Transport` в audio-engine: позиция (samples), play/stop/record, loop range, tempo
   map, time signature с групиране (7/8 = 2+2+3, 9/8 = 2+2+2+3, 5/8 = 2+3 или 3+2).
2. Метроном рендериран в engine-а (click семпъл, акцент на първо време и на група),
   count-in 1 или 2 такта, tap tempo, mute при playback.
3. Recorder и Player се закачат за Transport (punch-in от позиция).
Код: `crates/audio-engine/src/transport.rs`, `metronome.rs`.
Готово когато: тест за sample-accurate кликове при 60/120/174 BPM и 4/4, 7/8, 9/8;
loop без прекъсване; UI: Space play/stop, R record, Esc stop.

### F1.4 Устройства и устойчивост
Зависи от: Phase 0.
Стъпки:
1. Избор на sample rate/канал, където устройството го поддържа; input gain meter с
   noise floor (измерен за 2 s тишина); тест тон на изхода.
2. Stream error → чисто спиране, take се запазва, UI съобщение; hot-plug relist.
3. Latency calibration: loopback клик тест, запазва ms в настройките.
4. MIDI вход през `midir`: списък устройства, hot-plug, note on/off към engine
   (ring buffer). Ползва се от Trainer в Phase 2.
Код: `crates/audio-engine/src/devices.rs`, `midi_in.rs`; UI `Settings.tsx`.
Готово когато: изключване на USB микрофон по време на запис не губи данни; калибрация
дава стабилна стойност ±2 ms при три измервания; MIDI клавиатура светва нота в UI.

### F1.5 Импорт и non-destructive trim
Зависи от: F1.2.
Стъпки:
1. `symphonia` за MP3/FLAC/OGG decode (ADR-0004); `re.mp3` само локално.
2. Trim in/out в проекта (оригиналът непокътнат), fade in/out като параметри.
Код: `crates/audio-engine/src/decode.rs`.
Готово когато: MP3 fixture декодира с точна дължина; trim се пази след reopen.

**Phase 1 exit:** реален кавал take ≥3 min → reopen без загуба; 30 min запис + playback
без overrun; NSIS инсталатор, clean-machine smoke.

---

## Phase 2 — Височина, ноти, тунер, инструментен профил, Trainer

### F2.1 music-core: NoteEvent и теория
Зависи от: нищо (чист crate).
Стъпки:
1. `crates/music-core`: `NoteEvent {pitch_midi, start_tick, duration_tick, velocity,
   channel, track, pitch_bend[], confidence}`; `TempoMap` tick↔seconds; квантизация
   към grid с толеранс; имена на ноти (латиница, солфеж До Ре Ми); интервали;
   гами и ладове (мажор, минор, пентатоника, дорийски, фригийски, лидийски,
   миксолидийски, еолийски, локрийски + хиджаз/македонски като етно набор);
   акорд detection от набор ноти; транспониране.
2. 100% unit тестове върху теорията.
Код: `crates/music-core/src/{note,tempo,scale,chord,quantize}.rs`.
Готово когато: тестове за всяка функция; `cents_between(440.0, 466.16) ≈ 100`.

### F2.2 Pitch detection (реално време + офлайн)
Зависи от: F1.3.
Стъпки:
1. Monophonic pitch в Rust: pYIN или YIN + медиана, на analysis thread, вход от ring
   buffer, изход `PitchFrame {time, hz, midi_float, cents, confidence, rms}` на 10 ms.
2. Confidence gating: под праг = „тишина/шум", не нота. Onset detection (spectral flux).
3. Офлайн: същият алгоритъм върху take → `PitchContour` (пази се в `analysis/`),
   сегментация на ноти (onset + стабилна височина ≥ 60 ms) → `NoteEvent[]` с
   confidence, pitch bend от контура. Суровият контур никога не се изхвърля.
4. Benchmark: синтетични тонове (точност в центове), `synthetic_flute`, реални кавал
   записи; резултатите в `docs/BENCHMARKS.md`, не измислени проценти.
Код: `services/analysis` (Rust crate `ethnoflow-analysis`), `crates/audio-engine/src/analysis_tap.rs`.
Готово когато: синтетичен тон 440 Hz → 0 ±3 cents; sweep следи честотата; latency
на live pitch < 40 ms при 48 kHz; benchmark файл съществува с реални числа.

### F2.3 Тунер
Зависи от: F2.2, F2.5.
Стъпки:
1. UI: голяма стрелка с скала −50…+50 cents, Hz, име на нотата, октава, цвят
   (зелено/жълто/червено по толеранс), A4 референция настроявaема, транспониращ
   инструмент (кавал в ре показва написана и звучаща нота).
2. Втора маркировка: къде е тази нота в **профила на инструмента** (F2.5), ако има.
3. Режим „Дръж тона": лента във времето с дрейфа на дълъг тон, брояч секунди в толеранс.
Код: `apps/desktop/src/screens/Tuner.tsx`, команда `tuner_frame()`.
Готово когато: тунерът показва 440 Hz fixture като A4 0 cents; с профил показва двете
маркировки; „Дръж тона" брои коректно.

### F2.4 Нотен изглед и редактор на ноти
Зависи от: F2.1, F2.2.
Стъпки:
1. Три синхронизирани изгледа на `NoteEvent[]`: падащи ноти (canvas, 60 fps, само
   видимите ноти), piano roll (read-only тук), нотен лист (VexFlow след spike; ако
   spike-ът надхвърли 2 седмици, нотният лист отива в Phase 6 и това се записва в ADR).
2. Редактор: избор на нота, промяна на височина/дължина/начало, изтриване, добавяне;
   confidence се показва (блед цвят = несигурна). Undo/redo.
3. Експорт SMF (type 1) с pitch bend; импорт SMF (`midly`).
Код: `apps/desktop/src/views/{FallingNotes,PianoRoll,Score}.tsx`, `crates/music-core/src/smf.rs`.
Готово когато: запис на мелодия → ноти на екрана → редакция → експорт MIDI → импорт
в Reaper показва същите ноти; roundtrip SMF тест.

### F2.5 Инструментен профил и калибриране
Зависи от: F2.2, F1.2.
Стъпки:
1. `InstrumentProfile` (глобален за потребителя, в `%APPDATA%/EthnoFlow/instruments/`):
   `{id, type, name, tuning (напр. "D"), a4_hz, notes[{midi, measured_hz, cents_offset,
   stability, sample_path}], created, version}`; проектът закача id+version.
2. Wizard „Запознай ме с инструмента": избор на тип (кавал, флейта, гайда, глас,
   монохорд, пиано, друг) и строй; за всяка нота от очаквания диапазон: „Изсвири и
   задръж" → измерва се 1.5 s стабилен тон → записва се семпъл (оригинал, immutable)
   и статистики; може да се пропусне нота; прекалибриране по нота.
3. Диагностика: таблица с отклонения по нота, маркиране на систематични (инструментът)
   срещу случайни (свиренето).
Код: `packages/project-schema` (profile schema), `apps/desktop/src/screens/Calibrate.tsx`,
`crates/audio-engine/src/sampler_lite.rs` (възпроизвеждане на семпъл с pitch shift чрез
resampler).
Готово когато: пълен профил на кавал за 2 минути; профилът се запазва и зарежда;
тунерът показва профилната маркировка; семпъл на всяка нота се пуска при клик.

### F2.6 Апликатура (fingering)
Зависи от: F2.1.
Стъпки:
1. Данни `packages/lesson-content/fingerings/{kaval-d,kaval-c,recorder-soprano,flute}.json`:
   за всяка нота: списък дупки (open/closed/half), октавен режим, алтернативи, бележка.
   Лиценз CC-BY в README.
2. Компонент `FingeringDiagram`: схема на инструмента, текуща нота в плътен цвят,
   следваща в блед; при грешна нота показва най-близката апликатура по измерената
   височина. Редактор на таблиците (потребителят може да коригира и запази копие).
3. Синхронизация с падащите ноти и с калибриращия wizard (показва схемата при запис).
Код: `apps/desktop/src/components/FingeringDiagram.tsx`, данни в `packages/lesson-content`.
Готово когато: кавал в ре показва коректна апликатура за всичките си ноти (проверено
от Ники); редакцията се пази; диаграмата следва playback.

### F2.7 Play Along Player
Зависи от: F2.1, F2.4, F1.3.
Стъпки:
1. SF2 синтезатор (`oxisynth` или `rustysynth`, ADR) като източник в Transport; soundfont
   GeneralUser GS или FluidR3 (лиценз проверен в ADR); избор на програма по писта.
2. MIDI клип в проекта; tempo 25…200% (MIDI, без промяна на височината); loop A-B със
   snap към такт; mute/solo/volume по писта; лява/дясна ръка (по писта/канал, fallback
   split point с ръчна настройка); транспониране.
3. Вход за свирене: MIDI клавиатура (F1.4), компютърна клавиатура (два реда = две октави,
   Scale Lock), микрофон (F2.2).
4. Рендер на MIDI клип към WAV през синтезатора.
Код: `crates/audio-engine/src/synth.rs`, `apps/desktop/src/screens/PlayAlong.tsx`.
Готово когато: произволен .mid се пуска със звук и падащи ноти; tempo 50% не променя
височината; loop е безшевен; изсвирена нота от трите входа светва на клавиатурата.

### F2.8 Trainer (режими и оценка)
Зависи от: F2.7, F2.5, F2.6.
Стъпки:
1. Чиста функция в music-core: `evaluate(expected: &[NoteEvent], played: &[NoteEvent],
   tolerance {cents, ms}) -> Report {per_note: [hit|wrong_pitch(cents)|early(ms)|
   late(ms)|missed], summary}`. Без UI, с фикстури.
2. Режими: **Listen**; **Wait** (спира на всяка нота/акорд, чака вярната; грешна = червено,
   без време); **Rhythm** (не спира, оценява timing и височина, latency calibration се
   изважда); **Loop drill** (2 такта от 50%, +10% на 3 чисти преминавания до 100%).
3. Толерансите по ниво (начинаещ ±30 cents/±80 ms … напреднал ±10/±30) и по инструмент;
   микротонови отклонения в етно лад се показват като информация, не като грешка.
4. Отчет след упражнение: списък ноти с отклонение, предложение за следващ drill.
   Записва се в прогрес (F6.3).
Код: `crates/music-core/src/trainer.rs`, `apps/desktop/src/screens/Trainer.tsx`.
Готово когато: тестове на `evaluate` за всички случаи; Wait режим с кавал пред
микрофон работи на народна мелодия (проверено от Ники); отчетът е четим.

**Phase 2 exit (мастър план):** запис на мелодия → правдоподобни ноти → редакция →
експорт MIDI. Плюс: тунер с профил, апликатура, Play Along с Wait режим.

---

## Phase 3 — Хармония и drone

### F3.1 Тоника/лад хипотези
Зависи от: F2.2, F2.1.
Стъпки: pitch-class хистограма с тегла по дължина; сравнение с профилите на ладовете от
F2.1 (включително етно набора); изход: топ 5 `{tonic, mode, score}` нормализирани като
проценти, с обяснение („Ре се задържа 38% от времето, Фа е 60% по-често от Фа#").
Потребителят избира; изборът се пази в проекта. Regression fixtures с модален материал.
Готово когато: 10 фикстури с известен лад дават верния в топ 2; UI показва проценти и
алтернативи, никога само една „истина".

### F3.2 Drone / Liquid Resonator
Зависи от: F1.3, F3.1.
Стъпки: Rust инструмент в engine-а: root/fifth/octave, 2–3 осцилатора с detune,
нискочестотен филтър, бавен LFO, overtone shaping, интонация към профила на инструмента
(F2.5) или към темперация; „Liquid Resonator" режим върху записан монохорд (спектрална
заморозка/дълъг reverb). Параметри: root, warmth, movement, space. Preset „Traditional
Drone", „Deep Ethno", „Cinematic Ambient".
Готово когато: drone звучи под реален кавал take без клик и дропаут 10 минути; седи в
интонация с профила.

### F3.3 Liquid Pad + X/Y макро
Зависи от: F3.2.
Стъпки: pad инструмент (осцилатори + филтър + reverb + tempo-synced LFO), избор на
емоция (preset с 5 параметъра), X/Y пад (X яркост/движение, Y дълбочина/пространство),
voicing от F3.4.
Готово когато: 3 емоции звучат различимо; X/Y променя звука без клик; seed възпроизвежда.

### F3.4 Хармонични стратегии
Зависи от: F3.1, F2.1.
Стъпки: от `NoteEvent[]` по фрази → 3 предложения: drone-only, modal pads (sparse,
на фраза), contemporary liquid chords (7ths, 9ths, sus, квартови), с voice leading
penalties, avoid notes като меки ограничения, обяснение на всяко („Фа мажор 7 защото
мелодията стои на Ла и Ми"). Seed, A/B сравнение при изравнена сила.
Готово когато: 3 реални записа с убедителни drone/pad подложки (слушане от Ники);
обясненията са верни теоретично (тестове на правилата).

**Phase 3 exit:** убедителни drone/pad подложки на поне три различни записа.

---

## Phase 4 — Liquid DnB машина

### F4.1 Groove Engine
Зависи от: F1.3.
Стъпки: drum rack (kick/snare/hat/shaker/ghost) с лицензирани семпли; шаблони 170–175
BPM и halftime; humanize (микро timing + velocity) от seed; „Complexity" контрол; fills
на 4/8 такта; step sequencer и piano roll редакция; breakbeat slicing на потребителски
семпъл (onset от F2.2 → slices → пренареждане по шаблон).
Готово когато: GENERATE DRUMS дава различен, възпроизводим патерн при всеки seed;
редакцията се пази; slicing на фънк лууп звучи в темпо.

### F4.2 Sub-Bass Engine
Зависи от: F3.4, F4.1.
Стъпки: sub sine/triangle, pluck, Reese (два saw + detune + филтър + saturation); glide;
root-following от хармонията; sidechain към kick по подразбиране (envelope follower);
X/Y „Wobble & Glide" (Y топлина, X скорост на филтъра 1/8, 1/16); MIDI редактируем.
Готово когато: бас следва акордите на F3.4; sidechain се чува и се изключва; няма
честотен сблъсък с kick (тест: спектър под 80 Hz при kick удар пада с ≥6 dB).

### F4.3 Аранжимент и секции
Зависи от: F4.1, F4.2, F3.4.
Стъпки: Intro/A/B/Build/Drop/Break/Outro като маркери; tempo alignment на свободната
кавал фраза към grid **без** деструктивна квантизация (tempo map + offset, оригиналът
остава); locks по писта (freeze melody/harmony/bass/drums); regenerate само unlocked.
Готово когато: реален етно liquid DnB трак с редактируеми MIDI бас/барабани (exit).

### F4.4 CLAP инструментен host (spike, ADR-0018)
Зависи от: F3.2.
Стъпки: `crates/plugin-host` зарежда CLAP инструмент in-process; първи C++ инструмент
(проста синусова проба) през CLAP C++ helpers; Rust инструментите през `nih-plug`.
Готово когато: C++ CLAP свири в EthnoFlow; документирани ограничения (crash isolation).

**Phase 4 exit:** реален свирещ етно liquid DnB трак с редактируеми бас/барабани.

---

## Phase 5 — Studio, chopper, експорт

### F5.1 Multitrack timeline и клипове
Timeline, зум, snapping, аудио клипове (trim/split/move/duplicate/fade/gain/mute/solo),
MIDI клипове с piano roll редакция (F2.4 става editable), автоматизация по-късно.
Готово когато: аранжимент от Phase 4 се редактира и се запазва с undo/redo.

### F5.2 Mixer
Fader, pan, basic EQ, delay, reverb („Organic Space" preset за живи инструменти), sends;
„Auto Studio Engineer" = видима верига (auto-gain, smart EQ за духови) с обяснение и off.
Готово когато: верига се прилага на кавал take и се чува разликата; off връща оригинала.

### F5.3 Phrase & Vocal Chopper
Зависи от: F2.2 (onset), F1.5.
Стъпки: waveform canvas с автоматични „магнитни" маркери (onset), ръчно местене;
slices → клавиатура (A S D F…), после MIDI pads; crossfade на края на slice; Pitch &
Time Lock (time-stretch без промяна на височината, после pitch към скалата; spike за
алгоритъм: WSOLA/phase vocoder в Rust); Chop Randomizer с „Rhythm Density" и seed;
Formant Shifter „Voice Character" (spike: phase vocoder с formant preservation; ако
качеството не е приемливо, функцията остава зад flag); „Follow Melody"; „Mutate Voice"
(formant + vinyl saturation + space delay).
Готово когато: вокална фраза се реже, свири от клавиатурата в нов ред, time-stretch
към 174 BPM без артефакти на слух; formant ±; всичко seed-възпроизводимо.

### F5.4 Експорт за DAW
Master WAV 24/32f, stems с еднакъв старт/край, SMF по писти с pitch bend,
`tempo_map.json`, `markers.csv`, `README_DAW_IMPORT.txt`; SRC през `rubato`; reimport
тест в Reaper.
Готово когато: stems импортирани в Reaper са sample-accurate синхронизирани.

**Phase 5 exit:** аранжимент завършен в EthnoFlow и продължен във външна DAW.

---

## Phase 6 — Академия v1

### F6.1 Уроци като данни
`packages/lesson-content/lessons/*.json`: `{id, level, title_bg, title_en, objectives,
explanation (≤3 изречения), demo (midi/audio), exercise {type, params, tolerance},
next}`; валидатор; типове упражнения: play_along (F2.8), hear_and_repeat (F6.2),
ear_interval, ear_chord, ear_mode, rhythm_clap (onset от микрофон), sight_reading
(нота се появява, реакция), hold_tone (F2.3).
Готово когато: схемата е валидирана; всяко упражнение има генератор със seed.

### F6.2 „Чуй и повтори" със собствения инструмент
Зависи от: F2.5, F2.8.
Академията изсвирва нотата/фразата със семплите от профила (най-близък семпъл + pitch
shift, ако нотата липсва); потребителят повтаря; `evaluate` дава отклонение по нота;
апликатурата (F2.6) се показва паралелно.
Готово когато: урок за 5 ноти на кавал се минава от начинаещ без външна помощ.

### F6.3 Overlay ментор, прогрес, речник, Coach
„Следвай светещата диря" (затъмнява всичко освен нужния контрол); X-Ray карта на
структурата; прогрес локално, без streak и известия; речник с търсене; „Обясни ми" за
избран такт (ноти, интервали, акорд от F2.1); Coach за микс (бас твърде силен →
предложение с бутон и обяснение).
Готово когато: Ниво 0–3 авторски уроци на български; Урок №1 „Древният кавал среща
174 BPM" (по INSTRUMENT_MACHINE_CONCEPT §7, коригираната версия) минава от край до край.

**Phase 6 exit:** начинаещ завършва guided композиция без външни уроци.

---

## Phase 7 — Интелигентни вариации
3/5/10 варианта със seed и locks; комбиниране (бас от C + pads от B); technical fit
score (voice leading, clashes, density, register), не естетика; Evolve Melody
(rule-based: огледало, инверсия, секвенция, вариация, „Adventure Level"); Mutate Voice.
Готово когато: вариантите са различими и музикално кохерентни (слушане), seed ги
възпроизвежда байт по байт.

## Phase 8 — Live accompanist (R&D)
Live drone (стабилен MVP), beat follower с confidence, harmonic follower (при ниска
увереност държи предишната хармония), predictive режим експериментален и обозначен.
RØDECaster Smart Pads и NI NKS след лицензна проверка (ADR).

## Phase 9 — Release
Академия 4–6, локализация, инсталатори, подпис или ясно „unsigned", security review,
достъпност, crash reports opt-in, лицензи на всички семпли и soundfont-ове.

---

## Матрица на зависимостите (накратко)
```
F1.1 → F1.2 → F1.5
F1.3 → F2.7, F3.2, F4.1
F1.4 → F2.7 (MIDI in)
F2.1 → F2.4, F2.6, F2.8, F3.1, F3.4
F2.2 → F2.3, F2.4, F2.5, F3.1, F4.1(slicing), F5.3(onset)
F2.5 → F2.3, F3.2(intonation), F6.2
F2.7 → F2.8 → F6.1/F6.2
F3.1 → F3.2 → F3.3; F3.4 → F4.2 → F4.3
```

## Какво НЕ правим (за да не се връща)
YouTube/MP3→MIDI на чужди песни; библиотека с популярни песни; социални/облачни
функции; Kontakt/VST хостване вътре преди Phase 5+ (само през CLAP/VST3 host);
скрити вериги за обработка; автоматични корекции без преглед; streak-ове и push
известия; hardcoded ре минор/174/4/4.
