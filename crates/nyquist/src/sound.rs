//! Sound primitives on fixed arrays: arithmetic (mult / sum / diff / scale), filters
//! (hp / lp one-pole, lowpass2 / highpass2 / bandpass2 biquads), envelopes (pwl / pwlv /
//! env), generators (osc / hzosc / lfo / noise / s-rest), time (seq / sim / at / extract /
//! stretch-abs), measurements (peak / snd-length / snd-srate).

use crate::value::Sound;

pub fn map2(a: &Sound, b: &Sound, f: impl Fn(f32, f32) -> f32) -> Sound {
    // Align by start time; the result spans the union, missing samples count as 0.
    let sr = a.sr;
    let start = a.start.min(b.start);
    let end = (a.start + a.duration()).max(b.start + b.duration());
    let n = ((end - start) * sr).round() as usize;
    let mut out = vec![0.0f32; n];
    let oa = ((a.start - start) * sr).round() as usize;
    let ob = ((b.start - start) * sr).round() as usize;
    for (i, o) in out.iter_mut().enumerate() {
        let x = if i >= oa {
            a.data.get(i - oa).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        let y = if i >= ob {
            b.data.get(i - ob).copied().unwrap_or(0.0)
        } else {
            0.0
        };
        *o = f(x, y);
    }
    Sound {
        sr,
        start,
        data: out,
    }
}

/// Product keeps the shorter extent (Nyquist: mult of sounds is zero where either is).
pub fn mult(a: &Sound, b: &Sound) -> Sound {
    let sr = a.sr;
    let start = a.start.max(b.start);
    let end = (a.start + a.duration()).min(b.start + b.duration());
    if end <= start {
        return Sound {
            sr,
            start,
            data: Vec::new(),
        };
    }
    let n = ((end - start) * sr).round() as usize;
    let oa = ((start - a.start) * sr).round() as usize;
    let ob = ((start - b.start) * sr).round() as usize;
    let data = (0..n)
        .map(|i| {
            a.data.get(oa + i).copied().unwrap_or(0.0) * b.data.get(ob + i).copied().unwrap_or(0.0)
        })
        .collect();
    Sound { sr, start, data }
}

pub fn scale(k: f32, s: &Sound) -> Sound {
    Sound {
        sr: s.sr,
        start: s.start,
        data: s.data.iter().map(|v| v * k).collect(),
    }
}

pub fn offset(k: f32, s: &Sound) -> Sound {
    Sound {
        sr: s.sr,
        start: s.start,
        data: s.data.iter().map(|v| v + k).collect(),
    }
}

pub fn map1(s: &Sound, f: impl Fn(f32) -> f32) -> Sound {
    Sound {
        sr: s.sr,
        start: s.start,
        data: s.data.iter().map(|v| f(*v)).collect(),
    }
}

/// One-pole low-pass (Nyquist `lp`).
pub fn lp(s: &Sound, hz: f32) -> Sound {
    let a = 1.0 - (-2.0 * std::f32::consts::PI * hz.max(1.0) / s.sr as f32).exp();
    let mut y = 0.0f32;
    map1_state(s, |x| {
        y += a * (x - y);
        y
    })
}

/// One-pole high-pass (Nyquist `hp`).
pub fn hp(s: &Sound, hz: f32) -> Sound {
    let a = (-2.0 * std::f32::consts::PI * hz.max(1.0) / s.sr as f32).exp();
    let (mut xin, mut yout) = (0.0f32, 0.0f32);
    map1_state(s, |x| {
        yout = a * (yout + x - xin);
        xin = x;
        yout
    })
}

fn map1_state(s: &Sound, mut f: impl FnMut(f32) -> f32) -> Sound {
    Sound {
        sr: s.sr,
        start: s.start,
        data: s.data.iter().map(|v| f(*v)).collect(),
    }
}

#[derive(Clone, Copy)]
pub enum BiquadKind {
    Low,
    High,
    Band,
    Notch,
}

/// RBJ biquad (`lowpass2`, `highpass2`, `bandpass2`, `notch2`).
pub fn biquad(s: &Sound, kind: BiquadKind, hz: f32, q: f32) -> Sound {
    let sr = s.sr as f32;
    let w0 = 2.0 * std::f32::consts::PI * (hz / sr).clamp(0.0001, 0.49);
    let (sn, cs) = (w0.sin(), w0.cos());
    let alpha = sn / (2.0 * q.max(0.05));
    let (b0, b1, b2) = match kind {
        BiquadKind::Low => ((1.0 - cs) / 2.0, 1.0 - cs, (1.0 - cs) / 2.0),
        BiquadKind::High => ((1.0 + cs) / 2.0, -(1.0 + cs), (1.0 + cs) / 2.0),
        BiquadKind::Band => (alpha, 0.0, -alpha),
        BiquadKind::Notch => (1.0, -2.0 * cs, 1.0),
    };
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cs;
    let a2 = 1.0 - alpha;
    let (b0, b1, b2, a1, a2) = (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    let (mut z1, mut z2) = (0.0f32, 0.0f32);
    map1_state(s, |x| {
        let y = b0 * x + z1;
        z1 = b1 * x - a1 * y + z2;
        z2 = b2 * x - a2 * y;
        y
    })
}

/// Piecewise-linear envelope from (t, v) breakpoints starting at (0, 0) and ending at the
/// last time (Nyquist `pwl`: t1 l1 t2 l2 … tn, final value 0).
pub fn pwl(sr: f64, points: &[(f64, f64)]) -> Sound {
    let mut pts: Vec<(f64, f64)> = vec![(0.0, 0.0)];
    pts.extend_from_slice(points);
    envelope_from(sr, &pts)
}

/// `pwlv`: v1 t2 v2 … tn vn — starts at v1 (time 0).
pub fn pwlv(sr: f64, points: &[(f64, f64)]) -> Sound {
    envelope_from(sr, points)
}

fn envelope_from(sr: f64, pts: &[(f64, f64)]) -> Sound {
    let end = pts.last().map(|p| p.0).unwrap_or(0.0).max(0.0);
    let n = (end * sr).round() as usize;
    let mut data = vec![0.0f32; n];
    for (i, d) in data.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let mut v = pts[0].1;
        for w in pts.windows(2) {
            let (t0, v0) = w[0];
            let (t1, v1) = w[1];
            if t >= t0 && t <= t1 {
                v = if t1 > t0 {
                    v0 + (v1 - v0) * (t - t0) / (t1 - t0)
                } else {
                    v1
                };
                break;
            }
            if t > t1 {
                v = v1;
            }
        }
        *d = v as f32;
    }
    Sound::new(sr, data)
}

#[allow(clippy::too_many_arguments)]
/// `env`: attack t1, decay t2, release t4 (s), levels l1 l2 l3, total `dur`.
pub fn env(sr: f64, t1: f64, t2: f64, t4: f64, l1: f64, l2: f64, l3: f64, dur: f64) -> Sound {
    let t3 = (dur - t1 - t2 - t4).max(0.0);
    pwlv(
        sr,
        &[
            (0.0, 0.0),
            (t1, l1),
            (t1 + t2, l2),
            (t1 + t2 + t3, l3),
            (dur, 0.0),
        ],
    )
}

pub fn step_to_hz(step: f64) -> f64 {
    440.0 * 2f64.powf((step - 69.0) / 12.0)
}

pub fn osc(sr: f64, hz: f64, dur: f64, shape: &str) -> Sound {
    let n = (dur * sr).round() as usize;
    let data = (0..n)
        .map(|i| {
            let ph = (i as f64 * hz / sr).fract();
            match shape {
                "tri" => (4.0 * (ph - (ph + 0.5).floor()).abs() - 1.0) as f32,
                "saw" => (2.0 * ph - 1.0) as f32,
                "square" => {
                    if ph < 0.5 {
                        1.0
                    } else {
                        -1.0
                    }
                }
                _ => (ph * std::f64::consts::TAU).sin() as f32,
            }
        })
        .collect();
    Sound::new(sr, data)
}

pub fn noise(sr: f64, dur: f64, seed: u32) -> Sound {
    let mut s = seed.max(1);
    let n = (dur * sr).round() as usize;
    let data = (0..n)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (s >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
        })
        .collect();
    Sound::new(sr, data)
}

