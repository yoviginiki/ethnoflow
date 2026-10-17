//! Beat clock and the drum machine (F4.1, engine side). The clock is a shared,
//! lock-free anchor (frame, beat, bpm): every rhythmic source derives the same beat
//! position from the graph's frame counter, so drums, bass and ostinato stay in
//! sync and a rebuilt source lands back on the grid. Drums are synthesized (no
//! samples): kick, snare, ghost, closed/open hat, shaker.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use super::gen::{soft, Rng, Svf};
use crate::graph::params::{ramp_samples, undenormal, Ramp, SmoothedParam};
use crate::graph::Source;

/// Shared musical time. `beat_at(frame)` is defined while `playing`.
pub struct BeatClock {
    pub playing: AtomicBool,
    bpm_bits: AtomicU32,
    anchor_frame: AtomicU64,
    anchor_beat_bits: AtomicU64,
    /// Bumped on every start/seek so sources re-find their place in the pattern.
    pub generation: AtomicU32,
}

impl BeatClock {
    pub fn new(bpm: f32) -> Self {
        Self {
            playing: AtomicBool::new(false),
            bpm_bits: AtomicU32::new(bpm.clamp(20.0, 600.0).to_bits()),
            anchor_frame: AtomicU64::new(0),
            anchor_beat_bits: AtomicU64::new(0f64.to_bits()),
            generation: AtomicU32::new(0),
        }
    }

    pub fn bpm(&self) -> f32 {
        f32::from_bits(self.bpm_bits.load(Ordering::Relaxed))
    }

    /// Start at `from_beat` on graph frame `at_frame`.
    pub fn start(&self, at_frame: u64, from_beat: f64) {
        self.anchor_frame.store(at_frame, Ordering::Relaxed);
        self.anchor_beat_bits
            .store(from_beat.max(0.0).to_bits(), Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.playing.store(true, Ordering::Relaxed);
    }

    pub fn stop(&self) {
        self.playing.store(false, Ordering::Relaxed);
    }

    /// Change the tempo without a jump: the current beat becomes the new anchor.
    pub fn set_bpm(&self, bpm: f32, at_frame: u64, sample_rate: u32) {
        if let Some(beat) = self.beat_at(at_frame, sample_rate) {
            self.anchor_frame.store(at_frame, Ordering::Relaxed);
            self.anchor_beat_bits
                .store(beat.to_bits(), Ordering::Relaxed);
        }
        self.bpm_bits
            .store(bpm.clamp(20.0, 600.0).to_bits(), Ordering::Relaxed);
    }

    /// Beat position at a graph frame, or None when stopped.
    #[inline]
    pub fn beat_at(&self, frame: u64, sample_rate: u32) -> Option<f64> {
        if !self.playing.load(Ordering::Relaxed) {
            return None;
        }
        let anchor_frame = self.anchor_frame.load(Ordering::Relaxed);
        let anchor_beat = f64::from_bits(self.anchor_beat_bits.load(Ordering::Relaxed));
        let bpm = self.bpm() as f64;
        let elapsed = frame as f64 - anchor_frame as f64;
        Some(anchor_beat + elapsed * bpm / (60.0 * sample_rate.max(1) as f64))
    }

    /// Beats per frame at the current tempo.
    #[inline]
    pub fn beats_per_frame(&self, sample_rate: u32) -> f64 {
        self.bpm() as f64 / (60.0 * sample_rate.max(1) as f64)
    }
}

/// Walks a looped, beat-sorted event list sample by sample. `events` must be sorted
/// by `beat` and lie inside `[0, length)`.
pub struct Cursor {
    next: usize,
    loops: f64,
    generation: u32,
    primed: bool,
}

impl Cursor {
    pub fn new() -> Self {
        Self {
            next: 0,
            loops: 0.0,
            generation: u32::MAX,
            primed: false,
        }
    }

    /// Place the cursor at `beat` (after a start/seek or a pattern swap).
    pub fn seek(&mut self, beats: &[f64], length: f64, beat: f64) {
        if length <= 0.0 || beats.is_empty() {
            self.next = 0;
            self.loops = 0.0;
            self.primed = true;
            return;
        }
        self.loops = (beat / length).floor();
        let local = beat - self.loops * length;
        self.next = beats
            .iter()
            .position(|&b| b >= local)
            .unwrap_or(beats.len());
        self.primed = true;
    }

    /// Events (indices) due at or before `beat`, in order. Call once per sample.
    #[inline]
    pub fn due(&mut self, beats: &[f64], length: f64, beat: f64) -> Option<usize> {
        if beats.is_empty() || length <= 0.0 {
            return None;
        }
        if self.next >= beats.len() {
            // Wrap to the next loop.
            self.next = 0;
            self.loops += 1.0;
        }
        let at = beats[self.next] + self.loops * length;
        if beat >= at {
            let i = self.next;
            self.next += 1;
            Some(i)
        } else {
            None
        }
    }

