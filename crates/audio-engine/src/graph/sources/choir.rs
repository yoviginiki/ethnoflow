//! Choir drone (F1B.5): a vocal drone on the pitch set. Every voice is a glottal pulse
//! (band-limited saw ↔ narrow pulse with `brightness`) through three formant band-passes
//! whose centres morph along the vowel row a–e–i–o–u with `vowel` (and drift slowly with
//! `morph`), a slow breath envelope (`breath`), a little vibrato and per-voice detune
//! (`width`). Voices sit on the pitch set's notes in order (low to high); when the set
//! changes they glide. No allocation in `render`.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::{PitchSet, Rng, Svf};
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;

pub const CHOIR_VOICES: usize = 6;
const FORMANTS: usize = 3;
/// (F1, F2, F3) in Hz for a, e, i, o, u (a low-ish voice), and their relative levels.
const VOWELS: [[f32; FORMANTS]; 5] = [
    [730.0, 1090.0, 2440.0],
    [530.0, 1840.0, 2480.0],
    [390.0, 1990.0, 2550.0],
    [570.0, 840.0, 2410.0],
    [440.0, 1020.0, 2240.0],
];
const FORMANT_LEVEL: [f32; FORMANTS] = [1.0, 0.5, 0.25];

pub struct ChoirParams {
    pub pitches: PitchSet,
    /// 0..1 along a–e–i–o–u.
    pub vowel: SmoothedParam,
    /// 0..1 breath movement (amplitude swell depth).
    pub breath: SmoothedParam,
    /// Per-voice detune, cents (0..30).
    pub width: SmoothedParam,
    /// 0..1 glottal brightness (saw … narrow pulse) and formant tilt.
    pub brightness: SmoothedParam,
    /// 0..1 how far and fast the vowel drifts by itself.
    pub morph: SmoothedParam,
    /// Voices used (1..=CHOIR_VOICES).
    pub voices: AtomicU32,
    pub seed: AtomicU32,
}

impl Default for ChoirParams {
    fn default() -> Self {
        Self {
            pitches: PitchSet::default(),
            vowel: SmoothedParam::new(0.6, 0.0, 1.0),
            breath: SmoothedParam::new(0.4, 0.0, 1.0),
            width: SmoothedParam::new(8.0, 0.0, 30.0),
            brightness: SmoothedParam::new(0.4, 0.0, 1.0),
            morph: SmoothedParam::new(0.3, 0.0, 1.0),
            voices: AtomicU32::new(4),
            seed: AtomicU32::new(1),
        }
    }
}

#[derive(Clone, Copy)]
struct Voice {
    hz: f32,
    target_hz: f32,
    phase: f32,
    vib_phase: f32,
    vib_hz: f32,
    breath_phase: f32,
    breath_hz: f32,
    detune: f32,
    formants: [Svf; FORMANTS],
    amp: f32,
}

impl Voice {
    fn new(rng: &mut Rng) -> Self {
        Self {
            hz: 0.0,
            target_hz: 0.0,
            phase: rng.unit(),
            vib_phase: rng.unit(),
            vib_hz: 4.6 + rng.unit() * 1.2,
            breath_phase: rng.unit(),
            breath_hz: 0.05 + rng.unit() * 0.12,
            detune: rng.bipolar(),
            formants: [Svf::default(); FORMANTS],
            amp: 0.0,
        }
    }
}

pub struct ChoirSource {
    params: Arc<ChoirParams>,
    voices: [Voice; CHOIR_VOICES],
    vowel: Ramp,
    breath: Ramp,
    width: Ramp,
    brightness: Ramp,
    drift_phase: f32,
    seed_used: u32,
    sample_count: u64,
}

impl ChoirSource {
    pub fn new(params: Arc<ChoirParams>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        let mut rng = Rng::new(seed.wrapping_mul(747_796_405).wrapping_add(11));
        Self {
            vowel: Ramp::new(params.vowel.target()),
            breath: Ramp::new(params.breath.target()),
            width: Ramp::new(params.width.target()),
            brightness: Ramp::new(params.brightness.target()),
            voices: std::array::from_fn(|_| Voice::new(&mut rng)),
            drift_phase: 0.0,
            seed_used: seed,
            sample_count: 0,
            params,
        }
    }

