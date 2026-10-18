//! Tapan (P1 R3): the Balkan two-headed drum, synthesized. Three strokes on the shared
//! beat clock: the **boom** (low head, beater: a pitched sweep with a skin-noise burst
//! and a second membrane partial), the **rim** (thin stick on the high head / rim: a
//! band-passed click with a short ping) and the **ghost** (soft tap between accents).
//! The pattern reuses `DrumPatternData` (Kick = boom, Snare = rim, Ghost = ghost) so the
//! shell's pattern hand-off and the offline renderer need no new channel type.

use std::sync::Arc;

use super::gen::{soft, Rng, Svf};
use super::rhythm::{BeatClock, Cursor, DrumKind, DrumPatternData, PatternInbox};
use crate::graph::params::undenormal;
use crate::graph::params::{db_to_gain, ramp_samples, Ramp, SmoothedParam};
use crate::graph::Source;

pub const TAPAN_STROKES: usize = 3;

pub struct TapanParams {
    /// Boom fundamental, Hz (45..110).
    pub tune_hz: SmoothedParam,
    /// 0..1 overall decay (short, dry … long, open hall drum).
    pub decay: SmoothedParam,
    /// 0..1 skin noise in the boom (felt beater … hard beater).
    pub skin: SmoothedParam,
    /// 0..1 rim brightness (dull wood … bright metal ring).
    pub rim_tone: SmoothedParam,
    /// Level per stroke (boom, rim, ghost), dB (−60..+6).
    pub level_db: [SmoothedParam; TAPAN_STROKES],
}