    pub fn needs_seek(&self, generation: u32) -> bool {
        !self.primed || self.generation != generation
    }

    pub fn mark(&mut self, generation: u32) {
        self.generation = generation;
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DrumKind {
    Kick = 0,
    Snare = 1,
    Ghost = 2,
    HatClosed = 3,
    HatOpen = 4,
    Shaker = 5,
}

pub const DRUM_KINDS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrumEvent {
    pub beat: f64,
    pub kind: DrumKind,
    pub velocity: f32,
}

/// A looped drum pattern: events sorted by beat inside `[0, length_beats)`.
pub struct DrumPatternData {
    pub events: Vec<DrumEvent>,
    pub beats: Vec<f64>,
    pub length_beats: f64,
}

impl DrumPatternData {
    pub fn new(mut events: Vec<DrumEvent>, length_beats: f64) -> Self {
        events.retain(|e| e.beat >= 0.0 && e.beat < length_beats.max(1e-9));
        events.sort_by(|a, b| a.beat.partial_cmp(&b.beat).unwrap());
        let beats = events.iter().map(|e| e.beat).collect();
        Self {
            events,
            beats,
            length_beats,
        }
    }
}

pub struct DrumParams {
    /// Level per kind, dB (−60..+6).
    pub level_db: [SmoothedParam; DRUM_KINDS],
    /// Kick fundamental, Hz.
    pub kick_hz: SmoothedParam,
    /// 0..1 overall decay scale (short … long).
    pub decay: SmoothedParam,
    /// 0..1 brightness of the hats / snare noise.
    pub brightness: SmoothedParam,
    /// Kick envelope (0..1 bits) for sidechain readers.
    kick_env_bits: AtomicU32,
}

impl Default for DrumParams {
    fn default() -> Self {
        Self {
            level_db: std::array::from_fn(|k| {
                SmoothedParam::new(if k == 2 { -8.0 } else { 0.0 }, -60.0, 6.0)
            }),
            kick_hz: SmoothedParam::new(50.0, 35.0, 90.0),
            decay: SmoothedParam::new(0.5, 0.0, 1.0),
            brightness: SmoothedParam::new(0.5, 0.0, 1.0),
            kick_env_bits: AtomicU32::new(0),
        }
    }
}

impl DrumParams {
    /// Current kick envelope 0..1 (for sidechain ducking).
    pub fn kick_env(&self) -> f32 {
        f32::from_bits(self.kick_env_bits.load(Ordering::Relaxed))
    }
}

#[derive(Clone, Copy, Default)]
struct DrumVoice {
    env: f32,
    env2: f32,
    phase: f32,
    vel: f32,
    t: f32,
    active: bool,
}

pub struct DrumSource {
    params: Arc<DrumParams>,
    clock: Arc<BeatClock>,
    pattern: Arc<DrumPatternData>,
    cursor: Cursor,
    voices: [DrumVoice; DRUM_KINDS],
    rng: Rng,
    bp: Svf,
    hp: Svf,
    shaker_bp: Svf,
    levels: [Ramp; DRUM_KINDS],
    frame: u64,
}

impl DrumSource {
    pub fn new(
        params: Arc<DrumParams>,
        clock: Arc<BeatClock>,
        pattern: Arc<DrumPatternData>,
    ) -> Self {
        Self {
            levels: std::array::from_fn(|k| Ramp::new(params.level_db[k].target())),
            params,
            clock,
            pattern,
            cursor: Cursor::new(),
            voices: [DrumVoice::default(); DRUM_KINDS],
            rng: Rng::new(0xD2),
            bp: Svf::default(),
            hp: Svf::default(),
            shaker_bp: Svf::default(),
            frame: 0,
        }
    }

    fn trigger(&mut self, kind: DrumKind, vel: f32) {
        let v = &mut self.voices[kind as usize];
        v.env = 1.0;
        v.env2 = 1.0;
        v.phase = 0.0;
        v.t = 0.0;
        v.vel = vel.clamp(0.05, 1.0);
        v.active = true;
    }
}

fn db_to_gain(db: f32) -> f32 {
    crate::graph::params::db_to_gain(db)
}

impl Source for DrumSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let f = self.frame;
        self.render_at(out, sample_rate, f);
        self.frame += out.len() as u64;
    }

