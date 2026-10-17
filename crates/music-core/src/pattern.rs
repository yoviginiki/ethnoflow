//! Shared building blocks for the Phase 4 generators (groove, bass, ostinato,
//! arpeggio, countermelody): a seeded RNG, beat-based note events and the meter.
//! Everything here is pure; time is in beats so the engine can play the same pattern
//! at any tempo, and the same seed always gives the same pattern.

use serde::{Deserialize, Serialize};

/// xorshift32 (the same family the engine uses), deterministic per seed.
#[derive(Debug, Clone, Copy)]
pub struct Rng(u32);

impl Rng {
    pub fn new(seed: u32) -> Self {
        let mut x = seed ^ 0x9E37_79B9;
        if x == 0 {
            x = 0x2545_F491;
        }
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        Self(x)
    }
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
    /// Uniform in [0, 1).
    #[inline]
    pub fn unit(&mut self) -> f64 {
        (self.next_u32() >> 8) as f64 / 16_777_216.0
    }
    #[inline]
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }
    #[inline]
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            ((self.unit() * n as f64) as usize).min(n - 1)
        }
    }
    /// Uniform in [−1, 1).
    #[inline]
    pub fn bipolar(&mut self) -> f64 {
        self.unit() * 2.0 - 1.0
    }
}

/// Meter as the generators need it: beats per bar in quarter-note units and the
/// grouping of the bar (e.g. 7/8 = [2, 2, 3] eighths → group starts in beats).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Meter {
    pub numerator: u32,
    pub unit: u32,
    /// Groups in `unit` notes; empty = plain.
    pub grouping: Vec<u32>,
}

impl Meter {
    pub fn new(numerator: u32, unit: u32) -> Self {
        Self {
            numerator: numerator.max(1),
            unit: unit.max(1),
            grouping: Vec::new(),
        }
    }
    pub fn with_grouping(mut self, grouping: Vec<u32>) -> Self {
        if !grouping.is_empty() && grouping.iter().sum::<u32>() == self.numerator {
            self.grouping = grouping;
        }
        self
    }
    /// Length of one `unit` note in quarter-note beats.
    pub fn unit_beats(&self) -> f64 {
        4.0 / self.unit as f64
    }
    /// Bar length in quarter-note beats.
    pub fn bar_beats(&self) -> f64 {
        self.numerator as f64 * self.unit_beats()
    }
    /// Where the accent groups start inside the bar, in beats (always includes 0).
    pub fn group_starts(&self) -> Vec<f64> {
        if self.grouping.is_empty() {
            // Plain meters: an accent on every unit (beats in 4/4, eighths in 7/8).
            return (0..self.numerator)
                .map(|k| k as f64 * self.unit_beats())
                .collect();
        }
        let mut at = 0.0;
        let mut out = Vec::new();
        for g in &self.grouping {
            out.push(at);
            at += *g as f64 * self.unit_beats();
        }
        out
    }
    /// Is the bar a simple duple/quadruple meter where DnB templates apply?
    pub fn is_four_four_like(&self) -> bool {
        self.grouping.is_empty() && self.unit == 4 && (self.numerator == 4 || self.numerator == 2)
    }
}

/// A pitched note in beats (quarter notes from the pattern start).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BeatNote {
    pub start_beat: f64,
    pub length_beats: f64,
    pub midi: i32,
    /// Cents from the tempered note (scale degrees carry their own intonation).
    pub cents: f64,
    /// 0..1
    pub velocity: f64,
    /// Portamento from the previous note (bass).
    #[serde(default)]
    pub glide: bool,
}

/// Swing: delay every second subdivision by `amount` × half the subdivision.
pub fn swing_offset(start_beat: f64, subdivision_beats: f64, amount: f64) -> f64 {
    if amount <= 0.0 || subdivision_beats <= 0.0 {
        return 0.0;
    }
    let idx = (start_beat / subdivision_beats).round() as i64;
    if idx.rem_euclid(2) == 1 {
        amount.clamp(0.0, 1.0) * subdivision_beats * 0.5
    } else {
        0.0
    }
}

/// Chord or scale tones laid out inside a MIDI register, lowest first.
pub fn tones_in_register(pitch_classes: &[(u8, f64)], lo: i32, hi: i32) -> Vec<(i32, f64)> {
    let (lo, hi) = (lo.min(hi), hi.max(lo));
    let mut out = Vec::new();
    for midi in lo..=hi {
        for &(pc, cents) in pitch_classes {
            if (midi.rem_euclid(12)) as u8 == pc % 12 {
                out.push((midi, cents));
            }
        }
    }
    out.sort_by_key(|t| t.0);
    out.dedup_by(|a, b| a.0 == b.0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meter_groups_and_register_layout() {
        let m = Meter::new(7, 8).with_grouping(vec![2, 2, 3]);
        assert_eq!(m.bar_beats(), 3.5);
        assert_eq!(m.group_starts(), vec![0.0, 1.0, 2.0]);
        assert!(!m.is_four_four_like());
        assert!(Meter::new(4, 4).is_four_four_like());
        assert_eq!(Meter::new(4, 4).group_starts(), vec![0.0, 1.0, 2.0, 3.0]);
        // A bad grouping is ignored, not trusted.
        assert!(Meter::new(7, 8)
            .with_grouping(vec![2, 2])
            .grouping
            .is_empty());
        let t = tones_in_register(&[(2, 0.0), (9, -15.0)], 50, 62);
        assert_eq!(t, vec![(50, 0.0), (57, -15.0), (62, 0.0)]);
        assert_eq!(swing_offset(0.5, 0.5, 0.5), 0.125);
        assert_eq!(swing_offset(1.0, 0.5, 0.5), 0.0);
        let mut r = Rng::new(1);
        let mut s = Rng::new(1);
        assert_eq!(r.next_u32(), s.next_u32());
    }
}