pub fn rest(sr: f64, dur: f64) -> Sound {
    Sound::new(sr, vec![0.0; (dur * sr).round() as usize])
}

/// `extract start stop s` (relative to the sound's start).
pub fn extract(s: &Sound, start: f64, stop: f64) -> Sound {
    let a = ((start * s.sr).round().max(0.0) as usize).min(s.data.len());
    let b = ((stop * s.sr).round().max(0.0) as usize).clamp(a, s.data.len());
    Sound {
        sr: s.sr,
        start: s.start,
        data: s.data[a..b].to_vec(),
    }
}

/// Concatenate in time (`seq`).
pub fn seq(parts: &[Sound]) -> Sound {
    let sr = parts.first().map(|p| p.sr).unwrap_or(44_100.0);
    let mut data = Vec::new();
    for p in parts {
        data.extend_from_slice(&p.data);
    }
    Sound {
        sr,
        start: parts.first().map(|p| p.start).unwrap_or(0.0),
        data,
    }
}

/// Mix with start offsets (`sim`).
pub fn sim(parts: &[Sound]) -> Sound {
    let mut acc: Option<Sound> = None;
    for p in parts {
        acc = Some(match acc {
            None => p.clone(),
            Some(a) => map2(&a, p, |x, y| x + y),
        });
    }
    acc.unwrap_or_else(|| Sound::new(44_100.0, Vec::new()))
}