impl Default for TapanParams {
    fn default() -> Self {
        Self {
            tune_hz: SmoothedParam::new(70.0, 45.0, 110.0),
            decay: SmoothedParam::new(0.5, 0.0, 1.0),
            skin: SmoothedParam::new(0.4, 0.0, 1.0),
            rim_tone: SmoothedParam::new(0.5, 0.0, 1.0),
            level_db: std::array::from_fn(|k| {
                SmoothedParam::new(if k == 2 { -10.0 } else { 0.0 }, -60.0, 6.0)
            }),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct Stroke {
    env: f32,
    env2: f32,
    phase: f32,
    phase2: f32,
    vel: f32,
    t: f32,
    active: bool,
}

pub struct TapanSource {
    params: Arc<TapanParams>,
    clock: Arc<BeatClock>,
    pattern: Arc<DrumPatternData>,
    cursor: Cursor,
    strokes: [Stroke; TAPAN_STROKES],
    rng: Rng,
    skin_lp: Svf,
    rim_bp: Svf,
    ghost_bp: Svf,
    levels: [Ramp; TAPAN_STROKES],
    frame: u64,
    inbox: Option<PatternInbox<DrumPatternData>>,
}

impl TapanSource {
    pub fn with_inbox(mut self, inbox: PatternInbox<DrumPatternData>) -> Self {
        self.inbox = Some(inbox);
        self
    }

    pub fn new(
        params: Arc<TapanParams>,
        clock: Arc<BeatClock>,
        pattern: Arc<DrumPatternData>,
    ) -> Self {
        Self {
            levels: std::array::from_fn(|k| Ramp::new(params.level_db[k].target())),
            params,
            clock,
            pattern,
            cursor: Cursor::new(),
            strokes: [Stroke::default(); TAPAN_STROKES],
            rng: Rng::new(0x7A9),
            skin_lp: Svf::default(),
            rim_bp: Svf::default(),
            ghost_bp: Svf::default(),
            frame: 0,
            inbox: None,
        }
    }

    fn stroke_index(kind: DrumKind) -> Option<usize> {
        match kind {
            DrumKind::Kick => Some(0),
            DrumKind::Snare | DrumKind::HatClosed | DrumKind::HatOpen => Some(1),
            DrumKind::Ghost | DrumKind::Shaker => Some(2),
        }
    }

    fn trigger(&mut self, k: usize, vel: f32) {
        let s = &mut self.strokes[k];
        s.env = 1.0;
        s.env2 = 1.0;
        s.phase = 0.0;
        s.phase2 = 0.0;
        s.t = 0.0;
        s.vel = vel.clamp(0.05, 1.0);
        s.active = true;
    }
}

impl Source for TapanSource {
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
        let tune = p.tune_hz.target();
        let decay = 0.5 + p.decay.target(); // 0.5 … 1.5 × base times
        let skin = p.skin.target();
        let rim_tone = p.rim_tone.target();
        let k_boom = (-6.9078 / (0.42 * decay * sr)).exp();
        let k_boom_pitch = (-6.9078 / (0.06 * sr)).exp();
        let k_rim = (-6.9078 / (0.045 * (0.6 + 0.8 * decay) * sr)).exp();
        let k_rim_ping = (-6.9078 / (0.025 * sr)).exp();
        let k_ghost = (-6.9078 / (0.07 * decay * sr)).exp();
        let k_ghost_tone = (-6.9078 / (0.04 * sr)).exp();
        self.skin_lp.set(900.0 + 1800.0 * skin, 0.7, sr);
        self.rim_bp.set(2200.0 + 3800.0 * rim_tone, 3.0, sr);
        self.ghost_bp.set(500.0, 1.5, sr);
        let beat0 = self.clock.beat_at(frame, sample_rate);
        let bpf = self.clock.beats_per_frame(sample_rate);
        let generation = self
            .clock
            .generation
            .load(std::sync::atomic::Ordering::Relaxed);
        let swapped = self
            .inbox
            .as_mut()
            .map(|i| i.take(&mut self.pattern))
            .unwrap_or(false);
        if let Some(b) = beat0 {
            if swapped || self.cursor.needs_seek(generation) {
                self.cursor
                    .seek(&self.pattern.beats, self.pattern.length_beats, b);
                self.cursor.mark(generation);
            }
        }
        for (i, s) in out.iter_mut().enumerate() {
            if let Some(b0) = beat0 {
                let beat = b0 + i as f64 * bpf;
                while let Some(idx) =
                    self.cursor
                        .due(&self.pattern.beats, self.pattern.length_beats, beat)
                {
                    let e = self.pattern.events[idx];
                    if let Some(k) = Self::stroke_index(e.kind) {
                        self.trigger(k, e.velocity);
                    }
                }
            }
            let noise = self.rng.bipolar();
            let skin_n = self.skin_lp.process(noise).lp;
            let rim_n = self.rim_bp.process(noise).bp;
            let ghost_n = self.ghost_bp.process(noise).bp;
            let mut acc = 0.0f32;
            for k in 0..TAPAN_STROKES {
                let g = db_to_gain(self.levels[k].next(p.level_db[k].target()));
                let v = &mut self.strokes[k];
                if !v.active {
                    continue;
                }
                let y = match k {
                    0 => {
                        // Boom: fundamental sweeping from 1.8× down, a 1.5× membrane
                        // partial that dies faster, and a skin burst at the attack.
                        let hz = tune * (1.0 + 0.8 * v.env2);
                        v.phase += hz / sr;
                        if v.phase >= 1.0 {
                            v.phase -= 1.0;
                        }
                        v.phase2 += hz * 1.5 / sr;
                        if v.phase2 >= 1.0 {
                            v.phase2 -= 1.0;
                        }
                        let body = (v.phase * std::f32::consts::TAU).sin() * v.env
                            + (v.phase2 * std::f32::consts::TAU).sin() * v.env * v.env2 * 0.35;
                        let burst = if v.t < 0.03 {
                            skin_n * skin * 1.4 * (1.0 - v.t / 0.03)
                        } else {
                            0.0
                        };
                        v.env *= k_boom;
                        v.env2 *= k_boom_pitch;
                        soft((body + burst) * 1.3) * v.vel
                    }
                    1 => {
                        // Rim: band-passed click plus a short ping whose pitch rises with rim_tone.
                        v.phase += (900.0 + 700.0 * rim_tone) / sr;
                        if v.phase >= 1.0 {
                            v.phase -= 1.0;
                        }
                        let ping = (v.phase * std::f32::consts::TAU).sin() * v.env2 * 0.5;
                        let y = (rim_n * 1.6 + ping) * v.env;
                        v.env *= k_rim;
                        v.env2 *= k_rim_ping;
                        y * v.vel * 0.8
                    }
                    _ => {
                        // Ghost: a soft tap, mostly filtered noise with a touch of tone.
                        v.phase += 180.0 / sr;
                        if v.phase >= 1.0 {
                            v.phase -= 1.0;
                        }
                        let tone = (v.phase * std::f32::consts::TAU).sin() * v.env2 * 0.4;
                        let y = (ghost_n * 0.9 + tone) * v.env;
                        v.env *= k_ghost;
                        v.env2 *= k_ghost_tone;
                        y * v.vel * 0.6
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
    }

    fn reset(&mut self) {
        self.strokes = [Stroke::default(); TAPAN_STROKES];
        self.cursor = Cursor::new();
        self.frame = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::sources::rhythm::DrumEvent;
    use crate::graph::{Graph, GraphParams};

    fn render(kind: DrumKind, tune: f32) -> Vec<f32> {
        let clock = Arc::new(BeatClock::new(120.0));
        clock.start(0, 0.0);
        let pat = Arc::new(DrumPatternData::new(
            vec![DrumEvent {
                beat: 0.0,
                kind,
                velocity: 1.0,
            }],
            4.0,
        ));
        let p = Arc::new(TapanParams::default());
        p.tune_hz.set(tune);
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(TapanSource::new(p, clock, pat)));
        g.render_offline(1.0, 1).samples
    }

    /// Band energy (Goertzel at log-spaced probes) over the window [from, to) in samples.
    fn band_energy(x: &[f32], lo: f32, hi: f32, from: usize, to: usize) -> f32 {
        let sr = 48_000.0f32;
        let mut e = 0.0f32;
        let mut f = lo;
        while f <= hi {
            let w = std::f32::consts::TAU * f / sr;
            let (mut s1, mut s2) = (0.0f32, 0.0f32);
            for &v in x.iter().skip(from).take(to - from) {
                let s0 = v + 2.0 * w.cos() * s1 - s2;
                s2 = s1;
                s1 = s0;
            }
            e += s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2;
            f *= 1.25;
        }
        e
    }

    #[test]
    fn boom_is_low_rim_is_high_and_tune_moves_the_boom() {
        let boom = render(DrumKind::Kick, 70.0);
        let rim = render(DrumKind::Snare, 70.0);
        let peak = |x: &[f32]| x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(
            peak(&boom) > 0.1 && peak(&boom) <= 1.0,
            "boom audible {}",
            peak(&boom)
        );
        assert!(
            peak(&rim) > 0.05 && peak(&rim) <= 1.0,
            "rim audible {}",
            peak(&rim)
        );
        assert!(boom.iter().all(|v| v.is_finite()));
        // Boom measured 0.1–0.3 s after the hit (the skin burst is over): energy sits
        // below 200 Hz. Rim measured over its first 80 ms: energy above 1.5 kHz.
        let lo = band_energy(&boom, 50.0, 200.0, 4800, 14_400);
        let hi = band_energy(&boom, 1500.0, 8000.0, 4800, 14_400);
        assert!(lo > hi * 4.0, "boom low {lo} vs high {hi}");
        let lo_r = band_energy(&rim, 50.0, 200.0, 0, 3840);
        let hi_r = band_energy(&rim, 1500.0, 8000.0, 0, 3840);
        assert!(hi_r > lo_r, "rim high {hi_r} vs low {lo_r}");
        // Tuning: a 100 Hz boom has more energy around 100 Hz than a 55 Hz one.
        let high_tune = render(DrumKind::Kick, 100.0);
        let low_tune = render(DrumKind::Kick, 55.0);
        assert!(
            band_energy(&high_tune, 95.0, 105.0, 4800, 14_400)
                > band_energy(&low_tune, 95.0, 105.0, 4800, 14_400)
        );
        // Strokes decay: the last 100 ms of one second are quiet.
        let tail = peak(&boom[boom.len() - 4800..]);
        assert!(tail < 0.02, "boom tail {tail}");
    }
}
