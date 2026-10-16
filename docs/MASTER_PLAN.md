# ETHNOFLOW STUDIO — MASTER BUILD PROMPT & PRODUCT BLUEPRINT v1.0

> **Роля:** Ти си Claude Code, действащ като principal software architect, senior audio DSP engineer, music theorist, educational product designer, accessibility specialist, QA lead и Windows release engineer. Изгради истински, работещ Windows софтуер. Не създавай само прототип от статични екрани.
>
> **Работен език:** Български за UI, уроци, документация и обяснения; английски за имена на кодови символи и при нужда технически термини. Архитектурата трябва да позволява i18n.
>
> **Първо правило:** Работи на малки, проверими етапи. След всеки етап стартирай тестовете, демонстрирай реално работещата функционалност, запиши резултатите и поиска̀й одобрение за следващия голям етап. Не представяй фиктивни реализации за завършени.

---

## 0. Визия

**EthnoFlow Studio** е настолно приложение за Windows, което съчетава:

1. **Music Academy** — интерактивно обучение по музика от абсолютното начало до създаване на собствена композиция.
2. **Live Instrument Lab** — слушане през микрофон, тунер, разпознаване на ноти, интонация, ритъм, визуализация и запис на семпли.
3. **Music Studio** — истински многопистов аудио/MIDI редактор с основните функции на DAW.
4. **Intelligent Accompanist** — алгоритмичен и по-късно AI-подпомаган акомпаниатор, който анализира мелодия и предлага drone/ручило, акорди, бас, барабани и аранжименти.
5. **Composition Coach** — обяснява защо музикалните предложения работят, води начинаещия през процеса на създаване на песен.
6. **DAW Export** — преносими WAV stems, MIDI, tempo map, markers и проектен манифест.

**Основен сценарий:** Потребител свири етно мелодия на кавал или флейта; приложението я записва, показва нотите, предлага музикален център и лад с увереност, генерира 3–10 варианта на хармонична подложка, ручило, бас и Liquid Drum & Bass ритъм, позволява да се сравняват и комбинират, след което експортира отделните писти към Ableton, FL Studio, Reaper, Cubase и др.

**Важна музикална позиция:** Не насилвай етно мелодиите в западна мажорно-минорна хармония. Поддържай модалност, опорни тонове, бурдон, неравноделни размери, микротонови отклонения и свободно темпо.

## 1. Продуктови принципи

- **Real audio first:** всяка основна функция работи с реални аудио данни, не с декоративни графики.
- **Beginner first, expert capable:** два режима Beginner и Advanced, споделящи един и същ проектен формат.
- **Non-destructive:** оригиналният запис е неизменяем; редакциите са обратими, с история и snapshots.
- **Local-first / privacy:** записи и проекти остават локално по подразбиране; никакво качване без ясно съгласие.
- **Offline core:** запис, анализ, уроци, синтез, генерация по правила и експорт работят без интернет.
- **AI optional:** не блокирай основните функции при липса на GPU, облак или AI модел.
- **Explainability:** всяко предложено хармонично решение има кратко музикално обяснение.
- **Reproducibility:** генераторите приемат seed; вариантите могат да бъдат възпроизведени.
- **Accessible:** клавиатурна навигация, мащабиране, контраст, субтитри/текстови обяснения, достъпни контроли.
- **No vendor lock-in:** отворени формати, отделни stems, MIDI и документирован project schema.
- **No misleading scoring:** „хармонично подходящо“ е препоръка, не обективна оценка на красотата.

## 2. Платформа и технологична архитектура

### 2.1 Предпочитан стек

