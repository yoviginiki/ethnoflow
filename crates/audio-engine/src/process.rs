//! Non-destructive processing (F1A.6 Sample Lab): every function takes a buffer and
//! returns a new one; the original is never touched. A `Recipe` is the saved list of
//! steps a take is played through (trim is stored separately by the project).

use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;

/// The processing a take is heard through. Defaults are all "off".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recipe {
    /// Subtract the DC offset per channel.
    pub dc_remove: bool,
    /// 2nd-order high-pass, Hz (None = off).
    pub highpass_hz: Option<f64>,
    pub gain_db: f64,
    /// Normalise the peak to this level (dBFS, e.g. −3) after the gain; None = off.
    pub normalize_peak_dbfs: Option<f64>,
    pub fade_in_s: f64,
    pub fade_out_s: f64,
    /// Bake a seamless loop: the last `s` seconds are crossfaded into the start and
    /// dropped, so looping end→start has no seam. 0 = off.
    pub loop_crossfade_s: f64,
    pub reverse: bool,
}

impl Default for Recipe {
    fn default() -> Self {
        Self {
            dc_remove: false,
            highpass_hz: None,
            gain_db: 0.0,
            normalize_peak_dbfs: None,
            fade_in_s: 0.0,
            fade_out_s: 0.0,
            loop_crossfade_s: 0.0,
            reverse: false,
        }
    }
}

impl Recipe {
    pub fn is_identity(&self) -> bool {
        *self == Recipe::default()
    }

    /// Reject values that make no acoustic sense before they reach a file.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(hz) = self.highpass_hz {
            if !(5.0..=2000.0).contains(&hz) {
                return Err("high-pass must be 5..2000 Hz".into());
            }
        }
        if !(-60.0..=24.0).contains(&self.gain_db) {
            return Err("gain must be −60..+24 dB".into());
        }
        if let Some(p) = self.normalize_peak_dbfs {
            if !(-40.0..=0.0).contains(&p) {
                return Err("normalise target must be −40..0 dBFS".into());
            }
        }
        if self.fade_in_s < 0.0
            || self.fade_out_s < 0.0
            || self.loop_crossfade_s < 0.0
            || self.fade_in_s > 600.0
            || self.fade_out_s > 600.0
            || self.loop_crossfade_s > 60.0
        {
            return Err("fades must be positive and sane".into());
        }
        Ok(())
    }
}

fn db(x: f64) -> f32 {
    10f64.powf(x / 20.0) as f32
}

pub fn gain(buf: &AudioBuffer, gain_db: f64) -> AudioBuffer {
    let g = db(gain_db);
    AudioBuffer::new(
        buf.sample_rate,
        buf.channels,
        buf.samples.iter().map(|s| s * g).collect(),
    )
}

/// Scale so the peak sits at `target_dbfs` (a silent buffer stays silent).
pub fn normalize(buf: &AudioBuffer, target_dbfs: f64) -> AudioBuffer {
    let peak = buf.peak();
    if peak <= 1e-9 {
        return buf.clone();
    }
    let g = db(target_dbfs) / peak;
    AudioBuffer::new(
        buf.sample_rate,
        buf.channels,
        buf.samples.iter().map(|s| s * g).collect(),
    )
}

/// Remove the mean of each channel.
pub fn dc_remove(buf: &AudioBuffer) -> AudioBuffer {
    let ch = buf.channels.max(1) as usize;
    let frames = buf.frames();
    if frames == 0 {
        return buf.clone();
    }
    let mut means = vec![0.0f64; ch];
    for f in buf.samples.chunks(ch) {
        for (c, s) in f.iter().enumerate() {
            means[c] += *s as f64;
        }
    }
    for m in means.iter_mut() {
        *m /= frames as f64;
    }
    let mut out = buf.samples.clone();
    for f in out.chunks_mut(ch) {
        for (c, s) in f.iter_mut().enumerate() {
            *s -= means[c] as f32;
        }
    }
    AudioBuffer::new(buf.sample_rate, buf.channels, out)
}

