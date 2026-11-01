//! Sampler (home-studio item 3, owner 2026-11-01: "семплите… с ноти, фейдъри, pitch,
//! филтри"): a take played as *notes*. Each event picks a pitch from the pitch set,
//! starts a voice that reads the buffer at `pitch / root` speed (varispeed: a vocal
//! "ah" at 220 Hz becomes a chord tone an octave up by reading twice as fast), shapes
//! it with attack → hold (`note_s`) → release, and runs it through a resonant low-pass
//! (TPT state-variable filter). Up to `SAMPLER_VOICES` overlap. Allocation-free render.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use super::gen::{PitchSet, Trigger};
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;
use crate::AudioBuffer;

pub const SAMPLER_VOICES: usize = 8;

pub struct SamplerParams {
    pub pitches: PitchSet,
    /// The pitch the take itself sounds at (Hz): a note read at `root_hz` plays unchanged.
    pub root_hz: SmoothedParam,
    /// Notes per minute (0 = silent).
    pub density: SmoothedParam,
    /// Held length before the release starts, seconds.
    pub note_s: SmoothedParam,
    pub attack_s: SmoothedParam,
    pub release_s: SmoothedParam,
    /// Low-pass cutoff (Hz) and resonance 0..1 (self-oscillation stays out of reach).
    pub cutoff_hz: SmoothedParam,
    pub resonance: SmoothedParam,
    pub velocity_spread: SmoothedParam,
    /// Where in the take a note starts, 0..1 of its length.
    pub start: SmoothedParam,
    pub seed: AtomicU32,
}

