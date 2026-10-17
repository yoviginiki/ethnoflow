//! Groove engine (F4.1): drum patterns in beats for a liquid DnB two-step, halftime
//! and an "ethno" template that follows odd-meter groupings (7/8 = 2+2+3 …).
//! Complexity adds ghost notes and hat variation, humanize moves timing and velocity
//! from the seed, fills land every 4 or 8 bars. All drums are synthesized by the
//! engine; nothing here is a sample.

use serde::{Deserialize, Serialize};

use crate::pattern::{swing_offset, Meter, Rng};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Drum {
    Kick,
    Snare,
    /// Quiet snare between the backbeats.
    Ghost,
    HatClosed,
    HatOpen,
    Shaker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrooveStyle {
    /// Classic two-step: kick 1 and the "and" of 2 late, snares on 2 and 4.
    Liquid,
    /// Snare on 3 only, half the felt tempo.
    Halftime,
    /// Accents on the meter's groups (odd meters); works in 4/4 too.
    Ethno,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrooveParams {
    pub style: GrooveStyle,
    /// 0..1: ghost notes, hat variation, extra kicks.
    pub complexity: f64,
    /// 0..1: timing jitter (up to ±12 ms at 170 BPM ≈ ±0.034 beats) and velocity spread.
    pub humanize: f64,
    /// 0..1 swing on sixteenths.
    pub swing: f64,
    /// A fill in the last beat of every N-th bar (0 = never).
    pub fill_every_bars: u32,
    pub seed: u32,
    /// Bars to generate before the pattern repeats.
    pub bars: u32,
}

impl Default for GrooveParams {
    fn default() -> Self {
        Self {
            style: GrooveStyle::Liquid,
            complexity: 0.4,
            humanize: 0.3,
            swing: 0.0,
            fill_every_bars: 4,
            seed: 1,
            bars: 8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumHit {
    pub drum: Drum,
    pub start_beat: f64,
    /// 0..1
    pub velocity: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DrumPattern {
    pub bars: u32,
    pub bar_beats: f64,
    pub hits: Vec<DrumHit>,
}

impl DrumPattern {
    pub fn length_beats(&self) -> f64 {
        self.bars as f64 * self.bar_beats
    }
}

/// Maximum humanize jitter, in beats (≈ 12 ms at 170 BPM).
const JITTER_BEATS: f64 = 0.034;

fn bar_template(style: GrooveStyle, m: &Meter, bar: u32, c: f64, rng: &mut Rng) -> Vec<DrumHit> {
    let mut hits = Vec::new();
    let push = |hits: &mut Vec<DrumHit>, drum: Drum, at: f64, vel: f64| {
        hits.push(DrumHit {
            drum,
            start_beat: at,
            velocity: vel.clamp(0.05, 1.0),
        })
    };
    let bar_beats = m.bar_beats();
    let sixteenth = 0.25;
    if m.is_four_four_like() && style != GrooveStyle::Ethno {
        // Two-step backbone.
        push(&mut hits, Drum::Kick, 0.0, 1.0);
        match style {
            GrooveStyle::Liquid => {
                push(&mut hits, Drum::Snare, 1.0, 0.95);
                push(&mut hits, Drum::Snare, 3.0, 1.0);
                // The classic late kick: "and" of 2, pushed to the 11th sixteenth.
                push(&mut hits, Drum::Kick, 2.5, 0.85);
                if c > 0.5 && rng.chance((c - 0.5) * 1.6) {
                    push(&mut hits, Drum::Kick, 3.75, 0.6);
                }
            }
            _ => {
                // Halftime: snare on 3, a softer kick leading into it.
                push(&mut hits, Drum::Snare, 2.0, 1.0);
                if rng.chance(0.4 + c * 0.5) {
                    push(&mut hits, Drum::Kick, 1.5, 0.7);
                }
            }
        }
        // Hats: eighths, sixteenths as complexity grows; open hat on the last offbeat.
        let steps = if c > 0.6 { 16 } else { 8 };
        for k in 0..steps {
            let at = k as f64 * bar_beats / steps as f64;
            let on_beat = (at - at.round()).abs() < 1e-9;
            let vel = if on_beat { 0.6 } else { 0.38 } + rng.bipolar() * 0.05 * c;
            if k == steps - 1 && rng.chance(0.3 + c * 0.5) {
                push(&mut hits, Drum::HatOpen, at, 0.5);
            } else {
                push(&mut hits, Drum::HatClosed, at, vel);
            }
        }
        // Ghost snares between the backbeats.
        for at in [0.75, 1.75, 2.25, 3.5] {
            if rng.chance(c * 0.55) {
                push(&mut hits, Drum::Ghost, at, 0.25 + rng.unit() * 0.15);
            }
        }
        // Shaker sixteenths, accenting the offbeats, from medium complexity.
        if c > 0.3 {
            for k in 0..16 {
                let at = k as f64 * sixteenth;
                let vel = if k % 4 == 2 { 0.5 } else { 0.25 };
                push(&mut hits, Drum::Shaker, at, vel + rng.bipolar() * 0.05);
            }
        }
    } else {
        // Ethno / odd meters: kick on the first group, snare on the last group start,
        // kicks on the other group starts with complexity, hats on every unit.
        let groups = m.group_starts();
        let unit = m.unit_beats();
        for (i, &g) in groups.iter().enumerate() {
            if i == 0 {
                push(&mut hits, Drum::Kick, g, 1.0);
            } else if i == groups.len() - 1 && groups.len() > 1 {
                push(&mut hits, Drum::Snare, g, 0.95);
            } else if rng.chance(0.35 + c * 0.5) {
                push(&mut hits, Drum::Kick, g, 0.75);
            } else if rng.chance(c * 0.5) {
                push(&mut hits, Drum::Ghost, g, 0.3);
            }
        }
        let units = m.numerator;
        for k in 0..units {
            let at = k as f64 * unit;
            let accent = groups.iter().any(|&g| (g - at).abs() < 1e-9);
            push(
                &mut hits,
                Drum::HatClosed,
                at,
                if accent { 0.6 } else { 0.35 } + rng.bipolar() * 0.05 * c,
            );
            if c > 0.5 && rng.chance((c - 0.5) * 1.2) {
                push(&mut hits, Drum::Shaker, at + unit * 0.5, 0.3);
            }
        }
        if bar % 2 == 1 && rng.chance(c * 0.6) {
            push(&mut hits, Drum::Ghost, bar_beats - unit * 0.5, 0.3);
        }
    }
    hits
}

/// Replace the last beat of the bar with a snare roll that builds into the next bar.
fn fill(bar_beats: f64, c: f64, rng: &mut Rng) -> Vec<DrumHit> {
    let steps = if c > 0.5 { 4 } else { 2 };
    let start = bar_beats - 1.0;
    (0..steps)
        .map(|k| DrumHit {
            drum: if k == steps - 1 && rng.chance(0.5) {
                Drum::Kick
            } else {
                Drum::Snare
            },
            start_beat: start + k as f64 / steps as f64,
            velocity: 0.5 + 0.5 * (k + 1) as f64 / steps as f64,
        })
        .collect()
}

/// Generate `params.bars` bars for the meter. Deterministic per seed.
pub fn generate_groove(params: &GrooveParams, meter: &Meter) -> DrumPattern {
    let mut rng = Rng::new(params.seed);
    let c = params.complexity.clamp(0.0, 1.0);
    let h = params.humanize.clamp(0.0, 1.0);
    let bars = params.bars.clamp(1, 64);
    let bar_beats = meter.bar_beats();
    let mut hits = Vec::new();
    for bar in 0..bars {
        let mut bar_hits = bar_template(params.style, meter, bar, c, &mut rng);
        let is_fill_bar = params.fill_every_bars > 0
            && (bar + 1) % params.fill_every_bars == 0
            && bar_beats >= 2.0;
        if is_fill_bar {
            // Drop what sits in the last beat (except the hats) and put the roll there.
            bar_hits.retain(|x| {
                x.start_beat < bar_beats - 1.0 || matches!(x.drum, Drum::HatClosed | Drum::Shaker)
            });
            bar_hits.extend(fill(bar_beats, c, &mut rng));
        }
        let offset = bar as f64 * bar_beats;
        for mut x in bar_hits {
            let swung = swing_offset(x.start_beat, 0.25, params.swing);
            let jitter = if x.drum == Drum::Kick && x.start_beat == 0.0 {
                0.0 // the downbeat stays exactly on the grid
            } else {
                rng.bipolar() * JITTER_BEATS * h
            };
            x.start_beat = (offset + x.start_beat + swung + jitter).max(offset);
            x.velocity = (x.velocity * (1.0 + rng.bipolar() * 0.15 * h)).clamp(0.05, 1.0);
            hits.push(x);
        }
    }
    hits.sort_by(|a, b| a.start_beat.partial_cmp(&b.start_beat).unwrap());
    DrumPattern {
        bars,
        bar_beats,
        hits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(style: GrooveStyle, seed: u32) -> GrooveParams {
        GrooveParams {
            style,
            seed,
            ..Default::default()
        }
    }

    #[test]
    fn seed_reproduces_and_changes_the_groove_and_the_backbone_holds() {
        let m = Meter::new(4, 4);
        let a = generate_groove(&p(GrooveStyle::Liquid, 3), &m);
        let b = generate_groove(&p(GrooveStyle::Liquid, 3), &m);
        let c = generate_groove(&p(GrooveStyle::Liquid, 4), &m);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.length_beats(), 32.0);
        for bar in 0..8 {
            let off = bar as f64 * 4.0;
            assert!(
                a.hits
                    .iter()
                    .any(|h| h.drum == Drum::Kick && h.start_beat == off),
                "kick on bar {bar}"
            );
            // Snares on 2 and 4 (±jitter) in non-fill bars.
            if (bar + 1) % 4 != 0 {
                for beat in [1.0, 3.0] {
                    assert!(a.hits.iter().any(
                        |h| h.drum == Drum::Snare && (h.start_beat - (off + beat)).abs() < 0.05
                    ));
                }
            }
        }
        // Fills at bars 4 and 8: a snare roll in the last beat.
        for bar in [3u32, 7] {
            let off = bar as f64 * 4.0 + 3.0;
            let roll = a
                .hits
                .iter()
                .filter(|h| {
                    h.start_beat >= off - 0.05 && matches!(h.drum, Drum::Snare | Drum::Kick)
                })
                .count();
            assert!(roll >= 2, "fill in bar {}: {roll}", bar + 1);
        }
        assert!(a.hits.iter().all(|h| (0.05..=1.0).contains(&h.velocity)));
        assert!(a
            .hits
            .windows(2)
            .all(|w| w[0].start_beat <= w[1].start_beat));
    }

    #[test]
    fn halftime_and_odd_meters_follow_their_accents() {
        let m = Meter::new(4, 4);
        let h = generate_groove(&p(GrooveStyle::Halftime, 1), &m);
        assert!(h
            .hits
            .iter()
            .any(|x| x.drum == Drum::Snare && (x.start_beat - 2.0).abs() < 0.05));
        assert!(!h
            .hits
            .iter()
            .any(|x| x.drum == Drum::Snare && (x.start_beat - 1.0).abs() < 0.05));
        let seven = Meter::new(7, 8).with_grouping(vec![2, 2, 3]);
        let e = generate_groove(&p(GrooveStyle::Ethno, 1), &seven);
        assert_eq!(e.bar_beats, 3.5);
        // Kick on 1, snare on the last group (beat 2.0 = the 5th eighth).
        assert!(e
            .hits
            .iter()
            .any(|x| x.drum == Drum::Kick && x.start_beat == 0.0));
        assert!(e
            .hits
            .iter()
            .any(|x| x.drum == Drum::Snare && (x.start_beat - 2.0).abs() < 0.05));
        assert!(e.hits.iter().all(|x| x.start_beat < e.length_beats()));
        // Complexity adds hits.
        let sparse = generate_groove(
            &GrooveParams {
                complexity: 0.0,
                humanize: 0.0,
                ..p(GrooveStyle::Liquid, 2)
            },
            &m,
        );
        let dense = generate_groove(
            &GrooveParams {
                complexity: 1.0,
                humanize: 0.0,
                ..p(GrooveStyle::Liquid, 2)
            },
            &m,
        );
        assert!(dense.hits.len() > sparse.hits.len() * 3 / 2);
        // Humanize moves things but never the downbeat kick.
        let human = generate_groove(
            &GrooveParams {
                humanize: 1.0,
                ..p(GrooveStyle::Liquid, 2)
            },
            &m,
        );
        assert!(human
            .hits
            .iter()
            .any(|x| x.drum == Drum::Kick && x.start_beat == 4.0));
        assert!(human.hits.iter().any(|x| x.drum == Drum::HatClosed
            && (x.start_beat - x.start_beat.round()).abs() > 0.001
            && (x.start_beat - x.start_beat.round()).abs() < 0.04));
    }
}
