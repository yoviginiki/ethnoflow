//! Audio graph (FEATURE_SPEC F1.6): fixed layer slots → per-layer gain/pan/filter →
//! master limiter + meters → device. Sources are trait objects owned by the callback;
//! all slots are pre-allocated, parameters are smoothed, nothing allocates while
//! rendering. The same `Graph::render` serves the device callback and offline
//! rendering, so a bounce equals what was heard.

pub mod limiter;
pub mod params;

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;
use limiter::Limiter;
use params::{db_to_gain, ramp_samples, undenormal, Ramp, SmoothedParam};

pub const MAX_LAYERS: usize = 16;
/// Frames rendered per inner block (parameter ramps are recomputed per block).
pub const BLOCK: usize = 64;

/// A sound source: fills `out` (mono, `out.len()` frames) with its signal.
/// Runs in the audio callback: no allocation, no locks, no I/O.
pub trait Source: Send {
    fn render(&mut self, out: &mut [f32], sample_rate: u32);
    /// Called when the graph (re)starts, e.g. to reset phases.
    fn reset(&mut self) {}
}

/// Silence; what an empty slot holds.
pub struct Silence;
impl Source for Silence {
    fn render(&mut self, out: &mut [f32], _sample_rate: u32) {
        out.fill(0.0);
    }
}

/// Parameters of one layer, settable from any thread.
pub struct LayerParams {
    pub gain_db: SmoothedParam,
    /// −1 (left) … +1 (right).
    pub pan: SmoothedParam,
    /// One-pole low-pass cutoff in Hz; 20 kHz = effectively off.
    pub lowpass_hz: SmoothedParam,
    /// One-pole high-pass cutoff in Hz; 10 Hz = effectively off.
    pub highpass_hz: SmoothedParam,
    pub mute: AtomicBool,
    pub solo: AtomicBool,
    /// Peak of the last block (f32 bits), for meters.
    peak_bits: AtomicU32,
}

impl Default for LayerParams {
    fn default() -> Self {
        Self {
            gain_db: SmoothedParam::new(-120.0, -120.0, 12.0),
            pan: SmoothedParam::new(0.0, -1.0, 1.0),
            lowpass_hz: SmoothedParam::new(20_000.0, 40.0, 20_000.0),
            highpass_hz: SmoothedParam::new(10.0, 10.0, 8_000.0),
            mute: AtomicBool::new(false),
            solo: AtomicBool::new(false),
            peak_bits: AtomicU32::new(0),
        }
    }
}

impl LayerParams {
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak_bits.load(Ordering::Relaxed))
    }
}

pub struct MasterParams {
    pub gain_db: SmoothedParam,
    peak_bits: AtomicU32,
    limiter_gain_bits: AtomicU32,
}

impl Default for MasterParams {
    fn default() -> Self {
        Self {
            // Headphone safety (F1A.2): start at −18 dB.
            gain_db: SmoothedParam::new(-18.0, -120.0, 6.0),
            peak_bits: AtomicU32::new(0),
            limiter_gain_bits: AtomicU32::new(1.0f32.to_bits()),
        }
    }
}

impl MasterParams {
    pub fn peak(&self) -> f32 {
        f32::from_bits(self.peak_bits.load(Ordering::Relaxed))
    }
    pub fn limiter_gain(&self) -> f32 {
        f32::from_bits(self.limiter_gain_bits.load(Ordering::Relaxed))
    }
}

/// Shared handle: the UI holds this, the graph reads it.
pub struct GraphParams {
    pub layers: [LayerParams; MAX_LAYERS],
    pub master: MasterParams,
}

