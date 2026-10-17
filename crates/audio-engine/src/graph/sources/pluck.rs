//! Soft pluck (F1A.1/F1A.7): Karplus–Strong strings on notes from the pitch set,
//! triggered sparsely by the seeded process. Delay lines are allocated once for
//! the lowest playable note; the fractional read keeps the pitch true.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::{PitchSet, Trigger};
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;

pub const PLUCK_VOICES: usize = 6;
const MIN_HZ: f32 = 30.0;

pub struct PluckParams {
    pub pitches: PitchSet,
    /// Plucks per minute (0 = silent).
    pub density: SmoothedParam,
    /// 0..1: how quickly the string dies (0 = long sustain).
    pub damping: SmoothedParam,
    /// 0..1: tone of the pluck (0 = dull, 1 = bright).
    pub brightness: SmoothedParam,
    pub velocity_spread: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for PluckParams {
    fn default() -> Self {
        Self {
            pitches: PitchSet::default(),
            density: SmoothedParam::new(15.0, 0.0, 240.0),
            damping: SmoothedParam::new(0.3, 0.0, 1.0),
            brightness: SmoothedParam::new(0.5, 0.0, 1.0),
            velocity_spread: SmoothedParam::new(0.5, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

struct Str {
    buf: Vec<f32>,
    write: usize,
    delay: f32,
    prev: f32,
    amp: f32,
    energy: f32,
    active: bool,
}

pub struct PluckSource {
    params: Arc<PluckParams>,
    trigger: Trigger,
    seed_used: u32,
    density: Ramp,
    damping: Ramp,
    brightness: Ramp,
    spread: Ramp,
    strings: Vec<Str>,
    next_voice: usize,
}

impl PluckSource {
    pub fn new(params: Arc<PluckParams>, sample_rate: u32) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        let len = (sample_rate.max(8000) as f32 / MIN_HZ) as usize + 4;
        Self {
            trigger: Trigger::new(seed),
            seed_used: seed,
            density: Ramp::new(params.density.target()),
            damping: Ramp::new(params.damping.target()),
            brightness: Ramp::new(params.brightness.target()),
            spread: Ramp::new(params.velocity_spread.target()),
            strings: (0..PLUCK_VOICES)
                .map(|_| Str {
                    buf: vec![0.0; len],
                    write: 0,
                    delay: 100.0,
                    prev: 0.0,
                    amp: 0.0,
                    energy: 0.0,
                    active: false,
                })
                .collect(),
            next_voice: 0,
            params,
        }
    }

    fn pluck(&mut self, hz: f32, brightness: f32, spread: f32, sr: f32) {
        let i = (0..PLUCK_VOICES)
            .map(|k| (self.next_voice + k) % PLUCK_VOICES)
            .find(|&k| !self.strings[k].active)
            .unwrap_or(self.next_voice);
        self.next_voice = (i + 1) % PLUCK_VOICES;
        let rng = self.trigger.rng();
        let vel = 1.0 - spread * rng.unit() * 0.7;
        let s = &mut self.strings[i];
        let len = s.buf.len();
        // Delay minus the half-sample of the averaging filter keeps the pitch true.
        s.delay = (sr / hz.max(MIN_HZ) - 0.5).clamp(2.0, (len - 3) as f32);
        // Excite the loop with a noise burst, low-passed by (1 − brightness).
        let a = 0.15 + brightness * 0.85;
        let mut lp = 0.0f32;
        let fill = s.delay.ceil() as usize + 1;
        for k in 0..len {
            if k < fill {
                let w = rng.bipolar();
                lp += a * (w - lp);
                s.buf[k] = lp;
            } else {
                s.buf[k] = 0.0;
            }
        }
        s.write = fill % len;
        s.prev = 0.0;
        s.amp = 0.5 * vel;
        s.energy = 1.0;
        s.active = true;
    }
}

impl PluckSource {
    #[inline]
    fn render_span(&mut self, out: &mut [f32], p: &PluckParams, loss: f32) {
        for s in out.iter_mut() {
            let _ = self.density.next(p.density.target());
            let _ = self.damping.next(p.damping.target());
            let _ = self.brightness.next(p.brightness.target());
            let _ = self.spread.next(p.velocity_spread.target());
            let mut acc = 0.0f32;
            for st in self.strings.iter_mut() {
                if !st.active {
                    continue;
                }
                let len = st.buf.len();
                // Fractional read `delay` samples behind the write head.
                let rp = st.write as f32 - st.delay;
                let rp = if rp < 0.0 { rp + len as f32 } else { rp };
                let i0 = rp as usize % len;
                let i1 = (i0 + 1) % len;
                let frac = rp - rp.floor();
                let y = st.buf[i0] * (1.0 - frac) + st.buf[i1] * frac;
                // Two-point average = the string's loss filter, times the loop loss.
                let fed = undenormal(0.5 * (y + st.prev) * loss);
                st.prev = y;
                st.buf[st.write] = fed;
                st.write += 1;
                if st.write >= len {
                    st.write = 0;
                }
                acc += y * st.amp;
                // Track the energy cheaply to retire silent strings.
                st.energy = undenormal(st.energy * 0.9999 + y.abs() * 0.0001);
                if st.energy < 1e-5 {
                    st.active = false;
                }
            }
            *s = acc;
        }
    }
}

impl Source for PluckSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        // One refcount bump per block: lets the strike/reseed paths borrow `self` mutably.
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            self.trigger.reset(seed);
            self.seed_used = seed;
        }
        let ramp = ramp_samples(sample_rate);
        self.density.begin(p.density.target(), ramp);
        self.damping.begin(p.damping.target(), ramp);
        self.brightness.begin(p.brightness.target(), ramp);
        self.spread.begin(p.velocity_spread.target(), ramp);
        let n = out.len();
        let density = if p.pitches.is_empty() {
            0.0
        } else {
            self.density.current
        };
        let pluck_at = self.trigger.tick(n, density, sr);
        // Loop loss per sample: damping 0 → ~0.9995, 1 → ~0.98.
        let loss = 0.9996 - self.damping.current * 0.02;
        let at = pluck_at.unwrap_or(n).min(n);
        let (head, tail) = out.split_at_mut(at);
        self.render_span(head, &p, loss);
        if pluck_at.is_some() {
            if let Some(hz) = p.pitches.pick(self.trigger.rng()) {
                let (b, s) = (self.brightness.current, self.spread.current);
                self.pluck(hz, b, s, sr);
            }
        }
        self.render_span(tail, &p, loss);
    }

    fn reset(&mut self) {
        self.trigger.reset(self.seed_used);
        for s in self.strings.iter_mut() {
            s.buf.fill(0.0);
            s.active = false;
            s.energy = 0.0;
        }
        self.next_voice = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    fn render(set: impl Fn(&PluckParams), seconds: f64) -> AudioBuffer {
        let p = Arc::new(PluckParams::default());
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(PluckSource::new(p, 48_000)));
        g.render_offline(seconds, 1)
    }

    #[test]
    fn plucks_are_in_tune_across_the_register() {
        for hz in [110.0f32, 220.0, 440.0, 880.0] {
            let b = render(
                |p| {
                    p.pitches.set(&[hz]);
                    p.density.set(1.0);
                    p.damping.set(0.0);
                    p.brightness.set(1.0);
                    p.velocity_spread.set(0.0);
                },
                6.0,
            );
            assert!(b.samples.iter().all(|s| s.is_finite()));
            assert!(b.peak() <= 1.0, "peak {}", b.peak());
            let first = b
                .samples
                .iter()
                .position(|s| s.abs() > 0.01)
                .expect("a pluck") as f64
                / 48_000.0;
            let mut v: Vec<f64> = contour(&b, &PitchConfig::new(48_000), 0.02)
                .into_iter()
                .filter(|p| p.t > first + 0.1 && p.t < first + 1.0)
                .filter_map(|p| p.hz)
                .collect();
            assert!(!v.is_empty(), "pitch found at {hz}");
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let med = v[v.len() / 2];
            let cents = 1200.0 * (med / hz as f64).log2();
            assert!(cents.abs() < 8.0, "{hz} Hz → {med:.2} ({cents:.1} c)");
        }
    }

    #[test]
    fn damping_shortens_the_string_and_density_zero_is_silent() {
        let tail = |damping: f32| {
            let b = render(
                |p| {
                    p.pitches.set(&[330.0]);
                    p.density.set(1.0);
                    p.damping.set(damping);
                    p.velocity_spread.set(0.0);
                },
                6.0,
            );
            let first = b.samples.iter().position(|s| s.abs() > 0.01).unwrap();
            let late = first + 48_000; // one second after the pluck
            b.slice_frames(late, late + 4800).rms()
        };
        assert!(
            tail(0.0) > tail(1.0) * 3.0,
            "{} vs {}",
            tail(0.0),
            tail(1.0)
        );
        let quiet = render(
            |p| {
                p.pitches.set(&[330.0]);
                p.density.set(0.0);
            },
            3.0,
        );
        assert_eq!(quiet.peak(), 0.0);
    }

    #[test]
    fn seed_reproduces_the_phrase() {
        let mk = |seed: u32| {
            render(
                |p| {
                    p.pitches.set(&[196.0, 220.0, 261.6, 293.7]);
                    p.density.set(60.0);
                    p.seed.store(seed, Ordering::Relaxed);
                },
                8.0,
            )
        };
        assert_eq!(mk(3).samples, mk(3).samples);
        assert_ne!(mk(3).samples, mk(4).samples);
    }
}