impl Default for SamplerParams {
    fn default() -> Self {
        Self {
            pitches: PitchSet::default(),
            root_hz: SmoothedParam::new(261.63, 20.0, 4000.0),
            density: SmoothedParam::new(12.0, 0.0, 240.0),
            note_s: SmoothedParam::new(1.5, 0.05, 30.0),
            attack_s: SmoothedParam::new(0.02, 0.001, 5.0),
            release_s: SmoothedParam::new(0.8, 0.01, 20.0),
            cutoff_hz: SmoothedParam::new(8000.0, 40.0, 20_000.0),
            resonance: SmoothedParam::new(0.2, 0.0, 1.0),
            velocity_spread: SmoothedParam::new(0.4, 0.0, 1.0),
            start: SmoothedParam::new(0.0, 0.0, 1.0),
            seed: AtomicU32::new(1),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Attack,
    Hold,
    Release,
}

struct Voice {
    active: bool,
    zone: usize,
    pos: f64,
    rate: f64,
    /// Seconds since the note started.
    age: f32,
    amp: f32,
    stage: Stage,
    env: f32,
    /// Envelope level when the release began (the release ramps from there).
    rel_from: f32,
    rel_t: f32,
    ic1: f32,
    ic2: f32,
}

impl Voice {
    const IDLE: Voice = Voice {
        active: false,
        zone: 0,
        pos: 0.0,
        rate: 1.0,
        age: 0.0,
        amp: 0.0,
        stage: Stage::Attack,
        env: 0.0,
        rel_from: 0.0,
        rel_t: 0.0,
        ic1: 0.0,
        ic2: 0.0,
    };
}

/// One sample of a multisample instrument: the pitch it sounds at (Hz) and its audio.
pub struct SamplerZone {
    pub root_hz: f32,
    pub buf: Arc<AudioBuffer>,
}

pub struct SamplerSource {
    params: Arc<SamplerParams>,
    /// One zone = the plain sampler (root from the params); several = a multisample: the
    /// nearest root is read for every note, so varispeed stays small.
    zones: Vec<SamplerZone>,
    trigger: Trigger,
    seed_used: u32,
    density: Ramp,
    cutoff: Ramp,
    resonance: Ramp,
    voices: Vec<Voice>,
    next_voice: usize,
}

impl SamplerSource {
    /// `buf` must be mono at the graph's sample rate; its root is the params' `root_hz`.
    pub fn new(params: Arc<SamplerParams>, buf: Arc<AudioBuffer>) -> Self {
        Self::new_multi(params, vec![SamplerZone { root_hz: 0.0, buf }])
    }

    /// Several samples with their own roots (Hz); a zone with root 0 uses the params' root.
    pub fn new_multi(params: Arc<SamplerParams>, zones: Vec<SamplerZone>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        Self {
            trigger: Trigger::new(seed),
            seed_used: seed,
            density: Ramp::new(params.density.target()),
            cutoff: Ramp::new(params.cutoff_hz.target()),
            resonance: Ramp::new(params.resonance.target()),
            voices: (0..SAMPLER_VOICES).map(|_| Voice::IDLE).collect(),
            next_voice: 0,
            params,
            zones,
        }
    }

    /// The zone whose root is nearest to `hz` (in pitch), and that root in Hz.
    fn zone_for(&self, hz: f32) -> (usize, f32) {
        let fallback = self.params.root_hz.target().max(1.0);
        let mut best = (0usize, fallback, f32::MAX);
        for (i, z) in self.zones.iter().enumerate() {
            let root = if z.root_hz > 0.0 { z.root_hz } else { fallback };
            let d = (hz / root).ln().abs();
            if d < best.2 {
                best = (i, root, d);
            }
        }
        (best.0, best.1)
    }

    fn note_on(&mut self, hz: f32, spread: f32) {
        let i = (0..SAMPLER_VOICES)
            .map(|k| (self.next_voice + k) % SAMPLER_VOICES)
            .find(|&k| !self.voices[k].active)
            .unwrap_or(self.next_voice);
        self.next_voice = (i + 1) % SAMPLER_VOICES;
        let vel = 1.0 - spread * self.trigger.rng().unit() * 0.7;
        let (zone, root) = self.zone_for(hz);
        let p = &self.params;
        let frames = self.zones[zone].buf.samples.len() as f64;
        let v = &mut self.voices[i];
        *v = Voice::IDLE;
        v.active = true;
        v.zone = zone;
        v.rate = (hz / root.max(1.0)) as f64;
        v.pos = (p.start.target() as f64 * frames).clamp(0.0, (frames - 2.0).max(0.0));
        v.amp = 0.5 * vel;
    }

    fn render_span(&mut self, out: &mut [f32], sr: f32) {
        let p = Arc::clone(&self.params);
        let (attack, hold, release) = (
            p.attack_s.target().max(0.001),
            p.note_s.target(),
            p.release_s.target().max(0.01),
        );
        for s in out.iter_mut() {
            let _ = self.density.next(p.density.target());
            let fc = self
                .cutoff
                .next(p.cutoff_hz.target())
                .clamp(40.0, sr * 0.45);
            let res = self.resonance.next(p.resonance.target()).clamp(0.0, 1.0);
            // TPT state-variable low-pass (Cytomic): g from the cutoff, k from the resonance.
            let g = (std::f32::consts::PI * fc / sr).tan();
            let k = 2.0 - 1.9 * res;
            let a1 = 1.0 / (1.0 + g * (g + k));
            let a2 = g * a1;
            let a3 = g * a2;
            let dt = 1.0 / sr;
            let mut acc = 0.0f32;
            for k_v in 0..SAMPLER_VOICES {
                let v = &mut self.voices[k_v];
                if !v.active {
                    continue;
                }
                // Envelope.
                match v.stage {
                    Stage::Attack => {
                        v.env = (v.age / attack).min(1.0);
                        if v.env >= 1.0 {
                            v.stage = Stage::Hold;
                        }
                    }
                    Stage::Hold => {
                        if v.age >= attack + hold {
                            v.stage = Stage::Release;
                            v.rel_from = v.env;
                            v.rel_t = 0.0;
                        }
                    }
                    Stage::Release => {
                        v.rel_t += dt;
                        let t = (v.rel_t / release).min(1.0);
                        v.env = v.rel_from * (1.0 - t) * (1.0 - t);
                        if t >= 1.0 {
                            v.active = false;
                            continue;
                        }
                    }
                }
                v.age += dt;
                // Read the zone at the voice's rate; past the end the voice is done.
                let samples = &self.zones[v.zone.min(self.zones.len() - 1)].buf.samples;
                let n = samples.len();
                if n < 2 || v.pos >= n as f64 - 1.0 {
                    v.active = false;
                    continue;
                }
                let x = {
                    let i = v.pos.floor();
                    let frac = (v.pos - i) as f32;
                    let i0 = (i as usize).min(n - 1);
                    let i1 = (i0 + 1).min(n - 1);
                    samples[i0] * (1.0 - frac) + samples[i1] * frac
                };
                v.pos += v.rate;
                // Filter.
                let v3 = x - v.ic2;
                let v1 = a1 * v.ic1 + a2 * v3;
                let v2 = v.ic2 + a2 * v.ic1 + a3 * v3;
                v.ic1 = undenormal(2.0 * v1 - v.ic1);
                v.ic2 = undenormal(2.0 * v2 - v.ic2);
                acc += v2 * v.env * v.amp;
            }
            *s = acc;
        }
    }
}

impl Source for SamplerSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let sr = sample_rate.max(1) as f32;
        let p = Arc::clone(&self.params);
        let seed = p.seed.load(Ordering::Relaxed);
        if seed != self.seed_used {
            self.trigger.reset(seed);
            self.seed_used = seed;
        }
        let ramp = ramp_samples(sample_rate);
        self.density.begin(p.density.target(), ramp);
        self.cutoff.begin(p.cutoff_hz.target(), ramp);
        self.resonance.begin(p.resonance.target(), ramp);
        let n = out.len();
        let density = if p.pitches.is_empty() || self.zones.iter().all(|z| z.buf.samples.len() < 2)
        {
            0.0
        } else {
            self.density.current
        };
        let at = self.trigger.tick(n, density, sr).unwrap_or(n).min(n);
        let (head, tail) = out.split_at_mut(at);
        self.render_span(head, sr);
        if at < n {
            if let Some(hz) = p.pitches.pick(self.trigger.rng()) {
                let spread = p.velocity_spread.target();
                self.note_on(hz, spread);
            }
        }
        self.render_span(tail, sr);
    }

