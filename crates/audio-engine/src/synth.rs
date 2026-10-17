//! Built-in note synthesiser (offline): renders a note list to an `AudioBuffer`.
//!
//! Phase 2.7a: a simple, clean voice so extracted notes and imported MIDI files are
//! audible at once, with no soundfont. Pitch bend curves are honoured. An SF2 engine
//! comes later (2.7b) behind the same `SynthNote` input.

use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;

/// A note to render; times in seconds, pitch as MIDI + cents, bend as
/// (seconds from start, cents from `midi`) breakpoints (linear in between).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SynthNote {
    pub start: f64,
    pub end: f64,
    pub midi: i32,
    pub cents: f64,
    /// 0..1
    pub velocity: f64,
    pub bend: Vec<(f64, f64)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Voice {
    Sine,
    Triangle,
    /// Sine with a touch of 2nd/3rd harmonic and gentle breath-like attack.
    Flute,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SynthConfig {
    pub sample_rate: u32,
    pub a4_hz: f64,
    pub voice: Voice,
    pub attack_seconds: f64,
    pub release_seconds: f64,
    /// Output gain per voice before the limiter (0..1).
    pub gain: f64,
}

impl SynthConfig {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            a4_hz: 440.0,
            voice: Voice::Flute,
            attack_seconds: 0.012,
            release_seconds: 0.06,
            gain: 0.35,
        }
    }
}

fn bend_at(bend: &[(f64, f64)], t: f64) -> f64 {
    if bend.is_empty() {
        return 0.0;
    }
    if t <= bend[0].0 {
        return bend[0].1;
    }
    let last = bend[bend.len() - 1];
    if t >= last.0 {
        return last.1;
    }
    let i = bend.partition_point(|b| b.0 <= t);
    let (t0, c0) = bend[i - 1];
    let (t1, c1) = bend[i];
    if t1 <= t0 {
        return c0;
    }
    c0 + (c1 - c0) * (t - t0) / (t1 - t0)
}

pub(crate) fn wave(voice: Voice, phase: f64, t_in_note: f64) -> f64 {
    let p = phase * std::f64::consts::TAU;
    match voice {
        Voice::Sine => p.sin(),
        // −1 at phase 0, +1 at 0.5, back to −1 at 1: one period per cycle.
        Voice::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        Voice::Flute => {
            // Harmonics settle in over the first ~80 ms, like breath finding the tone.
            let bloom = (t_in_note / 0.08).min(1.0);
            (p.sin() + 0.25 * bloom * (2.0 * p).sin() + 0.08 * bloom * (3.0 * p).sin()) / 1.33
        }
    }
}

