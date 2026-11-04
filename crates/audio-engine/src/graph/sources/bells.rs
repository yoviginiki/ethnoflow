//! Bells (F1A.1/F1A.7): sparse FM/modal strikes on notes from the pitch set. The
//! trigger is a seeded sparse process (density in strikes per minute, silence is
//! kept), so a seed plays the same bells again; a different seed never repeats them.

use std::sync::atomic::{AtomicU32, AtomicU8, Ordering};
use std::sync::Arc;

use super::gen::{beat_grid, PitchSet, Rng, Trigger};
use super::live_notes::{midi_hz, LiveNotes};
use super::rhythm::BeatClock;
use crate::graph::params::{ramp_samples, Ramp, SmoothedParam};
use crate::graph::Source;

pub const BELL_VOICES: usize = 8;

pub struct BellParams {
    pub pitches: PitchSet,
    /// Strikes per minute (0 = silent). Default keeps the spec's ≤ 1 note / 4 s.
    pub density: SmoothedParam,
    /// Decay to −60 dB, seconds.
    pub decay_s: SmoothedParam,
    /// 0..1: FM index and inharmonic partial (0 = almost a sine).
    pub brightness: SmoothedParam,
    /// 0..1 random variation of strike loudness.
    pub velocity_spread: SmoothedParam,
    /// S2 tension: share of strikes that land a semitone / tritone off the mode and are
    /// resolved by the next strike (0 = none, 0.5 = half).
    pub tension: SmoothedParam,
    /// S5 orbit: 0 = every strike rings in the centre (as before), 1 = strikes circle the
    /// listener fast (one turn in ~3 s); in between, slow drifts.
    pub orbit: SmoothedParam,
    /// S5 distance: 0 = close (dry, bright), 1 = far (quieter, darker, mostly reverb).
    /// Each strike lands at its own distance around this value.
    pub distance: SmoothedParam,
    pub seed: AtomicU32,
    /// Beat grid for the strikes: 0 free, else grid lines per beat (1, 2, 4). Needs a
    /// clock that plays (the rhythm / song transport); item 7 "the atmosphere knows the bar".
    pub sync: AtomicU8,
    /// Keyboard strikes (K3): a key = one strike on its pitch.
    pub live: LiveNotes,
}

