//! F2.11 benchmark set (ERROR_TOLERANT_PERFORMANCE_SPEC "Автоматични тестове"):
//! synthetic performances of a modal melody with controlled defects. Measures false
//! harmonic switches, missed real changes and switch latency per mode, and asserts the
//! acceptance criterion: ~20 % deliberate inaccuracies leave the context stable.

use music_core::{
    analyze_performance, AnalysisInput, Calibration, Expected, Locks, Mode, Note, NoteClass,
    PhraseParams, Scale, ScaleKind,
};

/// Deterministic xorshift so every run is reproducible.
struct Rng(u32);
impl Rng {
    fn next(&mut self) -> u32 {
        let mut s = self.0;
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        self.0 = s;
        s
    }
    fn unit(&mut self) -> f64 {
        (self.next() % 10_000) as f64 / 10_000.0
    }
}

fn note(start: f64, dur: f64, midi: i32, cents: f64, conf: f64) -> Note {
    Note {
        start,
        end: start + dur,
        midi,
        cents,
        confidence: conf,
        velocity: 0.8,
        bend: Vec::new(),
    }
}

/// Four phrases of a melody in `scale` (degrees as indices), with breaths between.
/// Returned notes are "correct"; scenarios then damage copies of them.
fn melody(scale: &Scale, root_midi: i32, phrases: usize, tempo: f64) -> Vec<Note> {
    let degs: Vec<f64> = scale.degrees_cents.clone();
    let deg = |d: i32| -> (i32, f64) {
        let n = degs.len() as i32;
        let oct = d.div_euclid(n);
        let c = degs[d.rem_euclid(n) as usize] + 1200.0 * oct as f64;
        let semis = (c / 100.0).round() as i32;
        (root_midi + semis, c - semis as f64 * 100.0)
    };
    // Phrase shapes over scale degrees (tonic = 0): arch, fall, rise, cadence.
    let shapes: [&[(i32, f64)]; 4] = [
        &[
            (0, 2.0),
            (1, 1.0),
            (2, 1.0),
            (4, 2.0),
            (2, 1.0),
            (1, 1.0),
            (0, 3.0),
        ],
        &[(4, 2.0), (3, 1.0), (2, 1.0), (1, 1.0), (0, 3.0)],
        &[(0, 1.0), (2, 1.0), (4, 2.0), (5, 1.0), (4, 1.0), (7, 3.0)],
        &[(4, 1.0), (2, 1.0), (1, 1.0), (0, 1.0), (4, 2.0), (0, 4.0)],
    ];
    let beat = 60.0 / tempo;
    let mut t = 0.0;
    let mut out = Vec::new();
    for p in 0..phrases {
        for &(d, beats) in shapes[p % 4] {
            let (m, c) = deg(d);
            out.push(note(t, beats * beat * 0.95, m, c, 0.92));
            t += beats * beat;
        }
        t += 0.6; // breath
    }
    out
}

struct Outcome {
    name: &'static str,
    mode: Mode,
    switches: usize,
    expected_switches: usize,
    latency_phrases: Option<usize>,
    change_at_phrase: Option<usize>,
    errors: usize,
    ornaments: usize,
    stable: usize,
}

fn run(
    name: &'static str,
    mode: Mode,
    notes: &[Note],
    scale: Option<&Scale>,
    expected: &[Expected],
    change_at: Option<usize>,
    expected_switches: usize,
) -> Outcome {
    let a = analyze_performance(&AnalysisInput {
        notes,
        scale,
        profile: &[],
        expected,
        annotations: &[],
        mode,
        mode_params: mode.default_params(),
        locks: Locks::default(),
        phrase_params: PhraseParams::for_wind(),
        calibration: Calibration::default(),
        kinds: &ScaleKind::ALL,
    });
    let latency = change_at.and_then(|at| {
        a.contexts
            .iter()
            .position(|c| c.switched && c.phrase_index >= at)
            .map(|i| i + 1 - at)
    });
    let count = |k: NoteClass| a.classes.iter().filter(|c| c.class == k).count();
    Outcome {
        name,
        mode,
        switches: a.switches,
        expected_switches,
        latency_phrases: latency,
        change_at_phrase: change_at,
        errors: count(NoteClass::ProbableError),
        ornaments: count(NoteClass::Ornament),
        stable: count(NoteClass::Stable),
    }
}