    fn render_at(&mut self, out: &mut [f32], sample_rate: u32, frame: u64) {
        let sr = sample_rate.max(1) as f32;
        let p = Arc::clone(&self.params);
        let ramp = ramp_samples(sample_rate);
        for (k, l) in self.levels.iter_mut().enumerate() {
            l.begin(p.level_db[k].target(), ramp);
        }
        let kick_hz = p.kick_hz.target();
        let decay = 0.5 + p.decay.target(); // 0.5 … 1.5 × the base decays
        let bright = p.brightness.target();
        // Per-sample decay coefficients (to −60 dB over the base time × decay).
        let k_kick = (-6.9078 / (0.32 * decay * sr)).exp();
        let k_kick_pitch = (-6.9078 / (0.045 * sr)).exp();
        let k_snare = (-6.9078 / (0.18 * decay * sr)).exp();
        let k_snare_tone = (-6.9078 / (0.08 * sr)).exp();
        let k_ghost = (-6.9078 / (0.09 * decay * sr)).exp();
        let k_hat = (-6.9078 / (0.05 * decay * sr)).exp();
        let k_open = (-6.9078 / (0.32 * decay * sr)).exp();
        let k_shaker = (-6.9078 / (0.11 * decay * sr)).exp();
        self.bp.set(1500.0 + bright * 1500.0, 1.2, sr);
        self.hp.set(5500.0 + bright * 3500.0, 0.9, sr);
        self.shaker_bp.set(4500.0 + bright * 2500.0, 2.5, sr);
        let beat0 = self.clock.beat_at(frame, sample_rate);
        let bpf = self.clock.beats_per_frame(sample_rate);
        let generation = self.clock.generation.load(Ordering::Relaxed);
        if let Some(b) = beat0 {
            if self.cursor.needs_seek(generation) {
                self.cursor
                    .seek(&self.pattern.beats, self.pattern.length_beats, b);
                self.cursor.mark(generation);
            }
        }
        let mut kick_env_peak = 0.0f32;
        for (i, s) in out.iter_mut().enumerate() {
            if let Some(b0) = beat0 {
                let beat = b0 + i as f64 * bpf;
                while let Some(idx) =
                    self.cursor
                        .due(&self.pattern.beats, self.pattern.length_beats, beat)
                {
                    let e = self.pattern.events[idx];
                    self.trigger(e.kind, e.velocity);
                }
            }
            let noise = self.rng.bipolar();
            let bp = self.bp.process(noise);
            let hp = self.hp.process(noise);
            let sh = self.shaker_bp.process(noise);
            let mut acc = 0.0f32;
            for k in 0..DRUM_KINDS {
                let g = db_to_gain(self.levels[k].next(p.level_db[k].target()));
                let v = &mut self.voices[k];
                if !v.active {
                    continue;
                }
                let y = match k {
                    0 => {
                        // Kick: sine sweeping from 3× down to the fundamental, plus a click.
                        let hz = kick_hz * (1.0 + 2.2 * v.env2);
                        v.phase += hz / sr;
                        if v.phase >= 1.0 {
                            v.phase -= 1.0;
                        }
                        let body = (v.phase * std::f32::consts::TAU).sin() * v.env;
                        let click = if v.t < 0.003 {
                            noise * 0.4 * (1.0 - v.t / 0.003)
                        } else {
                            0.0
                        };
                        v.env *= k_kick;
                        v.env2 *= k_kick_pitch;
                        kick_env_peak = kick_env_peak.max(v.env * v.vel);
                        soft((body + click) * 1.2) * v.vel
                    }
                    1 | 2 => {
                        // Snare / ghost: band-passed noise plus a short tone.
                        v.phase += 190.0 / sr;
                        if v.phase >= 1.0 {
                            v.phase -= 1.0;
                        }
                        let tone = (v.phase * std::f32::consts::TAU).sin() * v.env2 * 0.6;
                        let y = (bp.bp * 0.9 + tone) * v.env;
                        v.env *= if k == 1 { k_snare } else { k_ghost };
                        v.env2 *= k_snare_tone;
                        y * v.vel * if k == 2 { 0.55 } else { 0.8 }
                    }
                    3 => {
                        let y = hp.hp * v.env;
                        v.env *= k_hat;
                        y * v.vel * 0.35
                    }
                    4 => {
                        let y = hp.hp * v.env;
                        v.env *= k_open;
                        y * v.vel * 0.3
                    }
                    _ => {
                        // Shaker: a soft attack so it rustles instead of ticking.
                        let attack = (v.t / 0.012).min(1.0);
                        let y = sh.bp * v.env * attack;
                        v.env *= k_shaker;
                        y * v.vel * 0.5
                    }
                };
                v.t += 1.0 / sr;
                if v.env < 1e-4 {
                    v.active = false;
                }
                acc += y * g;
            }
            *s = undenormal(soft(acc));
        }
        self.params
            .kick_env_bits
            .store(kick_env_peak.to_bits(), Ordering::Relaxed);
    }

