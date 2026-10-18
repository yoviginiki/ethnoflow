//! Audio graph (FEATURE_SPEC F1.6): fixed layer slots → per-layer gain/pan/filter →
//! master limiter + meters → device. Sources are trait objects owned by the callback;
//! all slots are pre-allocated, parameters are smoothed, nothing allocates while
//! rendering. The same `Graph::render` serves the device callback and offline
//! rendering, so a bounce equals what was heard.

pub mod limiter;
pub mod params;
pub mod player;
pub mod sources;

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;
use limiter::Limiter;
use params::{db_to_gain, ramp_samples, undenormal, Ramp, SmoothedParam};
use sources::Reverb;

/// 0 drone, 1 pad, 2 clips, 3 audition, 4..11 atmosphere, 12..15 rhythm parts,
/// 16..23 the Instrument Rack's extra parts (tapan, chord pad, lead …).
pub const MAX_LAYERS: usize = 24;
/// Frames rendered per inner block (parameter ramps are recomputed per block).
pub const BLOCK: usize = 64;

/// A sound source: fills `out` (mono, `out.len()` frames) with its signal.
/// Runs in the audio callback: no allocation, no locks, no I/O.
pub trait Source: Send {
    fn render(&mut self, out: &mut [f32], sample_rate: u32);
    /// Like `render`, told the graph frame the block starts at. Rhythmic sources use
    /// it to derive a shared beat position; the default ignores it.
    fn render_at(&mut self, out: &mut [f32], sample_rate: u32, _frame: u64) {
        self.render(out, sample_rate);
    }
    /// Called when the graph (re)starts, e.g. to reset phases.
    fn reset(&mut self) {}
    /// Stereo render with a reverb send (F5.2 track strips): `left`/`right` are the
    /// source's own stereo image (the layer's pan then acts as a balance), `send` is a
    /// mono signal for the master space on top of the global "space" amount. Returns
    /// false when the source is mono — the graph then uses `render_at`.
    fn render_stereo(
        &mut self,
        _left: &mut [f32],
        _right: &mut [f32],
        _send: &mut [f32],
        _sample_rate: u32,
        _frame: u64,
    ) -> bool {
        false
    }
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
    /// Length of the gain ramp towards a new target, ms; 0 = the default 20 ms.
    /// Long values make switch-on / switch-off a fade (background mixer).
    pub fade_ms: AtomicU32,
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
            fade_ms: AtomicU32::new(0),
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
    /// 0..1 reverb mix on the master ("space", F1A.2); 0 = the reverb is bypassed.
    pub space: SmoothedParam,
    /// 0..1 reverb size.
    pub space_size: SmoothedParam,
    /// 0..1 shimmer: how much of the reverb's output is fed back pitch-shifted (F1B.3).
    pub shimmer: SmoothedParam,
    /// Shimmer transposition, semitones (0..24; 12 = octave, 7 = fifth, 19 = octave+fifth).
    pub shimmer_st: SmoothedParam,
    peak_bits: AtomicU32,
    limiter_gain_bits: AtomicU32,
}

