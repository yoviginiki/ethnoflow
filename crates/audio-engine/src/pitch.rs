//! Monophonic pitch detection (YIN: de Cheveigné & Kawahara 2002).
//!
//! `estimate` works on one frame; `PitchTracker` feeds frames from a stream at a fixed
//! hop (used from the recorder's drain thread, never the audio callback); `contour`
//! analyses a whole buffer offline. Every estimate carries a confidence; silence and
//! noise give `hz = None`. No key, tempo or meter is involved anywhere.

use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PitchConfig {
    pub sample_rate: u32,
    /// Lowest detectable fundamental.
    pub min_hz: f64,
    /// Highest detectable fundamental.
    pub max_hz: f64,
    /// Integration window in samples (the frame must hold `window + sample_rate/min_hz`).
    pub window: usize,
    /// YIN absolute threshold on the cumulative mean normalised difference (0.1–0.2).
    pub threshold: f64,
    /// Frames quieter than this RMS (linear) are reported as unvoiced.
    pub silence_rms: f32,
}

impl PitchConfig {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            min_hz: 60.0,
            max_hz: 2000.0,
            window: 1024,
            threshold: 0.12,
            silence_rms: 0.003, // ≈ −50 dBFS
        }
    }

    /// Longest lag searched.
    pub fn tau_max(&self) -> usize {
        (self.sample_rate as f64 / self.min_hz).ceil() as usize
    }

    pub fn tau_min(&self) -> usize {
        (self.sample_rate as f64 / self.max_hz).floor().max(2.0) as usize
    }

    /// Samples one estimate needs.
    pub fn frame_len(&self) -> usize {
        self.window + self.tau_max() + 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct PitchEstimate {
    /// Fundamental in Hz, `None` when unvoiced (silence, noise, no dip under threshold).
    pub hz: Option<f64>,
    /// 1 − aperiodicity at the chosen lag, in [0, 1]; 0 when unvoiced.
    pub confidence: f64,
    /// RMS of the analysed window (linear).
    pub rms: f32,
}

/// YIN on one frame of mono samples. `frame.len()` must be ≥ `cfg.frame_len()`;
/// only the first `frame_len()` samples are used.
pub fn estimate(frame: &[f32], cfg: &PitchConfig) -> PitchEstimate {
    let w = cfg.window;
    let tau_max = cfg.tau_max();
    let tau_min = cfg.tau_min();
    debug_assert!(frame.len() > w + tau_max, "frame too short");
    if frame.len() < w + tau_max + 1 || tau_min >= tau_max {
        return PitchEstimate::default();
    }
    let rms = (frame[..w]
        .iter()
        .map(|&s| (s as f64) * (s as f64))
        .sum::<f64>()
        / w as f64)
        .sqrt() as f32;
    if rms < cfg.silence_rms {
        return PitchEstimate {
            hz: None,
            confidence: 0.0,
            rms,
        };
    }

    // Difference function d(τ) for τ in 0..=tau_max.
    let mut d = vec![0.0f64; tau_max + 1];
    for (tau, slot) in d.iter_mut().enumerate().skip(1) {
        let mut acc = 0.0f64;
        for j in 0..w {
            let x = (frame[j] - frame[j + tau]) as f64;
            acc += x * x;
        }
        *slot = acc;
    }
    // Cumulative mean normalised difference d'(τ).
    let mut cmndf = vec![1.0f64; tau_max + 1];
    let mut running = 0.0f64;
    for tau in 1..=tau_max {
        running += d[tau];
        cmndf[tau] = if running > 0.0 {
            d[tau] * tau as f64 / running
        } else {
            1.0
        };
    }
    // Absolute threshold: first dip below it, then walk to its local minimum.
    let mut tau = tau_min;
    let mut chosen = None;
    while tau < tau_max {
        if cmndf[tau] < cfg.threshold {
            while tau + 1 < tau_max && cmndf[tau + 1] < cmndf[tau] {
                tau += 1;
            }
            chosen = Some(tau);
            break;
        }
        tau += 1;
    }
    // Fallback: global minimum if it is reasonably periodic (keeps weak but clean tones).
    let tau =
        match chosen {
            Some(t) => t,
            None => {
                let (t, v) = (tau_min..tau_max).map(|t| (t, cmndf[t])).fold(
                    (tau_min, f64::INFINITY),
                    |m, x| if x.1 < m.1 { x } else { m },
                );
                if v > 0.5 {
                    return PitchEstimate {
                        hz: None,
                        confidence: 0.0,
                        rms,
                    };
                }
                t
            }
        };
    // Parabolic interpolation around the minimum, on the raw difference function
    // (the normalisation skews the parabola at short lags).
    let refined = if tau > tau_min && tau + 1 < tau_max {
        let (a, b, c) = (d[tau - 1], d[tau], d[tau + 1]);
        let denom = a - 2.0 * b + c;
        if denom.abs() > 1e-12 {
            tau as f64 + 0.5 * (a - c) / denom
        } else {
            tau as f64
        }
    } else {
        tau as f64
    };
    let confidence = (1.0 - cmndf[tau]).clamp(0.0, 1.0);
    PitchEstimate {
        hz: Some(cfg.sample_rate as f64 / refined),
        confidence,
        rms,
    }
}

/// Streams mono samples in and yields one estimate per `hop` samples. The analysis
/// frame is the most recent `frame_len()` samples ending at the hop boundary, so the
/// estimate's time stamp is the end of the frame.
pub struct PitchTracker {
    cfg: PitchConfig,
    hop: usize,
    buf: Vec<f32>,
    since_hop: usize,
    frames_seen: u64,
}

/// One point of a pitch contour.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ContourPoint {
    /// Seconds from the start, at the end of the analysed frame.
    pub t: f64,
    pub hz: Option<f64>,
    pub confidence: f64,
    pub rms: f32,
}

