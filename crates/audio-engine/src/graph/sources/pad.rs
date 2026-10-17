//! Liquid pad (F3.3): up to six chord voices of two detuned band-limited saws plus a
//! sub sine, a state-variable low-pass with a slow LFO, and a small reverb. The
//! "emotion" presets and the X/Y macro map onto these parameters in `PadMacro`;
//! `seed` fixes the LFO/detune phases so a render is reproducible.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::reverb::Reverb;
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;

pub const PAD_VOICES: usize = 6;

pub struct PadParams {
    /// Voice frequencies in Hz; 0 = the voice is off (released).
    pub voices: [SmoothedParam; PAD_VOICES],
    pub cutoff_hz: SmoothedParam,
    /// 0..1 filter resonance.
    pub resonance: SmoothedParam,
    pub lfo_hz: SmoothedParam,
    /// LFO depth in octaves around the cutoff.
    pub lfo_depth: SmoothedParam,
    pub detune_cents: SmoothedParam,
    /// 0..1 level of the sub sine an octave below each voice.
    pub warmth: SmoothedParam,
    pub attack_s: SmoothedParam,
    pub release_s: SmoothedParam,
    /// 0..1 reverb mix.
    pub space: SmoothedParam,
    /// 0..1 reverb size.
    pub size: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for PadParams {
    fn default() -> Self {
        Self {
            voices: std::array::from_fn(|_| SmoothedParam::new(0.0, 0.0, 20_000.0)),
            cutoff_hz: SmoothedParam::new(1200.0, 80.0, 8000.0),
            resonance: SmoothedParam::new(0.2, 0.0, 0.95),
            lfo_hz: SmoothedParam::new(0.08, 0.005, 8.0),
            lfo_depth: SmoothedParam::new(0.5, 0.0, 3.0),
            detune_cents: SmoothedParam::new(8.0, 0.0, 40.0),
            warmth: SmoothedParam::new(0.4, 0.0, 1.0),
            attack_s: SmoothedParam::new(1.5, 0.005, 20.0),
            release_s: SmoothedParam::new(2.5, 0.005, 30.0),
            space: SmoothedParam::new(0.35, 0.0, 1.0),
            size: SmoothedParam::new(0.6, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

/// An emotion preset: five musical parameters in 0..1.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PadMacro {
    pub brightness: f32,
    pub movement: f32,
    pub warmth: f32,
    pub space: f32,
    /// 0 = instant, 1 = very slow attack and release.
    pub slowness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emotion {
    Calm,
    Melancholy,
    Light,
    Dark,
}

impl Emotion {
    pub const ALL: [Emotion; 4] = [
        Emotion::Calm,
        Emotion::Melancholy,
        Emotion::Light,
        Emotion::Dark,
    ];

    pub fn preset(self) -> PadMacro {
        match self {
            Emotion::Calm => PadMacro {
                brightness: 0.35,
                movement: 0.25,
                warmth: 0.5,
                space: 0.45,
                slowness: 0.6,
            },
            Emotion::Melancholy => PadMacro {
                brightness: 0.2,
                movement: 0.45,
                warmth: 0.7,
                space: 0.7,
                slowness: 0.8,
            },
            Emotion::Light => PadMacro {
                brightness: 0.75,
                movement: 0.35,
                warmth: 0.2,
                space: 0.35,
                slowness: 0.35,
            },
            Emotion::Dark => PadMacro {
                brightness: 0.12,
                movement: 0.6,
                warmth: 0.9,
                space: 0.8,
                slowness: 0.7,
            },
        }
    }
}

impl PadMacro {
    /// X/Y pad: X moves brightness and movement, Y depth (warmth, slowness) and space.
    /// Both in 0..1; 0.5/0.5 leaves the preset as it is.
    pub fn with_xy(self, x: f32, y: f32) -> Self {
        let dx = x.clamp(0.0, 1.0) - 0.5;
        let dy = y.clamp(0.0, 1.0) - 0.5;
        let c = |v: f32| v.clamp(0.0, 1.0);
        Self {
            brightness: c(self.brightness + dx * 0.9),
            movement: c(self.movement + dx * 0.6),
            warmth: c(self.warmth + dy * 0.6),
            space: c(self.space + dy * 0.8),
            slowness: c(self.slowness + dy * 0.4),
        }
    }

    /// Write the macro onto the parameters (chord voices are untouched).
    pub fn apply(&self, p: &PadParams) {
        p.cutoff_hz.set(150.0 * 2f32.powf(self.brightness * 5.4)); // 150 Hz … ~6.3 kHz
        p.resonance.set(0.1 + 0.5 * self.brightness);
        p.lfo_depth.set(0.15 + 1.6 * self.movement);
        p.detune_cents.set(4.0 + 14.0 * self.movement);
        p.warmth.set(self.warmth);
        p.space.set(self.space * 0.8);
        p.size.set(0.3 + 0.7 * self.space);
        p.attack_s.set(0.05 + 4.0 * self.slowness * self.slowness);
        p.release_s.set(0.2 + 6.0 * self.slowness);
    }
}

struct Voice {
    hz: Ramp,
    phase_a: f32,
    phase_b: f32,
    phase_sub: f32,
    env: f32,
}

pub struct PadSource {
    params: Arc<PadParams>,
    voices: [Voice; PAD_VOICES],
    cutoff: Ramp,
    resonance: Ramp,
    lfo_hz: Ramp,
    lfo_depth: Ramp,
    detune: Ramp,
    warmth: Ramp,
    space: Ramp,
    size: Ramp,
    lfo_phase: f32,
    lp: f32,
    bp: f32,
    reverb: Reverb,
    seeded_with: u32,
}

#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

#[inline]
fn saw(phase: &mut f32, hz: f32, sr: f32) -> f32 {
    let dt = hz / sr;
    let v = 2.0 * *phase - 1.0 - poly_blep(*phase, dt);
    *phase += dt;
    if *phase >= 1.0 {
        *phase -= 1.0;
    }
    v
}

/// Deterministic phases from the seed (xorshift), so renders are reproducible.
fn seeded_phases(seed: u32) -> [f32; PAD_VOICES * 3 + 1] {
    let mut s = seed.max(1);
    std::array::from_fn(|_| {
        s ^= s << 13;
        s ^= s >> 17;
        s ^= s << 5;
        (s % 10_000) as f32 / 10_000.0
    })
}

impl PadSource {
    pub fn new(params: Arc<PadParams>) -> Self {
        let p = &params;
        let mut src = Self {
            voices: std::array::from_fn(|i| Voice {
                hz: Ramp::new(p.voices[i].target()),
                phase_a: 0.0,
                phase_b: 0.0,
                phase_sub: 0.0,
                env: 0.0,
            }),
            cutoff: Ramp::new(p.cutoff_hz.target()),
            resonance: Ramp::new(p.resonance.target()),
            lfo_hz: Ramp::new(p.lfo_hz.target()),
            lfo_depth: Ramp::new(p.lfo_depth.target()),
            detune: Ramp::new(p.detune_cents.target()),
            warmth: Ramp::new(p.warmth.target()),
            space: Ramp::new(p.space.target()),
            size: Ramp::new(p.size.target()),
            lfo_phase: 0.0,
            lp: 0.0,
            bp: 0.0,
            reverb: Reverb::new(48_000),
            seeded_with: 0,
            params: Arc::clone(&params),
        };
        src.seed(p.seed.load(Ordering::Relaxed));
        src
    }

    /// The reverb is sized for a sample rate; call once the rate is known.
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.reverb = Reverb::new(sample_rate);
        self
    }

    pub fn params(&self) -> &Arc<PadParams> {
        &self.params
    }

    fn seed(&mut self, seed: u32) {
        let ph = seeded_phases(seed);
        for (i, v) in self.voices.iter_mut().enumerate() {
            v.phase_a = ph[i * 3];
            v.phase_b = ph[i * 3 + 1];
            v.phase_sub = ph[i * 3 + 2];
        }
        self.lfo_phase = ph[PAD_VOICES * 3];
        self.seeded_with = seed;
    }
}

impl Source for PadSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        let ramp = ramp_samples(sample_rate);
        let p = &self.params;
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seeded_with {
            let ph = seeded_phases(seed);
            for (i, v) in self.voices.iter_mut().enumerate() {
                v.phase_a = ph[i * 3];
                v.phase_b = ph[i * 3 + 1];
                v.phase_sub = ph[i * 3 + 2];
            }
            self.lfo_phase = ph[PAD_VOICES * 3];
            self.seeded_with = seed;
        }
        for (i, v) in self.voices.iter_mut().enumerate() {
            let target = p.voices[i].target();
            if target > 0.0 {
                if v.hz.current <= 0.0 {
                    v.hz = Ramp::new(target); // a fresh voice starts on pitch, no glide from 0
                } else {
                    v.hz.begin(target, ramp * 8.0); // 80 ms legato glide
                }
            }
        }
        self.cutoff.begin(p.cutoff_hz.target(), ramp * 4.0);
        self.resonance.begin(p.resonance.target(), ramp);
        self.lfo_hz.begin(p.lfo_hz.target(), ramp);
        self.lfo_depth.begin(p.lfo_depth.target(), ramp * 4.0);
        self.detune.begin(p.detune_cents.target(), ramp * 4.0);
        self.warmth.begin(p.warmth.target(), ramp * 4.0);
        self.space.begin(p.space.target(), ramp * 4.0);
        self.size.begin(p.size.target(), ramp * 4.0);
        let attack = p.attack_s.target().max(0.005);
        let release = p.release_s.target().max(0.005);
        let attack_step = 1.0 / (attack * sr);
        let release_step = 1.0 / (release * sr);

        for s in out.iter_mut() {
            let cutoff = self.cutoff.next(p.cutoff_hz.target());
            let res = self.resonance.next(p.resonance.target());
            let lfo_hz = self.lfo_hz.next(p.lfo_hz.target());
            let depth = self.lfo_depth.next(p.lfo_depth.target());
            let det = self.detune.next(p.detune_cents.target());
            let warmth = self.warmth.next(p.warmth.target());
            let space = self.space.next(p.space.target());
            let size = self.size.next(p.size.target());

            let mut x = 0.0f32;
            let mut active = 0usize;
            for (i, v) in self.voices.iter_mut().enumerate() {
                let on = p.voices[i].target() > 0.0;
                if on {
                    v.env = (v.env + attack_step).min(1.0);
                } else {
                    v.env = (v.env - release_step).max(0.0);
                    if v.env <= 0.0 {
                        v.hz.current = 0.0;
                        continue;
                    }
                }
                let hz = v.hz.next(p.voices[i].target().max(v.hz.current));
                if hz <= 0.0 {
                    continue;
                }
                active += 1;
                let up = 2f32.powf(det / 1200.0);
                let a = saw(&mut v.phase_a, hz * up, sr);
                let b = saw(&mut v.phase_b, hz / up, sr);
                v.phase_sub += hz * 0.5 / sr;
                if v.phase_sub >= 1.0 {
                    v.phase_sub -= 1.0;
                }
                let sub = (v.phase_sub * std::f32::consts::TAU).sin();
                // Smooth envelope curve (perceptually even fade).
                let e = v.env * v.env * (3.0 - 2.0 * v.env);
                x += e * (0.5 * (a + b) * (1.0 - 0.5 * warmth) + warmth * 0.8 * sub);
            }
            if active > 1 {
                x /= (active as f32).sqrt();
            }
            // LFO on the cutoff, in octaves.
            self.lfo_phase += lfo_hz / sr;
            if self.lfo_phase >= 1.0 {
                self.lfo_phase -= 1.0;
            }
            let lfo = (self.lfo_phase * std::f32::consts::TAU).sin();
            let fc = (cutoff * 2f32.powf(lfo * depth * 0.5)).clamp(60.0, sr * 0.22);
            // Chamberlin state-variable filter, low-pass output.
            let f = 2.0 * (std::f32::consts::PI * fc / sr).sin();
            let q = 1.0 - res * 0.9;
            self.lp = undenormal(self.lp + f * self.bp);
            let hp = x - self.lp - q * self.bp;
            self.bp = undenormal(self.bp + f * hp);
            let dry = self.lp;
            self.reverb.set(size, 0.5 - 0.3 * warmth);
            let wet = self.reverb.process(dry * 0.6);
            *s = (dry * (1.0 - 0.5 * space) + wet * space) * 0.6;
        }
    }

    fn reset(&mut self) {
        for v in self.voices.iter_mut() {
            v.env = 0.0;
            v.hz.current = 0.0;
        }
        self.lp = 0.0;
        self.bp = 0.0;
        self.reverb.clear();
        let seed = self.seeded_with;
        self.seed(seed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};

    fn chord(p: &PadParams, hz: &[f32]) {
        for (i, v) in p.voices.iter().enumerate() {
            v.set(hz.get(i).copied().unwrap_or(0.0));
        }
    }

    fn render(p: &Arc<PadParams>, seconds: f64) -> AudioBuffer {
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(
            0,
            Box::new(PadSource::new(Arc::clone(p)).with_sample_rate(48_000)),
        );
        g.render_offline(seconds, 1)
    }

    /// Crude brightness: energy above ~1.5 kHz relative to the total.
    fn brightness(b: &AudioBuffer) -> f64 {
        let a = 1.0 - (-2.0 * std::f64::consts::PI * 1500.0 / 48_000.0).exp();
        let (mut lp, mut hi, mut tot) = (0.0f64, 0.0f64, 0.0f64);
        for &s in &b.samples {
            let s = s as f64;
            lp += a * (s - lp);
            hi += (s - lp) * (s - lp);
            tot += s * s;
        }
        if tot > 0.0 {
            hi / tot
        } else {
            0.0
        }
    }

    #[test]
    fn three_emotions_sound_different_and_the_pad_is_clean() {
        let mut stats = Vec::new();
        for e in [Emotion::Calm, Emotion::Melancholy, Emotion::Light] {
            let p = Arc::new(PadParams::default());
            chord(&p, &[146.83, 220.0, 329.63]); // any three notes; nothing about a key
            e.preset().apply(&p);
            p.attack_s.set(0.05);
            let b = render(&p, 4.0);
            assert!(b.samples.iter().all(|s| s.is_finite()));
            assert!(
                b.peak() <= 1.0 && b.rms() > 0.01,
                "{e:?}: peak {} rms {}",
                b.peak(),
                b.rms()
            );
            stats.push((e, brightness(&b), b.rms()));
        }
        let light = stats.iter().find(|s| s.0 == Emotion::Light).unwrap();
        let mel = stats.iter().find(|s| s.0 == Emotion::Melancholy).unwrap();
        let calm = stats.iter().find(|s| s.0 == Emotion::Calm).unwrap();
        assert!(
            light.1 > calm.1 * 1.5 && calm.1 > mel.1 * 1.2,
            "brightness {stats:?}"
        );
    }

    #[test]
    fn xy_changes_the_sound_without_clicks_and_chord_changes_crossfade() {
        let p = Arc::new(PadParams::default());
        chord(&p, &[196.0, 293.66, 392.0]);
        Emotion::Calm.preset().apply(&p);
        p.attack_s.set(0.3);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(
            0,
            Box::new(PadSource::new(Arc::clone(&p)).with_sample_rate(48_000)),
        );
        let _ = g.render_offline(1.0, 1);
        let mut all = Vec::new();
        for step in 0..20 {
            let x = step as f32 / 19.0;
            Emotion::Calm.preset().with_xy(x, 1.0 - x).apply(&p);
            if step == 10 {
                chord(&p, &[220.0, 329.63, 440.0, 554.37]);
            }
            all.extend_from_slice(&g.render_offline(0.15, 1).samples);
        }
        let steps = |v: &[f32]| {
            v.windows(2)
                .map(|w| (w[1] - w[0]).abs())
                .fold(0.0f32, f32::max)
        };
        let max_step = steps(&all);
        // A saw has steep edges by nature: a click is a step well above what the brightest
        // static setting produces on its own.
        let brightest = {
            let p2 = Arc::new(PadParams::default());
            chord(&p2, &[220.0, 329.63, 440.0, 554.37]);
            Emotion::Calm.preset().with_xy(1.0, 0.0).apply(&p2);
            p2.attack_s.set(0.05);
            steps(&render(&p2, 1.0).samples[24_000..])
        };
        assert!(
            max_step < brightest * 1.3 + 0.02,
            "no clicks across X/Y and chord changes: {max_step} vs static {brightest}"
        );
        let first = AudioBuffer::new(48_000, 1, all[..24_000].to_vec());
        let last = AudioBuffer::new(48_000, 1, all[all.len() - 24_000..].to_vec());
        assert!(
            (brightness(&first) - brightness(&last)).abs() > 0.02,
            "X moved the brightness"
        );
    }

    #[test]
    fn seed_reproduces_and_a_different_seed_differs() {
        let make = |seed: u32| {
            let p = Arc::new(PadParams::default());
            chord(&p, &[110.0, 164.81]);
            Emotion::Dark.preset().apply(&p);
            p.attack_s.set(0.02);
            p.seed.store(seed, Ordering::Relaxed);
            render(&p, 1.0)
        };
        let a = make(7);
        let b = make(7);
        let c = make(8);
        assert_eq!(a.samples, b.samples);
        assert!(a
            .samples
            .iter()
            .zip(&c.samples)
            .any(|(x, y)| (x - y).abs() > 1e-4));
    }
}
