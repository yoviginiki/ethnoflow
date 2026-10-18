//! Academy lessons (FEATURE_SPEC F6.1, docs/academy/CURRICULUM.md).
//!
//! Lessons are data (`packages/lesson-content/lessons/*.json`, embedded at build time so
//! the app works offline); this crate validates them, turns an exercise into a concrete,
//! seeded *instance* (what to play, what to ask, what counts as right) inside the user's
//! instrument range and scale, and scores answers. Nothing here touches audio or files.

use serde::{Deserialize, Serialize};

use music_core::{evaluate, Note, Report, Tolerance};
pub use music_core::{ChordKind, ScaleKind};

// ---------------------------------------------------------------- schema

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Text {
    pub bg: String,
    pub en: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    pub id: String,
    /// 0..=6 (CURRICULUM §4).
    pub level: u8,
    pub order: u32,
    pub title: Text,
    pub objectives: Vec<Text>,
    /// At most three sentences (checked loosely: ≤ 600 chars).
    pub explanation: Text,
    #[serde(default)]
    pub glossary: Vec<String>,
    #[serde(default)]
    pub demo: Option<Demo>,
    pub exercise: Exercise,
    /// 0..=1; the lesson counts as passed at or above this score.
    #[serde(default = "default_pass")]
    pub pass_score: f64,
    #[serde(default)]
    pub quiz: Vec<QuizItem>,
    #[serde(default)]
    pub mini_project: Option<Text>,
    #[serde(default)]
    pub next: Option<String>,
    /// Optional learning track besides the numbered levels (e.g. `kaval`).
    #[serde(default)]
    pub track: Option<String>,
    /// Heading ids in the knowledge base the lesson points to (docs/kaval-baza-znania.md).
    #[serde(default)]
    pub reading: Vec<String>,
    /// MIDI range override for the exercise (e.g. one kaval register).
    #[serde(default)]
    pub range: Option<[i32; 2]>,
}