    /// Formant centres for a vowel position 0..1 (linear between the row's vowels).
    fn formants_at(pos: f32) -> [f32; FORMANTS] {
        let x = pos.clamp(0.0, 1.0) * (VOWELS.len() - 1) as f32;
        let i = (x.floor() as usize).min(VOWELS.len() - 2);
        let f = x - i as f32;
        std::array::from_fn(|k| VOWELS[i][k] * (1.0 - f) + VOWELS[i + 1][k] * f)
    }
}

#[inline]
fn saw_blep(t: f32, dt: f32) -> f32 {
    let mut y = 2.0 * t - 1.0;
    if t < dt {
        let x = t / dt;
        y -= x + x - x * x - 1.0;
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        y -= x * x + x + x + 1.0;
    }
    y
}

impl Source for ChoirSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            let mut rng = Rng::new(seed.wrapping_mul(747_796_405).wrapping_add(11));
            for v in self.voices.iter_mut() {
                let fresh = Voice::new(&mut rng);
                v.vib_hz = fresh.vib_hz;
                v.breath_hz = fresh.breath_hz;
                v.detune = fresh.detune;
            }
            self.seed_used = seed;
        }
        let ramp = ramp_samples(sample_rate);
        self.vowel.begin(p.vowel.target(), ramp);
        self.breath.begin(p.breath.target(), ramp);
        self.width.begin(p.width.target(), ramp);
        self.brightness.begin(p.brightness.target(), ramp);
        let morph = p.morph.target();
        let n_voices = (p.voices.load(Ordering::Relaxed) as usize).clamp(1, CHOIR_VOICES);
        let n_pitches = p.pitches.len();
        // Targets: voice k takes pitch k (wrapping) of the set, low to high.
        for (k, v) in self.voices.iter_mut().enumerate() {
            v.target_hz = if n_pitches == 0 || k >= n_voices {
                0.0
            } else {
                p.pitches.get(k % n_pitches)
            };
            if v.hz <= 0.0 {
                v.hz = v.target_hz;
            }
        }
        // Per-block filter coefficients: the vowel drifts slowly with `morph`.
        self.drift_phase += (0.02 + 0.08 * morph) * out.len() as f32 / sr;
        if self.drift_phase >= 1.0 {
            self.drift_phase -= 1.0;
        }
        let drift = (self.drift_phase * std::f32::consts::TAU).sin() * 0.25 * morph;
        let centres = Self::formants_at(self.vowel.current + drift);
        let bright = self.brightness.current;
        for v in self.voices.iter_mut() {
            for (k, f) in v.formants.iter_mut().enumerate() {
                let q = 7.0 + 5.0 * bright;
                f.set(centres[k].clamp(80.0, 6000.0), q, sr);
            }
        }
        let glide = 1.0 - (-3.0 / (0.4 * sr)).exp();
        let amp_target = 1.0 / (n_voices as f32).sqrt();
        for s in out.iter_mut() {
            let vowel_pos = self.vowel.next(p.vowel.target());
            let _ = vowel_pos;
            let breath = self.breath.next(p.breath.target());
            let width = self.width.next(p.width.target());
            let mut acc = 0.0f32;
            for (k, v) in self.voices.iter_mut().enumerate() {
                let on = k < n_voices && v.target_hz > 0.0;
                v.amp += ((if on { amp_target } else { 0.0 }) - v.amp) * (1.0 / (0.3 * sr));
                if v.amp < 1e-4 && !on {
                    continue;
                }
                v.hz += (v.target_hz.max(1.0) - v.hz) * glide;
                v.vib_phase += v.vib_hz / sr;
                if v.vib_phase >= 1.0 {
                    v.vib_phase -= 1.0;
                }
                v.breath_phase += v.breath_hz / sr;
                if v.breath_phase >= 1.0 {
                    v.breath_phase -= 1.0;
                }
                let vib = 1.0 + 0.004 * (v.vib_phase * std::f32::consts::TAU).sin();
                let det = (v.detune * width / 1200.0).exp2();
                let dt = (v.hz * vib * det / sr).clamp(0.0, 0.5);
                // Glottal source: saw blended towards a narrow pulse with brightness.
                let saw = saw_blep(v.phase, dt);
                let pulse = saw - saw_blep((v.phase + 0.12 + 0.2 * (1.0 - bright)).fract(), dt);
                let src = saw * (1.0 - bright) + pulse * bright;
                v.phase += dt;
                if v.phase >= 1.0 {
                    v.phase -= 1.0;
                }
                let mut y = 0.0f32;
                for (fi, f) in v.formants.iter_mut().enumerate() {
                    y += f.process(src).bp
                        * FORMANT_LEVEL[fi]
                        * (if fi > 0 { 0.6 + 0.8 * bright } else { 1.0 });
                }
                let swell = 1.0
                    - breath * 0.5 * (0.5 + 0.5 * (v.breath_phase * std::f32::consts::TAU).sin());
                acc += y * v.amp * swell;
            }
            *s = undenormal(super::gen::soft(acc * 0.9));
        }
        self.sample_count += out.len() as u64;
    }

    fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.formants = [Svf::default(); FORMANTS];
            v.amp = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, GraphParams};

    fn goertzel(x: &[f32], hz: f32, sr: f32) -> f32 {
        let w = std::f32::consts::TAU * hz / sr;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &v in x {
            let s0 = v + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2).sqrt() / x.len() as f32
    }

    fn render(set: impl Fn(&ChoirParams)) -> Vec<f32> {
        let p = Arc::new(ChoirParams::default());
        p.pitches.set(&[146.83, 220.0]); // D3, A3
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(ChoirSource::new(p)));
        g.render_offline(3.0, 1).samples
    }

    #[test]
    fn choir_sings_the_pitch_set_and_the_vowel_moves_the_formants() {
        let a = render(|p| {
            p.vowel.set(0.0);
            p.morph.set(0.0);
            p.breath.set(0.0);
        });
        assert!(a.iter().all(|v| v.is_finite()));
        let tail = &a[96_000..];
        let peak = tail.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.03 && peak <= 1.0, "audible {peak}");
        // Harmonics of D3 and A3 dominate over an off-scale frequency.
        let on = goertzel(tail, 146.83 * 2.0, 48_000.0) + goertzel(tail, 220.0 * 2.0, 48_000.0);
        let off = goertzel(tail, 155.56 * 2.0, 48_000.0) + goertzel(tail, 233.08 * 2.0, 48_000.0);
        assert!(on > off * 2.0, "in tune: {on} vs {off}");
        // "a" (F2 ≈ 1090 Hz) has more energy near 1.1 kHz than "i" (F2 ≈ 1990 Hz), and the
        // other way round near 2 kHz.
        let i = render(|p| {
            p.vowel.set(0.5);
            p.morph.set(0.0);
            p.breath.set(0.0);
        });
        let band = |x: &[f32], lo: f32, hi: f32| {
            let mut e = 0.0;
            let mut f = lo;
            while f < hi {
                e += goertzel(&x[96_000..], f, 48_000.0);
                f += 36.7; // step through the D3 harmonic series
            }
            e
        };
        assert!(
            band(&a, 1000.0, 1200.0) > band(&i, 1000.0, 1200.0),
            "a has the lower F2"
        );
        assert!(
            band(&i, 1850.0, 2150.0) > band(&a, 1850.0, 2150.0),
            "i has the higher F2"
        );
    }

    #[test]
    fn empty_pitch_set_is_silent_and_breath_swells() {
        let p = Arc::new(ChoirParams::default());
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(ChoirSource::new(Arc::clone(&p))));
        let b = g.render_offline(1.0, 1);
        assert!(
            b.samples.iter().all(|v| v.abs() < 1e-6),
            "silent without pitches"
        );
        let swell = render(|p| {
            p.breath.set(1.0);
            p.morph.set(0.0);
        });
        let rms = |x: &[f32]| (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt();
        let blocks: Vec<f32> = swell[48_000..].chunks(12_000).map(rms).collect();
        let (lo, hi) = blocks
            .iter()
            .fold((f32::MAX, 0.0f32), |(l, h), &v| (l.min(v), h.max(v)));
        assert!(hi > lo * 1.15, "breath moves the level: {lo} .. {hi}");
    }
}
