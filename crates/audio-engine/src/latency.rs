//! Round-trip latency (F1.4 / F2.8): a burst of clicks is played on the output while the
//! input records; the delay between each click's scheduled time and the moment it shows up
//! in the recording is the latency the trainer subtracts. Pure functions here; the shell
//! does the streaming.

use serde::{Deserialize, Serialize};

use crate::buffer::AudioBuffer;

/// Click burst: `times` seconds into the buffer, each click `click_len_s` of a decaying
/// 2 kHz tone at `amplitude`; the buffer ends one second after the last click.
pub fn click_burst(
    sample_rate: u32,
    times: &[f64],
    click_len_s: f64,
    amplitude: f32,
) -> AudioBuffer {
    let sr = sample_rate as f64;
    let last = times.iter().cloned().fold(0.0, f64::max);
    let n = ((last + 1.0) * sr) as usize;
    let mut samples = vec![0.0f32; n];
    let len = (click_len_s * sr) as usize;
    for &t in times {
        let start = (t * sr) as usize;
        for i in 0..len {
            let Some(s) = samples.get_mut(start + i) else {
                break;
            };
            let x = i as f64 / sr;
            let env = (1.0 - i as f64 / len as f64).powi(2);
            *s += amplitude * (env * (2.0 * std::f64::consts::PI * 2000.0 * x).sin()) as f32;
        }
    }
    AudioBuffer::new(sample_rate, 1, samples)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LatencyEstimate {
    /// Median delay over the clicks that were heard.
    pub ms: f64,
    /// Per click: delay in ms, or None when nothing rose above the floor in its window.
    pub per_click_ms: Vec<Option<f64>>,
    /// Spread (max − min) of the heard delays; the trainer treats it as the ± uncertainty.
    pub spread_ms: f64,
    pub noise_floor: f32,
}

/// Find each click in `recorded` (mono or multi-channel) after its scheduled time and
/// report the delays. `offset_s` is how far into the recording the burst started.
/// Returns None when fewer than two clicks were heard.
pub fn estimate_roundtrip(
    recorded: &AudioBuffer,
    click_times: &[f64],
    offset_s: f64,
    window_s: f64,
) -> Option<LatencyEstimate> {
    let mono = recorded.to_mono();
    let sr = mono.sample_rate as f64;
    if mono.samples.is_empty() || click_times.is_empty() {
        return None;
    }
    // Noise floor: RMS of the quiet part before the first click (or the first 200 ms).
    let quiet_end = (((click_times[0] + offset_s) * sr) as usize).clamp(1, mono.samples.len());
    let quiet = &mono.samples[..quiet_end.min(mono.samples.len())];
    let floor = (quiet.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / quiet.len().max(1) as f64)
        .sqrt() as f32;
    let threshold = (floor * 8.0).max(0.02);
    let mut per = Vec::with_capacity(click_times.len());
    let mut heard = Vec::new();
    for &t in click_times {
        let from = ((t + offset_s) * sr) as usize;
        let to = (((t + offset_s + window_s) * sr) as usize).min(mono.samples.len());
        let mut found = None;
        if from < to {
            for (i, x) in mono.samples[from..to].iter().enumerate() {
                if x.abs() >= threshold {
                    found = Some(i as f64 / sr * 1000.0);
                    break;
                }
            }
        }
        if let Some(ms) = found {
            heard.push(ms);
        }
        per.push(found);
    }
    if heard.len() < 2 {
        return None;
    }
    let mut sorted = heard.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let ms = sorted[sorted.len() / 2];
    Some(LatencyEstimate {
        ms,
        per_click_ms: per,
        spread_ms: sorted[sorted.len() - 1] - sorted[0],
        noise_floor: floor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_clicks_are_measured_within_a_millisecond() {
        let sr = 48_000;
        let times = [0.5, 1.0, 1.5, 2.0];
        let burst = click_burst(sr, &times, 0.005, 0.8);
        // The "recording": the burst delayed by 37 ms, attenuated, with a little noise.
        let delay = (0.037 * sr as f64) as usize;
        let mut rec = vec![0.0f32; burst.samples.len() + delay];
        let mut seed = 7u32;
        for s in rec.iter_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *s = ((seed >> 9) as f32 / (1u32 << 23) as f32 - 1.0) * 0.002;
        }
        for (i, x) in burst.samples.iter().enumerate() {
            rec[i + delay] += x * 0.3;
        }
        let est = estimate_roundtrip(&AudioBuffer::new(sr, 1, rec), &times, 0.0, 0.4).unwrap();
        assert!((est.ms - 37.0).abs() < 1.0, "{est:?}");
        assert!(est.spread_ms < 1.0);
        assert!(est.per_click_ms.iter().all(|p| p.is_some()));
    }

    #[test]
    fn silence_gives_no_estimate_and_one_missing_click_is_reported() {
        let sr = 48_000;
        let times = [0.5, 1.0, 1.5];
        assert!(estimate_roundtrip(&AudioBuffer::silence(sr, 1, 3.0), &times, 0.0, 0.4).is_none());
        let burst = click_burst(sr, &[0.5, 1.5], 0.005, 0.8);
        let mut rec = burst.samples.clone();
        rec.resize(3 * sr as usize, 0.0);
        let est = estimate_roundtrip(&AudioBuffer::new(sr, 1, rec), &times, 0.0, 0.4).unwrap();
        assert_eq!(est.per_click_ms[1], None);
        assert!(est.ms.abs() < 0.5);
    }
}
