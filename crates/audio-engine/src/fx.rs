//! Offline effects on a buffer for the audio editor (Audacity programme, stage A7 =
//! E2 + E3, 2026-11-01): parametric EQ, compressor and limiter built on the same blocks
//! the track strips use (`graph::trackfx`, `graph::limiter`), so what the editor
//! previews is what the mix would do. Every function returns a new buffer (hard rule 3).

use crate::buffer::AudioBuffer;
use crate::error::{AudioError, Result};
use crate::graph::limiter::Limiter;
use crate::graph::params::db_to_gain;
use crate::graph::trackfx::{BiquadCoeffs, BiquadState, CompBlock};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqKind {
    LowShelf,
    Peak,
    HighShelf,
}

/// One band of the editor's EQ: a shelf or a peak at `hz` with `gain_db` (and `q` for peaks).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct EqBand {
    pub kind: EqKind,
    pub hz: f32,
    pub gain_db: f32,
    #[serde(default = "one")]
    pub q: f32,
}

fn one() -> f32 {
    1.0
}

impl EqBand {
    pub fn validate(&self) -> Result<()> {
        if !(20.0..=20_000.0).contains(&self.hz)
            || !(-24.0..=24.0).contains(&self.gain_db)
            || !(0.1..=10.0).contains(&self.q)
        {
            return Err(AudioError::InvalidArgument(
                "EQ band: 20..20000 Hz, ±24 dB, Q 0.1..10".into(),
            ));
        }
        Ok(())
    }
    fn coeffs(&self, sr: f32) -> BiquadCoeffs {
        match self.kind {
            EqKind::LowShelf => BiquadCoeffs::low_shelf(sr, self.hz, self.gain_db),
            EqKind::Peak => BiquadCoeffs::peak(sr, self.hz, self.q, self.gain_db),
            EqKind::HighShelf => BiquadCoeffs::high_shelf(sr, self.hz, self.gain_db),
        }
    }
}

/// Parametric EQ: the bands in series, per channel.
pub fn eq(buf: &AudioBuffer, bands: &[EqBand]) -> Result<AudioBuffer> {
    for b in bands {
        b.validate()?;
    }
    let sr = buf.sample_rate as f32;
    let ch = buf.channels.max(1) as usize;
    let coeffs: Vec<BiquadCoeffs> = bands.iter().map(|b| b.coeffs(sr)).collect();
    let mut states: Vec<Vec<BiquadState>> = (0..ch)
        .map(|_| vec![BiquadState::default(); coeffs.len()])
        .collect();
    let mut out = buf.samples.clone();
    for (i, v) in out.iter_mut().enumerate() {
        let c = i % ch;
        let mut y = *v;
        for (k, co) in coeffs.iter().enumerate() {
            y = co.run(&mut states[c][k], y);
        }
        *v = y;
    }
    Ok(AudioBuffer::new(buf.sample_rate, buf.channels, out))
}

/// Feed-forward compressor (hard knee, peak detector), channels linked through the
/// loudest one so the stereo image does not wander.
pub fn compress(
    buf: &AudioBuffer,
    threshold_db: f32,
    ratio: f32,
    attack_ms: f32,
    release_ms: f32,
    makeup_db: f32,
) -> Result<AudioBuffer> {
    if !(-60.0..=0.0).contains(&threshold_db)
        || !(1.0..=20.0).contains(&ratio)
        || !(0.1..=200.0).contains(&attack_ms)
        || !(1.0..=2000.0).contains(&release_ms)
        || !(0.0..=24.0).contains(&makeup_db)
    {
        return Err(AudioError::InvalidArgument(
            "compressor: threshold −60..0, ratio 1..20, attack 0.1..200 ms, release 1..2000 ms, make-up 0..24 dB".into(),
        ));
    }
    let sr = buf.sample_rate as f32;
    let ch = buf.channels.max(1) as usize;
    let block = CompBlock::new(sr, threshold_db, ratio, attack_ms, release_ms, makeup_db);
    let mut env = -120.0f32;
    let mut out = buf.samples.clone();
    for frame in out.chunks_mut(ch) {
        // Detector on the loudest channel, the same gain on all of them.
        let peak = frame.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let before = peak;
        let after = block.run(&mut env, before);
        let g = if before > 1e-9 {
            after / before
        } else {
            block.makeup
        };
        for v in frame.iter_mut() {
            *v *= g;
        }
    }
    Ok(AudioBuffer::new(buf.sample_rate, buf.channels, out))
}