    fn reset(&mut self) {
        self.voices = [DrumVoice::default(); DRUM_KINDS];
        self.cursor = Cursor::new();
        self.frame = 0;
    }
}

/// Convenience: linear gain of a level in dB (re-exported for the other rhythm sources).
pub fn level_gain(db: f32) -> f32 {
    db_to_gain(db)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::AudioBuffer;
    use crate::graph::{Graph, GraphParams};

    fn two_step() -> Arc<DrumPatternData> {
        let ev = |beat: f64, kind: DrumKind, velocity: f32| DrumEvent {
            beat,
            kind,
            velocity,
        };
        Arc::new(DrumPatternData::new(
            vec![
                ev(0.0, DrumKind::Kick, 1.0),
                ev(1.0, DrumKind::Snare, 1.0),
                ev(2.5, DrumKind::Kick, 0.9),
                ev(3.0, DrumKind::Snare, 1.0),
                ev(0.5, DrumKind::HatClosed, 0.5),
                ev(1.5, DrumKind::HatClosed, 0.5),
                ev(2.0, DrumKind::Ghost, 0.3),
                ev(3.75, DrumKind::Shaker, 0.4),
            ],
            4.0,
        ))
    }

    fn render(clock: &Arc<BeatClock>, params: &Arc<DrumParams>, seconds: f64) -> AudioBuffer {
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(
            0,
            Box::new(DrumSource::new(
                Arc::clone(params),
                Arc::clone(clock),
                two_step(),
            )),
        );
        g.render_offline(seconds, 1)
    }

    #[test]
    fn clock_math_and_tempo_change_keep_the_beat() {
        let c = BeatClock::new(120.0);
        assert!(c.beat_at(1000, 48_000).is_none());
        c.start(48_000, 2.0);
        assert!((c.beat_at(48_000 + 24_000, 48_000).unwrap() - 3.0).abs() < 1e-9);
        c.set_bpm(60.0, 48_000 + 24_000, 48_000);
        assert!(
            (c.beat_at(48_000 + 24_000, 48_000).unwrap() - 3.0).abs() < 1e-9,
            "no jump"
        );
        assert!((c.beat_at(48_000 + 72_000, 48_000).unwrap() - 4.0).abs() < 1e-9);
        c.stop();
        assert!(c.beat_at(0, 48_000).is_none());
        let mut cur = Cursor::new();
        let beats = [0.0, 1.0, 2.5, 3.0];
        cur.seek(&beats, 4.0, 6.0); // loop 1, local 2 → next is 2.5
        assert_eq!(cur.due(&beats, 4.0, 6.4), None);
        assert_eq!(cur.due(&beats, 4.0, 6.5), Some(2));
        assert_eq!(cur.due(&beats, 4.0, 7.0), Some(3));
        assert_eq!(cur.due(&beats, 4.0, 7.9), None);
        assert_eq!(cur.due(&beats, 4.0, 8.0), Some(0), "wraps to the next loop");
    }

    #[test]
    fn drums_hit_on_the_grid_publish_the_kick_envelope_and_are_deterministic() {
        let clock = Arc::new(BeatClock::new(120.0)); // beat = 0.5 s
        let params = Arc::new(DrumParams::default());
        clock.start(0, 0.0);
        let a = render(&clock, &params, 4.0);
        let b = render(&clock, &params, 4.0);
        assert_eq!(a.samples, b.samples);
        assert!(a.samples.iter().all(|s| s.is_finite()));
        assert!(a.peak() <= 1.0);
        // Transients: energy in the 30 ms after each kick/snare beat is far above the
        // 30 ms just before it.
        let sr = 48_000.0;
        for beat in [0.0f64, 1.0, 2.5, 3.0, 4.0, 5.0] {
            let at = (beat * 0.5 * sr) as usize;
            let after = a.slice_frames(at, at + 1440).rms();
            let before = if at >= 1440 {
                a.slice_frames(at - 1440, at).rms()
            } else {
                0.0
            };
            assert!(
                after > before * 2.0 + 0.01,
                "transient at beat {beat}: {before} → {after}"
            );
        }
        // The kick envelope was published (last block of the render is quiet, so check
        // by rendering a short slice right after a kick).
        let clock2 = Arc::new(BeatClock::new(120.0));
        let p2 = Arc::new(DrumParams::default());
        clock2.start(0, 0.0);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(
            0,
            Box::new(DrumSource::new(Arc::clone(&p2), clock2, two_step())),
        );
        let mut buf = vec![0.0f32; 512];
        g.render(&mut buf, 1);
        assert!(p2.kick_env() > 0.5, "kick env {}", p2.kick_env());
        // Stopped clock: silence after the tails.
        let stopped = Arc::new(BeatClock::new(120.0));
        let quiet = render(&stopped, &Arc::new(DrumParams::default()), 1.0);
        assert_eq!(quiet.peak(), 0.0);
    }
}