    fn reset(&mut self) {
        self.trigger.reset(self.seed_used);
        for v in self.voices.iter_mut() {
            *v = Voice::IDLE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    fn render(set: impl Fn(&SamplerParams), seconds: f64) -> AudioBuffer {
        // The "take": a 220 Hz sine, two seconds, so root = 220 Hz.
        let take = AudioBuffer::sine(48_000, 220.0, 2.0, 0.6);
        let p = Arc::new(SamplerParams::default());
        p.root_hz.set(220.0);
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(SamplerSource::new(p, Arc::new(take))));
        g.render_offline(seconds, 1)
    }

    #[test]
    fn a_note_an_octave_up_reads_the_take_twice_as_fast() {
        let b = render(
            |p| {
                p.pitches.set(&[440.0]);
                p.density.set(240.0);
                p.note_s.set(1.0);
                p.attack_s.set(0.01);
                p.release_s.set(0.1);
                p.cutoff_hz.set(20_000.0);
            },
            2.0,
        );
        assert!(b.samples.iter().all(|s| s.is_finite()));
        assert!(b.rms() > 0.02, "the sampler sounds: rms {}", b.rms());
        // The dominant pitch of the sounding part is 440 Hz, not the take's 220.
        let cfg = PitchConfig::for_min_hz(48_000, 70.0);
        let c = contour(&b, &cfg, 0.02);
        let mut hz: Vec<f32> = c
            .iter()
            .filter(|f| f.confidence > 0.6)
            .filter_map(|f| f.hz.map(|h| h as f32))
            .collect();
        assert!(hz.len() > 10, "voiced frames: {}", hz.len());
        hz.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = hz[hz.len() / 2];
        assert!((median - 440.0).abs() < 8.0, "median {median} Hz");
    }

    #[test]
    fn a_multisample_picks_the_nearest_zone() {
        // Two zones: a 220 Hz sine rooted at 220 and a 440 Hz sine rooted at 440. A note at
        // 450 Hz must come from the 440 zone (tiny varispeed), not from the 220 one.
        let p = Arc::new(SamplerParams::default());
        p.pitches.set(&[450.0]);
        p.density.set(240.0);
        p.note_s.set(1.0);
        p.attack_s.set(0.01);
        p.cutoff_hz.set(20_000.0);
        let zones = vec![
            SamplerZone {
                root_hz: 220.0,
                buf: Arc::new(AudioBuffer::sine(48_000, 220.0, 2.0, 0.6)),
            },
            SamplerZone {
                root_hz: 440.0,
                buf: Arc::new(AudioBuffer::sine(48_000, 440.0, 2.0, 0.6)),
            },
        ];
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        let src = SamplerSource::new_multi(p, zones);
        assert_eq!(src.zone_for(450.0).0, 1);
        assert_eq!(src.zone_for(230.0).0, 0);
        g.set_source(0, Box::new(src));
        let b = g.render_offline(2.0, 1);
        assert!(b.rms() > 0.02);
    }

    #[test]
    fn silent_without_pitches_and_the_filter_tames_the_top() {
        let silent = render(|p| p.density.set(240.0), 1.0);
        assert!(silent.rms() < 1e-6, "no pitch set → nothing plays");
        let open = render(
            |p| {
                p.pitches.set(&[880.0]);
                p.density.set(240.0);
                p.cutoff_hz.set(20_000.0);
            },
            1.5,
        );
        let closed = render(
            |p| {
                p.pitches.set(&[880.0]);
                p.density.set(240.0);
                p.cutoff_hz.set(200.0);
            },
            1.5,
        );
        assert!(open.rms() > 0.02);
        assert!(
            closed.rms() < open.rms() * 0.3,
            "a 200 Hz low-pass takes most of a 1760 Hz note: {} vs {}",
            closed.rms(),
            open.rms()
        );
    }
}