impl Default for MasterParams {
    fn default() -> Self {
        Self {
            // Headphone safety (F1A.2): start at −18 dB.
            gain_db: SmoothedParam::new(-18.0, -120.0, 6.0),
            space: SmoothedParam::new(0.0, 0.0, 1.0),
            space_size: SmoothedParam::new(0.6, 0.0, 1.0),
            shimmer: SmoothedParam::new(0.0, 0.0, 1.0),
            shimmer_st: SmoothedParam::new(12.0, 0.0, 24.0),
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
    /// Frames rendered so far (written by the graph), for beat clocks.
    pub frame: AtomicU64,
}

impl Default for GraphParams {
    fn default() -> Self {
        Self {
            layers: std::array::from_fn(|_| LayerParams::default()),
            master: MasterParams::default(),
            frame: AtomicU64::new(0),
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
    /// Right-channel filter states (stereo sources).
    lp_state_r: f32,
    hp_state_in_r: f32,
    hp_state_out_r: f32,
    mono: [f32; BLOCK],
    right: [f32; BLOCK],
    send: [f32; BLOCK],
}

/// The renderer. Lives in the audio callback (or in an offline loop).
pub struct Graph {
    params: Arc<GraphParams>,
    layers: Vec<LayerState>,
    sample_rate: u32,
    master_gain: Ramp,
    space: Ramp,
    /// One reverb per channel (slightly different sizes for width); mono uses the first.
    reverb: [Reverb; 2],
    /// Frames of tail still to render after the mix went to zero.
    reverb_tail: u32,
    /// Shimmer: pitch shifters in the reverb feedback, one per channel, and the last wet sample.
    shimmer: [sources::PitchShifter; 2],
    shimmer_fb: [f32; 2],
    limiter: Limiter,
    ramp: f32,
    /// Frames rendered so far.
    frame: u64,
    /// Sum of the layers' reverb sends for the current block (post-fader, mono).
    send_bus: [f32; BLOCK],
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
                    lp_state_r: 0.0,
                    hp_state_in_r: 0.0,
                    hp_state_out_r: 0.0,
                    mono: [0.0; BLOCK],
                    right: [0.0; BLOCK],
                    send: [0.0; BLOCK],
                }
            })
            .collect();
        Self {
            master_gain: Ramp::new(db_to_gain(params.master.gain_db.target())),
            space: Ramp::new(params.master.space.target()),
            reverb: [
                Reverb::new(sample_rate),
                Reverb::new(sample_rate + sample_rate / 40),
            ],
            reverb_tail: 0,
            shimmer: [
                sources::PitchShifter::new(sample_rate),
                sources::PitchShifter::new(sample_rate),
            ],
            shimmer_fb: [0.0; 2],
            limiter: Limiter::new(sample_rate, db_to_gain(-0.3), 0.08),
            ramp: ramp_samples(sample_rate),
            frame: 0,
            send_bus: [0.0; BLOCK],
            params,
            layers,
            sample_rate,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Frames rendered so far (the beat clocks anchor to this).
    pub fn frame(&self) -> u64 {
        self.frame
    }

    #[cfg(test)]
    pub(crate) fn params_for_test(&self) -> &Arc<GraphParams> {
        &self.params
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

    /// Swap a slot's source and hand the old one back (so the caller can drop it off
    /// the audio thread). Allocation-free.
    pub fn replace_source(
        &mut self,
        slot: usize,
        source: Box<dyn Source>,
    ) -> Option<Box<dyn Source>> {
        if slot >= MAX_LAYERS {
            return Some(source);
        }
        let l = &mut self.layers[slot];
        l.lp_state = 0.0;
        l.hp_state_in = 0.0;
        l.hp_state_out = 0.0;
        Some(std::mem::replace(&mut l.source, source))
    }

    pub fn reset(&mut self) {
        for l in self.layers.iter_mut() {
            l.source.reset();
        }
    }

    /// Render `out` (interleaved, `channels` per frame). Allocation-free.
    pub fn render(&mut self, out: &mut [f32], channels: usize) {
        self.render_inner(out, channels, None);
    }

    /// Like `render`, and also writes each layer's post-fader, pre-master signal into
    /// `stems[i]` (same interleaved layout and length as `out`). Offline use (stems).
    pub fn render_with_stems(&mut self, out: &mut [f32], channels: usize, stems: &mut [Vec<f32>]) {
        self.render_inner(out, channels, Some(stems));
    }

    fn render_inner(
        &mut self,
        out: &mut [f32],
        channels: usize,
        mut stems: Option<&mut [Vec<f32>]>,
    ) {
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
            self.send_bus[..n].fill(0.0);
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
                let fade_ms = p.fade_ms.load(Ordering::Relaxed);
                let gain_ramp = if fade_ms > 0 {
                    fade_ms as f32 * self.sample_rate as f32 / 1000.0
                } else {
                    self.ramp
                };
                layer.gain.begin(target_gain, gain_ramp);
                layer.pan.begin(p.pan.target(), self.ramp);
                layer.lp.begin(p.lowpass_hz.target(), self.ramp);
                layer.hp.begin(p.highpass_hz.target(), self.ramp);
                let at_frame = self.frame + done as u64;
                let stereo = {
                    let st = &mut *layer;
                    st.send[..n].fill(0.0);
                    st.source.render_stereo(
                        &mut st.mono[..n],
                        &mut st.right[..n],
                        &mut st.send[..n],
                        self.sample_rate,
                        at_frame,
                    )
                };
                if !stereo {
                    let mono = &mut layer.mono[..n];
                    layer.source.render_at(mono, self.sample_rate, at_frame);
                }
                let mono = &layer.mono[..n];
                let mut peak = 0.0f32;
                let stem = stems
                    .as_deref_mut()
                    .and_then(|s| s.get_mut(i))
                    .map(|s| &mut s[done * channels..(done + n) * channels]);
                let mut stem_frames = stem.map(|s| s.chunks_mut(channels));
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
                    let (yl, yr) = if stereo {
                        // The source's own right channel through its own filter states;
                        // the layer pan is a balance on the stereo image.
                        let xr = layer.right[k];
                        layer.lp_state_r =
                            undenormal(layer.lp_state_r + a_lp * (xr - layer.lp_state_r));
                        let yr_lp = layer.lp_state_r;
                        layer.hp_state_out_r =
                            undenormal(a_hp * (layer.hp_state_out_r + yr_lp - layer.hp_state_in_r));
                        layer.hp_state_in_r = yr_lp;
                        let yr = layer.hp_state_out_r * g;
                        peak = peak.max(y.abs()).max(yr.abs());
                        self.send_bus[k] += layer.send[k] * g;
                        if channels == 1 {
                            ((y + yr) * 0.5, (y + yr) * 0.5)
                        } else {
                            (y * (1.0 - pan).min(1.0), yr * (1.0 + pan).min(1.0))
                        }
                    } else {
                        peak = peak.max(y.abs());
                        // Constant-power pan.
                        let theta = (pan + 1.0) * 0.25 * std::f32::consts::PI;
                        let (l, r) = (theta.cos(), theta.sin());
                        if channels == 1 {
                            (y, y)
                        } else {
                            (
                                y * l * std::f32::consts::SQRT_2,
                                y * r * std::f32::consts::SQRT_2,
                            )
                        }
                    };
                    let y = if stereo { (yl + yr) * 0.5 } else { y };
                    if channels == 1 {
                        frame[0] += yl;
                    } else {
                        frame[0] += yl;
                        frame[1] += yr;
                        for ch in frame.iter_mut().skip(2) {
                            *ch += y;
                        }
                    }
                    if let Some(sf) = stem_frames.as_mut().and_then(|it| it.next()) {
                        if channels == 1 {
                            sf[0] += yl;
                        } else {
                            sf[0] += yl;
                            sf[1] += yr;
                        }
                    }
                }
                p.peak_bits.store(peak.to_bits(), Ordering::Relaxed);
            }
            // Master: gain ramp, space (reverb), limiter, meter.
            let target_master = db_to_gain(self.params.master.gain_db.target());
            self.master_gain.begin(target_master, self.ramp);
            let target_space = self.params.master.space.target();
            self.space.begin(target_space, self.ramp * 5.0);
            let size = self.params.master.space_size.target();
            let shimmer = self.params.master.shimmer.target();
            let shimmer_ratio = 2f32.powf(self.params.master.shimmer_st.target() / 12.0);
            let send_active = self.send_bus[..n].iter().any(|v| v.abs() > 1e-7);
            let wet_on = target_space > 0.0
                || self.space.current > 0.0
                || self.reverb_tail > 0
                || send_active;
            if wet_on {
                for r in self.reverb.iter_mut() {
                    r.set(size, 0.35 + 0.2 * shimmer);
                }
                for sh in self.shimmer.iter_mut() {
                    sh.set_ratio(shimmer_ratio);
                }
                if target_space > 0.0 || self.space.current > 0.0 || send_active {
                    self.reverb_tail = self.sample_rate * 6; // ring out up to 6 s
                } else {
                    self.reverb_tail = self.reverb_tail.saturating_sub(n as u32);
                    if self.reverb_tail == 0 {
                        for r in self.reverb.iter_mut() {
                            r.clear();
                        }
                        for sh in self.shimmer.iter_mut() {
                            sh.clear();
                        }
                        self.shimmer_fb = [0.0; 2];
                    }
                }
            }
            for (k, frame) in block.chunks_mut(channels).enumerate() {
                let g = self.master_gain.next(target_master);
                let mix = self.space.next(target_space);
                // Track sends enter the reverb at their own level, after the master gain.
                let send_in = self.send_bus[k] * g;
                for s in frame.iter_mut() {
                    *s *= g;
                }
                if wet_on {
                    // Wet is fed from the (gained) dry mix; stereo gets two decorrelated tails.
                    // Shimmer: the previous wet sample comes back transposed into the
                    // reverb's input (soft-clipped so the loop can never run away).
                    if channels == 1 {
                        let fb = if shimmer > 0.0 {
                            sources::gen::soft(
                                self.shimmer[0].process(self.shimmer_fb[0]) * shimmer * 0.08,
                            )
                        } else {
                            0.0
                        };
                        let w = self.reverb[0].process(frame[0] * mix + send_in + fb);
                        self.shimmer_fb[0] = w;
                        frame[0] += w * 0.8;
                    } else {
                        let input = (frame[0] + frame[1]) * 0.5 * mix + send_in;
                        let (fl, fr) = if shimmer > 0.0 {
                            (
                                sources::gen::soft(
                                    self.shimmer[0].process(self.shimmer_fb[0]) * shimmer * 0.08,
                                ),
                                sources::gen::soft(
                                    self.shimmer[1].process(self.shimmer_fb[1]) * shimmer * 0.08,
                                ),
                            )
                        } else {
                            (0.0, 0.0)
                        };
                        let wl = self.reverb[0].process(input + fl);
                        let wr = self.reverb[1].process(input + fr);
                        self.shimmer_fb = [wl, wr];
                        frame[0] += wl * 0.8;
                        frame[1] += wr * 0.8;
                    }
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
        self.frame += frames as u64;
        self.params.frame.store(self.frame, Ordering::Relaxed);
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
    // f64 so an hour of accumulation stays within a few ppb (f32 drifts ~5 ppm).
    phase: f64,
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
        let inc = self.hz as f64 / sample_rate.max(1) as f64;
        for s in out.iter_mut() {
            *s = self.amp * (self.phase * std::f64::consts::TAU).sin() as f32;
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
    fn shimmer_adds_an_octave_to_the_reverb_tail_without_running_away() {
        use crate::graph::sources::loop_player::{LoopParams, LoopSource};
        // One second of 220 Hz, then three seconds of silence, looped: the tail is exposed.
        let mut samples = vec![0.0f32; 4 * 48_000];
        for (i, s) in samples.iter_mut().enumerate().take(48_000) {
            *s = (i as f32 * 220.0 / 48_000.0 * std::f32::consts::TAU).sin() * 0.5;
        }
        let render = |shimmer: f32| {
            let p = Arc::new(GraphParams::default());
            p.master.gain_db.set(0.0);
            p.master.space.set(0.6);
            p.master.space_size.set(0.7);
            p.master.shimmer.set(shimmer);
            p.master.shimmer_st.set(12.0);
            p.layers[0].gain_db.set(0.0);
            let mut g = Graph::new(Arc::clone(&p), 48_000);
            let buf = Arc::new(AudioBuffer::new(48_000, 1, samples.clone()));
            g.set_source(
                0,
                Box::new(LoopSource::new(Arc::new(LoopParams::default()), buf)),
            );
            g.render_offline(3.5, 1)
        };
        let dry = render(0.0);
        let wet = render(1.0);
        // High-frequency share of a window: RMS of the first difference over the RMS.
        let bright = |b: &AudioBuffer, from: f64, to: f64| {
            let a = (from * 48_000.0) as usize;
            let z = (to * 48_000.0) as usize;
            let x = &b.samples[a..z];
            let d: f32 = x.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum::<f32>();
            let e: f32 = x.iter().map(|v| v * v).sum::<f32>();
            (d / e.max(1e-12)).sqrt()
        };
        assert!(wet.samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        let (bd, bw) = (bright(&dry, 1.6, 2.4), bright(&wet, 1.6, 2.4));
        assert!(
            bw > bd * 1.15,
            "shimmer tail is brighter (octave up fed back): {bw} vs {bd}"
        );
        assert!(
            rms(&wet, 1.6, 2.4) > rms(&dry, 1.6, 2.4) * 1.05,
            "the fed-back octave adds energy to the tail"
        );
        // The tail still decays (no self-oscillation).
        assert!(rms(&wet, 3.0, 3.4) < rms(&wet, 1.2, 1.6), "tail decays");
    }

    #[test]
    fn fade_ms_stretches_the_gain_ramp() {
        let p = Arc::new(GraphParams::default());
        p.master.gain_db.set(0.0);
        p.layers[0].gain_db.set(-120.0);
        let mut g = Graph::new(Arc::clone(&p), 48_000);
        g.set_source(0, Box::new(SineSource::new(220.0, 0.25)));
        p.layers[0].fade_ms.store(1000, Ordering::Relaxed);
        p.layers[0].gain_db.set(0.0);
        let b = g.render_offline(2.0, 1);
        let early = rms(&b, 0.2, 0.3);
        let late = rms(&b, 1.5, 2.0);
        assert!(early < late * 0.5, "1 s fade: early {early} vs late {late}");
        assert!(late > 0.15, "reached the level: {late}");
        // Back to the default: the next change settles within the 20 ms ramp.
        p.layers[0].fade_ms.store(0, Ordering::Relaxed);
        p.layers[0].gain_db.set(-12.0);
        let c = g.render_offline(0.5, 1);
        let soon = rms(&c, 0.05, 0.15);
        let settled = rms(&c, 0.4, 0.5);
        assert!(
            (soon - settled).abs() < 0.02,
            "20 ms ramp: {soon} vs {settled}"
        );
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

    /// F1.6 "1 hour without drift": the sample count is exact and the sine keeps its
    /// phase (zero crossings match the ideal count) across an hour of chunked rendering.
    #[test]
    fn one_hour_of_chunked_rendering_keeps_time() {
        let p = Arc::new(GraphParams::default());
        p.master.gain_db.set(0.0);
        p.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(Arc::clone(&p), 48_000);
        g.set_source(0, Box::new(SineSource::new(100.0, 0.5)));
        let mut buf = vec![0.0f32; 1000]; // odd chunk size on purpose
        let total = 48_000u64 * 3600;
        let mut rendered = 0u64;
        let mut crossings = 0u64;
        let mut prev = 0.0f32;
        while rendered < total {
            let n = ((total - rendered) as usize).min(buf.len());
            g.render(&mut buf[..n], 1);
            for &s in &buf[..n] {
                if prev < 0.0 && s >= 0.0 {
                    crossings += 1;
                }
                prev = s;
            }
            rendered += n as u64;
        }
        assert_eq!(rendered, total);
        // 100 Hz for 3600 s = 360 000 upward zero crossings (±1 for the edges).
        assert!(
            (crossings as i64 - 360_000).abs() <= 1,
            "crossings {crossings}"
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