- **Desktop shell:** Tauri 2.
- **UI:** React + TypeScript + Vite; състояние със Zustand или еквивалент, ясно разделено по домейни.
- **Real-time audio:** Rust + CPAL или друг доказан low-latency backend; минимални allocations/locks в audio callback; ring buffers между RT и UI/analysis нишките.
- **DSP:** Rust за latency-critical модули; Python за офлайн анализ и изследователски прототипи.
- **Music ML:** Python (librosa, numpy/scipy, Basic Pitch или друга валидирана алтернатива); проверявай лицензите, размерите и Windows packaging.
- **Notation:** VexFlow или Verovio след технически spike и проверка на лицензи/поддръжка.
- **MIDI:** стабилна библиотека с tempo map, note events, pitch bend, CC; MPE като бъдещо разширение.
- **Database:** SQLite с миграции; файловете се съхраняват извън БД, с метаданни и checksums.
- **Audio formats:** WAV 24-bit/32-bit float като основни; FLAC при възможност; MP3 import при законно и технически надеждно декодиране.
- **Windows:** Windows 10/11 x64; подписан инсталатор, когато е наличен сертификат; MSIX/MSI/NSIS според реалната поддръжка; no-admin install ако е практично.

**Преди имплементация:** направи 2–3-дневен architecture spike: устройство→запис→възпроизвеждане→waveform→WAV export в инсталиран Tauri build. Ако Rust/Tauri стекът затруднява стабилния аудио engine, документирай trade-offs и предложи JUCE/C++ engine като отделен процес или библиотека, без да сменяш целия UI импулсивно.

### 2.2 Архитектурни граници

```text
apps/desktop             Tauri + React UI
crates/audio-engine      RT audio I/O, transport, mixing, DSP, metronome
crates/music-core        note events, scales, modes, rhythm, harmony rules
crates/arranger          drone, chord voicing, bass, drum patterns, structure
services/analysis        offline pitch, onset, segmentation, mode hypotheses
packages/project-schema versioned project models + migrations
packages/lesson-content validated BG curriculum + exercises
packages/ui              reusable accessible components
assets/samples           properly licensed factory sounds
qa/fixtures              synthetic audio, short recorded instrument examples
scripts/release          Windows build, smoke test, packaging
```

Задължително раздели audio engine от UI. Използвай типизирани команди/събития; не изпращай raw audio frames през React state. Поддържай crash-safe autosave и schema migrations.

## 3. Персони и режими

1. **Абсолютен начинаещ:** не познава ноти; иска първа песен за 30–60 минути.
2. **Свирещ на акустичен инструмент:** кавал, флейта, гайда, цигулка, глас; иска анализ и акомпанимент.
3. **Продуцент:** иска MIDI/WAV stems, редакция, tempo map и интеграция с DAW.
4. **Преподавател:** създава упражнения, проследява напредък локално, демонстрира интонация и ритъм.

**Beginner Mode:** последователен wizard, контекстни подсказки, минимални контроли, undo, речник.
**Advanced Mode:** mixer, automation, pitch editor, chord inspector, groove templates, advanced export.
**Practice Mode:** микрофон, нотопис, ритъм, оценяване с толеранси и човешки обяснения.
**Studio Mode:** timeline и многопистов редактор.
**Live Accompanist:** отделна по-късна фаза; не обещавай безупречно предвиждане на импровизации.

## 4. Music Academy — пълна учебна програма

Всеки урок има: `learning objectives`, обяснение на прост български, аудио демонстрация, визуализация, интерактивно упражнение, автоматична проверка, полезна обратна връзка, повторение и мини проект. Уроците работят offline.

### Ниво 0 — Първи звук
- Какво е звук; височина, сила, продължителност, тембър; слушане срещу гледане на спектър.
- Как работят микрофонът, слушалките, звуковата карта; защо има латентност и clipping.
- Безопасна сила на звука и грижа за слуха.

### Ниво 1 — Ноти и ритъм
- Ноти, петолиние, ключ сол/фа, октави, диез/бемол/бекар.
- Нотни стойности, паузи, тактове, метроном, BPM, размери 2/4, 3/4, 4/4, 6/8.
- Ритмични упражнения с пляскане, микрофон или клавиатура; latency calibration.
- Практически тест: изсвири/изпляскай прост ритъм и получи обратна връзка.

