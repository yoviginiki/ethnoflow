//! Audio loop (F1A.1): plays a region of an immutable buffer round and round with an
//! equal-power crossfade at the seam, at a variable rate. The buffer is mono at the
//! stream rate (the caller resamples / reverses offline); the callback only reads.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use crate::buffer::AudioBuffer;
use crate::graph::params::{ramp_samples, Ramp, SmoothedParam};
use crate::graph::Source;

pub struct LoopParams {
    /// Playback rate (1 = original pitch and speed).
    pub rate: SmoothedParam,
    /// Loop region in frames of the buffer; end ≤ frames. end 0 = whole buffer.
    pub start_frames: AtomicU64,
    pub end_frames: AtomicU64,
    /// Crossfade length at the seam, frames (clamped to half the region).
    pub crossfade_frames: AtomicU32,
}

impl Default for LoopParams {
    fn default() -> Self {
        Self {
            rate: SmoothedParam::new(1.0, 0.25, 4.0),
            start_frames: AtomicU64::new(0),
            end_frames: AtomicU64::new(0),
            crossfade_frames: AtomicU32::new(2400),
        }
    }
}

pub struct LoopSource {
    params: Arc<LoopParams>,
    buf: Arc<AudioBuffer>,
    pos: f64,
    rate: Ramp,
}

impl LoopSource {
    /// `buf` must be mono (`to_mono`) at the graph's sample rate.
    pub fn new(params: Arc<LoopParams>, buf: Arc<AudioBuffer>) -> Self {
        let start = params.start_frames.load(Ordering::Relaxed) as f64;
        Self {
            rate: Ramp::new(params.rate.target()),
            pos: start,
            params,
            buf,
        }
    }

    #[inline]
    fn sample(&self, pos: f64) -> f32 {
        let n = self.buf.samples.len();
        if n == 0 {
            return 0.0;
        }
        let i = pos.floor();
        let frac = (pos - i) as f32;
        let i0 = (i as usize).min(n - 1);
        let i1 = (i0 + 1).min(n - 1);
        self.buf.samples[i0] * (1.0 - frac) + self.buf.samples[i1] * frac
    }
}

impl Source for LoopSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let p = &self.params;
        let frames = self.buf.samples.len() as u64;
        if frames < 2 {
            out.fill(0.0);
            return;
        }
        let start = p.start_frames.load(Ordering::Relaxed).min(frames - 1);
        let mut end = p.end_frames.load(Ordering::Relaxed);
        if end == 0 || end > frames {
            end = frames;
        }
        if end <= start + 1 {
            out.fill(0.0);
            return;
        }
        let len = (end - start) as f64;
        let xf = (p.crossfade_frames.load(Ordering::Relaxed) as f64).min(len * 0.5);
        self.rate.begin(p.rate.target(), ramp_samples(sample_rate));
        let (s, e) = (start as f64, end as f64);
        if self.pos < s || self.pos >= e {
            self.pos = s;
        }
        for o in out.iter_mut() {
            let rate = self.rate.next(p.rate.target()) as f64;
            let mut y = self.sample(self.pos);
            // Inside the last `xf` frames: fade towards the region's start.
            let remaining = e - self.pos;
            if xf > 0.0 && remaining < xf {
                let t = (1.0 - remaining / xf) as f32; // 0 → 1 across the fade
                let theta = t * std::f32::consts::FRAC_PI_2;
                let from_start = self.sample(self.pos - len);
                y = y * theta.cos() + from_start * theta.sin();
            }
            *o = y;
            self.pos += rate;
            if self.pos >= e {
                self.pos -= len;
            }
        }
    }

    fn reset(&mut self) {
        self.pos = self.params.start_frames.load(Ordering::Relaxed) as f64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    fn render(buf: AudioBuffer, set: impl Fn(&LoopParams), seconds: f64) -> AudioBuffer {
        let p = Arc::new(LoopParams::default());
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(LoopSource::new(p, Arc::new(buf))));
        g.render_offline(seconds, 1)
    }

    #[test]
    fn seam_is_crossfaded_and_the_loop_keeps_going() {
        // A sine whose period does not divide the loop length: a hard seam would click.
        let src = AudioBuffer::sine(48_000, 333.3, 1.0, 0.5);
        let b = render(
            src,
            |p| p.crossfade_frames.store(2400, Ordering::Relaxed),
            3.5,
        );
        assert!(b.samples.iter().all(|s| s.is_finite()));
        assert!(
            b.slice_frames(3 * 48_000, 3 * 48_000 + 4800).rms() > 0.2,
            "still playing"
        );
        let max_step = b
            .samples
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        // 333 Hz at 0.5 moves ≤ 0.022/sample; a seam click would be up to 1.0.
        assert!(max_step < 0.05, "no click at the seam: {max_step}");
    }

    #[test]
    fn region_and_rate_are_respected() {
        let mut samples = vec![0.0f32; 48_000];
        for (i, s) in samples.iter_mut().enumerate().take(24_000).skip(12_000) {
            *s = (i as f32 * 220.0 / 48_000.0 * std::f32::consts::TAU).sin() * 0.5;
        }
        let src = AudioBuffer::new(48_000, 1, samples);
        // Only the sounding quarter: the output never goes silent for long.
        let b = render(
            src.clone(),
            |p| {
                p.start_frames.store(12_000, Ordering::Relaxed);
                p.end_frames.store(24_000, Ordering::Relaxed);
                p.crossfade_frames.store(480, Ordering::Relaxed);
            },
            2.0,
        );
        for c in b.samples[4800..].chunks(2400) {
            let rms = (c.iter().map(|x| x * x).sum::<f32>() / c.len() as f32).sqrt();
            assert!(rms > 0.2, "region playing continuously: {rms}");
        }
        // Rate 2 doubles the pitch.
        let fast = render(
            src,
            |p| {
                p.start_frames.store(12_000, Ordering::Relaxed);
                p.end_frames.store(24_000, Ordering::Relaxed);
                p.rate.set(2.0);
            },
            2.0,
        );
        let mut v: Vec<f64> = contour(&fast, &PitchConfig::new(48_000), 0.02)
            .into_iter()
            .filter(|p| p.t > 0.3)
            .filter_map(|p| p.hz)
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = v[v.len() / 2];
        assert!((med - 440.0).abs() < 5.0, "{med}");
    }

    #[test]
    fn empty_or_degenerate_region_is_silent() {
        let b = render(AudioBuffer::empty(48_000, 1), |_| {}, 0.1);
        assert_eq!(b.peak(), 0.0);
        let b = render(
            AudioBuffer::sine(48_000, 100.0, 0.5, 0.5),
            |p| {
                p.start_frames.store(1000, Ordering::Relaxed);
                p.end_frames.store(1000, Ordering::Relaxed);
            },
            0.1,
        );
        assert_eq!(b.peak(), 0.0);
    }
}
