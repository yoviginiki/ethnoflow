//! F3.1 regression fixtures: melodic material with a known tonic and mode
//! (`qa/fixtures/modal/*.json`). The known answer must be in the top 2 hypotheses and
//! the explanation must be consistent with the histogram.

use music_core::{explain, rank_hypotheses, PitchClassHistogram, ScaleKind};
use serde::Deserialize;

#[derive(Deserialize)]
struct Fixture {
    name: String,
    tonic_pc: u8,
    kind: ScaleKind,
    notes: Vec<(f64, f64)>,
}

fn fixtures() -> Vec<Fixture> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../qa/fixtures/modal");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("qa/fixtures/modal exists") {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            let text = std::fs::read_to_string(&path).unwrap();
            out.push(
                serde_json::from_str::<Fixture>(&text).unwrap_or_else(|e| panic!("{path:?}: {e}")),
            );
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[test]
fn every_modal_fixture_has_its_mode_in_the_top_two() {
    let fx = fixtures();
    assert!(
        fx.len() >= 10,
        "need at least 10 fixtures, found {}",
        fx.len()
    );
    let mut report = Vec::new();
    for f in &fx {
        let h = PitchClassHistogram::from_midi(f.notes.iter().copied());
        let r = rank_hypotheses(&h, &ScaleKind::ALL);
        let pos = r
            .iter()
            .position(|x| x.tonic_pc == f.tonic_pc && x.kind == f.kind)
            .unwrap_or(usize::MAX);
        report.push(format!(
            "{:<32} expected {:>2} {:<18} at #{} (top: {} {:?} {:.0} %)",
            f.name,
            f.tonic_pc,
            format!("{:?}", f.kind),
            pos.wrapping_add(1),
            r[0].tonic_pc,
            r[0].kind,
            r[0].probability * 100.0
        ));
        assert!(
            pos < 2,
            "{}: expected mode at position {}\n{}",
            f.name,
            pos.wrapping_add(1),
            report.join("\n")
        );
        let e = explain(&h, &r[pos], &r).unwrap();
        assert!(e.coverage > 0.9, "{}: coverage {}", f.name, e.coverage);
        assert!(
            e.tonic_share > 0.15,
            "{}: tonic share {}",
            f.name,
            e.tonic_share
        );
        assert!(e.tonic_share <= 1.0 && e.fifth_share <= 1.0);
    }
    eprintln!("{}", report.join("\n"));
}
