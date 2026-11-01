//! Compatibility check: run one or more Audacity `.ny` plug-ins on a synthetic stereo tone
//! with the header defaults and report what happened.
//!
//! `cargo run -p ethnoflow-nyquist --example run_ny -- path/to/*.ny`

use nyquist::{parse_header, run_plugin, PluginOutput};

fn main() {
    let sr = 44_100.0;
    let n = (sr * 2.0) as usize;
    let l: Vec<f32> = (0..n)
        .map(|i| (i as f64 * 440.0 / sr * std::f64::consts::TAU).sin() as f32 * 0.5)
        .collect();
    let r: Vec<f32> = (0..n)
        .map(|i| (i as f64 * 220.0 / sr * std::f64::consts::TAU).sin() as f32 * 0.4)
        .collect();
    let mut ok = 0;
    let mut total = 0;
    for path in std::env::args().skip(1) {
        total += 1;
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                println!("{path}: cannot read: {e}");
                continue;
            }
        };
        let h = parse_header(&src);
        let input: Vec<Vec<f32>> = if h.kind == "generate" {
            Vec::new()
        } else {
            vec![l.clone(), r.clone()]
        };
        let t = std::time::Instant::now();
        match run_plugin(&h, &[], &input, sr, 0.0) {
            Ok(res) => {
                ok += 1;
                let what = match &res.output {
                    PluginOutput::Audio(ch) => format!(
                        "audio {} ch × {} samples",
                        ch.len(),
                        ch.first().map(|c| c.len()).unwrap_or(0)
                    ),
                    PluginOutput::Message(m) => {
                        format!("message: {}", m.lines().next().unwrap_or(""))
                    }
                    PluginOutput::Nothing => "nil".into(),
                    PluginOutput::Labels(l) => format!(
                        "{} labels, first at {:.3} s: {}",
                        l.len(),
                        l[0].start,
                        l[0].text
                    ),
                };
                println!(
                    "OK   {:<28} [{} {}] {} ({} ms){}",
                    h.name,
                    h.kind,
                    h.controls.len(),
                    what,
                    t.elapsed().as_millis(),
                    if res.printed.is_empty() {
                        String::new()
                    } else {
                        format!(" printed: {}", res.printed.lines().next().unwrap_or(""))
                    }
                );
            }
            Err(e) => println!(
                "FAIL {:<28} [{} {}] {}",
                h.name,
                h.kind,
                h.controls.len(),
                e
            ),
        }
    }
    println!("{ok}/{total} plug-ins ran");
}