impl PitchTracker {
    pub fn new(cfg: PitchConfig, hop: usize) -> Self {
        Self {
            cfg,
            hop: hop.max(1),
            buf: Vec::with_capacity(cfg.frame_len() + hop),
            since_hop: 0,
            frames_seen: 0,
        }
    }

    pub fn hop(&self) -> usize {
        self.hop
    }

    /// Feed mono samples; returns the estimates completed by this chunk.
    pub fn push(&mut self, mono: &[f32], out: &mut Vec<ContourPoint>) {
        let need = self.cfg.frame_len();
        for &s in mono {
            self.buf.push(s);
            self.frames_seen += 1;
            self.since_hop += 1;
            if self.since_hop >= self.hop && self.buf.len() >= need {
                self.since_hop = 0;
                let start = self.buf.len() - need;
                let e = estimate(&self.buf[start..], &self.cfg);
                out.push(ContourPoint {
                    t: self.frames_seen as f64 / self.cfg.sample_rate as f64,
                    hz: e.hz,
                    confidence: e.confidence,
                    rms: e.rms,
                });
                // Keep only what the next frame can need.
                if self.buf.len() > need + self.hop {
                    let drop = self.buf.len() - need;
                    self.buf.drain(..drop);
                }
            }
        }
    }
}

/// Offline contour of a whole buffer (mixed to mono) at `hop_seconds` resolution.
pub fn contour(buffer: &AudioBuffer, cfg: &PitchConfig, hop_seconds: f64) -> Vec<ContourPoint> {
    let mono = buffer.to_mono();
    let hop = ((hop_seconds * cfg.sample_rate as f64).round() as usize).max(1);
    let mut tracker = PitchTracker::new(*cfg, hop);
    let mut out = Vec::with_capacity(mono.samples.len() / hop + 1);
    tracker.push(&mono.samples, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(sr: u32) -> PitchConfig {
        PitchConfig {
            min_hz: 80.0,
            ..PitchConfig::new(sr)
        }
    }

    fn cents(a: f64, b: f64) -> f64 {
        1200.0 * (b / a).log2()
    }

    /// Tone with harmonics (a crude reed/flute-like spectrum).
    fn harmonic_tone(sr: u32, f0: f64, seconds: f64, amps: &[f64]) -> AudioBuffer {
        let n = (sr as f64 * seconds) as usize;
        let samples = (0..n)
            .map(|i| {
                let t = i as f64 / sr as f64;
                amps.iter()
                    .enumerate()
                    .map(|(k, &a)| a * (2.0 * std::f64::consts::PI * f0 * (k + 1) as f64 * t).sin())
                    .sum::<f64>() as f32
            })
            .collect();
        AudioBuffer::new(sr, 1, samples)
    }

    #[test]
    fn pure_tones_within_one_cent() {
        for sr in [44_100u32, 48_000] {
            for f in [110.0, 220.0, 293.66, 440.0, 880.0, 1760.0] {
                let b = AudioBuffer::sine(sr, f as f32, 0.2, 0.5);
                let c = cfg(sr);
                let e = estimate(&b.samples[..c.frame_len()], &c);
                let hz = e.hz.expect("voiced");
                let err = cents(f, hz);
                assert!(
                    err.abs() < 1.0,
                    "{f} Hz @ {sr}: got {hz:.3} ({err:.2} cents)"
                );
                assert!(e.confidence > 0.9, "{f} Hz: confidence {}", e.confidence);
            }
        }
    }

    #[test]
    fn silence_and_noise_are_unvoiced() {
        let c = cfg(48_000);
        let silence = vec![0.0f32; c.frame_len()];
        assert_eq!(estimate(&silence, &c).hz, None);
        // Deterministic white-ish noise (LCG), full scale.
        let mut x = 12345u32;
        let noise: Vec<f32> = (0..c.frame_len())
            .map(|_| {
                x = x.wrapping_mul(1664525).wrapping_add(1013904223);
                (x >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
            })
            .collect();
        let e = estimate(&noise, &c);
        assert!(e.hz.is_none() || e.confidence < 0.5, "noise: {e:?}");
    }

    #[test]
    fn harmonic_tones_rarely_octave_error() {
        let sr = 48_000;
        let c = cfg(sr);
        let mut total = 0;
        let mut octave_errors = 0;
        for f0 in [150.0, 196.0, 261.63, 329.63, 392.0, 523.25, 659.26, 783.99] {
            let b = harmonic_tone(sr, f0, 0.5, &[0.5, 0.4, 0.3, 0.15, 0.1]);
            for p in contour(&b, &c, 0.01) {
                if let Some(hz) = p.hz {
                    total += 1;
                    if cents(f0, hz).abs() > 100.0 {
                        octave_errors += 1;
                    }
                }
            }
        }
        assert!(total > 300, "too few voiced frames: {total}");
        let rate = octave_errors as f64 / total as f64;
        assert!(
            rate < 0.02,
            "octave/other error rate {rate:.3} ({octave_errors}/{total})"
        );
    }

    #[test]
    fn vibrato_is_tracked_around_its_centre() {
        let sr = 48_000u32;
        let f0 = 440.0;
        let n = sr as usize * 2;
        // ±50 cents at 6 Hz: integrate the instantaneous frequency into the phase.
        let mut phase = 0.0f64;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / sr as f64;
                let dev = 2f64.powf(0.5 / 12.0 * (2.0 * std::f64::consts::PI * 6.0 * t).sin());
                phase += 2.0 * std::f64::consts::PI * f0 * dev / sr as f64;
                (0.5 * phase.sin()) as f32
            })
            .collect();
        let b = AudioBuffer::new(sr, 1, samples);
        let pts: Vec<f64> = contour(&b, &cfg(sr), 0.005)
            .into_iter()
            .filter_map(|p| p.hz)
            .map(|hz| cents(f0, hz))
            .collect();
        assert!(pts.len() > 300);
        let mean = pts.iter().sum::<f64>() / pts.len() as f64;
        let max = pts.iter().fold(0.0f64, |m, &x| m.max(x.abs()));
        assert!(mean.abs() < 10.0, "mean {mean:.1} cents");
        assert!(max < 120.0, "max deviation {max:.1} cents");
    }

    #[test]
    fn sweep_is_followed() {
        let sr = 48_000u32;
        let n = sr as usize * 2;
        let (f_a, f_b) = (200.0f64, 800.0f64);
        let mut phase = 0.0f64;
        let samples: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f64 / n as f64;
                let f = f_a * (f_b / f_a).powf(t);
                phase += 2.0 * std::f64::consts::PI * f / sr as f64;
                (0.5 * phase.sin()) as f32
            })
            .collect();
        let b = AudioBuffer::new(sr, 1, samples);
        let c = cfg(sr);
        let mut bad = 0;
        let mut n_pts = 0;
        for p in contour(&b, &c, 0.01) {
            let Some(hz) = p.hz else { continue };
            // Expected frequency at the centre of the frame.
            let t_centre = p.t - c.frame_len() as f64 / 2.0 / sr as f64;
            let expect = f_a * (f_b / f_a).powf(t_centre / 2.0);
            n_pts += 1;
            if cents(expect, hz).abs() > 60.0 {
                bad += 1;
            }
        }
        assert!(n_pts > 150);
        assert!(
            (bad as f64) / (n_pts as f64) < 0.05,
            "{bad}/{n_pts} frames off"
        );
    }

    #[test]
    fn tracker_streaming_matches_offline_contour() {
        let sr = 48_000;
        let b = harmonic_tone(sr, 330.0, 1.0, &[0.5, 0.3]);
        let c = cfg(sr);
        let offline = contour(&b, &c, 0.01);
        let mut t = PitchTracker::new(c, 480);
        let mut streamed = Vec::new();
        for chunk in b.samples.chunks(337) {
            t.push(chunk, &mut streamed);
        }
        assert_eq!(offline.len(), streamed.len());
        for (a, b) in offline.iter().zip(&streamed) {
            assert_eq!(a.t, b.t);
            assert_eq!(a.hz, b.hz);
        }
        assert!((offline[1].t - offline[0].t - 0.01).abs() < 1e-9);
    }
}
