# EthnoFlow Studio — обединена функционална спецификация за изпълнение

Този файл е **единственият** работен документ за функциите след Phase 0. Той обединява и
замества като източник на задачи: `INSTRUMENT_MACHINE_CONCEPT.md`,
`MIDI_PLAYER_TRAINER_SPEC.md`, `ACADEMY_INSPIRATION.md`, `AMBIENT_HARMONY_SPEC.md`,
`HARMONIC_ACCOMPANIMENT_SPEC.md` (те остават като обяснение „защо").
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

## Статус спрямо репото (обновено 17.10.2026, версия 0.2.6)

Спецификацията (v2, с Phase 1A и IHAE) е получена, след като Phase 1 и почти цялата
Phase 2 вече са реализирани. Където кодът живее другаде, отколкото спецификацията
очаква, важи реалното място (с ADR). Таблицата се поддържа при всяка завършена функция.

**Ред на работа след Phase 2 (решение на собственика, 17.10.2026): хармониите са
най-важни.** Затова: F1.6 (аудио граф, защото drone и pad са живи източници) → Phase 3
(F3.1 → F3.2 → F3.3 → F3.5 Compose First → F3.4 → F3.6) → Phase 1A (Ambient Playground
върху същия граф) → Phase 4. Подробности в `ROADMAP.md`.

| F | статус | къде е реално / какво остава |
|---|---|---|
| F1.1 | **готово** (0.1.0) | `crates/audio-engine/src/{take_writer,recorder,peaks}.rs`; header checkpoint на всяка секунда, `repair_wav_header`, `LivePeaks`, `recording_waveform`. 35-min RAM тест е покрит от 30-min симулация (`--ignored`), без измерване на RAM. |
| F1.2 | **готово** | `crates/project` (ADR-0015, вместо `packages/project-schema`), SQLite миграции v1→v2, checksums, trash, relink, recovery при отваряне; начален екран. Autosave е на всяка промяна. Remaining: реален kill -9 тест от собственика. |
| F1.3 | **частично** | `metronome.rs` (sample-accurate, групи 2+2+3…), count-in, loop и клик в `player.rs`. Няма общ `Transport`, tap tempo, punch-in. |
| F1.4 | **частично** | sample rate/канали, noise floor, тест тон, device_lost → чисто спиране, **MIDI вход** (`midi_in.rs`, ADR-0020): порт, note on/off към живия синтезатор (~2 ms), клавиши, Trainer. Remaining: latency calibration (loopback), hot-plug relist. |
| F1.5 | **частично** | symphonia (ADR-0017), trim in/out в проекта. Remaining: fade in/out. |
| F1.6 | **не е започнато** | аудио граф с параметри и smoothing; offline render; метри. Следващото нещо (основа на Phase 3 и 1A). |
| F1A.1–F1A.8 | **не е започнато** | Ambient Playground; след Phase 3 по решение на собственика. F1A.8 (лицензи) може да върви по-рано, защото не зависи от код. |
| F2.1 | **готово** | `crates/music-core`: pitch/note, 13 лада вкл. хиджаз и двоен хармоничен, custom cents, хипотези, NoteEvent + tempo map, SMF reader/writer, интервали, акорди, транспониране, квантизация. Имената на нотите са в UI i18n. |
| F2.2 | **готово** | YIN в `audio-engine/src/pitch.rs`: live (`RecorderStatus`, `InputMonitor`), офлайн `contour`, ноти в `music-core/src/notes.rs`. Числа: `docs/BENCHMARKS.md`. Remaining: spectral-flux onset (сега RMS скок), измерена live latency. |
| F2.3 | **готово, чака проверка** | `Tuner.tsx`: нота, цента, Hz, увереност, в/извън скалата, профилна маркировка, написана нота при транспозиция, „Уцели нотата“, „Дръж тона“. |
| F2.4 | **частично** | piano roll с редакция (↑/↓, Delete, split, merge, влачене, Alt+клик, undo/redo), падащи ноти, SMF export/import. Remaining: нотен лист (VexFlow spike, виж MIDI_PLAYER_TRAINER_SPEC §7). „Reaper показва същите ноти“ чака Ники. |
| F2.5 | **готово, чака проверка** | `crates/project/src/instrument.rs`, `held_tone`, `instrument_cmds.rs`, панел „Инструмент“, wizard, семпли с pitch shift, диагностика. „Профил на кавал за 2 минути“ чака Ники. |
| F2.6 | **не е започнато** | чака таблицата с апликатури за кавал в Ре от Ники; после данни + диаграма + редактор. |
| F2.7 | **частично** | `.mid` като запис, вграден синтезатор, tempo, mute, глас, транспониране, loop по тактове, „Пусни нотите“, „Към плейъра“, **жив синтезатор** + компютърна клавиатура (Scale Lock) + MIDI клавиатура. Remaining: SF2 + soundfont (ADR), лява/дясна ръка. |
| F2.8 | **частично** | `trainer.rs` `evaluate` с тестове; панел „Трениране“: Listen, Wait, Rhythm с микрофон или MIDI. Remaining: Loop drill, latency от калибрация, прогрес (F6.3). „Wait с кавал на народна мелодия“ чака Ники. |
| F3.1 | **частично** | `rank_hypotheses` (проценти, 13 лада) е готово и в UI. Remaining: обяснение („Ре се задържа 38 %…“), 10 фикстури с известен лад. |
| F3.2–F3.6 | **не е започнато** | drone, pad, стратегии, Compose First, контроли. Приоритет след F1.6. |
| F4–F9 | **не са започнати** | |

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

### F1.6 Аудио граф и параметри (основа за всичко звучащо)
Зависи от: F1.3.
Стъпки:
1. Граф в engine-а: `Source` → per-layer DSP (gain, pan, HP/LP филтър, width) → `Bus` →
   master (limiter + meter) → device. Фиксиран брой слотове, предварително алокирани;
   никакви алокации в callback. Оценка на `fundsp`/`dasp` срещу собствен код (ADR).
2. Параметри: всеки `ParamId` с граници, default, единица; промяна през lock-free
   команда; **smoothing** (линейни рампи 5–20 ms) срещу zipper noise; denormal защита.
3. Offline render на същия граф (детерминистичен, без реално време) в WAV.
4. Метри per layer и master (peak hold) към UI с 20 Hz.
Код: `crates/audio-engine/src/graph/{mod,params,bus,limiter}.rs`, `render.rs`.
Готово когато: тест sum/mute/solo/pan; NaN/denormal тест; рампа на gain от 0 до 1 за
10 ms няма дискретни скокове > 1 dB; offline render == realtime render байт по байт при
seed; 1 час playback без drift (тест със синтетичен clock).

**Phase 1 exit:** реален кавал take ≥3 min → reopen без загуба; 30 min запис + playback
без overrun; NSIS инсталатор, clean-machine smoke; аудио графът свири осцилатор през
слоеве с плъзгачи без пукания.

---

## Phase 1A — Ambient Playground (първият звучащ продукт, AMBIENT_HARMONY_SPEC P1–P3)

### F1A.1 Слоеве и източници
Зависи от: F1.6.
Стъпки:
1. `Layer {id, source, gain_db, pan, pitch, filter, width, density, envelope, sends,
   loop_region, reverse, crossfade, seed, mute, solo, license_ref}`; 8 слота,
   разширяеми до 16.
2. Източници за старт (всички синтезирани, оригинални): noise colors (white/pink/brown),
   филтриран „вятър", sub drone (F3.2 lite: root + fifth, без хармония), harmonic pad
   (прост 2-osc + filter), bells (FM/модал), soft pluck, filtered texture; плюс audio loop
   източник (WAV с loop crossfade). Контроли по тип източник, не универсални копчета.
3. Drag-and-drop пренареждане, mute/solo, per-layer метри.
Код: `crates/audio-engine/src/sources/{noise,wind,drone_lite,pad_lite,bell,pluck,loop_player}.rs`,
`apps/desktop/src/screens/Ambient.tsx`.
Готово когато: 8 слоя свирят едновременно 10 минути без клик/дропаут; mute/solo
коректни; всеки плъзгач променя чуваемо звука (тест: спектър/RMS се променя).

### F1A.2 Глобални макроси и безопасност
Зависи от: F1A.1.
Стъпки: master level, scene intensity, warmth/brightness, motion, space (reverb send),
tonal gravity (дял на хармоничните слоеве), stereo width, randomness, tempo/free-time,
limiter, headphone safety (стартов gain −18 dB, предупреждение); **никога auto-play при
старт**; fade out, таймер за сън, loop.
Готово когато: limiter никога не пуска > −0.3 dBFS; старт без звук; fade out без клик.

### F1A.3 Preset-и и Sound DNA
Зависи от: F1A.1, F1.2.
Стъпки: preset JSON (`SoundDNA`: слоеве, параметри, автоматизация, сцени, license refs,
seed, schema_version); browser с тагове (Гора, Река, Нощ, Мъгла, Планина, Етно,
Медитация, Тъмно, Топло, Liquid, Без тоналност), favorites, recent, A/B, „Изненадай ме"
в музикално съвместими граници, „Запази каквото харесвам" (locks), undo на randomize,
snapshot история; 8–12 фабрични preset-а с качество, не количество (слушане от Ники).
Готово когато: save → reopen дава идентично състояние (snapshot тест); randomize с
locks не пипа заключените; preset-ите са слушани и одобрени.

### F1A.4 Автоматизация
Зависи от: F1A.1.
Стъпки: `AutomationLane {param, interpolation, keyframes[], generator?, seed}`; режими:
static, slow random walk (bounded, seeded), LFO, keyframes, рисувана крива, morph A→B,
MIDI learn (F1.4); range/period/depth/phase/humanize; детерминизъм: същият seed + време
→ същата стойност.
Готово когато: тест: стойност на параметър в t=37.25 s е идентична при 3 пускания и
след reopen; morph интерполира без скокове.

### F1A.5 Сцени и Програми, offline bounce
Зависи от: F1A.4, F1.6.
Стъпки: timeline от `Scene {name, start, duration, snapshot, transition}`; free-running и
beat-synced clock; preview от произволна точка; loop на секции; Program = именувана
последователност; **offline render** на 20-минутна програма за секунди, не в реално време
(F1.6.3), stems per layer от 0:00.
Готово когато: offline render съвпада с realtime референция (RMS по 1 s прозорци
< 0.1 dB разлика); 20 min програма се рендва < 60 s на базов лаптоп.

### F1A.6 Sample Lab (разширение на F1.1/F1.5)
Зависи от: F1.1, F1.5, F1.2.
Стъпки: trim/split/normalize с headroom/fade/DC removal/HP/silence detection/zero-cross
cuts/loop crossfade като non-destructive „recipe" върху immutable оригинал; тагове
(заглавие, категория, място като свободен текст, бележки, микрофон, лиценз);
`AudioAsset {sha256, origin, creator, license, path, channels, sr, duration, tags}`;
библиотека с търсене; семпъл → слой в Playground; path traversal защита при импорт.
Готово когато: преместване на папката на проекта не губи assets (relative paths +
hash relink); recipe се пази и прилага при reopen; оригиналът е байт-идентичен.

### F1A.7 Генеративни музикални слоеве (lite)
Зависи от: F2.1 (scale), F1A.1.
Стъпки: sparse bells/plucks по избрана скала и тонален център с вероятностни gates и
интервални ограничения; „без тоналност" режим; пази тишината (density ≤ 1 нота/4 s по
default). Пълният harmony engine е Phase 3.
Готово когато: 10 минути без две еднакви последователности при различен seed; при
еднакъв seed идентично; нотите винаги в скалата.