/// Render `notes` (any overlap) to a mono buffer. Length = last end + release.
pub fn render_notes(notes: &[SynthNote], cfg: &SynthConfig) -> AudioBuffer {
    let sr = cfg.sample_rate.max(1) as f64;
    if notes.is_empty() {
        return AudioBuffer::empty(cfg.sample_rate, 1);
    }
    let total_end =
        notes.iter().map(|n| n.end.max(n.start)).fold(0.0, f64::max) + cfg.release_seconds;
    let len = (total_end * sr).ceil().max(0.0) as usize;
    let mut out = vec![0.0f64; len];

    for n in notes {
        let positive = n.end.partial_cmp(&n.start) == Some(std::cmp::Ordering::Greater);
        if !positive || n.start < 0.0 {
            continue;
        }
        let start_i = (n.start * sr) as usize;
        let hold = n.end - n.start;
        let span = hold + cfg.release_seconds;
        let count = ((span * sr) as usize).min(len.saturating_sub(start_i));
        let base_midi = n.midi as f64 + n.cents / 100.0;
        let vel = n.velocity.clamp(0.0, 1.0);
        let amp = cfg.gain * (0.2 + 0.8 * vel);
        let mut phase = 0.0f64;
        for k in 0..count {
            let t = k as f64 / sr;
            let midi = base_midi + bend_at(&n.bend, t) / 100.0;
            let hz = cfg.a4_hz * 2f64.powf((midi - 69.0) / 12.0);
            // Envelope: linear attack, hold, linear release after the note ends.
            let env = if t < cfg.attack_seconds {
                t / cfg.attack_seconds
            } else if t <= hold {
                1.0
            } else {
                (1.0 - (t - hold) / cfg.release_seconds).max(0.0)
            };
            out[start_i + k] += amp * env * wave(cfg.voice, phase, t);
            phase += hz / sr;
            if phase >= 1.0 {
                phase -= 1.0;
            }
        }
    }
    // Soft limiter so chords never clip.
    let samples = out
        .into_iter()
        .map(|s| {
            (if s.abs() > 0.8 {
                0.8 + (s.abs() - 0.8).tanh() * 0.19
            } else {
                s.abs()
            }) as f32
                * s.signum() as f32
        })
        .collect();
    AudioBuffer::new(cfg.sample_rate, 1, samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pitch::{contour, PitchConfig};

    fn note(start: f64, end: f64, midi: i32, bend: Vec<(f64, f64)>) -> SynthNote {
        SynthNote {
            start,
            end,
            midi,
            cents: 0.0,
            velocity: 0.8,
            bend,
        }
    }

    fn median_hz(buf: &AudioBuffer, from: f64, to: f64) -> f64 {
        let cfg = PitchConfig::new(buf.sample_rate);
        let mut v: Vec<f64> = contour(buf, &cfg, 0.01)
            .into_iter()
            .filter(|p| p.t >= from && p.t <= to)
            .filter_map(|p| p.hz)
            .collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v[v.len() / 2]
    }

    #[test]
    fn renders_the_right_pitch_and_length_for_every_voice() {
        for voice in [Voice::Sine, Voice::Triangle, Voice::Flute] {
            let cfg = SynthConfig {
                voice,
                ..SynthConfig::new(48_000)
            };
            let buf = render_notes(&[note(0.0, 1.0, 69, vec![])], &cfg);
            assert_eq!(
                buf.frames(),
                ((1.0 + cfg.release_seconds) * 48_000.0).ceil() as usize
            );
            let hz = median_hz(&buf, 0.2, 0.9);
            let cents = 1200.0 * (hz / 440.0).log2();
            assert!(cents.abs() < 2.0, "{voice:?}: {hz} Hz ({cents:.1} c)");
            assert!(
                buf.peak() <= 1.0 && buf.peak() > 0.2,
                "{voice:?}: peak {}",
                buf.peak()
            );
        }
    }

    #[test]
    fn bend_curve_moves_the_pitch() {
        let cfg = SynthConfig::new(48_000);
        // Starts a semitone low, reaches the note after 0.3 s, holds.
        let buf = render_notes(&[note(0.0, 1.5, 69, vec![(0.0, -100.0), (0.3, 0.0)])], &cfg);
        let early = median_hz(&buf, 0.05, 0.12);
        let late = median_hz(&buf, 0.6, 1.4);
        assert!(
            (1200.0 * (early / 415.3).log2()).abs() < 40.0,
            "early {early}"
        );
        assert!((1200.0 * (late / 440.0).log2()).abs() < 2.0, "late {late}");
    }

    #[test]
    fn chords_do_not_clip_and_gaps_are_silent() {
        let cfg = SynthConfig::new(48_000);
        let chord: Vec<SynthNote> = [60, 64, 67, 72, 76]
            .iter()
            .map(|&m| note(0.0, 0.5, m, vec![]))
            .collect();
        let buf = render_notes(&chord, &cfg);
        assert!(buf.peak() <= 1.0, "peak {}", buf.peak());
        let two = render_notes(
            &[note(0.0, 0.3, 60, vec![]), note(1.0, 1.3, 62, vec![])],
            &cfg,
        );
        let gap = two.slice_frames(48_000 / 2, 48_000 * 9 / 10);
        assert_eq!(gap.peak(), 0.0, "silence between the notes");
        assert!(render_notes(&[], &cfg).is_empty());
    }

    #[test]
    fn velocity_scales_the_level() {
        let cfg = SynthConfig::new(48_000);
        let loud = render_notes(
            &[SynthNote {
                velocity: 1.0,
                ..note(0.0, 0.5, 60, vec![])
            }],
            &cfg,
        );
        let soft = render_notes(
            &[SynthNote {
                velocity: 0.1,
                ..note(0.0, 0.5, 60, vec![])
            }],
            &cfg,
        );
        assert!(loud.rms() > soft.rms() * 2.0);
    }
}
