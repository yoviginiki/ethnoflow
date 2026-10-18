//! Pure generators for the Instrument Rack's new parts (P1 R3): tapan strokes on the
//! meter's accent groups, chord events for the chord pad, and lead phrases over the
//! chord / mode tones. Deterministic per seed; nothing here knows about audio.

use serde::{Deserialize, Serialize};

use crate::pattern::{BeatNote, Meter};

/// Small deterministic generator (xorshift), the same family the other generators use.
struct Rng(u32);
impl Rng {
    fn new(seed: u32) -> Self {
        Self(seed.max(1) ^ 0x9E37_79B9)
    }
    fn next(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 8) as f64 / (1u32 << 24) as f64
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() as usize) % n
        }
    }
}

// ---------- tapan ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TapanStroke {
    Boom,
    Rim,
    Ghost,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TapanHit {
    pub start_beat: f64,
    pub stroke: TapanStroke,
    /// 0..1
    pub velocity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TapanParams {
    /// 0..1 share of the off-positions that get a rim or ghost stroke.
    pub density: f64,
    /// 0..1 random velocity spread.
    pub humanize: f64,
    /// A fill (rim roll in the last group) every N bars; 0 = never.
    pub fill_every_bars: u32,
    pub seed: u32,
    pub bars: u32,
}

/// Tapan pattern: a boom on every accent-group start (the first in the bar strongest,
/// the odd 3-group of an additive meter slightly stronger), rims on the off-units
/// between booms, ghosts filling the rest according to `density`, a rim roll as a fill.
pub fn generate_tapan(meter: &Meter, params: &TapanParams) -> Vec<TapanHit> {
    let bars = params.bars.clamp(1, 64) as usize;
    let bar = meter.bar_beats();
    let unit = meter.unit_beats();
    let starts = meter.group_starts();
    let mut rng = Rng::new(params.seed);
    let hum = params.humanize.clamp(0.0, 1.0);
    let density = params.density.clamp(0.0, 1.0);
    let mut out = Vec::new();
    let jitter =
        |rng: &mut Rng, base: f64| (base * (1.0 - hum * 0.35 * rng.unit())).clamp(0.05, 1.0);
    for b in 0..bars {
        let off = b as f64 * bar;
        let fill = params.fill_every_bars > 0 && (b + 1) % params.fill_every_bars as usize == 0;
        // Group lengths in units, to tell the long (3) groups from the short (2) ones.
        let lengths: Vec<f64> = starts
            .iter()
            .enumerate()
            .map(|(i, s)| starts.get(i + 1).copied().unwrap_or(bar) - s)
            .collect();
        for (gi, &gs) in starts.iter().enumerate() {
            let long = lengths[gi] > unit * 2.0 + 1e-9;
            let base = if gi == 0 {
                1.0
            } else if long {
                0.85
            } else {
                0.72
            };
            out.push(TapanHit {
                start_beat: off + gs,
                stroke: TapanStroke::Boom,
                velocity: jitter(&mut rng, base),
            });
            // Off-units inside the group: rim on the last unit of a group, ghosts elsewhere.
            let units = (lengths[gi] / unit).round().max(1.0) as usize;
            let is_last_group = gi + 1 == starts.len();
            for u in 1..units {
                let t = off + gs + u as f64 * unit;
                let last_unit = u + 1 == units;
                if fill && is_last_group {
                    // Fill: rim roll in sixteenths of the unit over the last group.
                    for k in 0..2 {
                        out.push(TapanHit {
                            start_beat: t + k as f64 * unit * 0.5,
                            stroke: TapanStroke::Rim,
                            velocity: jitter(&mut rng, 0.55 + 0.2 * k as f64),
                        });
                    }
                    continue;
                }
                if last_unit {
                    out.push(TapanHit {
                        start_beat: t,
                        stroke: TapanStroke::Rim,
                        velocity: jitter(&mut rng, 0.7),
                    });
                } else if rng.unit() < density {
                    out.push(TapanHit {
                        start_beat: t,
                        stroke: TapanStroke::Ghost,
                        velocity: jitter(&mut rng, 0.5),
                    });
                }
                // Between units at higher density: a ghost on the half-unit.
                if density > 0.6 && rng.unit() < (density - 0.6) * 2.0 {
                    out.push(TapanHit {
                        start_beat: t - unit * 0.5,
                        stroke: TapanStroke::Ghost,
                        velocity: jitter(&mut rng, 0.35),
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.start_beat.partial_cmp(&b.start_beat).unwrap());
    out
}

// ---------- chord pad ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordPadParams {
    /// Bars per chord event (1 = re-trigger every bar, 2 = every two bars …).
    pub bars_per_chord: u32,
    /// 0..1 gap before the next chord (0 = seamless, 0.5 = half a bar of release).
    pub gap: f64,
    /// Register in MIDI.
    pub register_low: i32,
    pub register_high: i32,
    pub bars: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChordStep {
    pub start_beat: f64,
    pub length_beats: f64,
    /// (midi, cents) per voice, low to high.
    pub tones: Vec<(i32, f64)>,
    pub velocity: f64,
}

/// One chord event per `bars_per_chord` bars: the chord's pitch classes voiced inside
/// the register, at most six tones, lowest first (a 7th / 9th stays on top).
pub fn generate_chord_pad(
    pcs: &[(u8, f64)],
    params: &ChordPadParams,
    meter: &Meter,
) -> Vec<ChordStep> {
    if pcs.is_empty() {
        return Vec::new();
    }
    let bar = meter.bar_beats();
    let bars = params.bars.clamp(1, 64) as f64;
    let per = params.bars_per_chord.clamp(1, 16) as f64;
    let mut tones =
        crate::pattern::tones_in_register(pcs, params.register_low, params.register_high);
    tones.truncate(6);
    let mut out = Vec::new();
    let mut t = 0.0;
    while t < bars * bar - 1e-9 {
        let len = (per * bar) * (1.0 - params.gap.clamp(0.0, 0.9));
        out.push(ChordStep {
            start_beat: t,
            length_beats: len.max(0.25),
            tones: tones.clone(),
            velocity: 0.8,
        });
        t += per * bar;
    }
    out
}

// ---------- lead ----------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LeadParams {
    /// 0..1 share of steps that sound.
    pub density: f64,
    /// Step length in beats (0.5 = eighths).
    pub step_beats: f64,
    /// Phrase length in bars; the phrase repeats with a seeded variation.
    pub phrase_bars: u32,
    /// 0..1 how often a step leaps instead of moving stepwise.
    pub leaps: f64,
    pub register_low: i32,
    pub register_high: i32,
    pub seed: u32,
    pub bars: u32,
}

/// A lead phrase over the given tones: mostly stepwise motion with occasional leaps,
/// long notes on the meter's accent-group starts, a rest at the end of each phrase,
/// legato pairs now and then. Monophonic: notes never overlap.
pub fn generate_lead(tones: &[(i32, f64)], params: &LeadParams, meter: &Meter) -> Vec<BeatNote> {
    if tones.is_empty() {
        return Vec::new();
    }
    let bar = meter.bar_beats();
    let step = params.step_beats.clamp(0.125, 2.0);
    let bars = params.bars.clamp(1, 64);
    let phrase_bars = params.phrase_bars.clamp(1, 8).min(bars);
    let steps_per_phrase = ((phrase_bars as f64 * bar) / step).round().max(1.0) as usize;
    let starts = meter.group_starts();
    let density = params.density.clamp(0.0, 1.0);
    let leaps = params.leaps.clamp(0.0, 1.0);
    let mut rng = Rng::new(params.seed);
    // Build one phrase as (step index → Option<(tone index, length in steps, velocity)>).
    let mut idx = tones.len() / 2;
    let mut phrase: Vec<Option<(usize, usize, f64)>> = vec![None; steps_per_phrase];
    let mut i = 0;
    while i < steps_per_phrase {
        let in_bar = (i as f64 * step) % bar;
        let on_group = starts.iter().any(|g| (g - in_bar).abs() < 1e-6);
        let last_steps = steps_per_phrase.saturating_sub(i) <= (bar / step).round() as usize / 2;
        if last_steps && !on_group {
            // Rest at the phrase end so it breathes.
            i += 1;
            continue;
        }
        let sounds = on_group || rng.unit() < density;
        if !sounds {
            i += 1;
            continue;
        }
        // Contour: stepwise by default, leap sometimes, pull back towards the middle.
        let dir: i32 = if rng.unit() < 0.5 { -1 } else { 1 };
        let span = if rng.unit() < leaps {
            2 + rng.below(3) as i32
        } else {
            1
        };
        let mid = tones.len() as i32 / 2;
        let pull = if (idx as i32 - mid).abs() > tones.len() as i32 / 3 {
            -(idx as i32 - mid).signum()
        } else {
            dir
        };
        let next = (idx as i32 + pull * span).clamp(0, tones.len() as i32 - 1) as usize;
        idx = next;
        let len = if on_group { 2 + rng.below(2) } else { 1 };
        let len = len.min(steps_per_phrase - i).max(1);
        let vel = if on_group {
            0.9
        } else {
            0.6 + 0.2 * rng.unit()
        };
        phrase[i] = Some((idx, len, vel));
        i += len;
    }
    let mut out = Vec::new();
    let mut p = 0usize;
    for b in (0..bars).step_by(phrase_bars as usize) {
        // Variation on repeats: every second repeat moves one note up or down a tone.
        let vary = p > 0 && p % 2 == 1;
        let vary_at = rng.below(steps_per_phrase);
        for (si, cell) in phrase.iter().enumerate() {
            let Some((ti, len, vel)) = *cell else {
                continue;
            };
            let start = b as f64 * bar + si as f64 * step;
            if start >= bars as f64 * bar - 1e-9 {
                break;
            }
            let mut ti = ti;
            if vary && si == vary_at {
                ti = (ti + 1).min(tones.len() - 1);
            }
            let (midi, cents) = tones[ti];
            let length = (len as f64 * step * 0.92).min(bars as f64 * bar - start);
            out.push(BeatNote {
                start_beat: start,
                length_beats: length,
                midi,
                cents,
                velocity: vel,
                glide: len == 1 && rng.unit() < 0.25,
            });
        }
        p += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tapan_booms_on_group_starts_and_density_adds_strokes() {
        let meter = Meter {
            numerator: 7,
            unit: 8,
            grouping: vec![2, 2, 3],
        };
        let sparse = generate_tapan(
            &meter,
            &TapanParams {
                density: 0.0,
                humanize: 0.0,
                fill_every_bars: 0,
                seed: 3,
                bars: 2,
            },
        );
        let bar = meter.bar_beats();
        for b in 0..2 {
            for g in meter.group_starts() {
                let t = b as f64 * bar + g;
                assert!(
                    sparse
                        .iter()
                        .any(|h| h.stroke == TapanStroke::Boom && (h.start_beat - t).abs() < 1e-9),
                    "boom at {t}"
                );
            }
        }
        assert!(sparse
            .iter()
            .all(|h| h.start_beat >= 0.0 && h.start_beat < 2.0 * bar));
        // The downbeat is the strongest boom.
        let first = sparse.iter().find(|h| h.start_beat == 0.0).unwrap();
        assert!(sparse.iter().all(|h| h.velocity <= first.velocity + 1e-9));
        let dense = generate_tapan(
            &meter,
            &TapanParams {
                density: 1.0,
                humanize: 0.0,
                fill_every_bars: 0,
                seed: 3,
                bars: 2,
            },
        );
        assert!(
            dense.len() > sparse.len(),
            "{} vs {}",
            dense.len(),
            sparse.len()
        );
        // Deterministic per seed.
        let again = generate_tapan(
            &meter,
            &TapanParams {
                density: 1.0,
                humanize: 0.0,
                fill_every_bars: 0,
                seed: 3,
                bars: 2,
            },
        );
        assert_eq!(dense, again);
    }

    #[test]
    fn chord_pad_voices_the_chord_in_register_once_per_period() {
        let meter = Meter::new(4, 4);
        let steps = generate_chord_pad(
            &[(2, 0.0), (5, 0.0), (9, 0.0)],
            &ChordPadParams {
                bars_per_chord: 2,
                gap: 0.25,
                register_low: 50,
                register_high: 72,
                bars: 4,
            },
            &meter,
        );
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[1].start_beat, 8.0);
        assert!((steps[0].length_beats - 6.0).abs() < 1e-9);
        assert!(steps[0].tones.len() <= 6 && !steps[0].tones.is_empty());
        assert!(steps[0]
            .tones
            .iter()
            .all(|(m, _)| (50..=72).contains(m) && [2, 5, 9].contains(&(m.rem_euclid(12) as u8))));
    }

    #[test]
    fn lead_is_monophonic_in_the_tones_and_lands_on_group_starts() {
        let meter = Meter {
            numerator: 7,
            unit: 8,
            grouping: vec![2, 2, 3],
        };
        let tones: Vec<(i32, f64)> = (62..=81)
            .filter(|m| [0, 2, 4, 5, 7, 9, 10].contains(&((m - 62) % 12)))
            .map(|m| (m, 0.0))
            .collect();
        let params = LeadParams {
            density: 0.6,
            step_beats: 0.5,
            phrase_bars: 2,
            leaps: 0.2,
            register_low: 62,
            register_high: 81,
            seed: 9,
            bars: 4,
        };
        let notes = generate_lead(&tones, &params, &meter);
        assert!(!notes.is_empty());
        for w in notes.windows(2) {
            assert!(
                w[0].start_beat + w[0].length_beats <= w[1].start_beat + 1e-9,
                "overlap {:?} {:?}",
                w[0],
                w[1]
            );
        }
        assert!(notes
            .iter()
            .all(|n| tones.iter().any(|(m, _)| *m == n.midi)));
        let bar = meter.bar_beats();
        for b in 0..4 {
            assert!(
                notes
                    .iter()
                    .any(|n| (n.start_beat - b as f64 * bar).abs() < 1e-9),
                "note on downbeat of bar {b}"
            );
        }
        // Same seed → same phrase; another seed → a different one.
        assert_eq!(generate_lead(&tones, &params, &meter), notes);
        let other = generate_lead(
            &tones,
            &LeadParams {
                seed: 10,
                ..params.clone()
            },
            &meter,
        );
        assert_ne!(other, notes);
    }
}
