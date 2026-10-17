//! Ostinato (F4.5) and arpeggiator (F4.6): repeating figures on chord or mode tones.
//! The ostinato is a motif (pitch choice + rhythmic mask) that varies every N bars
//! by rotation, adding or removing a hit, or octave displacement; a locked motif
//! survives regeneration with any seed. The arpeggiator walks the chord tones in a
//! direction with a gate, so no note hangs across a chord change.

use serde::{Deserialize, Serialize};

use crate::pattern::{swing_offset, BeatNote, Meter, Rng};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OstinatoParams {
    /// Steps in the motif (8 or 16 are the musical defaults).
    pub steps: u32,
    /// Length of one step in beats (0.5 = eighths, 0.25 = sixteenths).
    pub step_beats: f64,
    /// 0..1 share of steps that sound.
    pub density: f64,
    /// Probability each sounding step actually plays in a given bar (per-step gate).
    pub probability: f64,
    /// New variation every N bars (0 = never).
    pub variation_every_bars: u32,
    pub swing: f64,
    pub seed: u32,
    pub bars: u32,
}

impl Default for OstinatoParams {
    fn default() -> Self {
        Self {
            steps: 8,
            step_beats: 0.5,
            density: 0.6,
            probability: 1.0,
            variation_every_bars: 4,
            swing: 0.0,
            seed: 1,
            bars: 8,
        }
    }
}

/// A motif: for each step, an index into the tone list (None = rest) and an octave
/// offset. Lockable ("locked motifs" keep their shape when everything else changes).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Motif {
    pub cells: Vec<Option<(usize, i32)>>,
}

impl Motif {
    pub fn generate(tones_len: usize, params: &OstinatoParams) -> Motif {
        let mut rng = Rng::new(params.seed);
        let steps = params.steps.clamp(2, 32) as usize;
        let density = params.density.clamp(0.0, 1.0);
        let mut cells = Vec::with_capacity(steps);
        let mut last = 0usize;
        for k in 0..steps {
            // The first step always sounds; the rest by density, favouring the beat.
            let on = k == 0
                || rng.chance(if k % 2 == 0 {
                    density + 0.15
                } else {
                    density - 0.1
                });
            if on && tones_len > 0 {
                // Stepwise motion is more likely than leaps.
                let idx = if rng.chance(0.65) {
                    (last as i32 + if rng.chance(0.5) { 1 } else { -1 })
                        .rem_euclid(tones_len as i32) as usize
                } else {
                    rng.below(tones_len)
                };
                last = idx;
                cells.push(Some((idx, 0)));
            } else {
                cells.push(None);
            }
        }
        Motif { cells }
    }

    /// One variation step: rotate, add a hit, remove a hit or displace an octave.
    pub fn vary(&self, rng: &mut Rng, tones_len: usize) -> Motif {
        let mut cells = self.cells.clone();
        let n = cells.len();
        if n == 0 {
            return self.clone();
        }
        match rng.below(4) {
            0 => cells.rotate_right(1 + rng.below(n.min(3))),
            1 => {
                if let Some(k) = (0..n).map(|_| rng.below(n)).find(|&k| cells[k].is_none()) {
                    cells[k] = Some((rng.below(tones_len.max(1)), 0));
                }
            }
            2 => {
                let hits: Vec<usize> = (0..n).filter(|&k| cells[k].is_some()).collect();
                if hits.len() > 2 {
                    let k = hits[rng.below(hits.len())];
                    if k != 0 {
                        cells[k] = None;
                    }
                }
            }
            _ => {
                let k = rng.below(n);
                if let Some((i, o)) = cells[k] {
                    cells[k] = Some((i, if o == 0 { 1 } else { 0 }));
                }
            }
        }
        Motif { cells }
    }
}

