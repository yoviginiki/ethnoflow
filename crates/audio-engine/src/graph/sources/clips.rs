//! Clips on a timeline (F5.1): a `Source` that mixes audio clips (windows into
//! immutable buffers) at their timeline positions, driven by a shared transport
//! (play/stop/seek/loop through atomics). Track gain/mute/solo are smoothed params.
//! Everything is pre-allocated when the source is built; the callback only reads.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use crate::buffer::AudioBuffer;
use crate::graph::params::{db_to_gain, ramp_samples, Ramp, SmoothedParam};
use crate::graph::Source;

pub const MAX_TRACKS: usize = 8;

/// Shared transport: the UI writes play/seek/loop, the source writes the position.
pub struct Transport {
    pub playing: AtomicBool,
    /// Current timeline position in frames at the stream rate (written by the source).
    pub position_frames: AtomicU64,
    /// Seek request: frame + flag.
    seek_frames: AtomicU64,
    seek_pending: AtomicBool,
    /// Loop region in frames; `loop_on` false = play through.
    pub loop_on: AtomicBool,
    pub loop_start: AtomicU64,
    pub loop_end: AtomicU64,
    /// Timeline end in frames (playback stops there unless looping).
    pub end_frames: AtomicU64,
    pub finished: AtomicBool,
}

impl Default for Transport {
    fn default() -> Self {
        Self {
            playing: AtomicBool::new(false),
            position_frames: AtomicU64::new(0),
            seek_frames: AtomicU64::new(0),
            seek_pending: AtomicBool::new(false),
            loop_on: AtomicBool::new(false),
            loop_start: AtomicU64::new(0),
            loop_end: AtomicU64::new(0),
            end_frames: AtomicU64::new(0),
            finished: AtomicBool::new(false),
        }
    }
}

impl Transport {
    pub fn seek(&self, frame: u64) {
        self.seek_frames.store(frame, Ordering::Relaxed);
        self.seek_pending.store(true, Ordering::Release);
    }

    pub fn position(&self) -> u64 {
        self.position_frames.load(Ordering::Relaxed)
    }
}

pub struct TrackParams {
    pub gain_db: SmoothedParam,
    pub mute: AtomicBool,
    pub solo: AtomicBool,
    /// −1 … +1 constant-power pan (F5.2 track strip).
    pub pan: SmoothedParam,
    /// One-pole cuts per track; 20 kHz / 10 Hz = off.
    pub lowpass_hz: SmoothedParam,
    pub highpass_hz: SmoothedParam,
    /// 0..1 send to the master space (reverb), post-fader.
    pub send: SmoothedParam,
}

impl Default for TrackParams {
    fn default() -> Self {
        Self {
            gain_db: SmoothedParam::new(0.0, -120.0, 12.0),
            mute: AtomicBool::new(false),
            solo: AtomicBool::new(false),
            pan: SmoothedParam::new(0.0, -1.0, 1.0),
            lowpass_hz: SmoothedParam::new(20_000.0, 40.0, 20_000.0),
            highpass_hz: SmoothedParam::new(10.0, 10.0, 8_000.0),
            send: SmoothedParam::new(0.0, 0.0, 1.0),
        }
    }
}

pub struct ClipsParams {
    pub transport: Transport,
    pub tracks: [TrackParams; MAX_TRACKS],
}

impl Default for ClipsParams {
    fn default() -> Self {
        Self {
            transport: Transport::default(),
            tracks: std::array::from_fn(|_| TrackParams::default()),
        }
    }
}

/// A clip ready to play: its buffer is already at the stream rate.
pub struct ClipData {
    pub track: usize,
    pub buffer: Arc<AudioBuffer>,
    /// Timeline start in frames (stream rate).
    pub start: u64,
    /// First frame of `buffer` to play.
    pub offset: u64,
    pub length: u64,
    pub gain: f32,
    pub fade_in: u64,
    pub fade_out: u64,
    pub mute: bool,
}