/// 2nd-order Butterworth high-pass (RBJ biquad), per channel.
pub fn highpass(buf: &AudioBuffer, hz: f64) -> AudioBuffer {
    let sr = buf.sample_rate.max(1) as f64;
    let fc = hz.clamp(1.0, sr * 0.45);
    let w0 = std::f64::consts::TAU * fc / sr;
    let (sin, cos) = w0.sin_cos();
    let alpha = sin / (2.0 * std::f64::consts::FRAC_1_SQRT_2);
    let b0 = (1.0 + cos) / 2.0;
    let b1 = -(1.0 + cos);
    let b2 = (1.0 + cos) / 2.0;
    let a0 = 1.0 + alpha;
    let a1 = -2.0 * cos;
    let a2 = 1.0 - alpha;
    let (b0, b1, b2, a1, a2) = (b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0);
    let ch = buf.channels.max(1) as usize;
    let mut out = buf.samples.clone();
    for c in 0..ch {
        let (mut x1, mut x2, mut y1, mut y2) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for i in (c..out.len()).step_by(ch) {
            let x = out[i] as f64;
            let y = b0 * x + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2;
            x2 = x1;
            x1 = x;
            y2 = y1;
            y1 = y;
            out[i] = y as f32;
        }
    }
    AudioBuffer::new(buf.sample_rate, buf.channels, out)
}

/// Linear fades at both ends (clamped to half the length each).
pub fn fade(buf: &AudioBuffer, fade_in_s: f64, fade_out_s: f64) -> AudioBuffer {
    let ch = buf.channels.max(1) as usize;
    let frames = buf.frames();
    let sr = buf.sample_rate as f64;
    let fi = ((fade_in_s * sr) as usize).min(frames / 2);
    let fo = ((fade_out_s * sr) as usize).min(frames / 2);
    let mut out = buf.samples.clone();
    for (k, f) in out.chunks_mut(ch).enumerate() {
        let mut g = 1.0f32;
        if fi > 0 && k < fi {
            g *= k as f32 / fi as f32;
        }
        if fo > 0 && k >= frames - fo {
            g *= (frames - k) as f32 / fo as f32;
        }
        if g < 1.0 {
            for s in f.iter_mut() {
                *s *= g;
            }
        }
    }
    AudioBuffer::new(buf.sample_rate, buf.channels, out)
}

/// Bake a seamless loop: crossfade the last `xf_s` seconds (equal power) into the
/// first ones and drop the tail, so playing the result end→start has no seam.
pub fn loop_crossfade(buf: &AudioBuffer, xf_s: f64) -> AudioBuffer {
    let ch = buf.channels.max(1) as usize;
    let frames = buf.frames();
    let xf = ((xf_s * buf.sample_rate as f64) as usize).min(frames / 2);
    if xf == 0 {
        return buf.clone();
    }
    let keep = frames - xf;
    let mut out = vec![0.0f32; keep * ch];
    out.copy_from_slice(&buf.samples[..keep * ch]);
    for k in 0..xf {
        let t = k as f32 / xf as f32;
        let theta = t * std::f32::consts::FRAC_PI_2;
        let (g_head, g_tail) = (theta.sin(), theta.cos());
        for c in 0..ch {
            let head = buf.samples[k * ch + c];
            let tail = buf.samples[(keep + k) * ch + c];
            out[k * ch + c] = head * g_head + tail * g_tail;
        }
    }
    AudioBuffer::new(buf.sample_rate, buf.channels, out)
}

pub fn reverse(buf: &AudioBuffer) -> AudioBuffer {
    let ch = buf.channels.max(1) as usize;
    let mut out = Vec::with_capacity(buf.samples.len());
    for f in buf.samples.chunks(ch).rev() {
        out.extend_from_slice(f);
    }
    AudioBuffer::new(buf.sample_rate, buf.channels, out)
}

