//! Granular cloud (F1B.2): short windowed grains of an immutable buffer, scheduled
//! `density` times per second around a read `position` (spread by `spray`), each
//! `grain_ms` long with a Hann window, transposed by `pitch` semitones (± `jitter`
//! cents), sometimes played backwards or an octave up. `scan` drifts the read position
//! through the buffer (0 = frozen on one moment: a breath of a kaval becomes a pad).
//! The buffer is mono at the stream rate; `render` only reads it and never allocates.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use crate::buffer::AudioBuffer;
use crate::graph::params::SmoothedParam;
use crate::graph::sources::gen::Rng;
use crate::graph::Source;

pub const MAX_GRAINS: usize = 48;

pub struct GrainParams {
    /// Grain length, ms (10..500).
    pub grain_ms: SmoothedParam,
    /// Grains per second (0.5..80).
    pub density: SmoothedParam,
    /// Read position, 0..1 of the buffer.
    pub position: SmoothedParam,
    /// Random spread of the start around the position, 0..1 of the buffer.
    pub spray: SmoothedParam,
    /// Drift of the position through the buffer, × real time (0 = frozen, 1 = as recorded).
    pub scan: SmoothedParam,
    /// Transposition, semitones (−24..24).
    pub pitch: SmoothedParam,
    /// Random detune per grain, cents (0..100).
    pub jitter: SmoothedParam,
    /// Probability a grain plays backwards (0..1).
    pub reverse: SmoothedParam,
    /// Probability a grain plays an octave up (0..1).
    pub octave: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for GrainParams {
    fn default() -> Self {
        Self {
            grain_ms: SmoothedParam::new(120.0, 10.0, 500.0),
            density: SmoothedParam::new(20.0, 0.5, 80.0),
            position: SmoothedParam::new(0.3, 0.0, 1.0),
            spray: SmoothedParam::new(0.1, 0.0, 1.0),
            scan: SmoothedParam::new(0.0, 0.0, 2.0),
            pitch: SmoothedParam::new(0.0, -24.0, 24.0),
            jitter: SmoothedParam::new(10.0, 0.0, 100.0),
            reverse: SmoothedParam::new(0.2, 0.0, 1.0),
            octave: SmoothedParam::new(0.1, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

#[derive(Clone, Copy)]
struct Grain {
    active: bool,
    /// Read position, frames (fractional); wraps around the buffer.
    pos: f64,
    /// Frames advanced per output frame; negative = backwards.
    rate: f64,
    len: u32,
    age: u32,
    amp: f32,
}

const IDLE: Grain = Grain {
    active: false,
    pos: 0.0,
    rate: 1.0,
    len: 1,
    age: 0,
    amp: 0.0,
};

pub struct GrainSource {
    params: Arc<GrainParams>,
    buf: Arc<AudioBuffer>,
    grains: [Grain; MAX_GRAINS],
    rng: Rng,
    seed: u32,
    /// Frames until the next grain starts.
    next_in: f64,
    /// Scan offset, frames, added to the position.
    head: f64,
}

impl GrainSource {
    /// `buf` must be mono (`to_mono`) at the graph's sample rate.
    pub fn new(params: Arc<GrainParams>, buf: Arc<AudioBuffer>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        Self {
            params,
            buf,
            grains: [IDLE; MAX_GRAINS],
            rng: Rng::new(seed.wrapping_mul(747_796_405).wrapping_add(11)),
            seed,
            next_in: 0.0,
            head: 0.0,
        }
    }
}

/// Linear interpolation, wrapping around the buffer.
#[inline]
fn sample_buf(buf: &AudioBuffer, pos: f64) -> f32 {
    {
        let n = buf.samples.len();
        let nf = n as f64;
        let mut p = pos % nf;
        if p < 0.0 {
            p += nf;
        }
        let i = p.floor();
        let frac = (p - i) as f32;
        let i0 = (i as usize).min(n - 1);
        let i1 = (i0 + 1) % n;
        buf.samples[i0] * (1.0 - frac) + buf.samples[i1] * frac
    }
}

impl GrainSource {
    fn spawn(&mut self, sample_rate: f32, n: usize) {
        let p = Arc::clone(&self.params);
        let slot = match self.grains.iter().position(|g| !g.active) {
            Some(i) => i,
            None => return, // all voices busy: skip this grain, no allocation
        };
        let nf = n as f64;
        let grain_s = p.grain_ms.target() / 1000.0;
        let len = ((grain_s * sample_rate) as u32).clamp(32, (n as u32).saturating_sub(1).max(32));
        let center = (p.position.target() as f64 * nf + self.head).rem_euclid(nf);
        let spray = p.spray.target() as f64 * nf;
        let mut start = (center + self.rng.bipolar() as f64 * spray).rem_euclid(nf);
        let cents = self.rng.bipolar() * p.jitter.target();
        let mut rate = 2f64.powf((p.pitch.target() + cents / 100.0) as f64 / 12.0);
        if self.rng.unit() < p.octave.target() {
            rate *= 2.0;
        }
        if self.rng.unit() < p.reverse.target() {
            // Start at the grain's end and read backwards through the same material.
            start = (start + len as f64 * rate).rem_euclid(nf);
            rate = -rate;
        }
        // Equal loudness across densities: grains overlap `density × length` deep.
        let overlap = (p.density.target() * grain_s).max(1.0);
        let amp = 0.9 / overlap.sqrt();
        self.grains[slot] = Grain {
            active: true,
            pos: start,
            rate,
            len,
            age: 0,
            amp,
        };
    }
}

impl Source for GrainSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let n = self.buf.samples.len();
        if n < 64 {
            out.fill(0.0);
            return;
        }
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed {
            self.seed = seed;
            self.rng = Rng::new(seed.wrapping_mul(747_796_405).wrapping_add(11));
        }
        let sr = sample_rate as f32;
        let density = p.density.target().max(0.01);
        let interval = sr as f64 / density as f64;
        let scan = p.scan.target() as f64;
        let nf = n as f64;
        let buf = Arc::clone(&self.buf);
        for o in out.iter_mut() {
            // Scheduler: one grain every `interval` frames, ±20 % so the cloud breathes.
            self.next_in -= 1.0;
            if self.next_in <= 0.0 {
                self.spawn(sr, n);
                self.next_in += interval * (0.8 + 0.4 * self.rng.unit() as f64);
            }
            self.head = (self.head + scan).rem_euclid(nf);
            let mut y = 0.0f32;
            for g in self.grains.iter_mut() {
                if !g.active {
                    continue;
                }
                let t = g.age as f32 / g.len as f32;
                // Hann window: no clicks at either edge.
                let w = 0.5 - 0.5 * (t * std::f32::consts::TAU).cos();
                y += sample_buf(&buf, g.pos) * w * g.amp;
                g.pos += g.rate;
                g.age += 1;
                if g.age >= g.len {
                    g.active = false;
                }
            }
            *o = y;
        }
    }

    fn reset(&mut self) {
        self.grains = [IDLE; MAX_GRAINS];
        self.next_in = 0.0;
        self.head = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, GraphParams};

    fn render(set: impl Fn(&GrainParams), seconds: f64, block: usize) -> AudioBuffer {
        let src = AudioBuffer::sine(48_000, 220.0, 2.0, 0.5);
        let p = Arc::new(GrainParams::default());
        p.jitter.set(0.0);
        p.reverse.set(0.0);
        p.octave.set(0.0);
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(GrainSource::new(p, Arc::new(src))));
        let frames = (seconds * 48_000.0) as usize;
        let mut out = vec![0.0f32; frames];
        for chunk in out.chunks_mut(block) {
            g.render(chunk, 1);
        }
        AudioBuffer::new(48_000, 1, out)
    }

    fn zero_crossings_per_second(b: &AudioBuffer, from_s: f64, to_s: f64) -> f64 {
        let a = (from_s * 48_000.0) as usize;
        let z = (to_s * 48_000.0) as usize;
        let s = &b.samples[a..z];
        let crossings = s
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        crossings as f64 / (to_s - from_s)
    }

    #[test]
    fn a_dense_cloud_is_continuous_and_never_clips() {
        let b = render(
            |p| {
                p.density.set(30.0);
                p.grain_ms.set(150.0);
            },
            6.0,
            256,
        );
        assert!(b.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        // After the master ramp, no 50 ms window is silent.
        for (i, w) in b.samples[48_000..].chunks(2400).enumerate() {
            let rms = (w.iter().map(|x| x * x).sum::<f32>() / w.len() as f32).sqrt();
            assert!(rms > 0.03, "window {i} rms {rms}");
        }
    }

    #[test]
    fn pitch_up_an_octave_doubles_the_frequency() {
        let base = render(
            |p| {
                p.density.set(40.0);
                p.grain_ms.set(200.0);
            },
            4.0,
            256,
        );
        let up = render(
            |p| {
                p.density.set(40.0);
                p.grain_ms.set(200.0);
                p.pitch.set(12.0);
            },
            4.0,
            256,
        );
        let z0 = zero_crossings_per_second(&base, 1.0, 4.0);
        let z1 = zero_crossings_per_second(&up, 1.0, 4.0);
        // 220 Hz → ~440 crossings/s; +12 st → ~880.
        assert!((z0 - 440.0).abs() < 60.0, "base {z0}");
        assert!((z1 - 880.0).abs() < 120.0, "octave up {z1}");
    }

    #[test]
    fn density_changes_how_often_the_cloud_falls_silent() {
        let sparse = render(
            |p| {
                p.density.set(2.0);
                p.grain_ms.set(60.0);
            },
            6.0,
            256,
        );
        let dense = render(
            |p| {
                p.density.set(40.0);
                p.grain_ms.set(60.0);
            },
            6.0,
            256,
        );
        let silent = |b: &AudioBuffer| {
            b.samples[48_000..]
                .chunks(480)
                .filter(|w| w.iter().all(|x| x.abs() < 1e-4))
                .count()
        };
        assert!(
            silent(&sparse) > 100,
            "sparse has gaps: {}",
            silent(&sparse)
        );
        assert_eq!(silent(&dense), 0, "dense has no gaps");
    }

    #[test]
    fn output_does_not_depend_on_the_block_size() {
        let a = render(|p| p.density.set(25.0), 2.0, 64);
        let b = render(|p| p.density.set(25.0), 2.0, 480);
        let diff = a
            .samples
            .iter()
            .zip(&b.samples)
            .map(|(x, y)| (x - y).abs())
            .fold(0.0f32, f32::max);
        assert!(diff < 1e-5, "block-size dependence: {diff}");
    }
}
