//! Sub-bass engine (F4.2): a monophonic bass that plays a looped note pattern from
//! the beat clock with sub / pluck / Reese voices, glide, a wobble filter synced to
//! the beat, soft saturation and sidechain ducking from the drums' kick envelope.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

use super::gen::{soft, Svf};
use super::rhythm::{BeatClock, Cursor, DrumParams};
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum BassVoice {
    /// Sine with a triangle layer ("warmth").
    Sub = 0,
    /// Saw with a fast filter envelope.
    Pluck = 1,
    /// Two detuned saws through the filter, saturated.
    Reese = 2,
}

impl BassVoice {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => BassVoice::Pluck,
            2 => BassVoice::Reese,
            _ => BassVoice::Sub,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BassNoteEvent {
    pub beat: f64,
    pub length_beats: f64,
    pub hz: f32,
    pub velocity: f32,
    pub glide: bool,
}

pub struct BassPatternData {
    pub events: Vec<BassNoteEvent>,
    pub beats: Vec<f64>,
    pub length_beats: f64,
}

impl BassPatternData {
    pub fn new(mut events: Vec<BassNoteEvent>, length_beats: f64) -> Self {
        events.retain(|e| e.beat >= 0.0 && e.beat < length_beats.max(1e-9) && e.hz > 0.0);
        events.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap());
        let beats = events.iter().map(|e| e.beat).collect();
        Self {
            events,
            beats,
            length_beats,
        }
    }
}

pub struct BassParams {
    pub voice: AtomicU8,
    pub cutoff_hz: SmoothedParam,
    pub resonance: SmoothedParam,
    /// 0..1 saturation.
    pub drive: SmoothedParam,
    /// Glide time in seconds for notes marked `glide`.
    pub glide_s: SmoothedParam,
    /// 0..1 amount of ducking under the kick.
    pub sidechain: SmoothedParam,
    /// Wobble: filter LFO rate as a note division (0 = off, 4 = quarter, 8 = eighth, 16).
    pub wobble_div: AtomicU32,
    /// 0..1 depth (octaves of cutoff movement).
    pub wobble_depth: SmoothedParam,
    /// 0..1 level of the triangle layer (Sub) / low-pass warmth.
    pub warmth: SmoothedParam,
    pub release_s: SmoothedParam,
}

impl Default for BassParams {
    fn default() -> Self {
        Self {
            voice: AtomicU8::new(0),
            cutoff_hz: SmoothedParam::new(400.0, 60.0, 8000.0),
            resonance: SmoothedParam::new(0.2, 0.0, 0.95),
            drive: SmoothedParam::new(0.3, 0.0, 1.0),
            glide_s: SmoothedParam::new(0.08, 0.0, 1.0),
            sidechain: SmoothedParam::new(0.6, 0.0, 1.0),
            wobble_div: AtomicU32::new(0),
            wobble_depth: SmoothedParam::new(0.5, 0.0, 1.0),
            warmth: SmoothedParam::new(0.5, 0.0, 1.0),
            release_s: SmoothedParam::new(0.08, 0.01, 2.0),
        }
    }
}

pub struct BassSource {
    params: Arc<BassParams>,
    clock: Arc<BeatClock>,
    pattern: Arc<BassPatternData>,
    /// The drums' params, for the kick envelope (None = no ducking).
    drums: Option<Arc<DrumParams>>,
    cursor: Cursor,
    hz: f32,
    target_hz: f32,
    glide_coeff: f32,
    phase: f32,
    phase2: f32,
    env: f32,
    filt_env: f32,
    gate_until_beat: f64,
    gate: bool,
    vel: f32,
    filter: Svf,
    cutoff: Ramp,
    resonance: Ramp,
    drive: Ramp,
    sidechain: Ramp,
    wobble_depth: Ramp,
    warmth: Ramp,
    duck: f32,
    frame: u64,
}