pub struct ClipSource {
    params: Arc<ClipsParams>,
    clips: Vec<ClipData>,
    track_gain: [Ramp; MAX_TRACKS],
    track_pan: [Ramp; MAX_TRACKS],
    track_send: [Ramp; MAX_TRACKS],
    lp: [f32; MAX_TRACKS],
    hp_in: [f32; MAX_TRACKS],
    hp_out: [f32; MAX_TRACKS],
    position: u64,
}

impl ClipSource {
    pub fn new(params: Arc<ClipsParams>, mut clips: Vec<ClipData>) -> Self {
        clips.retain(|c| c.track < MAX_TRACKS && c.length > 0);
        let position = params.transport.position();
        let track_gain =
            std::array::from_fn(|i| Ramp::new(db_to_gain(params.tracks[i].gain_db.target())));
        let track_pan = std::array::from_fn(|i| Ramp::new(params.tracks[i].pan.target()));
        let track_send = std::array::from_fn(|i| Ramp::new(params.tracks[i].send.target()));
        Self {
            params,
            clips,
            track_gain,
            track_pan,
            track_send,
            lp: [0.0; MAX_TRACKS],
            hp_in: [0.0; MAX_TRACKS],
            hp_out: [0.0; MAX_TRACKS],
            position,
        }
    }

    pub fn params(&self) -> &Arc<ClipsParams> {
        &self.params
    }

    #[inline]
    fn clip_sample(c: &ClipData, frame_in_clip: u64) -> f32 {
        let b = &c.buffer;
        let f = (c.offset + frame_in_clip) as usize;
        let ch = b.channels.max(1) as usize;
        if f >= b.frames() {
            return 0.0;
        }
        let base = f * ch;
        let mut s = 0.0f32;
        for k in 0..ch {
            s += b.samples[base + k];
        }
        let mut g = c.gain / ch as f32;
        if frame_in_clip < c.fade_in {
            g *= frame_in_clip as f32 / c.fade_in.max(1) as f32;
        }
        let left = c.length - frame_in_clip;
        if left < c.fade_out {
            g *= left as f32 / c.fade_out.max(1) as f32;
        }
        s * g
    }
}