/// Linear resampling to a new duration (`stretch-abs` on a fixed array: same pitch is NOT
/// kept — Nyquist's `stretch` re-evaluates behaviours; on an array it is a speed change).
pub fn stretch(s: &Sound, factor: f64) -> Sound {
    let n = ((s.data.len() as f64) * factor.max(0.01)).round() as usize;
    if s.data.len() < 2 || n == 0 {
        return Sound {
            sr: s.sr,
            start: s.start,
            data: vec![0.0; n],
        };
    }
    let ratio = (s.data.len() - 1) as f64 / (n.max(2) - 1) as f64;
    let data = (0..n)
        .map(|i| {
            let p = i as f64 * ratio;
            let i0 = p.floor() as usize;
            let i1 = (i0 + 1).min(s.data.len() - 1);
            let t = (p - i0 as f64) as f32;
            s.data[i0] + (s.data[i1] - s.data[i0]) * t
        })
        .collect();
    Sound {
        sr: s.sr,
        start: s.start,
        data,
    }
}

/// `pwev`: breakpoints with exponential interpolation (values must be > 0).
pub fn pwev(sr: f64, pts: &[(f64, f64)]) -> Sound {
    let end = pts.last().map(|p| p.0).unwrap_or(0.0).max(0.0);
    let n = (end * sr).round() as usize;
    let mut data = vec![0.0f32; n];
    for (i, d) in data.iter_mut().enumerate() {
        let t = i as f64 / sr;
        let mut v = pts.first().map(|p| p.1).unwrap_or(0.0);
        for w in pts.windows(2) {
            let (t0, v0) = w[0];
            let (t1, v1) = w[1];
            if t >= t0 && t <= t1 {
                v = if t1 > t0 && v0 > 0.0 && v1 > 0.0 {
                    v0 * (v1 / v0).powf((t - t0) / (t1 - t0))
                } else {
                    v1
                };
                break;
            }
            if t > t1 {
                v = v1;
            }
        }
        *d = v as f32;
    }
    Sound::new(sr, data)
}

/// Direct-form biquad with Nyquist's sign convention: y = b0·x + b1·x1 + b2·x2 + a1·y1 + a2·y2.
pub fn biquad_raw(s: &Sound, b0: f32, b1: f32, b2: f32, a1: f32, a2: f32) -> Sound {
    let (mut x1, mut x2, mut y1, mut y2) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    map1_state(s, |x| {
        let y = b0 * x + b1 * x1 + b2 * x2 + a1 * y1 + a2 * y2;
        x2 = x1;
        x1 = x;
        y2 = y1;
        y1 = y;
        y
    })
}

/// Envelope follower (`snd-follow`): peak hold with exponential rise / fall times.
pub fn follow(s: &Sound, floor: f32, rise_s: f64, fall_s: f64) -> Sound {
    let rise = (-1.0 / (rise_s.max(1e-5) * s.sr)).exp() as f32;
    let fall = (-1.0 / (fall_s.max(1e-5) * s.sr)).exp() as f32;
    let mut env = floor.max(0.0);
    map1_state(s, |x| {
        let target = x.abs().max(floor);
        let k = if target > env { rise } else { fall };
        env = target + (env - target) * k;
        env
    })
}