/// Apply a whole recipe: DC → high-pass → gain → normalise → fades → loop → reverse.
pub fn apply_recipe(buf: &AudioBuffer, r: &Recipe) -> AudioBuffer {
    if r.is_identity() {
        return buf.clone();
    }
    let mut b = if r.dc_remove {
        dc_remove(buf)
    } else {
        buf.clone()
    };
    if let Some(hz) = r.highpass_hz {
        b = highpass(&b, hz);
    }
    if r.gain_db != 0.0 {
        b = gain(&b, r.gain_db);
    }
    if let Some(p) = r.normalize_peak_dbfs {
        b = normalize(&b, p);
    }
    if r.fade_in_s > 0.0 || r.fade_out_s > 0.0 {
        b = fade(&b, r.fade_in_s, r.fade_out_s);
    }
    if r.loop_crossfade_s > 0.0 {
        b = loop_crossfade(&b, r.loop_crossfade_s);
    }
    if r.reverse {
        b = reverse(&b);
    }
    b
}

/// Silent regions: windows of `window_s` whose RMS stays below `threshold_dbfs`,
/// merged, only those at least `min_len_s` long. Returned in seconds.
pub fn detect_silence(
    buf: &AudioBuffer,
    threshold_dbfs: f64,
    min_len_s: f64,
    window_s: f64,
) -> Vec<(f64, f64)> {
    let ch = buf.channels.max(1) as usize;
    let sr = buf.sample_rate.max(1) as f64;
    let win = ((window_s * sr) as usize).max(1);
    let thr = 10f64.powf(threshold_dbfs / 20.0) as f32;
    let frames = buf.frames();
    let mut out: Vec<(f64, f64)> = Vec::new();
    let mut run_start: Option<usize> = None;
    let mut k = 0usize;
    while k < frames {
        let end = (k + win).min(frames);
        let slice = &buf.samples[k * ch..end * ch];
        let rms = (slice.iter().map(|x| x * x).sum::<f32>() / slice.len().max(1) as f32).sqrt();
        let silent = rms < thr;
        match (silent, run_start) {
            (true, None) => run_start = Some(k),
            (false, Some(s)) => {
                out.push((s as f64 / sr, k as f64 / sr));
                run_start = None;
            }
            _ => {}
        }
        k = end;
    }
    if let Some(s) = run_start {
        out.push((s as f64 / sr, frames as f64 / sr));
    }
    out.retain(|(a, b)| b - a >= min_len_s);
    out
}

/// Where the sound starts and ends (seconds), ignoring leading/trailing silence
/// below `threshold_dbfs`; None when everything is silent.
pub fn sound_bounds(buf: &AudioBuffer, threshold_dbfs: f64) -> Option<(f64, f64)> {
    let ch = buf.channels.max(1) as usize;
    let sr = buf.sample_rate.max(1) as f64;
    let thr = 10f64.powf(threshold_dbfs / 20.0) as f32;
    let frames = buf.frames();
    let loud = |k: usize| {
        buf.samples[k * ch..(k + 1) * ch]
            .iter()
            .any(|s| s.abs() >= thr)
    };
    let first = (0..frames).find(|&k| loud(k))?;
    let last = (0..frames).rev().find(|&k| loud(k))?;
    Some((first as f64 / sr, (last + 1) as f64 / sr))
}