/// Brick-wall limiter at `ceiling_db` (instant attack, 80 ms release), as on the master.
pub fn limit(buf: &AudioBuffer, ceiling_db: f32) -> Result<AudioBuffer> {
    if !(-24.0..=0.0).contains(&ceiling_db) {
        return Err(AudioError::InvalidArgument(
            "limiter ceiling −24..0 dBFS".into(),
        ));
    }
    let ch = buf.channels.max(1) as usize;
    let mut lim = Limiter::new(buf.sample_rate, db_to_gain(ceiling_db), 0.08);
    let mut out = buf.samples.clone();
    for frame in out.chunks_mut(ch) {
        lim.process_frame(frame);
    }
    Ok(AudioBuffer::new(buf.sample_rate, buf.channels, out))
}

/// Cubic interpolation of a sample run from its neighbours (the run itself is unknown).
fn interpolate_run(x: &mut [f32], a: usize, b: usize) {
    // a..b is the run (exclusive end): a cubic Hermite between the last good sample
    // before and the first good sample after, with the slopes measured just outside the
    // run (a clipped sine arc comes back as a bump, not a flat line).
    let n = x.len();
    if a < 2 || b + 1 >= n {
        let v = if a == 0 { x[b.min(n - 1)] } else { x[a - 1] };
        for s in &mut x[a..b.min(n)] {
            *s = v;
        }
        return;
    }
    let p1 = x[a - 1];
    let p2 = x[b];
    let len = (b - a + 1) as f32;
    let m1 = (x[a - 1] - x[a - 2]) * len;
    let m2 = (x[b + 1] - x[b]) * len;
    for (k, s) in x[a..b].iter_mut().enumerate() {
        let t = (k + 1) as f32 / len;
        let t2 = t * t;
        let t3 = t2 * t;
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        *s = h00 * p1 + h10 * m1 + h01 * p2 + h11 * m2;
    }
}

