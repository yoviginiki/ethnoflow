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
}