/// Nearest frame to `frame` (within ±`max_search` frames) where the first channel
/// crosses zero upwards; `frame` itself when none is found.
pub fn nearest_zero_crossing(buf: &AudioBuffer, frame: usize, max_search: usize) -> usize {
    let ch = buf.channels.max(1) as usize;
    let frames = buf.frames();
    if frames < 2 {
        return frame.min(frames);
    }
    let s = |k: usize| buf.samples[k.min(frames - 1) * ch];
    let crossing = |k: usize| k > 0 && k < frames && s(k - 1) < 0.0 && s(k) >= 0.0;
    for d in 0..=max_search {
        if frame >= d && crossing(frame - d) {
            return frame - d;
        }
        if frame + d < frames && crossing(frame + d) {
            return frame + d;
        }
    }
    frame.min(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(seconds: f32) -> AudioBuffer {
        AudioBuffer::sine(48_000, 440.0, seconds, 0.25)
    }

    #[test]
    fn recipe_steps_do_what_they_say_and_leave_the_original_alone() {
        let src = tone(1.0);
        let before = src.samples.clone();
        let n = normalize(&src, -3.0);
        assert!((n.peak() - 10f32.powf(-3.0 / 20.0)).abs() < 1e-3);
        let g = gain(&src, -6.0206);
        assert!((g.peak() - 0.125).abs() < 1e-3);
        let mut offset = src.clone();
        for s in offset.samples.iter_mut() {
            *s += 0.3;
        }
        let dc = dc_remove(&offset);
        let mean: f32 = dc.samples.iter().sum::<f32>() / dc.samples.len() as f32;
        assert!(mean.abs() < 1e-3, "{mean}");
        let f = fade(&src, 0.1, 0.2);
        assert_eq!(f.samples[0], 0.0);
        assert!(f.slice_frames(0, 2400).peak() < src.slice_frames(0, 2400).peak() * 0.6);
        assert!(
            (f.slice_frames(20_000, 28_000).peak() - 0.25).abs() < 1e-3,
            "middle untouched"
        );
        let r = reverse(&src);
        assert_eq!(r.samples[0], src.samples[src.samples.len() - 1]);
        assert_eq!(src.samples, before, "original immutable");
        assert_eq!(apply_recipe(&src, &Recipe::default()).samples, src.samples);
        assert!(Recipe {
            gain_db: 30.0,
            ..Default::default()
        }
        .validate()
        .is_err());
    }

    #[test]
    fn highpass_removes_rumble_and_keeps_the_tone() {
        let low = AudioBuffer::sine(48_000, 20.0, 1.0, 0.5);
        let high = AudioBuffer::sine(48_000, 1000.0, 1.0, 0.5);
        let lo = highpass(&low, 120.0);
        let hi = highpass(&high, 120.0);
        assert!(
            lo.slice_frames(24_000, 48_000).rms() < low.rms() * 0.1,
            "{}",
            lo.rms()
        );
        assert!((hi.slice_frames(24_000, 48_000).rms() - high.rms()).abs() < high.rms() * 0.05);
    }

    #[test]
    fn loop_crossfade_removes_the_seam() {
        // 333.3 Hz does not divide 1 s: a plain loop has a hard seam; the baked one does not.
        let src = AudioBuffer::sine(48_000, 333.3, 1.0, 0.5);
        let baked = loop_crossfade(&src, 0.05);
        assert_eq!(baked.frames(), 48_000 - 2400);
        let seam_jump = (baked.samples[0] - baked.samples[baked.samples.len() - 1]).abs();
        let plain_jump = (src.samples[0] - src.samples[src.samples.len() - 1]).abs();
        assert!(
            seam_jump < 0.03,
            "baked seam {seam_jump} (plain {plain_jump})"
        );
        assert!(baked.samples.iter().all(|s| s.abs() <= 0.5 + 1e-6));
    }

    #[test]
    fn silence_detection_bounds_and_zero_crossings() {
        // 0.5 s silence, 1 s tone, 0.25 s silence.
        let mut samples = vec![0.0f32; 24_000];
        samples.extend(tone(1.0).samples);
        samples.extend(vec![0.0f32; 12_000]);
        let b = AudioBuffer::new(48_000, 1, samples);
        let sil = detect_silence(&b, -50.0, 0.2, 0.02);
        assert_eq!(sil.len(), 2, "{sil:?}");
        assert!(sil[0].0 == 0.0 && (sil[0].1 - 0.5).abs() < 0.03);
        assert!((sil[1].0 - 1.5).abs() < 0.03 && (sil[1].1 - 1.75).abs() < 1e-6);
        let (a, z) = sound_bounds(&b, -50.0).unwrap();
        assert!(
            (a - 0.5).abs() < 0.002 && (z - 1.5).abs() < 0.002,
            "{a} {z}"
        );
        assert!(sound_bounds(&AudioBuffer::silence(48_000, 1, 0.5), -50.0).is_none());
        // Zero crossing: the tone's period is ~109 frames; a crossing is within 55.
        let k = nearest_zero_crossing(&b, 24_000 + 500, 200);
        assert!((k as i64 - 24_500).unsigned_abs() <= 60);
        assert!(b.samples[k - 1] < 0.0 && b.samples[k] >= 0.0);
    }
}