impl Default for BellParams {
    fn default() -> Self {
        Self {
            pitches: PitchSet::default(),
            density: SmoothedParam::new(12.0, 0.0, 120.0),
            decay_s: SmoothedParam::new(4.0, 0.2, 30.0),
            brightness: SmoothedParam::new(0.4, 0.0, 1.0),
            velocity_spread: SmoothedParam::new(0.5, 0.0, 1.0),
            tension: SmoothedParam::new(0.0, 0.0, 0.5),
            orbit: SmoothedParam::new(0.0, 0.0, 1.0),
            distance: SmoothedParam::new(0.0, 0.0, 1.0),
            seed: AtomicU32::new(1),
            sync: AtomicU8::new(0),
            live: LiveNotes::new(),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Voice {
    hz: f32,
    car: f32,
    modp: f32,
    part: f32,
    env: f32,
    env2: f32,
    amp: f32,
    index: f32,
    /// S5: azimuth phase 0..1, its speed (turns per second, signed), distance 0..1 and the
    /// voice's own low-pass state per side (distance darkens).
    az: f32,
    az_speed: f32,
    dist: f32,
    lp_l: f32,
    lp_r: f32,
    active: bool,
}

pub struct BellSource {
    params: Arc<BellParams>,
    trigger: Trigger,
    clock: Option<Arc<BeatClock>>,
    frame: u64,
    /// The mode note a tense strike must resolve to (0 = none pending).
    resolve_to: f32,
    seed_used: u32,
    density: Ramp,
    decay: Ramp,
    brightness: Ramp,
    spread: Ramp,
    voices: [Voice; BELL_VOICES],
    next_voice: usize,
}

impl BellSource {
    /// Strikes can wait for a beat grid (`sync` lines per beat) while `clock` plays.
    pub fn with_clock(mut self, clock: Option<Arc<BeatClock>>) -> Self {
        self.clock = clock;
        self
    }

    pub fn new(params: Arc<BellParams>) -> Self {
        let seed = params.seed.load(Ordering::Relaxed);
        Self {
            trigger: Trigger::new(seed),
            clock: None,
            frame: 0,
            resolve_to: 0.0,
            seed_used: seed,
            density: Ramp::new(params.density.target()),
            decay: Ramp::new(params.decay_s.target()),
            brightness: Ramp::new(params.brightness.target()),
            spread: Ramp::new(params.velocity_spread.target()),
            voices: [Voice::default(); BELL_VOICES],
            next_voice: 0,
            params,
        }
    }

    fn strike(&mut self, hz: f32, brightness: f32, spread: f32) {
        // Prefer a free voice, else the oldest (round robin).
        let i = (0..BELL_VOICES)
            .map(|k| (self.next_voice + k) % BELL_VOICES)
            .find(|&k| !self.voices[k].active)
            .unwrap_or(self.next_voice);
        self.next_voice = (i + 1) % BELL_VOICES;
        let rng = self.trigger.rng();
        let vel = 1.0 - spread * rng.unit() * 0.7;
        let orbit = self.params.orbit.target();
        let distance = self.params.distance.target();
        // Draw from the strike RNG only when the knobs are on: with orbit 0 and distance 0
        // the strike sequence of a seed stays exactly what it was before S5.
        let (az, az_speed) = if orbit > 0.0 {
            let az = rng.unit();
            let dir = if rng.unit() < 0.5 { -1.0 } else { 1.0 };
            (az, dir * orbit * orbit * 0.33 * (0.6 + 0.8 * rng.unit()))
        } else {
            (0.0, 0.0)
        };
        let dist = if distance > 0.0 {
            (distance + (rng.unit() - 0.5) * 0.4 * distance).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.voices[i] = Voice {
            az,
            az_speed,
            dist,
            lp_l: 0.0,
            lp_r: 0.0,
            hz,
            car: 0.0,
            modp: 0.0,
            part: 0.0,
            env: 1.0,
            env2: 1.0,
            amp: 0.35 * vel,
            index: 0.3 + brightness * 3.5,
            active: true,
        };
    }
}

impl BellSource {
    #[allow(clippy::too_many_arguments)]
    #[inline]
    fn render_span(
        &mut self,
        out: &mut [f32],
        mut right: Option<&mut [f32]>,
        mut send: Option<&mut [f32]>,
        p: &BellParams,
        sr: f32,
        k1: f32,
        k2: f32,
        bright: f32,
        mod_ratio: f32,
    ) {
        let stereo = right.is_some();
        for (i, s) in out.iter_mut().enumerate() {
            let _ = self.density.next(p.density.target());
            let _ = self.decay.next(p.decay_s.target());
            let _ = self.brightness.next(p.brightness.target());
            let _ = self.spread.next(p.velocity_spread.target());
            let mut acc = 0.0f32;
            let (mut acc_l, mut acc_r, mut acc_send) = (0.0f32, 0.0f32, 0.0f32);
            for v in self.voices.iter_mut() {
                if !v.active {
                    continue;
                }
                let m = (v.modp * std::f32::consts::TAU).sin() * v.index * v.env * v.env;
                let tone = (v.car * std::f32::consts::TAU + m).sin();
                let partial = (v.part * std::f32::consts::TAU).sin() * 0.3 * bright * v.env2;
                let x = (tone * v.env + partial) * v.amp;
                acc += x;
                if stereo {
                    // Orbit: constant power around the head, pan = sin(azimuth).
                    let (pl, pr) = if v.az != 0.0 || v.az_speed != 0.0 {
                        let pan = (v.az * std::f32::consts::TAU).sin() * 0.9;
                        let th = (pan + 1.0) * 0.25 * std::f32::consts::PI;
                        (
                            th.cos() * std::f32::consts::SQRT_2,
                            th.sin() * std::f32::consts::SQRT_2,
                        )
                    } else {
                        (1.0, 1.0)
                    };
                    if v.dist > 0.0 {
                        // Distance: quieter, darker, more of it in the reverb.
                        let near = 1.0 - v.dist;
                        let g = 1.0 / (1.0 + 2.5 * v.dist);
                        let cutoff = 1500.0 + 13_500.0 * near * near;
                        let a = 1.0 - (-std::f32::consts::TAU * cutoff / sr).exp();
                        v.lp_l += a * (x * pl - v.lp_l);
                        v.lp_r += a * (x * pr - v.lp_r);
                        acc_l += v.lp_l * g;
                        acc_r += v.lp_r * g;
                        acc_send += x * v.dist * 0.8;
                    } else {
                        acc_l += x * pl;
                        acc_r += x * pr;
                    }
                    v.az = (v.az + v.az_speed / sr).rem_euclid(1.0);
                }
                v.car += v.hz / sr;
                v.modp += v.hz * mod_ratio / sr;
                v.part += v.hz * 2.756 / sr;
                if v.car >= 1.0 {
                    v.car -= 1.0;
                }
                if v.modp >= 1.0 {
                    v.modp -= 1.0;
                }
                if v.part >= 1.0 {
                    v.part -= 1.0;
                }
                v.env *= k1;
                v.env2 *= k2;
                if v.env < 1e-4 {
                    v.active = false;
                }
            }
            if let Some(r) = right.as_deref_mut() {
                *s = acc_l;
                r[i] = acc_r;
                if let Some(sd) = send.as_deref_mut() {
                    sd[i] = acc_send;
                }
            } else {
                *s = acc;
            }
        }
    }
}

/// S2 "tension": with probability `tension` the strike lands a semitone or a tritone
/// above `base`; the caller resolves to `base` on the next strike.
pub fn tense_pitch(rng: &mut Rng, base: f32, tension: f32) -> (f32, bool) {
    if base > 0.0 && tension > 0.0 && rng.unit() < tension {
        let ratio = if rng.unit() < 0.5 {
            2f32.powf(1.0 / 12.0)
        } else {
            2f32.sqrt()
        };
        (base * ratio, true)
    } else {
        (base, false)
    }
}

impl Source for BellSource {
    fn render_at(&mut self, out: &mut [f32], sample_rate: u32, frame: u64) {
        self.frame = frame;
        self.render(out, sample_rate);
    }

    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        self.render_inner(out, None, None, sample_rate);
    }

    /// S5: strikes orbit and sit at a distance (their own pan, darkening and reverb send).
    /// With orbit 0 and distance 0 both sides equal the mono render.
    fn render_stereo(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        send: &mut [f32],
        sample_rate: u32,
        frame: u64,
    ) -> bool {
        self.frame = frame;
        self.render_inner(left, Some(right), Some(send), sample_rate);
        true
    }

    fn reset(&mut self) {
        self.trigger.reset(self.seed_used);
        self.voices = [Voice::default(); BELL_VOICES];
        self.next_voice = 0;
    }
}

impl BellSource {
    fn render_inner(
        &mut self,
        out: &mut [f32],
        right: Option<&mut [f32]>,
        send: Option<&mut [f32]>,
        sample_rate: u32,
    ) {
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
        self.decay.begin(p.decay_s.target(), ramp);
        self.brightness.begin(p.brightness.target(), ramp);
        self.spread.begin(p.velocity_spread.target(), ramp);
        let n = out.len();
        let density = if p.pitches.is_empty() {
            0.0
        } else {
            self.density.current
        };
        let strike_at = self.trigger.tick_on_grid(
            n,
            density,
            sr,
            beat_grid(
                self.clock.as_deref(),
                self.frame,
                sample_rate,
                p.sync.load(Ordering::Relaxed),
            ),
        );
        // Decay coefficients per block: env reaches 1e-3 after decay_s.
        let decay = self.decay.current.max(0.05);
        let k1 = (-6.9078 / (decay * sr)).exp();
        let k2 = (-6.9078 / (decay * 0.35 * sr)).exp(); // the bright partial dies faster
        let bright = self.brightness.current;
        let mod_ratio = 1.4 + bright * 2.1; // 1.4 (soft) … 3.5 (metallic)
                                            // The strike lands on its exact sample, so the output does not depend on how
                                            // the host chunks the audio (offline bounce == live).
                                            // Keyboard strikes (K3) land at the block start.
        while let Some(k) = p.live.pop() {
            if k.on {
                let spread = self.spread.current;
                self.strike(midi_hz(k.midi), bright, spread);
            }
        }
        let at = strike_at.unwrap_or(n).min(n);
        let (head, tail) = out.split_at_mut(at);
        let (r_head, r_tail) = match right {
            Some(r) => {
                let (a, b) = r.split_at_mut(at);
                (Some(a), Some(b))
            }
            None => (None, None),
        };
        let (s_head, s_tail) = match send {
            Some(s) => {
                let (a, b) = s.split_at_mut(at);
                (Some(a), Some(b))
            }
            None => (None, None),
        };
        self.render_span(head, r_head, s_head, &p, sr, k1, k2, bright, mod_ratio);
        if strike_at.is_some() {
            let tension = p.tension.target();
            let hz = if self.resolve_to > 0.0 {
                std::mem::replace(&mut self.resolve_to, 0.0)
            } else {
                let base = p.pitches.pick(self.trigger.rng()).unwrap_or(0.0);
                let (hz, tense) = tense_pitch(self.trigger.rng(), base, tension);
                if tense {
                    self.resolve_to = base;
                }
                hz
            };
            if hz > 0.0 {
                let (b, s) = (self.brightness.current, self.spread.current);
                self.strike(hz, b, s);
            }
        }
        self.render_span(tail, r_tail, s_tail, &p, sr, k1, k2, bright, mod_ratio);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};
    use crate::pitch::{contour, PitchConfig};

    fn render(set: impl Fn(&BellParams), seconds: f64) -> AudioBuffer {
        let p = Arc::new(BellParams::default());
        set(&p);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(BellSource::new(p)));
        g.render_offline(seconds, 1)
    }

    fn strikes(b: &AudioBuffer) -> usize {
        // Count rising edges of the 20 ms envelope above −30 dBFS.
        let mut count = 0;
        let mut above = false;
        for c in b.samples.chunks(960) {
            let peak = c.iter().fold(0.0f32, |m, x| m.max(x.abs()));
            let now = peak > 0.03;
            if now && !above {
                count += 1;
            }
            above = now;
        }
        count
    }

    #[test]
    fn density_sets_how_often_bells_ring_and_silence_is_kept() {
        let b = render(
            |p| {
                p.pitches.set(&[440.0, 554.4, 659.3]);
                p.density.set(30.0);
                p.decay_s.set(0.5);
            },
            60.0,
        );
        assert!(b.samples.iter().all(|s| s.is_finite()));
        assert!(b.peak() <= 1.0);
        let n = strikes(&b);
        assert!((15..=50).contains(&n), "{n} strikes in a minute at 30/min");
        let quiet = render(
            |p| {
                p.pitches.set(&[440.0]);
                p.density.set(0.0);
            },
            5.0,
        );
        assert_eq!(quiet.peak(), 0.0);
        let no_pitches = render(|p| p.density.set(60.0), 5.0);
        assert_eq!(no_pitches.peak(), 0.0);
    }

    #[test]
    fn a_soft_bell_is_in_tune() {
        let b = render(
            |p| {
                p.pitches.set(&[440.0]);
                p.density.set(1.0); // one strike early, then nothing for a while
                p.brightness.set(0.0);
                p.velocity_spread.set(0.0);
                p.decay_s.set(6.0);
            },
            8.0,
        );
        let first = b
            .samples
            .iter()
            .position(|s| s.abs() > 0.01)
            .expect("a strike") as f64
            / 48_000.0;
        let mut hz: Vec<f64> = contour(&b, &PitchConfig::new(48_000), 0.02)
            .into_iter()
            .filter(|p| p.t > first + 0.1 && p.t < first + 1.5)
            .filter_map(|p| p.hz)
            .collect();
        hz.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = hz[hz.len() / 2];
        let cents = 1200.0 * (med / 440.0).log2();
        assert!(cents.abs() < 5.0, "{med:.2} Hz ({cents:.1} c)");
    }

    #[test]
    fn seed_reproduces_and_changes_the_sequence() {
        let a = render(
            |p| {
                p.pitches.set(&[220.0, 330.0]);
                p.density.set(40.0);
            },
            10.0,
        );
        let b = render(
            |p| {
                p.pitches.set(&[220.0, 330.0]);
                p.density.set(40.0);
            },
            10.0,
        );
        let c = render(
            |p| {
                p.pitches.set(&[220.0, 330.0]);
                p.density.set(40.0);
                p.seed.store(99, Ordering::Relaxed);
            },
            10.0,
        );
        assert_eq!(a.samples, b.samples);
        assert_ne!(a.samples, c.samples);
    }

    #[test]
    fn tension_lands_off_the_mode_about_as_often_as_asked() {
        let mut rng = Rng::new(42);
        let semi = 2f32.powf(1.0 / 12.0);
        let tri = 2f32.sqrt();
        let mut tense = 0;
        for _ in 0..10_000 {
            let (hz, t) = tense_pitch(&mut rng, 440.0, 0.3);
            if t {
                tense += 1;
                let r = hz / 440.0;
                assert!(
                    (r - semi).abs() < 1e-4 || (r - tri).abs() < 1e-4,
                    "ratio {r}"
                );
            } else {
                assert_eq!(hz, 440.0);
            }
        }
        assert!((2_700..=3_300).contains(&tense), "{tense} tense of 10000");
        for _ in 0..1000 {
            assert!(!tense_pitch(&mut rng, 440.0, 0.0).1, "no tension at 0");
        }
    }

    #[test]
    fn a_key_strikes_a_silent_bell() {
        let p = Arc::new(BellParams::default());
        p.pitches.set(&[440.0]);
        p.density.set(0.0);
        let quiet = {
            let mut src = BellSource::new(Arc::clone(&p));
            let mut out = vec![0.0f32; 48_000];
            src.render(&mut out, 48_000);
            out.iter().fold(0.0f32, |m, v| m.max(v.abs()))
        };
        assert!(quiet < 1e-6, "no density, no key: silence ({quiet})");
        p.live.note(81, true, 0.8);
        let mut src = BellSource::new(Arc::clone(&p));
        let mut out = vec![0.0f32; 48_000];
        src.render(&mut out, 48_000);
        let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.01, "a key strikes the bell ({peak})");
    }

    fn stereo(set: impl Fn(&BellParams), seconds: f64) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let p = Arc::new(BellParams::default());
        p.pitches.set(&[440.0, 554.4, 659.3]);
        p.density.set(40.0);
        p.decay_s.set(3.0);
        set(&p);
        let mut src = BellSource::new(p);
        let n = (seconds * 48_000.0) as usize;
        let (mut l, mut r, mut s) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        for ((cl, cr), cs) in l
            .chunks_mut(512)
            .zip(r.chunks_mut(512))
            .zip(s.chunks_mut(512))
        {
            src.render_stereo(cl, cr, cs, 48_000, 0);
        }
        (l, r, s)
    }