/// De-clipper: runs of samples at or above `threshold` (0.5..1 of full scale) are treated as
/// clipped and rebuilt by cubic interpolation from the samples around them, then the
/// whole buffer is scaled so the rebuilt peaks fit under 0 dBFS.
pub fn declip(buf: &AudioBuffer, threshold: f32) -> Result<AudioBuffer> {
    if !(0.5..=1.0).contains(&threshold) {
        return Err(AudioError::InvalidArgument(
            "declip threshold 0.5..1".into(),
        ));
    }
    let ch = buf.channels.max(1) as usize;
    let mut out = buf.samples.clone();
    for c in 0..ch {
        let mut x: Vec<f32> = out.iter().skip(c).step_by(ch).copied().collect();
        let mut i = 0;
        while i < x.len() {
            if x[i].abs() >= threshold {
                let a = i;
                while i < x.len() && x[i].abs() >= threshold {
                    i += 1;
                }
                if i - a >= 2 {
                    interpolate_run(&mut x, a, i);
                }
            } else {
                i += 1;
            }
        }
        for (k, v) in x.iter().enumerate() {
            out[k * ch + c] = *v;
        }
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if peak > 0.999 {
        let g = 0.999 / peak;
        for v in &mut out {
            *v *= g;
        }
    }
    Ok(AudioBuffer::new(buf.sample_rate, buf.channels, out))
}

/// De-esser: a dynamic high shelf at 4 kHz. A one-pole high-pass sidechain (attack 1 ms,
/// release 40 ms) measures the top; while its envelope sits above `threshold_db` the shelf
/// cuts by up to `amount_db` (coefficients refreshed every 64 samples, state continuous).
/// Everything below the shelf is untouched.
pub fn deess(buf: &AudioBuffer, threshold_db: f32, amount_db: f32) -> Result<AudioBuffer> {
    if !(-60.0..=0.0).contains(&threshold_db) || !(0.0..=24.0).contains(&amount_db) {
        return Err(AudioError::InvalidArgument(
            "de-esser: threshold −60..0 dBFS, amount 0..24 dB".into(),
        ));
    }
    let sr = buf.sample_rate as f32;
    let ch = buf.channels.max(1) as usize;
    let thr = db_to_gain(threshold_db);
    let a_hp = (-2.0 * std::f32::consts::PI * 4000.0 / sr).exp();
    let att = (-1.0 / (0.001 * sr)).exp();
    let rel = (-1.0 / (0.04 * sr)).exp();
    let mut out = buf.samples.clone();
    for c in 0..ch {
        let (mut hp_in, mut hp_out, mut env) = (0.0f32, 0.0f32, 0.0f32);
        let mut state = BiquadState::default();
        let mut coeffs = BiquadCoeffs::bypass();
        for (k, i) in (c..out.len()).step_by(ch).enumerate() {
            let x = out[i];
            hp_out = a_hp * (hp_out + x - hp_in);
            hp_in = x;
            let level = hp_out.abs();
            env = if level > env {
                att * env + (1.0 - att) * level
            } else {
                rel * env + (1.0 - rel) * level
            };
            if k.is_multiple_of(64) {
                let cut = if env > thr {
                    (20.0 * (env / thr).log10()).clamp(0.0, amount_db)
                } else {
                    0.0
                };
                coeffs = BiquadCoeffs::high_shelf(sr, 4000.0, -cut);
            }
            out[i] = coeffs.run(&mut state, x);
        }
    }
    Ok(AudioBuffer::new(buf.sample_rate, buf.channels, out))
}

/// Click removal: a sample whose jump from its neighbour exceeds `sensitivity` × the local
/// RMS (over ±10 ms) starts a click; the click (up to 1 ms) is rebuilt by interpolation.
pub fn declick(buf: &AudioBuffer, sensitivity: f32) -> Result<AudioBuffer> {
    if !(2.0..=40.0).contains(&sensitivity) {
        return Err(AudioError::InvalidArgument(
            "declick sensitivity 2..40".into(),
        ));
    }
    let sr = buf.sample_rate as usize;
    let ch = buf.channels.max(1) as usize;
    let win = (sr / 100).max(16);
    let max_len = (sr / 1000).max(4);
    let mut out = buf.samples.clone();
    let mut fixed = 0usize;
    for c in 0..ch {
        let mut x: Vec<f32> = out.iter().skip(c).step_by(ch).copied().collect();
        let n = x.len();
        if n < 4 * win {
            continue;
        }
        // Running RMS of the derivative (clicks are derivative outliers).
        let d: Vec<f32> = (0..n)
            .map(|i| if i == 0 { 0.0 } else { x[i] - x[i - 1] })
            .collect();
        let mut i = win;
        while i + win < n {
            let local: f32 = (d[i - win..i].iter().map(|v| v * v).sum::<f32>() / win as f32)
                .sqrt()
                .max(1e-5);
            if d[i].abs() > sensitivity * local {
                // Extend over the click until the derivative calms down (≤ max_len samples).
                let mut j = i + 1;
                while j < n && j - i < max_len && d[j].abs() > sensitivity * local * 0.5 {
                    j += 1;
                }
                interpolate_run(&mut x, i, j.min(n - 1));
                fixed += 1;
                i = j + 1;
            } else {
                i += 1;
            }
        }
        for (k, v) in x.iter().enumerate() {
            out[k * ch + c] = *v;
        }
    }
    let _ = fixed;
    Ok(AudioBuffer::new(buf.sample_rate, buf.channels, out))
}

/// Echo: the signal plus a feedback delay line (`delay_s` 0.02..2, `feedback` 0..0.9),
/// `mix` 0..1 is how much of the echoes is added. Same length as the input (the echoes
/// that would fall past the end are cut; select a little silence after the phrase).
pub fn echo(buf: &AudioBuffer, delay_s: f32, feedback: f32, mix: f32) -> Result<AudioBuffer> {
    if !(0.02..=2.0).contains(&delay_s)
        || !(0.0..=0.9).contains(&feedback)
        || !(0.0..=1.0).contains(&mix)
    {
        return Err(AudioError::InvalidArgument(
            "echo: delay 0.02..2 s, feedback 0..0.9, mix 0..1".into(),
        ));
    }
    let ch = buf.channels.max(1) as usize;
    let d = ((delay_s * buf.sample_rate as f32) as usize).max(1);
    let frames = buf.frames();
    let mut out = buf.samples.clone();
    for c in 0..ch {
        let mut line = vec![0.0f32; d];
        let mut w = 0usize;
        for f in 0..frames {
            let i = f * ch + c;
            let delayed = line[w];
            let x = buf.samples[i];
            line[w] = x + delayed * feedback;
            w = (w + 1) % d;
            out[i] = x + delayed * mix;
        }
    }
    Ok(AudioBuffer::new(buf.sample_rate, buf.channels, out))
}

/// Reverb (the master's Schroeder room) on a take: `size` 0..1, `damp` 0..1, `mix` 0..1
/// (dry + mix × wet). Same length as the input.
pub fn reverb(buf: &AudioBuffer, size: f32, damp: f32, mix: f32) -> Result<AudioBuffer> {
    if !(0.0..=1.0).contains(&size) || !(0.0..=1.0).contains(&damp) || !(0.0..=1.0).contains(&mix) {
        return Err(AudioError::InvalidArgument(
            "reverb: size, damp and mix 0..1".into(),
        ));
    }
    let ch = buf.channels.max(1) as usize;
    let frames = buf.frames();
    let mut out = buf.samples.clone();
    for c in 0..ch {
        let mut rv = crate::graph::sources::reverb::Reverb::new(buf.sample_rate);
        rv.set(size, damp);
        for f in 0..frames {
            let i = f * ch + c;
            let wet = rv.process(buf.samples[i]);
            out[i] = buf.samples[i] + wet * mix;
        }
    }
    Ok(AudioBuffer::new(buf.sample_rate, buf.channels, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(b: &AudioBuffer) -> f32 {
        b.rms()
    }

    #[test]
    fn the_eq_moves_its_bands_and_rejects_nonsense() {
        let low = AudioBuffer::sine(48_000, 60.0, 1.0, 0.5);
        let high = AudioBuffer::sine(48_000, 8000.0, 1.0, 0.5);
        let shelf = [EqBand {
            kind: EqKind::LowShelf,
            hz: 200.0,
            gain_db: -24.0,
            q: 1.0,
        }];
        assert!(rms(&eq(&low, &shelf).unwrap()) < rms(&low) * 0.3);
        assert!((rms(&eq(&high, &shelf).unwrap()) / rms(&high) - 1.0).abs() < 0.1);
        let peak = [EqBand {
            kind: EqKind::Peak,
            hz: 1000.0,
            gain_db: 12.0,
            q: 1.0,
        }];
        let mid = AudioBuffer::sine(48_000, 1000.0, 1.0, 0.1);
        assert!(rms(&eq(&mid, &peak).unwrap()) > rms(&mid) * 3.0);
        assert!(eq(
            &mid,
            &[EqBand {
                kind: EqKind::Peak,
                hz: 5.0,
                gain_db: 0.0,
                q: 1.0
            }]
        )
        .is_err());
    }

    #[test]
    fn the_compressor_and_the_limiter_hold_the_peaks() {
        let tone = AudioBuffer::sine(48_000, 440.0, 1.0, 0.9);
        let c = compress(&tone, -30.0, 20.0, 0.1, 100.0, 0.0).unwrap();
        let peak = c.samples[24_000..]
            .iter()
            .fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak < 0.1, "compressed peak {peak}");
        let l = limit(&tone, -12.0).unwrap();
        let ceiling = db_to_gain(-12.0);
        assert!(l.samples.iter().all(|v| v.abs() <= ceiling + 1e-4));
        assert!(compress(&tone, 5.0, 2.0, 10.0, 100.0, 0.0).is_err());
        assert!(limit(&tone, 3.0).is_err());
    }

    #[test]
    fn declip_rebuilds_a_squashed_sine_and_deess_ducks_only_the_top() {
        let sr = 48_000;
        let clean = AudioBuffer::sine(sr, 220.0, 0.5, 0.8);
        let clipped = AudioBuffer::new(
            sr,
            1,
            clean.samples.iter().map(|v| v.clamp(-0.5, 0.5)).collect(),
        );
        let fixed = declip(&clipped, 0.5).unwrap();
        let err = |a: &AudioBuffer| {
            (a.samples
                .iter()
                .zip(&clean.samples)
                .map(|(x, y)| (x - y).powi(2))
                .sum::<f32>()
                / a.samples.len() as f32)
                .sqrt()
        };
        // The rebuilt wave is much closer to the (rescaled) original than the clipped one.
        assert!(
            err(&fixed) < err(&clipped) * 0.5,
            "declip {} vs {}",
            err(&fixed),
            err(&clipped)
        );
        assert!(declip(&clipped, 0.2).is_err());

        let low = AudioBuffer::sine(sr, 440.0, 0.5, 0.5);
        let hiss = AudioBuffer::sine(sr, 7000.0, 0.5, 0.5);
        let d_low = deess(&low, -30.0, 12.0).unwrap();
        let d_hi = deess(&hiss, -30.0, 12.0).unwrap();
        assert!(
            (rms(&d_low) / rms(&low) - 1.0).abs() < 0.1,
            "440 Hz untouched"
        );
        assert!(
            rms(&d_hi) < rms(&hiss) * 0.5,
            "7 kHz ducked: {} → {}",
            rms(&hiss),
            rms(&d_hi)
        );
    }

    #[test]
    fn declick_removes_an_impulse_and_keeps_the_music() {
        let sr = 48_000;
        let mut x = AudioBuffer::sine(sr, 330.0, 0.5, 0.3).samples;
        x[12_000] += 0.9;
        x[12_001] -= 0.9;
        let dirty = AudioBuffer::new(sr, 1, x.clone());
        let clean = declick(&dirty, 8.0).unwrap();
        let spike = |b: &AudioBuffer| {
            b.samples[11_990..12_010]
                .iter()
                .fold(0.0f32, |m, v| m.max(v.abs()))
        };
        assert!(
            spike(&dirty) > 0.8 && spike(&clean) < 0.4,
            "spike {} → {}",
            spike(&dirty),
            spike(&clean)
        );
        let diff = clean.samples[..11_000]
            .iter()
            .zip(&dirty.samples[..11_000])
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(
            diff < 1e-6,
            "the music before the click is untouched ({diff})"
        );
    }

    #[test]
    fn echo_repeats_an_impulse_and_reverb_leaves_a_tail() {
        let sr = 48_000;
        let mut x = vec![0.0f32; sr as usize];
        x[1000] = 0.5;
        let dry = AudioBuffer::new(sr, 1, x);
        let e = echo(&dry, 0.25, 0.5, 1.0).unwrap();
        let at = 1000 + (0.25 * sr as f32) as usize;
        assert!(
            (e.samples[at] - 0.5).abs() < 1e-3,
            "first echo {}",
            e.samples[at]
        );
        assert!(
            (e.samples[at + at - 1000] - 0.25).abs() < 1e-3,
            "second echo fed back"
        );
        assert!(echo(&dry, 5.0, 0.5, 1.0).is_err());
        let r = reverb(&dry, 0.7, 0.3, 1.0).unwrap();
        let tail: f32 = r.samples[sr as usize / 10..sr as usize / 2]
            .iter()
            .map(|v| v.abs())
            .sum();
        assert!(tail > 0.05, "reverb tail energy {tail}");
        assert!(reverb(&dry, 2.0, 0.3, 1.0).is_err());
    }
}