/// Karplus–Strong plucked string (`snd-pluck`): decays to `final_amp` at `dur`.
pub fn pluck(sr: f64, hz: f64, dur: f64, final_amp: f64, seed: u32) -> Sound {
    let period = (sr / hz.max(20.0)).round().max(2.0) as usize;
    let n = (dur * sr).round() as usize;
    let mut out = noise(sr, period as f64 / sr, seed).data;
    out.resize(n, 0.0);
    // Per-period loss so that the amplitude reaches final_amp after dur seconds.
    let periods = (dur * hz).max(1.0);
    let loss = final_amp.clamp(1e-5, 1.0).powf(1.0 / periods) as f32;
    for i in period + 1..n {
        out[i] = loss * 0.5 * (out[i - period] + out[i - period - 1]);
    }
    Sound::new(sr, out)
}

/// Chowning / Schroeder reverb (`jcrev`): four parallel combs into three series allpasses,
/// mixed `mix` wet. `decay` is the time in seconds for the combs to fall by 60 dB.
pub fn jcrev(s: &Sound, decay: f64, mix: f32) -> Sound {
    let scale = s.sr / 44_100.0;
    let combs = [1237usize, 1381, 1621, 1801].map(|d| ((d as f64 * scale) as usize).max(1));
    let allpasses = [347usize, 113, 37].map(|d| ((d as f64 * scale) as usize).max(1));
    let n = s.data.len();
    let mut wet = vec![0.0f32; n];
    for d in combs {
        let g = 10f64.powf(-3.0 * d as f64 / (decay * s.sr)) as f32;
        let mut buf = vec![0.0f32; d];
        for i in 0..n {
            let y = s.data[i] + g * buf[i % d];
            buf[i % d] = y;
            wet[i] += 0.25 * y;
        }
    }
    for d in allpasses {
        let g = 0.7f32;
        let mut buf = vec![0.0f32; d];
        for (i, w) in wet.iter_mut().enumerate() {
            let x = *w;
            let delayed = buf[i % d];
            let v = x + g * delayed;
            buf[i % d] = v;
            *w = -g * v + delayed;
        }
    }
    let data = s
        .data
        .iter()
        .zip(&wet)
        .map(|(x, w)| x * (1.0 - mix) + w * mix)
        .collect();
    Sound::at(s.sr, s.start, data)
}

/// Forward FFT of one frame in Nyquist's `snd-fft` layout:
/// `[DC, re1, im1, …, re(n/2-1), im(n/2-1), nyquist]` (n even) — n values for n samples.
pub fn fft_frame(frame: &[f32]) -> Vec<f32> {
    use rustfft::num_complex::Complex;
    let n = frame.len();
    if n < 2 {
        return frame.to_vec();
    }
    let mut buf: Vec<Complex<f32>> = frame.iter().map(|x| Complex::new(*x, 0.0)).collect();
    rustfft::FftPlanner::new()
        .plan_fft_forward(n)
        .process(&mut buf);
    let mut out = Vec::with_capacity(n);
    out.push(buf[0].re);
    for c in &buf[1..n / 2] {
        out.push(c.re);
        out.push(c.im);
    }
    if n.is_multiple_of(2) {
        out.push(buf[n / 2].re);
    } else {
        out.push(buf[n / 2].re);
        out.push(buf[n / 2].im);
        out.truncate(n);
    }
    out
}

/// Inverse of [`fft_frame`]: the frame's samples back (scaled by 1/n).
pub fn ifft_frame(spec: &[f32]) -> Vec<f32> {
    use rustfft::num_complex::Complex;
    let n = spec.len();
    if n < 2 {
        return spec.to_vec();
    }
    let mut buf = vec![Complex::new(0.0f32, 0.0); n];
    buf[0] = Complex::new(spec[0], 0.0);
    for k in 1..n / 2 {
        let c = Complex::new(spec[2 * k - 1], spec[2 * k]);
        buf[k] = c;
        buf[n - k] = c.conj();
    }
    if n.is_multiple_of(2) {
        buf[n / 2] = Complex::new(spec[n - 1], 0.0);
    }
    rustfft::FftPlanner::new()
        .plan_fft_inverse(n)
        .process(&mut buf);
    buf.iter().map(|c| c.re / n as f32).collect()
}

pub fn peak(s: &Sound) -> f32 {
    s.data.iter().fold(0.0f32, |m, v| m.max(v.abs()))
}