### Ниво 2 — Мелодия
- Интервали, цели/полутонове, гами, мажор/минор, пентатоника, мотив, фраза, повторение, вариация.
- Мелодичен контур, дишане, артикулация, динамика, ornamentation.
- Нотопис + piano roll + waveform като три изгледа на една музикална идея.

### Ниво 3 — Хармония
- Тоника, опорен тон, бурдон/ручило, квинта, тризвучия, обръщения, септакорди.
- Напрежение/разрешение, voice leading, chord tones и passing tones.
- Модална хармония: дорийски, фригийски, миксолидийски и други ладове.
- Кога **да не** добавяме акорди; защо drone може да звучи по-естествено.

### Ниво 4 — Ритъм и жанрове
- Kick/snare/hi-hat, backbeat, breakbeat, swing, ghost notes, syncopation.
- 170–174 BPM Liquid DnB, halftime feel, rolling bass, pads и атмосферичен аранжимент.
- Български неравноделни размери (например 5/8, 7/8, 9/8) с реални групирания; без да ги свеждаме до „неправилен 4/4“.
- Как да съчетаем свободна етно фраза с фиксиран electronic groove.

### Ниво 5 — Композиция
- Идея→мотив→тема→хармония→ритъм→структура.
- Intro, A/B sections, build, breakdown, drop, outro; жанровите структури са предложения, не задължения.
- Call and response, контрапункт, текстура, пространство, енергия.
- Финален guided project: „Моята първа композиция“.

### Ниво 6 — Продукция
- Gain staging, EQ, компресия, reverb, delay, stereo field, headroom, loudness.
- Разлика между запис, редакция, аранжимент, микс и мастеринг.
- Експорт WAV/MIDI/stems и прехвърляне към външна DAW.

**Допълнителни инструменти:** речник с търсене, визуални карти на понятия, интерактивен квиз, проследяване на напредък, режим „обясни ми като на начинаещ“, помощ по контекст, упражнения за слух и ритъм, настройки за темпо и трудност.

## 5. Live Instrument Lab

### 5.1 Вход и устройства
- Избор на микрофон, sample rate, input channel, gain meter, device test, noise floor.
- Windows WASAPI shared mode като начален минимум; ASIO при проверена юридическа и техническа интеграция.
- Input monitoring с предупреждение за feedback; headphone recommendation.
- Latency calibration; защита от clipping; device disconnect/reconnect recovery.

### 5.2 Анализ
- Реално време: monophonic pitch estimate, confidence, note name, octave, Hz, cents, envelope, onset.
- Изгледи: тунер, спектър, pitch contour, scrolling notation, piano roll, waveform.
- Разпознаване на паузи, дъх и фрази; confidence gating при шум и несигурни сигнали.
- Офлайн: multi-hypothesis tonic/mode detection; tempo/beat hypotheses; segmentation.
- Запазвай суров pitch contour отделно от quantized MIDI; pitch bend и microtonal deviations.
- Не показвай фалшива сигурност: „вероятен лад“, confidence, алтернативи и ръчен избор.

### 5.3 Кавал/етно специализация
- Дълги тонове, орнаменти, grace notes, slides, вибрато, свободен метрум.
- Drone generator с root, fifth, octave, спектрални тембри и настройка на интонацията.
- Различавай концертна настройка, транспониращи инструменти и реално звучащ тон.
- Избор на tuning reference (например A4=440 Hz), cents offset и евентуални алтернативни настройки.
- Не премахвай автоматично всички breath noises — те може да са част от изпълнението.

### 5.4 Sample Recorder
- Record/stop/retake, trim, fades, normalize (non-destructive), denoise preview, tagging.
- Sample library: инструмент, тон, артикулация, BPM, key, лиценз, дата, favorite, търсене.
- One-shot и loop; sample mapping към клавиатура; export WAV; без загуба на оригинала.