/// Render the ostinato over `params.bars` bars from `tones` (chord or mode tones in
/// a register, lowest first, with cents). `locked` keeps that motif for every bar.
pub fn generate_ostinato(
    tones: &[(i32, f64)],
    params: &OstinatoParams,
    meter: &Meter,
    locked: Option<&Motif>,
) -> Vec<BeatNote> {
    if tones.is_empty() {
        return Vec::new();
    }
    let mut rng = Rng::new(params.seed ^ 0x51ED_270F);
    let base = locked
        .cloned()
        .unwrap_or_else(|| Motif::generate(tones.len(), params));
    let mut motif = base.clone();
    let bar_beats = meter.bar_beats();
    let step = params.step_beats.max(0.0625);
    let steps_per_bar = (bar_beats / step).round().max(1.0) as usize;
    let mut out = Vec::new();
    let top = tones.iter().map(|t| t.0).max().unwrap_or(0);
    for bar in 0..params.bars.clamp(1, 64) {
        if locked.is_none()
            && params.variation_every_bars > 0
            && bar > 0
            && bar % params.variation_every_bars == 0
        {
            motif = if rng.chance(0.3) {
                base.clone() // come home to the original now and then
            } else {
                motif.vary(&mut rng, tones.len())
            };
        }
        let offset = bar as f64 * bar_beats;
        for k in 0..steps_per_bar {
            let cell = motif.cells[k % motif.cells.len().max(1)];
            let Some((idx, oct)) = cell else { continue };
            if !rng.chance(params.probability.clamp(0.0, 1.0)) {
                continue;
            }
            let (midi, cents) = tones[idx.min(tones.len() - 1)];
            let mut m = midi + 12 * oct;
            if m > top + 12 {
                m -= 12;
            }
            let t = k as f64 * step;
            out.push(BeatNote {
                start_beat: offset + t + swing_offset(t, step, params.swing),
                length_beats: step * 0.9,
                midi: m,
                cents,
                velocity: if k == 0 { 0.9 } else { 0.65 + rng.unit() * 0.2 },
                glide: false,
            });
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArpDirection {
    Up,
    Down,
    UpDown,
    Random,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArpParams {
    pub direction: ArpDirection,
    /// 1..3 octaves spanned.
    pub octaves: u32,
    /// Step length in beats (0.25 = sixteenths).
    pub step_beats: f64,
    /// 0..1 share of the step the note sounds.
    pub gate: f64,
    /// 0..1 probability each step plays.
    pub density: f64,
    pub swing: f64,
    pub seed: u32,
    pub bars: u32,
}

impl Default for ArpParams {
    fn default() -> Self {
        Self {
            direction: ArpDirection::Up,
            octaves: 1,
            step_beats: 0.25,
            gate: 0.6,
            density: 1.0,
            swing: 0.0,
            seed: 1,
            bars: 4,
        }
    }
}

/// Arpeggiate the chord tones (lowest first, with cents) over the bars.
pub fn generate_arpeggio(chord: &[(i32, f64)], params: &ArpParams, meter: &Meter) -> Vec<BeatNote> {
    if chord.is_empty() {
        return Vec::new();
    }
    let mut rng = Rng::new(params.seed ^ 0xA2B3_C4D5);
    let mut pool: Vec<(i32, f64)> = Vec::new();
    for o in 0..params.octaves.clamp(1, 3) as i32 {
        for &(m, c) in chord {
            pool.push((m + 12 * o, c));
        }
    }
    pool.sort_by_key(|t| t.0);
    pool.dedup_by(|a, b| a.0 == b.0);
    let order: Vec<usize> = match params.direction {
        ArpDirection::Up => (0..pool.len()).collect(),
        ArpDirection::Down => (0..pool.len()).rev().collect(),
        ArpDirection::UpDown => {
            let mut v: Vec<usize> = (0..pool.len()).collect();
            if pool.len() > 2 {
                v.extend((1..pool.len() - 1).rev());
            }
            v
        }
        ArpDirection::Random => Vec::new(),
    };
    let step = params.step_beats.max(0.0625);
    let total_steps =
        ((params.bars.clamp(1, 64) as f64 * meter.bar_beats()) / step).round() as usize;
    let gate = params.gate.clamp(0.05, 1.0);
    let mut out = Vec::new();
    let mut last_random = usize::MAX;
    for k in 0..total_steps {
        if !rng.chance(params.density.clamp(0.0, 1.0)) {
            continue;
        }
        let idx = if params.direction == ArpDirection::Random {
            // Never the same note twice in a row when there is a choice.
            let mut i = rng.below(pool.len());
            if pool.len() > 1 && i == last_random {
                i = (i + 1) % pool.len();
            }
            last_random = i;
            i
        } else {
            order[k % order.len()]
        };
        let (midi, cents) = pool[idx];
        let t = k as f64 * step;
        out.push(BeatNote {
            start_beat: t + swing_offset(t, step, params.swing),
            length_beats: step * gate,
            midi,
            cents,
            velocity: if (t - t.floor()).abs() < 1e-9 {
                0.85
            } else {
                0.65
            },
            glide: false,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const TONES: [(i32, f64); 4] = [(62, 0.0), (65, 0.0), (69, 0.0), (72, 0.0)];

    #[test]
    fn ostinato_stays_on_its_tones_varies_and_keeps_a_locked_motif() {
        let m = Meter::new(4, 4);
        let p = OstinatoParams {
            seed: 5,
            ..Default::default()
        };
        let a = generate_ostinato(&TONES, &p, &m, None);
        let b = generate_ostinato(&TONES, &p, &m, None);
        assert_eq!(a, b);
        assert!(!a.is_empty());
        let allowed: Vec<i32> = TONES.iter().flat_map(|t| [t.0, t.0 + 12]).collect();
        assert!(
            a.iter().all(|n| allowed.contains(&n.midi)),
            "all notes from the tones (± octave)"
        );
        assert!(a
            .iter()
            .all(|n| n.start_beat < 32.0 && n.length_beats > 0.0));
        // Variation: bars 5–8 differ from bars 1–4 (same length window compared by shape).
        let shape = |from: f64, to: f64| -> Vec<(i64, i32)> {
            a.iter()
                .filter(|n| n.start_beat >= from && n.start_beat < to)
                .map(|n| (((n.start_beat - from) * 1000.0).round() as i64, n.midi))
                .collect()
        };
        assert_ne!(shape(0.0, 16.0), shape(16.0, 32.0), "a variation happened");
        // Locked motif: any seed gives the same notes in every bar.
        let motif = Motif::generate(TONES.len(), &p);
        let l1 = generate_ostinato(
            &TONES,
            &OstinatoParams {
                seed: 1,
                ..p.clone()
            },
            &m,
            Some(&motif),
        );
        let l2 = generate_ostinato(
            &TONES,
            &OstinatoParams {
                seed: 99,
                ..p.clone()
            },
            &m,
            Some(&motif),
        );
        // The notes (time, pitch) are identical; only the humanized velocities follow the seed.
        let keys = |v: &Vec<BeatNote>| {
            v.iter()
                .map(|n| ((n.start_beat * 1000.0).round() as i64, n.midi))
                .collect::<Vec<_>>()
        };
        assert_eq!(keys(&l1), keys(&l2));
        let s1 = l1
            .iter()
            .filter(|n| n.start_beat < 4.0)
            .map(|n| ((n.start_beat * 1000.0) as i64, n.midi))
            .collect::<Vec<_>>();
        let s5 = l1
            .iter()
            .filter(|n| n.start_beat >= 16.0 && n.start_beat < 20.0)
            .map(|n| (((n.start_beat - 16.0) * 1000.0) as i64, n.midi))
            .collect::<Vec<_>>();
        assert_eq!(s1, s5, "locked motif repeats unchanged");
        // Different seed, no lock → different motif.
        let c = generate_ostinato(&TONES, &OstinatoParams { seed: 6, ..p }, &m, None);
        assert_ne!(a, c);
    }

    #[test]
    fn arpeggio_directions_density_and_no_hanging_notes() {
        let m = Meter::new(4, 4);
        let base = ArpParams {
            bars: 1,
            ..Default::default()
        };
        let up = generate_arpeggio(&TONES, &base, &m);
        assert_eq!(up.len(), 16);
        assert_eq!(
            up.iter().take(4).map(|n| n.midi).collect::<Vec<_>>(),
            vec![62, 65, 69, 72]
        );
        let down = generate_arpeggio(
            &TONES,
            &ArpParams {
                direction: ArpDirection::Down,
                ..base.clone()
            },
            &m,
        );
        assert_eq!(
            down.iter().take(4).map(|n| n.midi).collect::<Vec<_>>(),
            vec![72, 69, 65, 62]
        );
        let ud = generate_arpeggio(
            &TONES,
            &ArpParams {
                direction: ArpDirection::UpDown,
                ..base.clone()
            },
            &m,
        );
        assert_eq!(
            ud.iter().take(6).map(|n| n.midi).collect::<Vec<_>>(),
            vec![62, 65, 69, 72, 69, 65]
        );
        let two = generate_arpeggio(
            &TONES,
            &ArpParams {
                octaves: 2,
                ..base.clone()
            },
            &m,
        );
        assert!(two.iter().any(|n| n.midi == 84));
        let rnd = generate_arpeggio(
            &TONES,
            &ArpParams {
                direction: ArpDirection::Random,
                ..base.clone()
            },
            &m,
        );
        assert!(
            rnd.windows(2).all(|w| w[0].midi != w[1].midi),
            "no immediate repeats"
        );
        assert_eq!(
            rnd,
            generate_arpeggio(
                &TONES,
                &ArpParams {
                    direction: ArpDirection::Random,
                    ..base.clone()
                },
                &m
            )
        );
        // Gate: every note ends before the next step → nothing hangs across a change.
        for n in &up {
            assert!(n.length_beats <= 0.25 * 0.6 + 1e-9);
        }
        let sparse = generate_arpeggio(
            &TONES,
            &ArpParams {
                density: 0.4,
                seed: 3,
                ..base
            },
            &m,
        );
        assert!(sparse.len() < 12 && !sparse.is_empty());
    }
}