### F1A.8 Лицензи и атрибуция
Зависи от: нищо.
Стъпки: `docs/AUDIO-LICENSING.md` (правила: CC0 само с документиран произход, CC-BY с
атрибуция, CC-BY-NC забранено, myNoise/Sonic Pi само като идеи), `assets/ATTRIBUTION.md`,
`assets/asset-manifest.json` (машинно четим), тест, който проверява, че всеки asset в
инсталатора има запис с лиценз; механизъм за премахване на съмнителен asset.
Готово когато: CI тестът пада при asset без лиценз.

**Phase 1A exit (= първи release, AMBIENT_HARMONY_SPEC §14):** на чиста Windows 11
машина: инсталира, избира изход, Play → 8-слоен еволюиращ soundscape, плъзгачи без
пукания, запазва Program, записва микрофон, добавя го като loop, експортира стерео WAV,
отваря проекта офлайн. Звучи музикално убедително (слушане от Ники), не само тестове.

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
   (зелено/жълто/червено по толеранс), A4 референция настройваема, транспониращ
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

### F3.5 Compose First / „Подготви фон" (HARMONIC_ACCOMPANIMENT_SPEC режим 1)
Зависи от: F3.2, F3.3, F1A.3, F1.3.
Стъпки: екран с ръчен избор на тонален център, лад (от F2.1, плюс „без тоналност"),
темпо/свободно време, метрум (вкл. 5/8, 7/8, 9/8), настроение, стил (Traditional
Drone / Emotional Liquid / Deep Ethno DnB / Cinematic Ambient / Ambient No Key);
включване на Drone, Pad, Ostinato (F4.5, lite), Bass (F4.2 lite root/fifth) като слоеве
в Playground; „Пусни и записвай" стартира фона и записва инструмента на отделна писта
(F1.1); drone интонация към профила (F2.5).
Готово когато: без запис на мелодия потребител получава звучащ drone + pad в избран
лад за < 1 минута; MIDI нотите са в лада освен при включен tension; няма висящи ноти
при смяна на лад (смяна на музикална граница или crossfade).

### F3.6 Контроли, обяснения, seed и locks
Зависи от: F3.4, F3.5.
Стъпки: плъзгачи Drone Strength, Harmonic Richness, Complexity, Harmonic Movement,
Tension/Dissonance, Density, Warmth, Register, Humanization, Space, Variation Rate;
всеки с tooltip на български (≤2 изречения + „чуй пример" бутон), double-click reset,
числова стойност, automation arm, MIDI learn, accessibility label; Lock Melody/Chords/
Bass; Generate 3/10; Compare A/B при изравнена сила; Undo/Redo; Save Favorite; Seed
видим и редактируем; „Защо работи" карта („Тук Ре остава опорен тон, а Ла е квинта")
плюс предупреждение при несигурен анализ.
Готово когато: UI тест: всеки плъзгач променя съответния параметър в engine-а (няма
фиктивни); seed възпроизвежда байт по байт; обясненията минават проверка на правилата.

Критерии за F3.4 (score), допълнени от спеца: melodic compatibility, modal fidelity,
voice leading, tension target, rhythmic fit, register separation, variety; показват се
**различни стилови решения**, не само най-високият score.

**Phase 3 exit:** убедителни drone/pad подложки на поне три различни записа **и**
Compose First работи без запис (MVP критерий на HARMONIC_ACCOMPANIMENT_SPEC, без stems,
които идват с F5.4).

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

### F4.5 Ostinato
Зависи от: F3.4, F1.3.
Стъпки: повтаряеми фигури от акордовите/ладовите тонове; ритмични трансформации
(ротация, добавяне/махане на удар), вероятност per step, вариации на N такта, диапазон,
swing, phrase-aware регенерация, locked motifs, seed.
Готово когато: ostinato остава в лада; при locked motif регенерацията не го променя;
seed детерминизъм.

### F4.6 Arpeggiator
Зависи от: F3.4.
Стъпки: посоки (up/down/updown/random seeded), octave span, swing, gate, density, sync
към Transport или free; работи върху текущия акорд/drone тонове.
Готово когато: тест за всички посоки; без висящи ноти при смяна на акорд.

### F4.7 Countermelody
Зависи от: F2.2 (фрази), F3.4.
Стъпки: call-and-response (отговор в паузите на водещата мелодия), запълване на паузи,
избягване на сблъсък (не свири по време на дълги тонове на мелодията в същия регистър,
избягва секунди и тритони срещу текущата нота освен при tension), регистър под/над
мелодията по избор, „Adventure Level".
Готово когато: тест: 0 ноти на countermelody се припокриват с мелодия в радиус 1 полутон
при tension=0; слушане от Ники на 3 записа.

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

### F8.1 Live следене с хистерезис и fallback
Зависи от: F2.2, F3.4, F1.4.
Стъпки: RT pitch → analysis thread; тонален център се сменя само след N секунди
стабилна евидентност (хистерезис), хармонията се сменя само на фразови граници, никога
на всяка орнаментна нота; при confidence под праг → задържа предишната хармония или
пада към „безопасен drone"; ръчни „Lock tonic", „Freeze harmony", emergency bypass;
измерен latency budget и мониторинг (underrun/overrun брояч); WASAPI exclusive опция.
Готово когато: тест с орнаментирана кавал фраза: 0 хармонични смени в рамките на една
фраза; измерена latency документирана; dropout recovery без срив.

Останало: live beat follower с confidence; predictive режим експериментален и обозначен;
RØDECaster Smart Pads и NI NKS след лицензна проверка (ADR).

## Phase 9 — Release
Академия 4–6, локализация, инсталатори, подпис или ясно „unsigned", security review,
достъпност, crash reports opt-in, лицензи на всички семпли и soundfont-ове.

---

## Матрица на зависимостите (накратко)
```
F1.1 → F1.2 → F1.5
F1.3 → F1.6 → F1A.1 → F1A.2/F1A.3/F1A.4 → F1A.5; F1A.1 → F3.5
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
