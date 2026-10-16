//! Generate synthetic QA fixtures into qa/fixtures (run from the repo root):
//!   cargo run -p ethnoflow-audio-engine --example gen_fixtures
//!
//! Real instrument recordings (kaval, flute) are added by hand and never generated.

use audio_engine::{write_wav, AudioBuffer, WavFormat};
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "qa/fixtures".into()),
    );
    std::fs::create_dir_all(&out).expect("create fixtures dir");

    // 1. A4 reference tone, 2 s, -6 dBFS
    write_wav(
        out.join("sine_a4_440hz_48k.wav"),
        &AudioBuffer::sine(48_000, 440.0, 2.0, 0.5),
        WavFormat::Float32,
    )
    .unwrap();

    // 2. Digital silence, 1 s stereo
    write_wav(
        out.join("silence_1s_stereo_44k1.wav"),
        &AudioBuffer::silence(44_100, 2, 1.0),
        WavFormat::Pcm24,
    )
    .unwrap();

    // 3. Clipped square-ish tone (over-driven sine), for clipping detection
    let mut clip = AudioBuffer::sine(48_000, 220.0, 1.0, 3.0);
    for s in &mut clip.samples {
        *s = s.clamp(-1.0, 1.0);
    }
    write_wav(out.join("clipped_220hz_48k.wav"), &clip, WavFormat::Pcm16).unwrap();

    // 4. Linear sine sweep 50 Hz → 8 kHz over 4 s (analysis / resampler tests later)
    let sr = 48_000u32;
    let secs = 4.0f32;
    let n = (sr as f32 * secs) as usize;
    let (f0, f1) = (50.0f32, 8_000.0f32);
    let mut phase = 0.0f32;
    let mut sweep = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let f = f0 + (f1 - f0) * (t / secs);
        phase += 2.0 * std::f32::consts::PI * f / sr as f32;
        sweep.push(0.5 * phase.sin());
    }
    write_wav(
        out.join("sweep_50_8000hz_48k.wav"),
        &AudioBuffer::new(sr, 1, sweep),
        WavFormat::Float32,
    )
    .unwrap();

    // 5. "Breathy flute": fundamental + harmonics + low noise + slow vibrato, 3 s at 44.1 kHz.
    let sr = 44_100u32;
    let n = (sr as f32 * 3.0) as usize;
    let mut seed = 0x9E37_79B9u32;
    let mut noise = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let mut flute = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / sr as f32;
        let vib = 1.0 + 0.004 * (2.0 * std::f32::consts::PI * 5.5 * t).sin();
        let f = 587.33 * vib; // D5, chosen arbitrarily — not a key assumption
        let env = (t * 8.0).min(1.0) * (1.0 - ((t - 2.6) * 4.0).max(0.0)).max(0.0);
        let s = 0.45 * (2.0 * std::f32::consts::PI * f * t).sin()
            + 0.18 * (2.0 * std::f32::consts::PI * 2.0 * f * t).sin()
            + 0.07 * (2.0 * std::f32::consts::PI * 3.0 * f * t).sin()
            + 0.02 * noise();
        flute.push(s * env);
    }
    write_wav(
        out.join("synthetic_flute_d5_44k1.wav"),
        &AudioBuffer::new(sr, 1, flute),
        WavFormat::Pcm24,
    )
    .unwrap();

    println!("fixtures written to {}", out.display());
}
