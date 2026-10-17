//! Modal resonator (F1B.4): struck bodies tuned to the pitch set. Every voice is four
//! two-pole resonators (partials) whose ratios slide from harmonic (1, 2, 3, 4: a string
//! or a tube) to bell-like (1, 2.76, 5.40, 8.93: a bar or a bowl) with `structure`; a
//! short noise burst strikes them (`strike` = hardness), `brightness` tilts the partials
//! and their damping, `decay_s` is the fundamental's ring-out. Sparse seeded strikes as
//! in the bells; silence is kept. No allocation in `render`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::{PitchSet, Trigger};
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;

pub const RESONATOR_VOICES: usize = 6;
const PARTIALS: usize = 4;
const HARMONIC: [f32; PARTIALS] = [1.0, 2.0, 3.0, 4.0];
const BELL: [f32; PARTIALS] = [1.0, 2.756, 5.404, 8.933];

pub struct ResonatorParams {
    pub pitches: PitchSet,
    /// Strikes per minute (0 = silent).
    pub density: SmoothedParam,
    /// Ring-out of the fundamental to −60 dB, seconds.
    pub decay_s: SmoothedParam,
    /// 0 harmonic (string/tube) … 1 inharmonic (bar/bowl/bell).
    pub structure: SmoothedParam,
    /// 0..1 dull … bright: partial levels and how fast the upper partials die.
    pub brightness: SmoothedParam,
    /// 0..1 soft mallet … hard stick (exciter burst length).
    pub strike: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for ResonatorParams {
    fn default() -> Self {
        Self {
            pitches: PitchSet::default(),
            density: SmoothedParam::new(20.0, 0.0, 120.0),
            decay_s: SmoothedParam::new(3.0, 0.2, 30.0),
            structure: SmoothedParam::new(0.5, 0.0, 1.0),
            brightness: SmoothedParam::new(0.5, 0.0, 1.0),
            strike: SmoothedParam::new(0.5, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Partial {
    /// 2-pole resonator coefficients and state.
    a1: f32,
    a2: f32,
    gain: f32,
    y1: f32,
    y2: f32,
}

#[derive(Clone, Copy, Default)]
struct Voice {
    partials: [Partial; PARTIALS],
    /// Remaining exciter samples and its envelope.
    burst: u32,
    burst_len: u32,
    amp: f32,
    /// Samples since the strike (for voice stealing and silence detection).
    age: u32,
    life: u32,
    active: bool,
}

pub struct ResonatorSource {
    params: Arc<ResonatorParams>,
    trigger: Trigger,
    seed_used: u32,
    density: Ramp,
    voices: [Voice; RESONATOR_VOICES],
    next_voice: usize,
}

impl ResonatorSource {
    pub fn new(params: Arc<ResonatorParams>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        Self {
            trigger: Trigger::new(seed),
            seed_used: seed,
            density: Ramp::new(params.density.target()),
            voices: [Voice::default(); RESONATOR_VOICES],
            next_voice: 0,
            params,
        }
    }

    fn strike(&mut self, hz: f32, sr: f32, p: &ResonatorParams) {
        let i = (0..RESONATOR_VOICES)
            .map(|k| (self.next_voice + k) % RESONATOR_VOICES)
            .find(|&k| !self.voices[k].active)
            .unwrap_or(self.next_voice);
        self.next_voice = (i + 1) % RESONATOR_VOICES;
        let structure = p.structure.target();
        let bright = p.brightness.target();
        let decay = p.decay_s.target().max(0.05);
        let hardness = p.strike.target();
        let mut v = Voice {
            burst_len: ((1.0 + 9.0 * (1.0 - hardness)) * 0.001 * sr) as u32 + 8,
            amp: 0.5,
            life: (decay * 1.5 * sr) as u32 + sr as u32,
            active: true,
            ..Voice::default()
        };
        v.burst = v.burst_len;
        for (k, part) in v.partials.iter_mut().enumerate() {
            // Geometric interpolation between the two ratio sets.
            let ratio = HARMONIC[k] * (BELL[k] / HARMONIC[k]).powf(structure);
            let f = hz * ratio;
            if f >= sr * 0.45 {
                *part = Partial::default();
                continue;
            }
            // Upper partials die faster, more so when dull.
            let tau = decay / (1.0 + k as f32 * (1.6 - bright));
            let r = (-6.9078 / (tau * sr)).exp();
            let w = std::f32::consts::TAU * f / sr;
            let level = 1.0 / (1.0 + k as f32).powf(1.6 - bright);
            *part = Partial {
                a1: 2.0 * r * w.cos(),
                a2: -r * r,
                // Normalised so a hit excites each partial to about `level`.
                gain: level * (1.0 - r) * 32.0,
                y1: 0.0,
                y2: 0.0,
            };
        }
        self.voices[i] = v;
    }

    #[inline]
    fn render_span(&mut self, out: &mut [f32], density_target: f32) {
        for s in out.iter_mut() {
            let _ = self.density.next(density_target);
            let mut acc = 0.0f32;
            for v in self.voices.iter_mut() {
                if !v.active {
                    continue;
                }
                // Exciter: a short noise burst with a falling envelope.
                let x = if v.burst > 0 {
                    let env = v.burst as f32 / v.burst_len as f32;
                    v.burst -= 1;
                    self.trigger.rng().bipolar() * env * v.amp
                } else {
                    0.0
                };
                let mut sum = 0.0f32;
                for part in v.partials.iter_mut() {
                    if part.gain == 0.0 {
                        continue;
                    }
                    let y = undenormal(part.a1 * part.y1 + part.a2 * part.y2 + part.gain * x);
                    part.y2 = part.y1;
                    part.y1 = y;
                    sum += y;
                }
                acc += sum;
                v.age += 1;
                if v.age > v.life {
                    v.active = false;
                }
            }
            *s = acc;
        }
    }
}

impl Source for ResonatorSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            self.trigger.reset(seed);
            self.seed_used = seed;
        }
        self.density
            .begin(p.density.target(), ramp_samples(sample_rate));
        let n = out.len();
        let density = if p.pitches.is_empty() {
            0.0
        } else {
            self.density.current
        };
        let strike_at = self.trigger.tick(n, density, sr);
        let at = strike_at.unwrap_or(n).min(n);
        let target = p.density.target();
        let (head, tail) = out.split_at_mut(at);
        self.render_span(head, target);
        if strike_at.is_some() {
            let hz = p.pitches.pick(self.trigger.rng()).unwrap_or(0.0);
            if hz > 0.0 {
                self.strike(hz, sr, &p);
            }
        }
        self.render_span(tail, target);
    }

    fn reset(&mut self) {
        self.trigger.reset(self.seed_used);
        self.voices = [Voice::default(); RESONATOR_VOICES];
        self.next_voice = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    fn render(set: impl Fn(&ResonatorParams), seconds: f64) -> AudioBuffer {
        let p = Arc::new(ResonatorParams::default());
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(ResonatorSource::new(p)));
        g.render_offline(seconds, 1)
    }

    fn first_strike(b: &AudioBuffer) -> f64 {
        b.samples
            .iter()
            .position(|s| s.abs() > 0.01)
            .expect("a strike") as f64
            / 48_000.0
    }

    #[test]
    fn a_harmonic_body_rings_in_tune_and_silence_is_kept() {
        let b = render(
            |p| {
                p.pitches.set(&[220.0]);
                p.density.set(1.0);
                p.structure.set(0.0);
                p.brightness.set(0.3);
                p.decay_s.set(6.0);
            },
            8.0,
        );
        assert!(b.samples.iter().all(|s| s.is_finite()) && b.peak() <= 1.0);
        let t0 = first_strike(&b);
        let mut hz: Vec<f64> = contour(&b, &PitchConfig::new(48_000), 0.02)
            .into_iter()
            .filter(|p| p.t > t0 + 0.15 && p.t < t0 + 1.5)
            .filter_map(|p| p.hz)
            .collect();
        hz.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = hz[hz.len() / 2];
        let cents = 1200.0 * (med / 220.0).log2();
        assert!(cents.abs() < 8.0, "{med:.2} Hz ({cents:.1} c)");
        assert_eq!(
            render(|p| p.density.set(30.0), 3.0).peak(),
            0.0,
            "no pitches → silence"
        );
        assert_eq!(
            render(
                |p| {
                    p.pitches.set(&[220.0]);
                    p.density.set(0.0);
                },
                3.0
            )
            .peak(),
            0.0
        );
    }

    /// Energy at one frequency (Goertzel) over a window.
    fn goertzel(b: &AudioBuffer, hz: f32, from_s: f64, to_s: f64) -> f32 {
        let a = (from_s * 48_000.0) as usize;
        let z = (to_s * 48_000.0) as usize;
        let w = std::f32::consts::TAU * hz / 48_000.0;
        let c = 2.0 * w.cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in &b.samples[a..z] {
            let s0 = x + c * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - c * s1 * s2).max(0.0).sqrt()
    }

    #[test]
    fn structure_brightness_and_decay_are_measurable() {
        let mk = |structure: f32, bright: f32, decay: f32| {
            render(
                |p| {
                    p.pitches.set(&[220.0]);
                    p.density.set(1.0);
                    p.structure.set(structure);
                    p.brightness.set(bright);
                    p.decay_s.set(decay);
                },
                6.0,
            )
        };
        let string = mk(0.0, 0.5, 4.0);
        let bell = mk(1.0, 0.5, 4.0);
        let t = first_strike(&string);
        let (f, to) = (t + 0.05, t + 0.8);
        // Harmonic: the second partial is at 440 Hz; bell-like: at 2.756 × 220 = 606 Hz.
        assert!(
            goertzel(&string, 440.0, f, to) > goertzel(&string, 606.0, f, to) * 2.0,
            "string has the octave partial"
        );
        assert!(
            goertzel(&bell, 606.0, f, to) > goertzel(&bell, 440.0, f, to) * 2.0,
            "bell has the 2.756 partial"
        );
        let dull = mk(0.0, 0.0, 4.0);
        let shiny = mk(0.0, 1.0, 4.0);
        let tilt =
            |b: &AudioBuffer| goertzel(b, 880.0, f, to) / goertzel(b, 220.0, f, to).max(1e-9);
        assert!(
            tilt(&shiny) > tilt(&dull) * 1.5,
            "brightness lifts the 4th partial: {} vs {}",
            tilt(&shiny),
            tilt(&dull)
        );
        let short = mk(0.0, 0.5, 0.4);
        let long = mk(0.0, 0.5, 8.0);
        let late = |b: &AudioBuffer| {
            b.slice_frames(
                ((t + 1.5) * 48_000.0) as usize,
                ((t + 2.0) * 48_000.0) as usize,
            )
            .rms()
        };
        assert!(
            late(&long) > late(&short) * 5.0,
            "decay: {} vs {}",
            late(&long),
            late(&short)
        );
        assert!(
            string.peak() > 0.1 && string.peak() <= 1.0,
            "usable level: {}",
            string.peak()
        );
    }

    #[test]
    fn seed_reproduces_and_chunking_does_not_matter() {
        let p = Arc::new(ResonatorParams::default());
        p.pitches.set(&[196.0, 294.0, 392.0]);
        p.density.set(40.0);
        let mk = |block: usize| {
            let gp = Arc::new(GraphParams::default());
            gp.master.gain_db.set(0.0);
            gp.layers[0].gain_db.set(0.0);
            let mut g = Graph::new(gp, 48_000);
            g.set_source(0, Box::new(ResonatorSource::new(Arc::clone(&p))));
            let mut out = vec![0.0f32; 3 * 48_000];
            for c in out.chunks_mut(block) {
                g.render(c, 1);
            }
            out
        };
        let a = mk(64);
        let b = mk(500);
        let diff = a
            .iter()
            .zip(&b)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(diff < 1e-5, "block independence: {diff}");
        assert!(a.iter().any(|v| v.abs() > 0.02));
    }
}