## 6. Intelligent Accompanist — музикален двигател

### 6.1 Pipeline
1. Import/record → preserve source audio.
2. Analyze pitch/onsets/phrases → confidence + human corrections.
3. Generate tonic/mode hypotheses → explain evidence and let user choose.
4. Detect/choose pulse; allow **free-time** or quantized arrangement with tempo map.
5. Propose harmonic strategies: drone-only, modal pads, sparse chords, contemporary liquid chords.
6. Generate bass compatible with chosen harmonic strategy.
7. Generate drum grooves and fills, with controlled humanization.
8. Assemble arrangement sections, dynamics and transitions.
9. Rank *technical fit* (voice leading, clashes, density, register, rhythm), not artistic worth.
10. Render multiple reproducible variations and let user A/B compare.

### 6.2 Harmony engine
- Note-event representation with duration, accent, confidence, pitch bend.
- Candidate tonic/mode posterior, no forced D minor assumptions.
- Drone: root/fifth/octave, overtone shaping, filter, slow movement.
- Chord vocabulary: triads, sevenths, add9, sus2/sus4, quartal and modal voicings.
- Voice-leading penalties, avoid-notes as soft constraints, register/density control.
- Preserve melody primacy; optional harmonic rhythm per phrase instead of every bar.
- Explicit presets: Traditional Drone / Emotional Liquid / Deep Ethno / Cinematic Ambient.
- User locks: freeze melody, harmony, bass, drums or sections; regenerate only unlocked tracks.

### 6.3 Bass engine
- Sub sine/triangle, pluck, Reese bass preset; glide, saturation, sidechain envelope.
- Root-following and melodic patterns; rhythmic relation to kick; frequency-space checks.
- MIDI editable and reproducible; audio rendered with included instruments.

### 6.4 Drum engine
- Kick/snare/hat/shaker/ghost notes, breakbeat slicing, fills, velocity variation.
- 174 BPM liquid templates; halftime phrasing, groove and swing.
- Do not claim sampled drums are „realistic“ without listening tests; ship legal/licensed samples.
- Patterns editable in step sequencer and MIDI piano roll.

### 6.5 Variation lab
- Generate 3, 5, 10 variants; seed and parameter snapshots.
- A/B comparison with matched loudness (avoid louder-is-better bias).
- Save favorites; combine bass from C + pads from B + drone from A; detect incompatibilities.
- Controls: melancholy, warmth, rhythmic intensity, harmonic complexity, organic/electronic balance, arrangement density, variation strength.
- Explain tradeoffs, not universal music judgments.

## 7. Studio / Mini DAW

- Multitrack timeline, transport, looping, snapping, zoom, waveform rendering.
- Audio clips: trim, split, move, duplicate, fade, gain, mute/solo.
- MIDI clips: piano roll, notes, velocity, pitch bend, CC basics.
- Notation editor synced with MIDI; retain expressive pitch curve as separate data.
- Mixer: channel fader, pan, basic EQ, delay, reverb, sends (incrementally).
- Metronome, count-in, tempo, time signature, tempo map, section markers.
- Basic sampler, pad synth, bass synth, drum rack; no dependence on commercial VST.
- Autosave, undo/redo, project recovery after crash, missing-file relinking.
- Export full mix, stems, MIDI tracks, tempo map and README import instructions.
- Sample-rate conversion, export tail handling, peak protection, consistent start offset for stems.
- **Later:** VST3 hosting, automation lanes, comping, sidechain routing, plugin sandboxing.

## 8. Live Accompanist — отделен R&D етап

Раздели на:
- **Live Drone:** стабилен бурдон, управляван от потребителя; надежден MVP.
- **Live Beat Follower:** следи пулса и предлага groove; показва confidence.
- **Live Harmonic Follower:** сменя хармония при достатъчно надеждни музикални сигнали.
- **Predictive Improvisation:** експериментален режим с ясно обозначени ограничения.