impl BassSource {
    pub fn new(
        params: Arc<BassParams>,
        clock: Arc<BeatClock>,
        pattern: Arc<BassPatternData>,
        drums: Option<Arc<DrumParams>>,
    ) -> Self {
        Self {
            cutoff: Ramp::new(params.cutoff_hz.target()),
            resonance: Ramp::new(params.resonance.target()),
            drive: Ramp::new(params.drive.target()),
            sidechain: Ramp::new(params.sidechain.target()),
            wobble_depth: Ramp::new(params.wobble_depth.target()),
            warmth: Ramp::new(params.warmth.target()),
            params,
            clock,
            pattern,
            drums,
            cursor: Cursor::new(),
            hz: 55.0,
            target_hz: 55.0,
            glide_coeff: 1.0,
            phase: 0.0,
            phase2: 0.0,
            env: 0.0,
            filt_env: 0.0,
            gate_until_beat: -1.0,
            gate: false,
            vel: 0.0,
            filter: Svf::default(),
            duck: 1.0,
            frame: 0,
        }
    }

    #[inline]
    fn saw(phase: f32) -> f32 {
        2.0 * phase - 1.0
    }
}

impl Source for BassSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let f = self.frame;
        self.render_at(out, sample_rate, f);
        self.frame += out.len() as u64;
    }

    fn render_at(&mut self, out: &mut [f32], sample_rate: u32, frame: u64) {
        let sr = sample_rate.max(1) as f32;
        let p = Arc::clone(&self.params);
        let ramp = ramp_samples(sample_rate);
        self.cutoff.begin(p.cutoff_hz.target(), ramp);
        self.resonance.begin(p.resonance.target(), ramp);
        self.drive.begin(p.drive.target(), ramp);
        self.sidechain.begin(p.sidechain.target(), ramp);
        self.wobble_depth.begin(p.wobble_depth.target(), ramp);
        self.warmth.begin(p.warmth.target(), ramp);
        let voice = BassVoice::from_u8(p.voice.load(Ordering::Relaxed));
        let glide_s = p.glide_s.target();
        let release = p.release_s.target();
        let k_rel = (-6.9078 / (release * sr)).exp();
        let k_att = 1.0 - (-1.0 / (0.004 * sr)).exp();
        let k_filt = (-6.9078 / (0.14 * sr)).exp();
        let beat0 = self.clock.beat_at(frame, sample_rate);
        let bpf = self.clock.beats_per_frame(sample_rate);
        let generation = self.clock.generation.load(Ordering::Relaxed);
        if let Some(b) = beat0 {
            if self.cursor.needs_seek(generation) {
                self.cursor
                    .seek(&self.pattern.beats, self.pattern.length_beats, b);
                self.cursor.mark(generation);
                self.gate = false;
            }
        } else {
            self.gate = false;
        }
        let wobble_div = p.wobble_div.load(Ordering::Relaxed);
        let kick_env = self.drums.as_ref().map(|d| d.kick_env()).unwrap_or(0.0);
        // Block-rate filter coefficients (the wobble is sampled per block, 64 frames).
        let depth = self.wobble_depth.current;
        let wobble = if wobble_div > 0 {
            beat0
                .map(|b| {
                    let cycles = b * wobble_div as f64 / 4.0;
                    let ph = cycles.fract() as f32;
                    // Saw-shaped wobble: opens fast, closes over the division.
                    (1.0 - ph) * 2.0 - 1.0
                })
                .unwrap_or(0.0)
        } else {
            0.0
        };
        for (i, s) in out.iter_mut().enumerate() {
            if let Some(b0) = beat0 {
                let beat = b0 + i as f64 * bpf;
                while let Some(idx) =
                    self.cursor
                        .due(&self.pattern.beats, self.pattern.length_beats, beat)
                {
                    let e = self.pattern.events[idx];
                    self.target_hz = e.hz;
                    self.glide_coeff = if e.glide && glide_s > 0.0 {
                        1.0 - (-1.0 / (glide_s * sr)).exp()
                    } else {
                        1.0
                    };
                    let loops = ((beat) / self.pattern.length_beats).floor();
                    self.gate_until_beat =
                        e.beat + loops * self.pattern.length_beats + e.length_beats;
                    self.gate = true;
                    self.vel = e.velocity.clamp(0.05, 1.0);
                    self.filt_env = 1.0;
                    if self.env < 0.001 {
                        self.phase = 0.0;
                        self.phase2 = 0.0;
                    }
                }
                if self.gate && beat >= self.gate_until_beat {
                    self.gate = false;
                }
            }
            let cutoff = self.cutoff.next(p.cutoff_hz.target());
            let q = 0.6 + self.resonance.next(p.resonance.target()) * 8.0;
            let drive = self.drive.next(p.drive.target());
            let sc = self.sidechain.next(p.sidechain.target());
            let warmth = self.warmth.next(p.warmth.target());
            // Glide towards the target pitch (exponential in Hz is close enough here).
            self.hz += (self.target_hz - self.hz) * self.glide_coeff;
            // Envelope: fast attack while gated, release after.
            if self.gate {
                self.env += (1.0 - self.env) * k_att;
            } else {
                self.env *= k_rel;
            }
            self.filt_env *= k_filt;
            let inc = self.hz / sr;
            self.phase += inc;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
            let raw = match voice {
                BassVoice::Sub => {
                    let sine = (self.phase * std::f32::consts::TAU).sin();
                    let tri = 1.0 - 4.0 * (self.phase - 0.5).abs();
                    sine * 0.8 + tri * 0.35 * warmth
                }
                BassVoice::Pluck => Self::saw(self.phase),
                BassVoice::Reese => {
                    self.phase2 += inc * 1.007; // +12 cents
                    if self.phase2 >= 1.0 {
                        self.phase2 -= 1.0;
                    }
                    (Self::saw(self.phase) + Self::saw(self.phase2)) * 0.5
                }
            };
            // Filter: cutoff moved by the pluck envelope and the wobble.
            let fc = match voice {
                BassVoice::Sub => (cutoff * 2.0).max(200.0),
                BassVoice::Pluck => cutoff * (1.0 + 3.0 * self.filt_env),
                BassVoice::Reese => cutoff * 2f32.powf(wobble * depth * 1.5),
            };
            if i % 16 == 0 {
                self.filter.set(fc.clamp(40.0, 12_000.0), q, sr);
            }
            let filtered = self.filter.process(raw).lp;
            let driven = soft(filtered * (1.0 + drive * 4.0)) / (1.0 + drive * 0.6);
            // Sidechain: the kick envelope pulls the level down by up to 16 dB.
            let target_duck = 1.0 - sc * kick_env.min(1.0) * 0.85;
            self.duck += (target_duck - self.duck) * 0.002;
            *s = undenormal(driven * self.env * self.vel * self.duck * 0.9);
        }
    }

    fn reset(&mut self) {
        self.cursor = Cursor::new();
        self.env = 0.0;
        self.gate = false;
        self.frame = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::sources::rhythm::{DrumEvent, DrumKind, DrumPatternData, DrumSource};
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    fn line() -> Arc<BassPatternData> {
        Arc::new(BassPatternData::new(
            vec![
                BassNoteEvent {
                    beat: 0.0,
                    length_beats: 1.5,
                    hz: 110.0,
                    velocity: 0.9,
                    glide: false,
                },
                BassNoteEvent {
                    beat: 2.0,
                    length_beats: 1.0,
                    hz: 164.81,
                    velocity: 0.8,
                    glide: true,
                },
            ],
            4.0,
        ))
    }

    fn render(
        params: &Arc<BassParams>,
        drums: Option<Arc<DrumParams>>,
        seconds: f64,
    ) -> AudioBuffer {
        let clock = Arc::new(BeatClock::new(120.0));
        clock.start(0, 0.0);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        gp.layers[1].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        if let Some(d) = drums.as_ref() {
            let kick_only = Arc::new(DrumPatternData::new(
                vec![DrumEvent {
                    beat: 0.0,
                    kind: DrumKind::Kick,
                    velocity: 1.0,
                }],
                1.0,
            ));
            g.set_source(
                1,
                Box::new(DrumSource::new(
                    Arc::clone(d),
                    Arc::clone(&clock),
                    kick_only,
                )),
            );
            // Drums almost silent at the layer (−60 dB keeps them rendering, so their
            // kick envelope reaches the bass without being heard in the measurement).
            g.params_for_test().layers[1].gain_db.set(-60.0);
        }
        g.set_source(
            0,
            Box::new(BassSource::new(Arc::clone(params), clock, line(), drums)),
        );
        g.render_offline(seconds, 1)
    }

    fn pitch_between(b: &AudioBuffer, from: f64, to: f64) -> f64 {
        let mut v: Vec<f64> = contour(b, &PitchConfig::new(48_000), 0.02)
            .into_iter()
            .filter(|p| p.t >= from && p.t < to)
            .filter_map(|p| p.hz)
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    }

    #[test]
    fn bass_plays_its_notes_in_tune_glides_and_stops() {
        let p = Arc::new(BassParams::default());
        let b = render(&p, None, 2.0); // beat = 0.5 s: note 1 at 0–0.75 s, note 2 at 1.0–1.5 s
        assert!(b.samples.iter().all(|s| s.is_finite()));
        assert!(b.peak() <= 1.0);
        let f1 = pitch_between(&b, 0.2, 0.7);
        assert!((1200.0 * (f1 / 110.0).log2()).abs() < 10.0, "{f1}");
        let f2 = pitch_between(&b, 1.25, 1.5);
        assert!((1200.0 * (f2 / 164.81).log2()).abs() < 10.0, "{f2}");
        // Between the notes (0.75 + release … 1.0) the bass is quiet.
        assert!(
            b.slice_frames(43_000, 47_500).rms() < 0.02,
            "{}",
            b.slice_frames(43_000, 47_500).rms()
        );
        // Glide: right after the second onset the pitch is still below the target.
        let early = pitch_between(&b, 1.0, 1.04);
        assert!(early < 160.0, "glide from 110 towards 165: {early}");
    }

    #[test]
    fn voices_differ_and_sidechain_ducks_under_the_kick() {
        let sub = render(&Arc::new(BassParams::default()), None, 1.0);
        let reese_p = Arc::new(BassParams::default());
        reese_p.voice.store(2, Ordering::Relaxed);
        reese_p.wobble_div.store(8, Ordering::Relaxed);
        let reese = render(&reese_p, None, 1.0);
        // Reese has far more high-frequency content than the sub.
        let hf = |b: &AudioBuffer| {
            let a = (-2.0 * std::f32::consts::PI * 300.0 / 48_000.0).exp();
            let (mut lp, mut hi, mut all) = (0.0f32, 0.0f32, 0.0f32);
            for &x in &b.samples[4800..36_000] {
                lp = a * lp + (1.0 - a) * x;
                hi += (x - lp) * (x - lp);
                all += x * x;
            }
            hi / all.max(1e-9)
        };
        assert!(
            hf(&reese) > hf(&sub) * 2.0,
            "{} vs {}",
            hf(&reese),
            hf(&sub)
        );
        // Sidechain: with a kick every beat, the 60 ms after each beat are quieter
        // than the 60 ms before it; without sidechain they are not.
        let drums = Arc::new(DrumParams::default());
        let ducked_p = Arc::new(BassParams::default());
        ducked_p.sidechain.set(1.0);
        let ducked = render(&ducked_p, Some(Arc::clone(&drums)), 1.5);
        let flat_p = Arc::new(BassParams::default());
        flat_p.sidechain.set(0.0);
        let flat = render(&flat_p, Some(drums), 1.5);
        let ratio = |b: &AudioBuffer| {
            // Kick on beat 1 (0.5 s) while the bass holds its first note.
            let before = b.slice_frames(21_000, 24_000).rms();
            let after = b.slice_frames(24_200, 27_000).rms();
            after / before.max(1e-6)
        };
        assert!(ratio(&ducked) < 0.7, "ducked ratio {}", ratio(&ducked));
        assert!(ratio(&flat) > 0.85, "flat ratio {}", ratio(&flat));
    }
}