fn default_pass() -> f64 {
    0.7
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Demo {
    /// Notes as semitones from the tonic, each `beats` long.
    Notes { notes: Vec<DemoNote> },
    /// Metronome clicks with an accent on the first of each group.
    Clicks {
        bpm: f64,
        numerator: u32,
        #[serde(default = "four")]
        unit: u32,
        #[serde(default)]
        grouping: Vec<u32>,
        bars: u32,
    },
    /// The scale ascending from the tonic plus the octave.
    Scale { scale: ScaleKind },
    /// A block chord on the tonic.
    Chord { chord: ChordKind },
}

fn four() -> u32 {
    4
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoNote {
    pub degree: i32,
    #[serde(default = "one")]
    pub beats: f64,
    #[serde(default = "vel")]
    pub velocity: f64,
}

fn one() -> f64 {
    1.0
}
fn vel() -> f64 {
    0.7
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Target {
    #[default]
    Any,
    Tonic,
    /// Scale degree, 1-based.
    Degree {
        n: u32,
    },
    /// A fixed note (instrument lessons: "low A on the kaval").
    Midi {
        midi: i32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeterChoice {
    pub numerator: u32,
    pub unit: u32,
    #[serde(default)]
    pub grouping: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Check {
    /// The tuner shows one stable pitch for this long.
    TunerStable {
        seconds: f64,
    },
    RhythmRunning {
        bpm: Option<f64>,
    },
    /// A take recorded after the lesson started, with at least this many notes.
    NewTake {
        min_notes: u32,
    },
    ScaleChosen,
    /// Master space (reverb) at least this much, 0..1.
    SpaceAtLeast {
        value: f64,
    },
    /// An ambient preset saved after the lesson started.
    PresetSaved,
    MetronomeOn,
    /// The project has a tempo and a meter.
    TimingSet,
    /// The song map has at least this many sections.
    SongMapSections {
        min: u32,
    },
    /// At least one background layer (drone, pad, atmosphere, rhythm) is switched on.
    BackgroundOn,
    /// The arrangement holds at least this many clips.
    ArrangementClips {
        min: u32,
    },
    /// The whole song was exported after the lesson started.
    SongExported,
    /// The user visited this screen.
    Screen {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuidedStep {
    pub text: Text,
    pub check: Check,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Exercise {
    /// Only the `quiz` items.
    Quiz,
    HoldTone {
        seconds: f64,
        #[serde(default)]
        max_sigma_cents: Option<f64>,
        #[serde(default)]
        target: Target,
    },
    SightReading {
        count: u32,
        #[serde(default)]
        accidentals: bool,
    },
    HearAndRepeat {
        notes: u32,
        /// Fixed phrase as semitones from the tonic; random stepwise walk when absent.
        #[serde(default)]
        phrase: Option<Vec<i32>>,
        #[serde(default = "beat")]
        beat_seconds: f64,
    },
    EarInterval {
        /// Semitones.
        choices: Vec<u32>,
        rounds: u32,
    },
    EarChord {
        choices: Vec<ChordKind>,
        rounds: u32,
    },
    EarMode {
        choices: Vec<ScaleKind>,
        rounds: u32,
    },
    EarMeter {
        choices: Vec<MeterChoice>,
        rounds: u32,
        bpm: f64,
    },
    RhythmClap {
        bpm: f64,
        numerator: u32,
        bars: u32,
        /// Beat positions (0-based, within a bar) to clap on; every beat when absent.
        #[serde(default)]
        pattern: Option<Vec<f64>>,
    },
    PlayAlong {
        phrase: Vec<i32>,
        #[serde(default = "beat")]
        beat_seconds: f64,
    },
    Guided {
        steps: Vec<GuidedStep>,
    },
}

fn beat() -> f64 {
    0.6
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuizItem {
    pub q: Text,
    pub choices: Vec<Text>,
    pub answer: u32,
    #[serde(default)]
    pub why: Option<Text>,
}

// ---------------------------------------------------------------- embedded content

const FILES: [(&str, &str); 6] = [
    (
        "level0.json",
        include_str!("../../../packages/lesson-content/lessons/level0.json"),
    ),
    (
        "level1.json",
        include_str!("../../../packages/lesson-content/lessons/level1.json"),
    ),
    (
        "level2.json",
        include_str!("../../../packages/lesson-content/lessons/level2.json"),
    ),
    (
        "level3.json",
        include_str!("../../../packages/lesson-content/lessons/level3.json"),
    ),
    (
        "kaval.json",
        include_str!("../../../packages/lesson-content/lessons/kaval.json"),
    ),
    (
        "level5.json",
        include_str!("../../../packages/lesson-content/lessons/level5.json"),
    ),
];

/// Parse and validate every embedded lesson file; sorted by (level, order).
pub fn builtin() -> Result<Vec<Lesson>, String> {
    let mut all = Vec::new();
    for (name, text) in FILES {
        let mut v: Vec<Lesson> = serde_json::from_str(text).map_err(|e| format!("{name}: {e}"))?;
        all.append(&mut v);
    }
    validate(&all)?;
    all.sort_by_key(|l| (l.level, l.order));
    Ok(all)
}

fn text_ok(t: &Text, what: &str, id: &str) -> Result<(), String> {
    if t.bg.trim().is_empty() || t.en.trim().is_empty() {
        return Err(format!("{id}: {what} needs both bg and en"));
    }
    Ok(())
}

fn range(v: f64, lo: f64, hi: f64, what: &str, id: &str) -> Result<(), String> {
    if !v.is_finite() || v < lo || v > hi {
        return Err(format!("{id}: {what} {v} outside {lo}..{hi}"));
    }
    Ok(())
}

/// Every rule a lesson file must satisfy (CURRICULUM §2).
pub fn validate(lessons: &[Lesson]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for l in lessons {
        let id = &l.id;
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!("bad lesson id {id:?}"));
        }
        if !ids.insert(id.as_str()) {
            return Err(format!("duplicate lesson id {id}"));
        }
        if l.level > 6 {
            return Err(format!("{id}: level {} outside 0..6", l.level));
        }
        text_ok(&l.title, "title", id)?;
        text_ok(&l.explanation, "explanation", id)?;
        if l.explanation.bg.chars().count() > 600 {
            return Err(format!("{id}: explanation is longer than three sentences"));
        }
        if l.objectives.is_empty() {
            return Err(format!("{id}: no objectives"));
        }
        for o in &l.objectives {
            text_ok(o, "objective", id)?;
        }
        if let Some(m) = &l.mini_project {
            text_ok(m, "mini_project", id)?;
        }
        range(l.pass_score, 0.0, 1.0, "pass_score", id)?;
        if let Some([lo, hi]) = l.range {
            if lo >= hi || lo < 36 || hi > 108 {
                return Err(format!("{id}: range {lo}..{hi} must be within 36..108"));
            }
        }
        if let Some(t) = &l.track {
            if t.is_empty() || !t.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                return Err(format!("{id}: bad track {t:?}"));
            }
        }
        if l.reading.iter().any(|r| r.trim().is_empty()) {
            return Err(format!("{id}: empty reading id"));
        }
        for (i, q) in l.quiz.iter().enumerate() {
            text_ok(&q.q, "quiz question", id)?;
            if q.choices.len() < 2 || q.choices.len() > 4 {
                return Err(format!("{id}: quiz {i} needs 2..4 choices"));
            }
            for c in &q.choices {
                text_ok(c, "quiz choice", id)?;
            }
            if q.answer as usize >= q.choices.len() {
                return Err(format!("{id}: quiz {i} answer out of range"));
            }
            if let Some(w) = &q.why {
                text_ok(w, "quiz why", id)?;
            }
        }
        if let Some(d) = &l.demo {
            match d {
                Demo::Notes { notes } => {
                    if notes.is_empty() || notes.len() > 64 {
                        return Err(format!("{id}: demo needs 1..64 notes"));
                    }
                    for n in notes {
                        range(n.beats, 0.1, 8.0, "demo beats", id)?;
                        range(n.velocity, 0.05, 1.0, "demo velocity", id)?;
                        if n.degree.abs() > 36 {
                            return Err(format!("{id}: demo degree beyond ±36"));
                        }
                    }
                }
                Demo::Clicks {
                    bpm,
                    numerator,
                    unit,
                    grouping,
                    bars,
                } => {
                    range(*bpm, 30.0, 300.0, "demo bpm", id)?;
                    meter_ok(*numerator, *unit, grouping, id)?;
                    if *bars == 0 || *bars > 8 {
                        return Err(format!("{id}: demo bars 1..8"));
                    }
                }
                Demo::Scale { .. } | Demo::Chord { .. } => {}
            }
        }
        match &l.exercise {
            Exercise::Quiz => {
                if l.quiz.is_empty() {
                    return Err(format!("{id}: a quiz exercise needs quiz items"));
                }
            }
            Exercise::HoldTone {
                seconds,
                max_sigma_cents,
                target,
            } => {
                range(*seconds, 0.5, 60.0, "seconds", id)?;
                if let Some(s) = max_sigma_cents {
                    range(*s, 5.0, 100.0, "max_sigma_cents", id)?;
                }
                match target {
                    Target::Degree { n } if *n == 0 || *n > 7 => {
                        return Err(format!("{id}: degree 1..7"));
                    }
                    Target::Midi { midi } if !(36..=108).contains(midi) => {
                        return Err(format!("{id}: target midi 36..108"));
                    }
                    _ => {}
                }
            }
            Exercise::SightReading { count, .. } => {
                if *count == 0 || *count > 20 {
                    return Err(format!("{id}: count 1..20"));
                }
            }
            Exercise::HearAndRepeat {
                notes,
                phrase,
                beat_seconds,
            } => {
                if *notes == 0 || *notes > 16 {
                    return Err(format!("{id}: notes 1..16"));
                }
                range(*beat_seconds, 0.15, 3.0, "beat_seconds", id)?;
                if let Some(p) = phrase {
                    if p.is_empty() || p.len() > 16 || p.iter().any(|d| d.abs() > 24) {
                        return Err(format!("{id}: phrase 1..16 degrees within ±24"));
                    }
                }
            }
            Exercise::EarInterval { choices, rounds } => {
                rounds_ok(*rounds, id)?;
                if choices.len() < 2 || choices.iter().any(|s| *s > 24) {
                    return Err(format!("{id}: interval choices ≥ 2, each ≤ 24"));
                }
            }
            Exercise::EarChord { choices, rounds } => {
                rounds_ok(*rounds, id)?;
                if choices.len() < 2 {
                    return Err(format!("{id}: chord choices ≥ 2"));
                }
            }
            Exercise::EarMode { choices, rounds } => {
                rounds_ok(*rounds, id)?;
                if choices.len() < 2 {
                    return Err(format!("{id}: mode choices ≥ 2"));
                }
            }
            Exercise::EarMeter {
                choices,
                rounds,
                bpm,
            } => {
                rounds_ok(*rounds, id)?;
                range(*bpm, 30.0, 300.0, "bpm", id)?;
                if choices.len() < 2 {
                    return Err(format!("{id}: meter choices ≥ 2"));
                }
                for c in choices {
                    meter_ok(c.numerator, c.unit, &c.grouping, id)?;
                }
            }
            Exercise::RhythmClap {
                bpm,
                numerator,
                bars,
                pattern,
            } => {
                range(*bpm, 30.0, 300.0, "bpm", id)?;
                meter_ok(*numerator, 4, &[], id)?;
                if *bars == 0 || *bars > 8 {
                    return Err(format!("{id}: bars 1..8"));
                }
                if let Some(p) = pattern {
                    if p.is_empty() || p.iter().any(|b| *b < 0.0 || *b >= *numerator as f64) {
                        return Err(format!("{id}: pattern beats within the bar"));
                    }
                }
            }
            Exercise::PlayAlong {
                phrase,
                beat_seconds,
            } => {
                range(*beat_seconds, 0.15, 3.0, "beat_seconds", id)?;
                if phrase.is_empty() || phrase.len() > 64 {
                    return Err(format!("{id}: phrase 1..64"));
                }
            }
            Exercise::Guided { steps } => {
                if steps.is_empty() || steps.len() > 12 {
                    return Err(format!("{id}: guided steps 1..12"));
                }
                for s in steps {
                    text_ok(&s.text, "step", id)?;
                    match &s.check {
                        Check::TunerStable { seconds } => {
                            range(*seconds, 0.5, 30.0, "seconds", id)?
                        }
                        Check::SpaceAtLeast { value } => range(*value, 0.0, 1.0, "value", id)?,
                        Check::RhythmRunning { bpm: Some(b) } => range(*b, 30.0, 300.0, "bpm", id)?,
                        Check::SongMapSections { min } | Check::ArrangementClips { min }
                            if *min == 0 || *min > 64 =>
                        {
                            return Err(format!("{id}: min 1..64"));
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    for l in lessons {
        if let Some(n) = &l.next {
            if !ids.contains(n.as_str()) {
                return Err(format!("{}: next {n:?} does not exist", l.id));
            }
        }
    }
    Ok(())
}

fn rounds_ok(r: u32, id: &str) -> Result<(), String> {
    if r == 0 || r > 20 {
        return Err(format!("{id}: rounds 1..20"));
    }
    Ok(())
}

fn meter_ok(numerator: u32, unit: u32, grouping: &[u32], id: &str) -> Result<(), String> {
    if numerator == 0 || numerator > 16 || !matches!(unit, 2 | 4 | 8 | 16) {
        return Err(format!("{id}: meter {numerator}/{unit}"));
    }
    if !grouping.is_empty() && grouping.iter().sum::<u32>() != numerator {
        return Err(format!("{id}: grouping does not add up to {numerator}"));
    }
    Ok(())
}

// ---------------------------------------------------------------- instances

/// What the exercise is generated inside: the project's scale (or a neutral default
/// chosen by the caller) and the instrument's playable range.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Context {
    pub tonic_pc: u8,
    pub kind: ScaleKind,
    pub lo_midi: i32,
    pub hi_midi: i32,
}

/// A note for the demo synth (seconds, MIDI, 0..1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayNote {
    pub start: f64,
    pub end: f64,
    pub midi: i32,
    pub velocity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Round {
    pub prompt: Vec<PlayNote>,
    /// Choice keys the UI translates: `interval:7`, `chord:major`, `mode:dorian`,
    /// `meter:7/8:2+2+3`.
    pub choices: Vec<String>,
    pub answer: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Instance {
    Quiz,
    HoldTone {
        /// None = any note.
        target_midi: Option<i32>,
        seconds: f64,
        max_sigma_cents: Option<f64>,
    },
    SightReading {
        targets: Vec<i32>,
    },
    HearAndRepeat {
        expected: Vec<PlayNote>,
    },
    Ear {
        rounds: Vec<Round>,
    },
    RhythmClap {
        /// Count-in bar plus the exercise bars.
        clicks: Vec<PlayNote>,
        /// Seconds (on the click timeline) where a clap is expected.
        grid: Vec<f64>,
        bpm: f64,
    },
    PlayAlong {
        notes: Vec<PlayNote>,
    },
    Guided,
}

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    fn next(&mut self) -> u64 {
        // xorshift64*
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

const CLICK_HI: i32 = 96;
const CLICK_LO: i32 = 91;
const CLICK_LEN: f64 = 0.05;

/// Lowest MIDI note with the tonic's pitch class inside the range (or the range floor).
pub fn tonic_midi(ctx: &Context) -> i32 {
    let lo = ctx.lo_midi;
    let mut m = lo;
    while m <= ctx.hi_midi {
        if m.rem_euclid(12) == ctx.tonic_pc as i32 {
            return m;
        }
        m += 1;
    }
    lo
}

/// Scale notes of the context's kind within the range, ascending.
pub fn scale_notes(ctx: &Context) -> Vec<i32> {
    let semis = ctx.kind.semitones();
    (ctx.lo_midi..=ctx.hi_midi)
        .filter(|m| {
            let rel = (m - ctx.tonic_pc as i32).rem_euclid(12) as u8;
            semis.contains(&rel)
        })
        .collect()
}

fn clamp_range(ctx: &Context, midi: i32) -> i32 {
    let mut m = midi;
    while m > ctx.hi_midi {
        m -= 12;
    }
    while m < ctx.lo_midi {
        m += 12;
    }
    m
}

fn clicks(
    bpm: f64,
    numerator: u32,
    unit: u32,
    grouping: &[u32],
    bars: u32,
    t0: f64,
) -> Vec<PlayNote> {
    let beat = 60.0 / bpm * 4.0 / unit as f64;
    let mut accents = vec![false; numerator as usize];
    if grouping.is_empty() {
        accents[0] = true;
    } else {
        let mut i = 0usize;
        for g in grouping {
            if i < accents.len() {
                accents[i] = true;
            }
            i += *g as usize;
        }
    }
    let mut out = Vec::new();
    for bar in 0..bars {
        for b in 0..numerator {
            let t = t0 + (bar * numerator + b) as f64 * beat;
            let acc = accents[b as usize];
            out.push(PlayNote {
                start: t,
                end: t + CLICK_LEN,
                midi: if acc { CLICK_HI } else { CLICK_LO },
                velocity: if acc { 1.0 } else { 0.45 },
            });
        }
    }
    out
}

fn phrase_notes(degrees: &[i32], ctx: &Context, beat_seconds: f64, t0: f64) -> Vec<PlayNote> {
    let tonic = tonic_midi(ctx);
    degrees
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let t = t0 + i as f64 * beat_seconds;
            PlayNote {
                start: t,
                end: t + beat_seconds * 0.9,
                midi: clamp_range(ctx, tonic + d),
                velocity: 0.7,
            }
        })
        .collect()
}

/// Notes for a lesson's demonstration.
pub fn demo_notes(demo: &Demo, ctx: &Context) -> Vec<PlayNote> {
    match demo {
        Demo::Notes { notes } => {
            let tonic = tonic_midi(ctx);
            let mut t = 0.2;
            let mut out = Vec::new();
            for n in notes {
                let len = n.beats * 0.6;
                out.push(PlayNote {
                    start: t,
                    end: t + len * 0.92,
                    midi: clamp_range(ctx, tonic + n.degree),
                    velocity: n.velocity,
                });
                t += len;
            }
            out
        }
        Demo::Clicks {
            bpm,
            numerator,
            unit,
            grouping,
            bars,
        } => clicks(*bpm, *numerator, *unit, grouping, *bars, 0.2),
        Demo::Scale { scale } => {
            let degrees: Vec<i32> = scale
                .semitones()
                .iter()
                .map(|s| *s as i32)
                .chain(std::iter::once(12))
                .collect();
            phrase_notes(&degrees, ctx, 0.4, 0.2)
        }
        Demo::Chord { chord } => {
            let tonic = tonic_midi(ctx);
            chord
                .intervals()
                .iter()
                .map(|i| PlayNote {
                    start: 0.2,
                    end: 1.8,
                    midi: clamp_range(ctx, tonic + *i as i32),
                    velocity: 0.6,
                })
                .collect()
        }
    }
}

fn chord_key(k: ChordKind) -> String {
    format!(
        "chord:{}",
        serde_json::to_value(k)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    )
}

fn mode_key(k: ScaleKind) -> String {
    format!(
        "mode:{}",
        serde_json::to_value(k)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default()
    )
}

fn meter_key(m: &MeterChoice) -> String {
    if m.grouping.is_empty() {
        format!("meter:{}/{}", m.numerator, m.unit)
    } else {
        let g: Vec<String> = m.grouping.iter().map(|x| x.to_string()).collect();
        format!("meter:{}/{}:{}", m.numerator, m.unit, g.join("+"))
    }
}

/// Build a concrete, reproducible instance of the lesson's exercise.
pub fn instance(lesson: &Lesson, seed: u64, ctx: &Context) -> Instance {
    let mut rng = Rng::new(seed ^ hash(&lesson.id));
    let (lo, hi) = match lesson.range {
        Some([lo, hi]) => (lo, hi),
        None => (ctx.lo_midi, ctx.hi_midi),
    };
    let ctx = &Context {
        lo_midi: lo.min(hi),
        hi_midi: hi.max(lo),
        ..*ctx
    };
    match &lesson.exercise {
        Exercise::Quiz => Instance::Quiz,
        Exercise::HoldTone {
            seconds,
            max_sigma_cents,
            target,
        } => {
            let target_midi = match target {
                Target::Any => None,
                Target::Tonic => Some(tonic_midi(ctx)),
                Target::Degree { n } => {
                    let semis = ctx.kind.semitones();
                    let i = (*n as usize - 1).min(semis.len() - 1);
                    Some(clamp_range(ctx, tonic_midi(ctx) + semis[i] as i32))
                }
                Target::Midi { midi } => Some(*midi),
            };
            Instance::HoldTone {
                target_midi,
                seconds: *seconds,
                max_sigma_cents: *max_sigma_cents,
            }
        }
        Exercise::SightReading { count, accidentals } => {
            let pool: Vec<i32> = if *accidentals {
                (ctx.lo_midi..=ctx.hi_midi).collect()
            } else {
                scale_notes(ctx)
            };
            let mut targets = Vec::new();
            let mut last = None;
            for _ in 0..*count {
                let mut pick = pool[rng.below(pool.len())];
                if pool.len() > 1 {
                    while Some(pick) == last {
                        pick = pool[rng.below(pool.len())];
                    }
                }
                last = Some(pick);
                targets.push(pick);
            }
            Instance::SightReading { targets }
        }
        Exercise::HearAndRepeat {
            notes,
            phrase,
            beat_seconds,
        } => {
            let expected = if let Some(p) = phrase {
                phrase_notes(p, ctx, *beat_seconds, 0.3)
            } else {
                // Stepwise walk on the scale, starting on the tonic, staying in range.
                let pool = scale_notes(ctx);
                let tonic = tonic_midi(ctx);
                let mut idx = pool.iter().position(|m| *m == tonic).unwrap_or(0);
                let mut out = Vec::new();
                for i in 0..*notes as usize {
                    let t = 0.3 + i as f64 * beat_seconds;
                    out.push(PlayNote {
                        start: t,
                        end: t + beat_seconds * 0.9,
                        midi: pool[idx],
                        velocity: 0.7,
                    });
                    let step = match rng.below(5) {
                        0 => -2i32,
                        1 => -1,
                        2 => 0,
                        3 => 1,
                        _ => 2,
                    };
                    idx = (idx as i32 + step).clamp(0, pool.len() as i32 - 1) as usize;
                }
                out
            };
            Instance::HearAndRepeat { expected }
        }
        Exercise::EarInterval { choices, rounds } => {
            let keys: Vec<String> = choices.iter().map(|s| format!("interval:{s}")).collect();
            let rounds = (0..*rounds)
                .map(|_| {
                    let answer = rng.below(choices.len());
                    let semi = choices[answer] as i32;
                    let span = (ctx.hi_midi - ctx.lo_midi - semi).max(0);
                    let root = ctx.lo_midi + rng.below(span as usize + 1) as i32;
                    let prompt = vec![
                        PlayNote {
                            start: 0.2,
                            end: 0.85,
                            midi: root,
                            velocity: 0.7,
                        },
                        PlayNote {
                            start: 0.95,
                            end: 1.6,
                            midi: (root + semi).min(ctx.hi_midi),
                            velocity: 0.7,
                        },
                    ];
                    Round {
                        prompt,
                        choices: keys.clone(),
                        answer,
                    }
                })
                .collect();
            Instance::Ear { rounds }
        }
        Exercise::EarChord { choices, rounds } => {
            let keys: Vec<String> = choices.iter().map(|c| chord_key(*c)).collect();
            let rounds = (0..*rounds)
                .map(|_| {
                    let answer = rng.below(choices.len());
                    let kind = choices[answer];
                    let top = *kind.intervals().iter().max().unwrap_or(&0) as i32;
                    let span = (ctx.hi_midi - ctx.lo_midi - top).max(0);
                    let root = ctx.lo_midi + rng.below(span as usize + 1) as i32;
                    let prompt = kind
                        .intervals()
                        .iter()
                        .map(|i| PlayNote {
                            start: 0.2,
                            end: 1.8,
                            midi: (root + *i as i32).min(ctx.hi_midi),
                            velocity: 0.6,
                        })
                        .collect();
                    Round {
                        prompt,
                        choices: keys.clone(),
                        answer,
                    }
                })
                .collect();
            Instance::Ear { rounds }
        }
        Exercise::EarMode { choices, rounds } => {
            let keys: Vec<String> = choices.iter().map(|k| mode_key(*k)).collect();
            let rounds = (0..*rounds)
                .map(|_| {
                    let answer = rng.below(choices.len());
                    let kind = choices[answer];
                    let span = (ctx.hi_midi - ctx.lo_midi - 12).max(0);
                    let root = ctx.lo_midi + rng.below(span as usize + 1) as i32;
                    let prompt = kind
                        .semitones()
                        .iter()
                        .map(|s| *s as i32)
                        .chain(std::iter::once(12))
                        .enumerate()
                        .map(|(i, s)| PlayNote {
                            start: 0.2 + i as f64 * 0.38,
                            end: 0.2 + i as f64 * 0.38 + 0.34,
                            midi: (root + s).min(ctx.hi_midi),
                            velocity: 0.7,
                        })
                        .collect();
                    Round {
                        prompt,
                        choices: keys.clone(),
                        answer,
                    }
                })
                .collect();
            Instance::Ear { rounds }
        }
        Exercise::EarMeter {
            choices,
            rounds,
            bpm,
        } => {
            let keys: Vec<String> = choices.iter().map(meter_key).collect();
            let rounds = (0..*rounds)
                .map(|_| {
                    let answer = rng.below(choices.len());
                    let m = &choices[answer];
                    Round {
                        prompt: clicks(*bpm, m.numerator, m.unit, &m.grouping, 2, 0.2),
                        choices: keys.clone(),
                        answer,
                    }
                })
                .collect();
            Instance::Ear { rounds }
        }
        Exercise::RhythmClap {
            bpm,
            numerator,
            bars,
            pattern,
        } => {
            let beat = 60.0 / bpm;
            let t0 = 0.3;
            let all = clicks(*bpm, *numerator, 4, &[], bars + 1, t0);
            let pat: Vec<f64> = pattern
                .clone()
                .unwrap_or_else(|| (0..*numerator).map(|b| b as f64).collect());
            let mut grid = Vec::new();
            for bar in 1..=*bars {
                for b in &pat {
                    grid.push(t0 + (bar * numerator) as f64 * beat + b * beat);
                }
            }
            Instance::RhythmClap {
                clicks: all,
                grid,
                bpm: *bpm,
            }
        }
        Exercise::PlayAlong {
            phrase,
            beat_seconds,
        } => Instance::PlayAlong {
            notes: phrase_notes(phrase, ctx, *beat_seconds, 0.3),
        },
        Exercise::Guided { .. } => Instance::Guided,
    }
}

fn hash(s: &str) -> u64 {
    // FNV-1a
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

// ---------------------------------------------------------------- scoring

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RhythmScore {
    /// matched / expected, each matched clap weighted by its closeness.
    pub score: f64,
    /// For every grid point: signed ms deviation of the matched clap, or None.
    pub deviations_ms: Vec<Option<f64>>,
    pub extra: usize,
    pub mean_abs_ms: f64,
}

/// Match claps (onset seconds) to the grid: nearest within the window, one each.
pub fn score_rhythm(onsets: &[f64], grid: &[f64], tol_ms: f64) -> RhythmScore {
    let window = (tol_ms * 3.0).max(250.0) / 1000.0;
    let mut used = vec![false; onsets.len()];
    let mut deviations = Vec::with_capacity(grid.len());
    let mut sum = 0.0;
    let mut n = 0usize;
    let mut credit = 0.0;
    for g in grid {
        let mut best: Option<(usize, f64)> = None;
        for (i, o) in onsets.iter().enumerate() {
            if used[i] {
                continue;
            }
            let d = o - g;
            if d.abs() <= window && best.is_none_or(|(_, bd)| d.abs() < bd.abs()) {
                best = Some((i, d));
            }
        }
        match best {
            Some((i, d)) => {
                used[i] = true;
                let ms = d * 1000.0;
                deviations.push(Some(ms));
                sum += ms.abs();
                n += 1;
                credit += if ms.abs() <= tol_ms {
                    1.0
                } else {
                    (1.0 - (ms.abs() - tol_ms) / (window * 1000.0 - tol_ms)).clamp(0.0, 1.0) * 0.5
                };
            }
            None => deviations.push(None),
        }
    }
    let extra = used.iter().filter(|u| !**u).count();
    RhythmScore {
        score: if grid.is_empty() {
            0.0
        } else {
            (credit / grid.len() as f64) * (1.0 - 0.1 * extra as f64).max(0.5)
        },
        deviations_ms: deviations,
        extra,
        mean_abs_ms: if n == 0 { 0.0 } else { sum / n as f64 },
    }
}

/// Fraction of required seconds held, capped at 1.
pub fn score_hold(seconds_in_tolerance: f64, required: f64) -> f64 {
    if required <= 0.0 {
        return 1.0;
    }
    (seconds_in_tolerance / required).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepeatScore {
    pub score: f64,
    pub report: Report,
    /// Seconds the played notes were shifted so the first onset lines up.
    pub shift_seconds: f64,
}

/// "Hear and repeat": the player starts whenever; the first played onset is aligned to
/// the first expected onset, then the trainer's evaluation applies.
pub fn score_repeat(expected: &[PlayNote], played: &[Note], tol: &Tolerance) -> RepeatScore {
    let exp: Vec<Note> = expected
        .iter()
        .map(|p| Note {
            start: p.start,
            end: p.end,
            midi: p.midi,
            cents: 0.0,
            confidence: 1.0,
            velocity: p.velocity,
            bend: Vec::new(),
        })
        .collect();
    let shift = match (
        exp.iter().map(|n| n.start).fold(f64::INFINITY, f64::min),
        played.iter().map(|n| n.start).fold(f64::INFINITY, f64::min),
    ) {
        (e, p) if e.is_finite() && p.is_finite() => e - p,
        _ => 0.0,
    };
    let shifted: Vec<Note> = played
        .iter()
        .map(|n| Note {
            start: n.start + shift,
            end: n.end + shift,
            ..n.clone()
        })
        .collect();
    let report = evaluate(&exp, &shifted, tol, 0.0);
    let hits = report.summary.hits as f64;
    let expected_n = report.summary.expected.max(1) as f64;
    RepeatScore {
        score: hits / expected_n,
        report,
        shift_seconds: shift,
    }
}

// ---------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> Context {
        Context {
            tonic_pc: 2,
            kind: ScaleKind::Dorian,
            lo_midi: 62,
            hi_midi: 86,
        }
    }

    #[test]
    fn builtin_lessons_validate_and_cover_levels_0_to_3() {
        let all = builtin().expect("lesson files valid");
        for level in [0u8, 1, 2, 3, 5] {
            assert!(
                all.iter().any(|l| l.level == level),
                "no lessons for level {level}"
            );
        }
        assert!(all.len() >= 24, "{} lessons", all.len());
        // The owner's path (CURRICULUM §6) exists.
        for id in [
            "l0-01-sound",
            "l1-01-names",
            "l1-04-pulse",
            "l2-05-five-notes",
            "l3-01-tonic",
            "l3-09-lesson-one",
            "l5-06-first-song",
        ] {
            assert!(all.iter().any(|l| l.id == id), "missing {id}");
        }
        // Every exercise instantiates inside a kaval-like range without panicking.
        for l in &all {
            let _ = instance(l, 7, &ctx());
            if let Some(d) = &l.demo {
                assert!(!demo_notes(d, &ctx()).is_empty(), "{}: empty demo", l.id);
            }
        }
        // The kaval track (owner's knowledge base) is there and its lessons point into it.
        let kaval: Vec<&Lesson> = all
            .iter()
            .filter(|l| l.track.as_deref() == Some("kaval"))
            .collect();
        assert!(kaval.len() >= 12, "{} kaval lessons", kaval.len());
        assert!(kaval.iter().all(|l| !l.reading.is_empty()));
        // A register-limited lesson keeps its targets inside the register.
        let first = all.iter().find(|l| l.id == "k-03-first-register").unwrap();
        if let Instance::SightReading { targets } = instance(first, 3, &ctx()) {
            assert!(targets.iter().all(|t| (62..=71).contains(t)), "{targets:?}");
        } else {
            panic!("expected sight reading");
        }
    }

    #[test]
    fn validator_rejects_broken_lessons() {
        let mut ok = builtin().unwrap();
        let mut dup = ok[0].clone();
        dup.order += 100;
        let mut bad = ok.clone();
        bad.push(dup);
        assert!(validate(&bad).unwrap_err().contains("duplicate"));
        ok[0].next = Some("nope".into());
        assert!(validate(&ok).unwrap_err().contains("does not exist"));
        let mut q = builtin().unwrap();
        let l = q.iter_mut().find(|l| !l.quiz.is_empty()).unwrap();
        l.quiz[0].answer = 9;
        assert!(validate(&q).unwrap_err().contains("answer out of range"));
        let bad_json = r#"[{"id":"x","level":1,"order":1,"title":{"bg":"a","en":"b"},"objectives":[{"bg":"a","en":"b"}],"explanation":{"bg":"a","en":"b"},"exercise":{"type":"teleport"}}]"#;
        assert!(serde_json::from_str::<Vec<Lesson>>(bad_json).is_err());
    }

    #[test]
    fn instances_are_seeded_and_stay_in_range() {
        let all = builtin().unwrap();
        let l = all.iter().find(|l| l.id == "l2-02-intervals").unwrap();
        let a = instance(l, 42, &ctx());
        let b = instance(l, 42, &ctx());
        let c = instance(l, 43, &ctx());
        assert_eq!(a, b);
        assert_ne!(a, c);
        if let Instance::Ear { rounds } = &a {
            assert!(!rounds.is_empty());
            for r in rounds {
                assert!(r.answer < r.choices.len());
                for n in &r.prompt {
                    assert!((62..=86).contains(&n.midi), "{} out of range", n.midi);
                }
            }
        } else {
            panic!("expected ear rounds");
        }
        let sr = all.iter().find(|l| l.id == "l1-01-names").unwrap();
        if let Instance::SightReading { targets } = instance(sr, 1, &ctx()) {
            let scale = scale_notes(&ctx());
            assert!(targets.iter().all(|t| scale.contains(t)));
        } else {
            panic!("expected sight reading");
        }
    }

    #[test]
    fn rhythm_score_matches_claps_and_counts_misses() {
        let grid = [1.0, 1.5, 2.0, 2.5];
        let perfect = score_rhythm(&[1.0, 1.5, 2.0, 2.5], &grid, 80.0);
        assert!((perfect.score - 1.0).abs() < 1e-9);
        let late = score_rhythm(&[1.04, 1.55, 2.0], &grid, 80.0);
        assert_eq!(late.deviations_ms[3], None);
        assert!((late.deviations_ms[0].unwrap() - 40.0).abs() < 1e-6);
        assert!(late.score < 1.0 && late.score >= 0.7, "{}", late.score);
        let noisy = score_rhythm(&[1.0, 1.1, 1.2, 1.5, 2.0, 2.5, 3.3], &grid, 80.0);
        assert_eq!(noisy.extra, 3);
        assert!(noisy.score < perfect.score);
    }

    #[test]
    fn repeat_score_aligns_the_first_onset() {
        let expected = vec![
            PlayNote {
                start: 0.3,
                end: 0.8,
                midi: 62,
                velocity: 0.7,
            },
            PlayNote {
                start: 0.9,
                end: 1.4,
                midi: 64,
                velocity: 0.7,
            },
        ];
        let played: Vec<Note> = [(5.0, 62), (5.6, 64)]
            .iter()
            .map(|(s, m)| Note {
                start: *s,
                end: s + 0.5,
                midi: *m,
                cents: 5.0,
                confidence: 1.0,
                velocity: 0.7,
                bend: Vec::new(),
            })
            .collect();
        let r = score_repeat(&expected, &played, &Tolerance::beginner());
        assert!((r.shift_seconds + 4.7).abs() < 1e-9);
        assert_eq!(r.report.summary.hits, 2);
        assert!((r.score - 1.0).abs() < 1e-9);
        assert!((score_hold(1.0, 2.0) - 0.5).abs() < 1e-9);
    }
}