    #[test]
    fn orbit_and_distance_move_the_strikes_and_off_is_the_mono_sound() {
        // Off: both sides equal the mono render, nothing to the reverb.
        let (l, r, s) = stereo(|_| {}, 10.0);
        let p = Arc::new(BellParams::default());
        p.pitches.set(&[440.0, 554.4, 659.3]);
        p.density.set(40.0);
        p.decay_s.set(3.0);
        let mut mono_src = BellSource::new(p);
        let mut mono = vec![0.0f32; l.len()];
        for c in mono.chunks_mut(512) {
            mono_src.render(c, 48_000);
        }
        assert_eq!(l, r);
        assert_eq!(l, mono);
        assert!(s.iter().all(|x| *x == 0.0));
        // Orbit: the sides differ and their balance moves over time.
        let (l, r, _) = stereo(|p| p.orbit.set(1.0), 10.0);
        let diff: f32 = l.iter().zip(&r).map(|(a, b)| (a - b).abs()).sum();
        let total: f32 = l.iter().zip(&r).map(|(a, b)| a.abs() + b.abs()).sum();
        assert!(
            diff > total * 0.1,
            "orbit moves the image ({diff} of {total})"
        );
        let balance: Vec<f32> = l
            .chunks(24_000)
            .zip(r.chunks(24_000))
            .map(|(a, b)| {
                let el: f32 = a.iter().map(|x| x * x).sum();
                let er: f32 = b.iter().map(|x| x * x).sum();
                (el - er) / (el + er + 1e-9)
            })
            .collect();
        let (lo, hi) = balance
            .iter()
            .fold((1.0f32, -1.0f32), |(a, b), x| (a.min(*x), b.max(*x)));
        assert!(hi - lo > 0.3, "the balance moves: {lo}..{hi}");
        assert!(l.iter().chain(&r).all(|x| x.is_finite() && x.abs() <= 1.5));
        // Distance: quieter at the dry outputs, sent to the reverb instead.
        let (near_l, _, near_s) = stereo(|p| p.distance.set(0.0), 10.0);
        let (far_l, _, far_s) = stereo(|p| p.distance.set(1.0), 10.0);
        let e = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>();
        assert!(e(&far_l) < e(&near_l) * 0.5, "far is quieter");
        assert!(e(&far_s) > e(&near_s) + 1e-3, "far goes to the reverb");
    }
}
