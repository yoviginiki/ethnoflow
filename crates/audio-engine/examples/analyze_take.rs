//! Pitch-contour statistics for a real recording, so the Phase 2 log holds measured
//! numbers instead of guesses.
//!
//! Usage: cargo run --release -p ethnoflow-audio-engine --example analyze_take -- <file.wav|mp3>
//! (the owner's kaval takes stay local; nothing is written anywhere).

use std::time::Instant;

use audio_engine::{contour, decode_file, linear_to_dbfs, PitchConfig};

fn main() {
    let path = match std::env::args().nth(1) {
        Some(p) => p,
        None => {
            eprintln!("usage: analyze_take <audio file>");
            std::process::exit(2);
        }
    };
    let t0 = Instant::now();
    let buf = decode_file(std::path::Path::new(&path)).expect("decode");
    let t_decode = t0.elapsed();
    let cfg = PitchConfig::new(buf.sample_rate);
    let t1 = Instant::now();
    let pts = contour(&buf, &cfg, 0.01);
    let t_pitch = t1.elapsed();

    let voiced: Vec<_> = pts
        .iter()
        .filter_map(|p| p.hz.map(|hz| (hz, p.confidence)))
        .collect();
    let mut hzs: Vec<f64> = voiced.iter().map(|v| v.0).collect();
    hzs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut confs: Vec<f64> = voiced.iter().map(|v| v.1).collect();
    confs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pct = |v: &[f64], q: f64| {
        v.get(((v.len() as f64 - 1.0) * q) as usize)
            .copied()
            .unwrap_or(0.0)
    };
    let dur = buf.duration_seconds();

    println!("file: {path}");
    println!(
        "format: {} Hz x{}, {:.2} s, peak {:.1} dBFS, rms {:.1} dBFS",
        buf.sample_rate,
        buf.channels,
        dur,
        linear_to_dbfs(buf.peak()),
        linear_to_dbfs(buf.rms())
    );
    println!(
        "decode: {:.0} ms; pitch (hop 10 ms, {} frames): {:.0} ms = {:.1}x realtime",
        t_decode.as_secs_f64() * 1000.0,
        pts.len(),
        t_pitch.as_secs_f64() * 1000.0,
        dur / t_pitch.as_secs_f64().max(1e-9)
    );
    println!(
        "voiced: {}/{} frames ({:.0} %)",
        voiced.len(),
        pts.len(),
        100.0 * voiced.len() as f64 / pts.len().max(1) as f64
    );
    if !hzs.is_empty() {
        println!(
            "f0 range: p05 {:.1} Hz, median {:.1} Hz, p95 {:.1} Hz",
            pct(&hzs, 0.05),
            pct(&hzs, 0.5),
            pct(&hzs, 0.95)
        );
        println!(
            "confidence: p10 {:.2}, median {:.2}, p90 {:.2}",
            pct(&confs, 0.1),
            pct(&confs, 0.5),
            pct(&confs, 0.9)
        );
        // Frame-to-frame jumps of about an octave hint at octave errors.
        let mut jumps = 0;
        let mut pairs = 0;
        for w in pts.windows(2) {
            if let (Some(a), Some(b)) = (w[0].hz, w[1].hz) {
                pairs += 1;
                let c = 1200.0 * (b / a).log2();
                if (c.abs() - 1200.0).abs() < 100.0 {
                    jumps += 1;
                }
            }
        }
        println!("octave-sized jumps between consecutive voiced frames: {jumps}/{pairs}");
    }
}