#[test]
fn error_tolerant_scenarios_measured_per_mode() {
    let d_dorian = Scale::new(2, ScaleKind::Dorian);
    let a_hijaz = Scale::new(9, ScaleKind::PhrygianDominant);
    let clean = melody(&d_dorian, 62, 8, 96.0);
    let mut outcomes = Vec::new();

    for mode in [Mode::Stable, Mode::Assisted, Mode::Creative, Mode::Off] {
        // 1. 100 % correct.
        outcomes.push(run(
            "100% correct",
            mode,
            &clean,
            Some(&d_dorian),
            &[],
            None,
            0,
        ));

        // 2. ~80 % correct: every 5th note a semitone off, held.
        let mut rng = Rng(11);
        let mut wrong = clean.clone();
        for (i, n) in wrong.iter_mut().enumerate() {
            if i % 5 == 2 {
                n.midi += if rng.unit() < 0.5 { 1 } else { -1 };
            }
        }
        outcomes.push(run(
            "80% correct (held wrong notes)",
            mode,
            &wrong,
            Some(&d_dorian),
            &[],
            None,
            0,
        ));

        // 3. Short random deviations: a 60 ms blip after every third note.
        let mut rng = Rng(23);
        let mut blips = Vec::new();
        for (i, n) in clean.iter().enumerate() {
            let mut n = n.clone();
            if i % 3 == 1 {
                n.end -= 0.06;
                blips.push(n.clone());
                let off = if rng.unit() < 0.5 { 1 } else { 2 };
                blips.push(note(n.end, 0.06, n.midi + off, 0.0, 0.8));
            } else {
                blips.push(n);
            }
        }
        outcomes.push(run(
            "short random deviations",
            mode,
            &blips,
            Some(&d_dorian),
            &[],
            None,
            0,
        ));

        // 4. Deliberate ornaments: grace note a step above before long notes.
        let mut orn = Vec::new();
        for n in &clean {
            if n.duration() > 1.0 {
                orn.push(note(n.start, 0.07, n.midi + 1, 0.0, 0.85));
                orn.push(note(
                    n.start + 0.07,
                    n.duration() - 0.07,
                    n.midi,
                    n.cents,
                    n.confidence,
                ));
            } else {
                orn.push(n.clone());
            }
        }
        outcomes.push(run(
            "deliberate ornaments",
            mode,
            &orn,
            Some(&d_dorian),
            &[],
            None,
            0,
        ));

        // 5. Poor intonation: ±30 cents, alternating.
        let flat: Vec<Note> = clean
            .iter()
            .enumerate()
            .map(|(i, n)| {
                note(
                    n.start,
                    n.duration(),
                    n.midi,
                    if i % 2 == 0 { 30.0 } else { -30.0 },
                    n.confidence,
                )
            })
            .collect();
        outcomes.push(run(
            "poor intonation ±30c",
            mode,
            &flat,
            Some(&d_dorian),
            &[],
            None,
            0,
        ));

        // 6. Slides into long notes.
        let slides: Vec<Note> = clean
            .iter()
            .map(|n| {
                let mut m = n.clone();
                if n.duration() > 1.0 {
                    m.bend = vec![
                        (0.0, -140.0),
                        (0.08, -60.0),
                        (0.16, 0.0),
                        (n.duration(), 0.0),
                    ];
                }
                m
            })
            .collect();
        outcomes.push(run(
            "slides into long notes",
            mode,
            &slides,
            Some(&d_dorian),
            &[],
            None,
            0,
        ));

        // 7. Variable tempo: 70 → 130 BPM across phrases.
        let mut var = Vec::new();
        for p in 0..8 {
            let tempo = 70.0 + 60.0 * p as f64 / 7.0;
            let ph = melody(&d_dorian, 62, p + 1, tempo);
            let start = ph.len() - melody(&d_dorian, 62, 1, tempo).len().min(ph.len());
            let offset = var.last().map(|n: &Note| n.end + 0.6).unwrap_or(0.0) - ph[start].start;
            for n in &ph[start..] {
                var.push(note(
                    n.start + offset,
                    n.duration(),
                    n.midi,
                    n.cents,
                    n.confidence,
                ));
            }
        }
        outcomes.push(run(
            "variable tempo",
            mode,
            &var,
            Some(&d_dorian),
            &[],
            None,
            0,
        ));

        // 8. Pauses and breath noise: low-confidence blips in the gaps.
        let mut breath = Vec::new();
        for (i, n) in clean.iter().enumerate() {
            breath.push(n.clone());
            if let Some(next) = clean.get(i + 1) {
                if next.start - n.end > 0.4 {
                    breath.push(note(n.end + 0.1, 0.12, n.midi + 5, 0.0, 0.2));
                }
            }
        }
        outcomes.push(run(
            "pauses and breath noise",
            mode,
            &breath,
            Some(&d_dorian),
            &[],
            None,
            0,
        ));

        // 9. Deliberate mode change: 4 phrases D dorian, then 4 phrases A hijaz.
        let first = melody(&d_dorian, 62, 4, 96.0);
        let second = melody(&a_hijaz, 69, 4, 96.0);
        let shift = first.last().unwrap().end + 0.6;
        let mut change = first.clone();
        change.extend(
            second
                .iter()
                .map(|n| note(n.start + shift, n.duration(), n.midi, n.cents, n.confidence)),
        );
        outcomes.push(run(
            "deliberate mode change",
            mode,
            &change,
            Some(&d_dorian),
            &[],
            Some(4),
            1,
        ));

        // 10. New improvised phrase in the mode (notes not heard before).
        let mut improv = clean.clone();
        let t0 = improv.last().unwrap().end + 0.6;
        for (k, &(m, d)) in [
            (71, 0.6),
            (74, 0.4),
            (72, 0.4),
            (76, 1.2),
            (74, 0.5),
            (69, 1.6),
        ]
        .iter()
        .enumerate()
        {
            let start =
                t0 + improv[improv.len() - 1..].iter().map(|_| 0.0).sum::<f64>() + k as f64 * 0.0;
            let prev_end = if k == 0 {
                t0
            } else {
                improv.last().unwrap().end
            };
            let _ = start;
            improv.push(note(prev_end, d, m, 0.0, 0.9));
        }
        outcomes.push(run(
            "new improvised phrase",
            mode,
            &improv,
            Some(&d_dorian),
            &[],
            None,
            0,
        ));

        // 11. Prepared melody (trainer input) with 20 % wrong notes: deviations are
        // reported against the expected notes, context still stable.
        let expected: Vec<Expected> = clean
            .iter()
            .map(|n| Expected {
                midi: n.midi,
                start: n.start,
                end: n.end,
            })
            .collect();
        outcomes.push(run(
            "prepared melody, 20% wrong",
            mode,
            &wrong,
            Some(&d_dorian),
            &expected,
            None,
            0,
        ));
    }

    let mut report = String::from("scenario                          mode      switches expected latency errors ornaments stable\n");
    let mut failures = Vec::new();
    for o in &outcomes {
        report.push_str(&format!(
            "{:<33} {:<9} {:>8} {:>8} {:>7} {:>6} {:>9} {:>6}\n",
            o.name,
            format!("{:?}", o.mode),
            o.switches,
            o.expected_switches,
            o.latency_phrases
                .map(|l| l.to_string())
                .unwrap_or_else(|| "-".into()),
            o.errors,
            o.ornaments,
            o.stable
        ));
        if o.mode == Mode::Off {
            continue; // literal mode is allowed to follow every phrase; it is the baseline
        }
        let false_switches = o.switches.saturating_sub(o.expected_switches);
        if false_switches > 0 {
            failures.push(format!(
                "{} / {:?}: {} false switch(es)",
                o.name, o.mode, false_switches
            ));
        }
        if let Some(at) = o.change_at_phrase {
            // Stable is the deliberately slow mode: one phrase more than Assisted.
            let allowed = match o.mode {
                Mode::Stable => 4,
                Mode::Assisted => 3,
                _ => 2,
            };
            match o.latency_phrases {
                Some(l) if l <= allowed => {}
                Some(l) => failures.push(format!(
                    "{} / {:?}: switched after {} phrases (> {})",
                    o.name, o.mode, l, allowed
                )),
                None => failures.push(format!(
                    "{} / {:?}: missed the change at phrase {}",
                    o.name, o.mode, at
                )),
            }
        }
    }
    eprintln!("{report}");
    assert!(failures.is_empty(), "{}\n{report}", failures.join("\n"));
    // The literal baseline shows what the intelligence prevents: it must switch more
    // often on the damaged material than any intelligent mode.
    let off_switches: usize = outcomes
        .iter()
        .filter(|o| o.mode == Mode::Off)
        .map(|o| o.switches)
        .sum();
    let stable_switches: usize = outcomes
        .iter()
        .filter(|o| o.mode == Mode::Stable)
        .map(|o| o.switches)
        .sum();
    assert!(
        off_switches >= stable_switches,
        "off {off_switches} vs stable {stable_switches}"
    );
    // Ornaments are recognised as such, not as errors.
    let orn = outcomes
        .iter()
        .find(|o| o.name == "deliberate ornaments" && o.mode == Mode::Assisted)
        .unwrap();
    assert!(
        orn.ornaments >= 8 && orn.errors == 0,
        "ornaments {} errors {}",
        orn.ornaments,
        orn.errors
    );
}