За реално време използвай отделни RT и analysis threads, bounded queues, underrun/overrun counters и профилиране на latency. При ниска увереност задържай предишната хармония или преминавай към безопасен drone, вместо да сменяш акорди хаотично. Осигури manual override и footswitch/MIDI control като бъдеща опция.

## 9. UI / UX информационна архитектура

Начален екран:
- **Научи музика** — учебни пътеки.
- **Свири и записвай** — микрофон, тунер, нотопис, recorder.
- **Създай песен** — guided wizard.
- **Моето студио** — проекти и многопистов редактор.
- **Акомпаниатор** — хармония, ручило, бас, ритъм, варианти.
- **Моите семпли** — библиотека.
- **Помощ и речник** — търсене и контекстни подсказки.

Изисквания:
- Български UI с качествена терминология и английски като опция.
- Тъмна/светла тема; четим контраст; keyboard shortcuts; tooltips.
- Първо стартиране: audio device check, headphone test, 2-минутен walkthrough.
- Empty states, loading, errors, recovery, undo, confirmations при destructive actions.
- Никога не прави „Generate“ бутон, който симулира резултат.
- Дизайн система и component storybook/test gallery.

## 10. Проектни формати и експорт

Проектен пакет `.ethnoflow` (или директория със същата структура):

```text
project.json             version, tempo map, time signatures, markers
tracks/                  audio and MIDI clip metadata
assets/originals/        immutable source recordings
assets/derived/          generated/processed audio
samples/                 referenced/embedded licensed samples
analysis/                pitch contours, note candidates, confidence
variations/              seeds, parameter snapshots, user favorites
lessons/                 optional progress/annotations
```

Export:
- Master WAV 24-bit PCM / 32-bit float; MP3 optional.
- Stems с еднакъв start/end, tempo, sample rate и bit depth.
- Standard MIDI files (SMF), separate tracks, pitch bend where possible.
- `tempo_map.json`, `markers.csv`, `README_DAW_IMPORT.txt`.
- Проверявай, че повторният импорт в DAW възстановява синхронизацията.
- Отбелязвай авторство, лиценз на включени семпли и използвани AI услуги.

## 11. AI стратегия

**MVP не изисква генеративен AI.** Ползвай DSP, music theory rules и малки ML модели за анализ. Това е по-предвидимо, локално, обяснимо и редактираемо.

Постепенно добавяй:
- Pitch-to-MIDI model с възможност за ръчна корекция.
- Style/groove retrieval от локални, законно лицензирани шаблони.
- AI coach за естественоезикови обяснения, с проверка срещу учебната база.
- Optional cloud music generation само с изрично opt-in и ясни условия за права.
- Local generative models само след измерване на качество, латентност, лиценз и нужния хардуер.

Никога не обучавай върху потребителски записи без съгласие; никога не изпращай файлове към външни API без разрешение. Никакво имитиране на конкретни защитени записи/артисти като скрито подразбиране.

## 12. Измерими критерии за качество

- Устойчив audio I/O без сривове при 30+ мин запис и възпроизвеждане на тестов хардуер.
- Pitch detection: измери note accuracy, cents error, latency и failure cases върху синтетични тонове и реални кавал/флейта записи; не измисляй процент без benchmark.
- Ритъм: onset accuracy и timing error след калибриране на латентността.
- Harmony: набор от музикални fixtures с очаквани modal/drone/chord кандидати и regression tests.
- Export: sample-accurate или документирано ограничена синхронизация; stems с еднакъв старт.
- Project persistence: roundtrip save/load, миграции, recovery от прекъснат запис.
- Accessibility: keyboard-only critical flows, screen reader labels, contrast checks.
- Education: поне един напълно работещ учебен маршрут с оценка, преди масово генериране на съдържание.
- Performance: CPU/memory/audio callback budget измерени на базов Windows лаптоп.
- Windows install: clean-machine smoke test; no Python/Node/Rust toolchain needed on end-user machine.