impl Default for GraphParams {
    fn default() -> Self {
        Self {
            layers: std::array::from_fn(|_| LayerParams::default()),
            master: MasterParams::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphMeters {
    pub layers: Vec<f32>,
    pub master_peak: f32,
    pub limiter_gain: f32,
}

impl GraphParams {
    pub fn meters(&self, active_layers: usize) -> GraphMeters {
        GraphMeters {
            layers: self
                .layers
                .iter()
                .take(active_layers)
                .map(|l| l.peak())
                .collect(),
            master_peak: self.master.peak(),
            limiter_gain: self.master.limiter_gain(),
        }
    }
}

struct LayerState {
    source: Box<dyn Source>,
    gain: Ramp,
    pan: Ramp,
    lp: Ramp,
    hp: Ramp,
    lp_state: f32,
    hp_state_in: f32,
    hp_state_out: f32,
    mono: [f32; BLOCK],
}

/// The renderer. Lives in the audio callback (or in an offline loop).
pub struct Graph {
    params: Arc<GraphParams>,
    layers: Vec<LayerState>,
    sample_rate: u32,
    master_gain: Ramp,
    limiter: Limiter,
    ramp: f32,
}

impl Graph {
    pub fn new(params: Arc<GraphParams>, sample_rate: u32) -> Self {
        let layers = (0..MAX_LAYERS)
            .map(|i| {
                let p = &params.layers[i];
                LayerState {
                    source: Box::new(Silence),
                    gain: Ramp::new(db_to_gain(p.gain_db.target())),
                    pan: Ramp::new(p.pan.target()),
                    lp: Ramp::new(p.lowpass_hz.target()),
                    hp: Ramp::new(p.highpass_hz.target()),
                    lp_state: 0.0,
                    hp_state_in: 0.0,
                    hp_state_out: 0.0,
                    mono: [0.0; BLOCK],
                }
            })
            .collect();
        Self {
            master_gain: Ramp::new(db_to_gain(params.master.gain_db.target())),
            limiter: Limiter::new(sample_rate, db_to_gain(-0.3), 0.08),
            ramp: ramp_samples(sample_rate),
            params,
            layers,
            sample_rate,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Put a source into a slot (call before the graph is handed to the callback, or
    /// through a command the callback applies).
    pub fn set_source(&mut self, slot: usize, source: Box<dyn Source>) {
        if slot < MAX_LAYERS {
            self.layers[slot].source = source;
            self.layers[slot].lp_state = 0.0;
            self.layers[slot].hp_state_in = 0.0;
            self.layers[slot].hp_state_out = 0.0;
        }
    }

    pub fn clear_source(&mut self, slot: usize) {
        self.set_source(slot, Box::new(Silence));
    }

    pub fn reset(&mut self) {
        for l in self.layers.iter_mut() {
            l.source.reset();
        }
    }

    /// Render `out` (interleaved, `channels` per frame). Allocation-free.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        let channels = channels.max(1);
        let frames = out.len() / channels;
        let any_solo = self
            .params
            .layers
            .iter()
            .any(|l| l.solo.load(Ordering::Relaxed));
        let sr = self.sample_rate as f32;
        let mut done = 0;
        while done < frames {
            let n = (frames - done).min(BLOCK);
            let block = &mut out[done * channels..(done + n) * channels];
            block.fill(0.0);
            for (i, layer) in self.layers.iter_mut().enumerate() {
                let p = &self.params.layers[i];
                let muted =
                    p.mute.load(Ordering::Relaxed) || (any_solo && !p.solo.load(Ordering::Relaxed));
                let target_gain = if muted {
                    0.0
                } else {
                    db_to_gain(p.gain_db.target())
                };
                // Skip silent, settled layers entirely (keeps CPU low with empty slots).
                if target_gain == 0.0 && layer.gain.current == 0.0 {
                    p.peak_bits.store(0, Ordering::Relaxed);
                    continue;
                }
                layer.gain.begin(target_gain, self.ramp);
                layer.pan.begin(p.pan.target(), self.ramp);
                layer.lp.begin(p.lowpass_hz.target(), self.ramp);
                layer.hp.begin(p.highpass_hz.target(), self.ramp);
                let mono = &mut layer.mono[..n];
                layer.source.render(mono, self.sample_rate);
                let mut peak = 0.0f32;
                for (k, frame) in block.chunks_mut(channels).enumerate() {
                    let g = layer.gain.next(target_gain);
                    let pan = layer.pan.next(p.pan.target());
                    let lp_hz = layer.lp.next(p.lowpass_hz.target());
                    let hp_hz = layer.hp.next(p.highpass_hz.target());
                    // One-pole filters (cheap, stable): coefficient from cutoff.
                    let a_lp = 1.0 - (-2.0 * std::f32::consts::PI * lp_hz / sr).exp();
                    let a_hp = (-2.0 * std::f32::consts::PI * hp_hz / sr).exp();
                    let x = mono[k];
                    layer.lp_state = undenormal(layer.lp_state + a_lp * (x - layer.lp_state));
                    let y_lp = layer.lp_state;
                    layer.hp_state_out =
                        undenormal(a_hp * (layer.hp_state_out + y_lp - layer.hp_state_in));
                    layer.hp_state_in = y_lp;
                    let y = layer.hp_state_out * g;
                    peak = peak.max(y.abs());
                    // Constant-power pan.
                    let theta = (pan + 1.0) * 0.25 * std::f32::consts::PI;
                    let (l, r) = (theta.cos(), theta.sin());
                    if channels == 1 {
                        frame[0] += y;
                    } else {
                        frame[0] += y * l * std::f32::consts::SQRT_2;
                        frame[1] += y * r * std::f32::consts::SQRT_2;
                        for ch in frame.iter_mut().skip(2) {
                            *ch += y;
                        }
                    }
                }
                p.peak_bits.store(peak.to_bits(), Ordering::Relaxed);
            }
            // Master: gain ramp, limiter, meter.
            let target_master = db_to_gain(self.params.master.gain_db.target());
            self.master_gain.begin(target_master, self.ramp);
            for frame in block.chunks_mut(channels) {
                let g = self.master_gain.next(target_master);
                for s in frame.iter_mut() {
                    *s *= g;
                }
                self.limiter.process_frame(frame);
            }
            self.params
                .master
                .peak_bits
                .store(self.limiter.peak().to_bits(), Ordering::Relaxed);
            self.params
                .master
                .limiter_gain_bits
                .store(self.limiter.gain_reduction().to_bits(), Ordering::Relaxed);
            done += n;
        }
    }

    /// Offline render of `seconds` (F1.6.3): same code path as the live callback.
    pub fn render_offline(&mut self, seconds: f64, channels: u16) -> AudioBuffer {
        let ch = channels.max(1) as usize;
        let frames = (seconds * self.sample_rate as f64).round() as usize;
        let mut out = vec![0.0f32; frames * ch];
        // Same chunking as a typical device callback so smoothing behaves identically.
        for chunk in out.chunks_mut(512 * ch) {
            self.render(chunk, ch);
        }
        AudioBuffer::new(self.sample_rate, channels.max(1), out)
    }
}

/// Test/utility source: a sine at a fixed frequency and amplitude.
pub struct SineSource {
    pub hz: f32,
    pub amp: f32,
    phase: f32,
}

impl SineSource {
    pub fn new(hz: f32, amp: f32) -> Self {
        Self {
            hz,
            amp,
            phase: 0.0,
        }
    }
}

impl Source for SineSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let inc = self.hz / sample_rate.max(1) as f32;
        for s in out.iter_mut() {
            *s = self.amp * (self.phase * std::f32::consts::TAU).sin();
            self.phase += inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
    fn reset(&mut self) {
        self.phase = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_with_two_sines() -> (Arc<GraphParams>, Graph) {
        let p = Arc::new(GraphParams::default());
        p.master.gain_db.set(0.0);
        let mut g = Graph::new(Arc::clone(&p), 48_000);
        g.set_source(0, Box::new(SineSource::new(220.0, 0.25)));
        g.set_source(1, Box::new(SineSource::new(330.0, 0.25)));
        p.layers[0].gain_db.set(0.0);
        p.layers[1].gain_db.set(0.0);
        (p, g)
    }

    fn rms(b: &AudioBuffer, from: f64, to: f64) -> f32 {
        let sr = b.sample_rate as f64;
        b.slice_frames((from * sr) as usize, (to * sr) as usize)
            .rms()
    }

    #[test]
    fn sum_mute_solo_and_pan() {
        let (p, mut g) = graph_with_two_sines();
        let both = g.render_offline(0.5, 2);
        let r_both = rms(&both, 0.1, 0.5);
        p.layers[1].mute.store(true, Ordering::Relaxed);
        let one = g.render_offline(0.5, 2);
        let r_one = rms(&one, 0.1, 0.5);
        assert!(
            r_one < r_both * 0.8 && r_one > r_both * 0.5,
            "mute halves the power: {r_one} vs {r_both}"
        );
        p.layers[1].mute.store(false, Ordering::Relaxed);
        p.layers[0].solo.store(true, Ordering::Relaxed);
        let solo = g.render_offline(0.5, 2);
        assert!(
            (rms(&solo, 0.1, 0.5) - r_one).abs() < 0.01,
            "solo on 0 == mute of 1"
        );
        p.layers[0].solo.store(false, Ordering::Relaxed);
        // Hard left pan: right channel silent.
        p.layers[1].mute.store(true, Ordering::Relaxed);
        p.layers[0].pan.set(-1.0);
        let left = g.render_offline(0.5, 2);
        let (mut l, mut r) = (0.0f32, 0.0f32);
        for f in left.samples[24_000..].chunks(2) {
            l = l.max(f[0].abs());
            r = r.max(f[1].abs());
        }
        assert!(l > 0.2 && r < 1e-3, "l {l} r {r}");
    }

    #[test]
    fn gain_ramp_has_no_jumps_above_one_db() {
        let (p, mut g) = graph_with_two_sines();
        p.layers[1].mute.store(true, Ordering::Relaxed);
        p.layers[0].gain_db.set(-120.0);
        g.render_offline(0.1, 1); // settle at silence
        p.layers[0].gain_db.set(0.0); // jump the target: the ramp does the work
        let b = g.render_offline(0.1, 1);
        // A 220 Hz sine at 0.25 moves at most 0.0072 per sample; the 10 ms ramp adds
        // 0.25/480. Any zipper step (e.g. per-block gain updates) would be far larger.
        let max_step = b
            .samples
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(max_step < 0.01, "sample-to-sample jump {max_step}");
        // And the ramp did happen: quiet at the start, full level after 10 ms.
        assert!(b.slice_frames(0, 48).peak() < 0.05);
        assert!(rms(&b, 0.05, 0.1) > 0.1);
    }

    #[test]
    fn no_nans_or_denormals_and_offline_is_deterministic() {
        let (p, mut g) = graph_with_two_sines();
        p.layers[0].lowpass_hz.set(300.0);
        p.layers[1].highpass_hz.set(500.0);
        let a = g.render_offline(1.0, 2);
        assert!(a.samples.iter().all(|s| s.is_finite()));
        g.reset();
        let (_, mut g2) = graph_with_two_sines();
        let p2 = Arc::clone(&g2.params);
        p2.layers[0].lowpass_hz.set(300.0);
        p2.layers[1].highpass_hz.set(500.0);
        let b = g2.render_offline(1.0, 2);
        assert_eq!(
            a.samples, b.samples,
            "same params and sources → identical bytes"
        );
    }

    #[test]
    fn limiter_holds_the_master_ceiling_and_meters_report() {
        let (p, mut g) = graph_with_two_sines();
        p.layers[0].gain_db.set(12.0);
        p.layers[1].gain_db.set(12.0);
        p.master.gain_db.set(6.0);
        let b = g.render_offline(0.5, 2);
        assert!(b.peak() <= db_to_gain(-0.3) + 1e-5, "peak {}", b.peak());
        let m = p.meters(2);
        assert_eq!(m.layers.len(), 2);
        assert!(m.layers[0] > 0.0 && m.master_peak > 0.0 && m.limiter_gain < 1.0);
    }
}