impl Source for ClipSource {
    /// Mono fallback (tests, hosts without the stereo path): the stereo render folded.
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let mut l = [0.0f32; crate::graph::BLOCK];
        let mut r = [0.0f32; crate::graph::BLOCK];
        let mut s = [0.0f32; crate::graph::BLOCK];
        for chunk in out.chunks_mut(crate::graph::BLOCK) {
            let n = chunk.len();
            self.render_stereo(&mut l[..n], &mut r[..n], &mut s[..n], sample_rate, 0);
            for (k, o) in chunk.iter_mut().enumerate() {
                *o = (l[k] + r[k]) * 0.5;
            }
        }
    }

    fn render_stereo(
        &mut self,
        left: &mut [f32],
        right: &mut [f32],
        send: &mut [f32],
        sample_rate: u32,
        _frame: u64,
    ) -> bool {
        let p = Arc::clone(&self.params);
        let out_len = left.len();
        let sr = sample_rate.max(1) as f32;
        let t = &p.transport;
        if t.seek_pending.swap(false, Ordering::AcqRel) {
            self.position = t.seek_frames.load(Ordering::Relaxed);
            t.finished.store(false, Ordering::Relaxed);
        }
        let ramp = ramp_samples(sample_rate);
        let any_solo = p.tracks.iter().any(|tr| tr.solo.load(Ordering::Relaxed));
        let mut audible = [false; MAX_TRACKS];
        let mut a_lp = [0.0f32; MAX_TRACKS];
        let mut a_hp = [0.0f32; MAX_TRACKS];
        for (i, tr) in p.tracks.iter().enumerate() {
            let solo = tr.solo.load(Ordering::Relaxed);
            let mute = tr.mute.load(Ordering::Relaxed);
            audible[i] = !mute && (!any_solo || solo);
            let target = if audible[i] {
                db_to_gain(tr.gain_db.target())
            } else {
                0.0
            };
            self.track_gain[i].begin(target, ramp);
            self.track_pan[i].begin(tr.pan.target(), ramp);
            self.track_send[i].begin(tr.send.target(), ramp);
            a_lp[i] = 1.0 - (-2.0 * std::f32::consts::PI * tr.lowpass_hz.target() / sr).exp();
            a_hp[i] = (-2.0 * std::f32::consts::PI * tr.highpass_hz.target() / sr).exp();
        }
        let playing = t.playing.load(Ordering::Relaxed);
        let loop_on = t.loop_on.load(Ordering::Relaxed);
        let (ls, le) = (
            t.loop_start.load(Ordering::Relaxed),
            t.loop_end.load(Ordering::Relaxed),
        );
        let end = t.end_frames.load(Ordering::Relaxed);
        for k in 0..out_len {
            let mut gains = [0.0f32; MAX_TRACKS];
            let mut pans = [0.0f32; MAX_TRACKS];
            let mut sends = [0.0f32; MAX_TRACKS];
            for i in 0..MAX_TRACKS {
                let target = if audible[i] {
                    db_to_gain(p.tracks[i].gain_db.target())
                } else {
                    0.0
                };
                gains[i] = self.track_gain[i].next(target);
                pans[i] = self.track_pan[i].next(p.tracks[i].pan.target());
                sends[i] = self.track_send[i].next(p.tracks[i].send.target());
            }
            if !playing {
                left[k] = 0.0;
                right[k] = 0.0;
                send[k] = 0.0;
                continue;
            }
            let pos = self.position;
            let mut tx = [0.0f32; MAX_TRACKS];
            for c in &self.clips {
                if c.mute || pos < c.start || pos >= c.start + c.length {
                    continue;
                }
                tx[c.track] += Self::clip_sample(c, pos - c.start);
            }
            let (mut l, mut r, mut s) = (0.0f32, 0.0f32, 0.0f32);
            for i in 0..MAX_TRACKS {
                if gains[i] == 0.0 && tx[i] == 0.0 {
                    continue;
                }
                // Per-track one-pole cuts, then gain, pan and send.
                self.lp[i] =
                    crate::graph::params::undenormal(self.lp[i] + a_lp[i] * (tx[i] - self.lp[i]));
                let y_lp = self.lp[i];
                self.hp_out[i] = crate::graph::params::undenormal(
                    a_hp[i] * (self.hp_out[i] + y_lp - self.hp_in[i]),
                );
                self.hp_in[i] = y_lp;
                let y = self.hp_out[i] * gains[i];
                let theta = (pans[i] + 1.0) * 0.25 * std::f32::consts::PI;
                l += y * theta.cos() * std::f32::consts::SQRT_2;
                r += y * theta.sin() * std::f32::consts::SQRT_2;
                s += y * sends[i];
            }
            left[k] = l;
            right[k] = r;
            send[k] = s;
            self.position += 1;
            if loop_on && le > ls && self.position >= le {
                self.position = ls;
            } else if !loop_on && end > 0 && self.position >= end {
                t.playing.store(false, Ordering::Relaxed);
                t.finished.store(true, Ordering::Relaxed);
            }
        }
        t.position_frames.store(self.position, Ordering::Relaxed);
        true
    }

    fn reset(&mut self) {
        self.position = self.params.transport.position();
        self.lp = [0.0; MAX_TRACKS];
        self.hp_in = [0.0; MAX_TRACKS];
        self.hp_out = [0.0; MAX_TRACKS];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Graph, GraphParams};

    fn tone(hz: f32, seconds: f32, sr: u32) -> Arc<AudioBuffer> {
        let n = (seconds * sr as f32) as usize;
        let v: Vec<f32> = (0..n)
            .map(|i| (i as f32 / sr as f32 * hz * std::f32::consts::TAU).sin() * 0.5)
            .collect();
        Arc::new(AudioBuffer::new(sr, 1, v))
    }

    fn render(params: &Arc<ClipsParams>, clips: Vec<ClipData>, seconds: f64) -> AudioBuffer {
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(ClipSource::new(Arc::clone(params), clips)));
        g.render_offline(seconds, 1)
    }

    fn render_stereo_graph(
        params: &Arc<ClipsParams>,
        clips: Vec<ClipData>,
        seconds: f64,
        space: f32,
    ) -> AudioBuffer {
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.master.space.set(space);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(ClipSource::new(Arc::clone(params), clips)));
        g.render_offline(seconds, 2)
    }

    fn channel_rms(b: &AudioBuffer, ch: usize, from_s: f64, to_s: f64) -> f64 {
        let sr = b.sample_rate as f64;
        let (a, z) = ((from_s * sr) as usize, (to_s * sr) as usize);
        let n = b.channels as usize;
        let mut sum = 0.0f64;
        let mut count = 0usize;
        for f in a..z.min(b.frames()) {
            let v = b.samples[f * n + ch] as f64;
            sum += v * v;
            count += 1;
        }
        (sum / count.max(1) as f64).sqrt()
    }

    #[test]
    fn track_strip_pan_cuts_and_send_are_audible_in_the_stereo_path() {
        let sr = 48_000;
        let clip = |track: usize, hz: f32| ClipData {
            track,
            buffer: tone(hz, 2.0, sr),
            start: 0,
            offset: 0,
            length: 2 * sr as u64,
            gain: 1.0,
            fade_in: 0,
            fade_out: 0,
            mute: false,
        };
        // Pan: track 0 hard left → the right channel is (almost) empty.
        let p = Arc::new(ClipsParams::default());
        p.transport.playing.store(true, Ordering::Relaxed);
        p.transport
            .end_frames
            .store(3 * sr as u64, Ordering::Relaxed);
        p.tracks[0].pan.set(-1.0);
        let b = render_stereo_graph(&p, vec![clip(0, 220.0)], 2.5, 0.0);
        let l = channel_rms(&b, 0, 0.3, 1.9);
        let r = channel_rms(&b, 1, 0.3, 1.9);
        assert!(l > 0.1 && l > r * 20.0, "pan -1: L {l} R {r}");
        // Send: with the global space at 0, a send still leaves a reverb tail after the clip.
        let p2 = Arc::new(ClipsParams::default());
        p2.transport.playing.store(true, Ordering::Relaxed);
        p2.transport
            .end_frames
            .store(5 * sr as u64, Ordering::Relaxed);
        p2.tracks[0].send.set(1.0);
        let wet = render_stereo_graph(&p2, vec![clip(0, 220.0)], 4.0, 0.0);
        let p3 = Arc::new(ClipsParams::default());
        p3.transport.playing.store(true, Ordering::Relaxed);
        p3.transport
            .end_frames
            .store(5 * sr as u64, Ordering::Relaxed);
        let dry = render_stereo_graph(&p3, vec![clip(0, 220.0)], 4.0, 0.0);
        let wet_tail = channel_rms(&wet, 0, 2.3, 3.5);
        let dry_tail = channel_rms(&dry, 0, 2.3, 3.5);
        assert!(
            wet_tail > 1e-3 && wet_tail > dry_tail * 10.0,
            "send tail {wet_tail} vs dry {dry_tail}"
        );
        // Cuts: a high-pass at 2 kHz removes most of a 110 Hz tone; a low-pass at 100 Hz
        // removes most of a 3 kHz tone.
        let p4 = Arc::new(ClipsParams::default());
        p4.transport.playing.store(true, Ordering::Relaxed);
        p4.transport
            .end_frames
            .store(3 * sr as u64, Ordering::Relaxed);
        let flat = channel_rms(
            &render_stereo_graph(&p4, vec![clip(0, 110.0)], 1.5, 0.0),
            0,
            0.5,
            1.4,
        );
        p4.tracks[0].highpass_hz.set(2000.0);
        let cut = channel_rms(
            &render_stereo_graph(&p4, vec![clip(0, 110.0)], 1.5, 0.0),
            0,
            0.5,
            1.4,
        );
        assert!(cut < flat * 0.2, "HP 2 kHz on 110 Hz: {cut} vs {flat}");
        p4.tracks[0].highpass_hz.set(10.0);
        p4.tracks[0].lowpass_hz.set(100.0);
        let flat_hi = {
            let q = Arc::new(ClipsParams::default());
            q.transport.playing.store(true, Ordering::Relaxed);
            q.transport
                .end_frames
                .store(3 * sr as u64, Ordering::Relaxed);
            channel_rms(
                &render_stereo_graph(&q, vec![clip(0, 3000.0)], 1.5, 0.0),
                0,
                0.5,
                1.4,
            )
        };
        let cut_hi = channel_rms(
            &render_stereo_graph(&p4, vec![clip(0, 3000.0)], 1.5, 0.0),
            0,
            0.5,
            1.4,
        );
        assert!(
            cut_hi < flat_hi * 0.2,
            "LP 100 Hz on 3 kHz: {cut_hi} vs {flat_hi}"
        );
    }

    #[test]
    fn clips_play_at_their_positions_with_fades_and_track_mutes() {
        let sr = 48_000;
        let p = Arc::new(ClipsParams::default());
        p.transport.playing.store(true, Ordering::Relaxed);
        p.transport
            .end_frames
            .store(4 * sr as u64, Ordering::Relaxed);
        let clips = vec![
            ClipData {
                track: 0,
                buffer: tone(220.0, 2.0, sr),
                start: sr as u64, // at 1 s
                offset: sr as u64 / 2,
                length: sr as u64, // 1 s long
                gain: 1.0,
                fade_in: 480,
                fade_out: 480,
                mute: false,
            },
            ClipData {
                track: 1,
                buffer: tone(330.0, 2.0, sr),
                start: 3 * sr as u64,
                offset: 0,
                length: sr as u64 / 2,
                gain: 1.0,
                fade_in: 0,
                fade_out: 0,
                mute: false,
            },
        ];
        p.tracks[1].mute.store(true, Ordering::Relaxed);
        let b = render(&p, clips, 4.0);
        let rms = |a: f64, c: f64| {
            b.slice_frames((a * sr as f64) as usize, (c * sr as f64) as usize)
                .rms()
        };
        assert!(rms(0.0, 0.9) < 1e-6, "silence before the clip");
        assert!(rms(1.2, 1.8) > 0.3, "the clip sounds");
        assert!(rms(2.1, 2.9) < 1e-6, "silence after it");
        assert!(rms(3.0, 3.5) < 1e-6, "muted track is silent");
        // Fades: the very first samples of the clip are quieter than the middle.
        let head = b.slice_frames(sr as usize, sr as usize + 100).peak();
        assert!(head < 0.2, "fade in: {head}");
        assert_eq!(p.transport.position(), 4 * sr as u64);
        assert!(p.transport.finished.load(Ordering::Relaxed));
        assert!(!p.transport.playing.load(Ordering::Relaxed));
    }

    #[test]
    fn seek_loop_and_stop_are_honoured() {
        let sr = 48_000;
        let p = Arc::new(ClipsParams::default());
        let clips = vec![ClipData {
            track: 0,
            buffer: tone(220.0, 3.0, sr),
            start: 0,
            offset: 0,
            length: 3 * sr as u64,
            gain: 1.0,
            fade_in: 0,
            fade_out: 0,
            mute: false,
        }];
        // Stopped: silence and no movement.
        let b = render(&p, clips, 0.5);
        assert!(b.rms() < 1e-9 && p.transport.position() == 0);
        // Play from 2.0 s with a loop 2.0–2.5: after 2 s rendered we are inside the loop.
        p.transport.seek(2 * sr as u64);
        p.transport.loop_on.store(true, Ordering::Relaxed);
        p.transport
            .loop_start
            .store(2 * sr as u64, Ordering::Relaxed);
        p.transport
            .loop_end
            .store(5 * sr as u64 / 2, Ordering::Relaxed);
        p.transport.playing.store(true, Ordering::Relaxed);
        let clips = vec![ClipData {
            track: 0,
            buffer: tone(220.0, 3.0, sr),
            start: 0,
            offset: 0,
            length: 3 * sr as u64,
            gain: 1.0,
            fade_in: 0,
            fade_out: 0,
            mute: false,
        }];
        let b = render(&p, clips, 2.0);
        assert!(b.rms() > 0.3);
        let pos = p.transport.position();
        assert!(pos >= 2 * sr as u64 && pos < 5 * sr as u64 / 2, "pos {pos}");
        assert!(!p.transport.finished.load(Ordering::Relaxed));
    }
}