## 13. Автоматични тестове

### Unit
- pitch-to-note conversion, cent offsets, scales/modes, chords, voice leading.
- beat grids, tempo maps, note durations, quantization.
- deterministic seeds and variation locks.
- serialization, validation, project migrations.

### DSP/audio
- sine sweeps, silence, clipping, noisy flute fixture, varying sample rates.
- latency, buffer underruns, dropouts, resampling, mono/stereo routing.
- record/playback WAV roundtrip, file corruption handling.

### Integration
- microphone permission/device error handling.
- record → analyze → arrange → render → export → reimport.
- sample recording → trim → save → reuse in project.
- lesson → perform → evaluate → feedback → progress save.

### UI/E2E
- first run setup, beginner lesson, project creation, variation selection.
- undo/redo, autosave, export, missing assets, offline mode.
- Windows installer and upgrade paths.

### Human music QA
- Blind listening comparisons by musicians where feasible.
- Check preservation of ornaments and musical phrasing.
- A/B variants at matched loudness.
- Document subjective feedback separately from technical correctness.

## 14. Етапи и контролни точки

**Важно:** Не започвай всичко наведнъж. Всяка фаза има `definition of done` и реално демо.

### Phase 0 — Discovery & feasibility
- Repository, ADRs, architecture diagrams, dependency/license audit.
- Working Windows Tauri skeleton, Rust audio spike, Python packaging spike.
- Decision log with risks and fallback options.
- **Exit:** installable app that records microphone and plays back WAV reliably.

### Phase 1 — Core recording & project system
- Device settings, waveform, record/playback, transport, metronome, import/export.
- SQLite project metadata, autosave, crash recovery, tests.
- **Exit:** record and reopen an original kaval take with no data loss.

### Phase 2 — Pitch, notation & practice
- Tuner, offline note extraction, pitch contour, note editor, notation viewer.
- Confidence display, manual correction, first ear/rhythm lessons.
- **Exit:** user records a melody, sees plausible notes, edits mistakes, exports MIDI.

### Phase 3 — Harmony / drone MVP
- Tonic/mode hypotheses, drone generator, sparse modal pads, 3 harmony choices.
- Clear explanation of musical choices, A/B compare, seed reproducibility.
- **Exit:** convincing drone/pad accompaniments on at least three different recordings.

### Phase 4 — Liquid DnB engine
- Drum rack, 174 BPM groove templates, sub/Reese bass, basic mixer.
- Phrase-aware arrangement and tempo alignment, no destructive quantization.
- **Exit:** real playable ethno liquid DnB track with editable bass/drum MIDI.

### Phase 5 — Mini DAW + export
- Multitrack editing, clip operations, MIDI piano roll, mix and stems.
- DAW roundtrip tests and guided import instructions.
- **Exit:** finish an arrangement in EthnoFlow and continue it in external DAW.

### Phase 6 — Beginner Academy v1
- Fully authored BG lessons levels 0–3, guided first-song wizard.
- Interactive exercises, evaluation, progress, glossary, accessibility QA.
- **Exit:** novice can complete a guided composition without outside tutorials.

### Phase 7 — Intelligent Variations
- 3/5/10 variants, locks, combine tracks, score technical fit, explanations.
- Better structure and dynamics; presets and saved favorites.
- **Exit:** users can create and compare distinct, musically coherent alternatives.

### Phase 8 — Live Accompanist R&D
- Live drone, beat follower, harmonic follower, confidence handling.
- Profiling and explicit latency/performance limits.
- **Exit:** safe, stable interactive demo; predictive mode remains experimental.

### Phase 9 — Release readiness
- Full Academy levels 4–6, localization review, installers, updates.
- Security review, accessibility, crash reports opt-in, licensing documentation.
- **Exit:** reproducible signed-or-clearly-unsigned Windows release, clean machine verified.

## 15. Изисквания към Claude Code като изпълнител

1. **Първо инспектирай текущото repository** и не презаписвай потребителски файлове.
2. Създай `docs/PRODUCT_VISION.md`, `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, `docs/DECISIONS.md`, `docs/TEST_PLAN.md`, `docs/RISKS.md`, `docs/CHANGELOG.md`.
3. Създай `CLAUDE.md` с команди за build/test/lint и правила за безопасни промени.
4. Направи първо минимален vertical slice: реален Windows audio input → запис → waveform → playback → WAV export.
5. Работи с малки комити и ясни описания; не смесвай несвързани функции.
6. След всеки етап изпълни build, unit/integration/e2e tests; докладвай резултатите, включително провалите.
7. Ако тест не може да се стартира, посочи точно защо; не маркирай като PASS.
8. Не използвай mock implementation в production без ясно обозначение и план за подмяна.
9. Преди добавяне на зависимости проверявай поддръжка, лицензи, Windows compatibility и supply-chain risk.
10. Използвай реални audio fixtures и синтетични тестови сигнали; запазвай оригиналните записи.
11. Създавай функционалност с feature flags за незавършени модули, без фалшиви UI обещания.
12. Документирай изборите и компромисите; ако откриеш по-добра технология, предложи ADR, не мигрирай без аргумент.
13. Подготвяй reproducible release artifacts; не твърди, че има `.exe`, ако не е изграден и проверен.
14. Изисквай одобрение преди платени API, cloud uploads, destructive operations или големи архитектурни промени.
15. Давай кратък отчет след всяка фаза: `Implemented / Tested / Known Issues / Next / User Decisions`.

## 16. Първа задача — изпълни сега

**Не имплементирай всички фази едновременно.**

1. Анализирай изискванията, предложи финална архитектура и изброи рисковете.
2. Направи Phase 0 и реален вертикален прототип: Windows app с избор на входно устройство, запис на микрофон, waveform, playback и WAV export.
3. Създай автоматични тестове за audio pipeline и save/load.
4. Покажи команди за build и точния път към генерирания installer **само ако реално съществува**.
5. Изготви подробен план за Phase 1 и спри за одобрение.

### Критичен тест с реален кавал

При наличен потребителски файл `re.mp3` използвай го само локално и с разрешение за тест на import, waveform, trimming и playback. **Не приемай предварително, че мелодията е в ре минор.** Анализът на тоналност и лад се реализира в следваща фаза и трябва да показва вероятности/алтернативи.

## 17. Какво да НЕ правиш

- Не създавай само статичен React dashboard с фиктивен waveform.
- Не генерирай фалшиви AI резултати или несъществуващи аудио файлове.
- Не hardcode-вай D minor, 174 BPM или 4/4 за всички записи.
- Не премахвай орнаменти, pitch bends и свободно фразиране при import.
- Не прави тежък ML inference вътре в real-time audio callback.
- Не изисквай Python или Node от крайния Windows потребител.
- Не смесвай „теорията твърди“ с „естетически е най-добре“.
- Не копирай защитени семпли, песни или стилове от конкретни изпълнители без съответните права.
- Не прави неконтролирани промени в архитектурата и зависимостите.

---

## Краен продукт

**EthnoFlow Studio** трябва да се усеща като истинска музикална работна среда: човек може да започне без никакви знания, да научи нотите и ритъма, да свири на акустичен инструмент, да записва и обработва семпли, да разбере какво е изсвирил, да чуе музикални подложки и вариации, да аранжира собствена композиция и да я довърши в любимата си DAW.

**Мисия:** Да превърнем спонтанната мелодия в завършена музика, без да губим човешкото изпълнение и без да изискваме предварително музикално образование.
