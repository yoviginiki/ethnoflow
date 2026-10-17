//! F1A.1 acceptance: eight ambient layers on the graph render for minutes without a
//! NaN, a dropout (a silent block while sources are on) or a click, and every
//! slider changes the sound measurably.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::{
    AudioBuffer, BellParams, BellSource, DroneParams, DroneSource, Graph, GraphParams, LoopParams,
    LoopSource, NoiseParams, NoiseSource, PadParams, PadSource, PluckParams, PluckSource,
    TextureParams, TextureSource, WindParams, WindSource,
};

const SR: u32 = 48_000;

struct Rack {
    graph: Graph,
    params: Arc<GraphParams>,
    noise: Arc<NoiseParams>,
    wind: Arc<WindParams>,
    texture: Arc<TextureParams>,
    bells: Arc<BellParams>,
    pluck: Arc<PluckParams>,
}

fn rack() -> Rack {
    let params = Arc::new(GraphParams::default());
    params.master.gain_db.set(-6.0);
    let mut graph = Graph::new(Arc::clone(&params), SR);
    let scale = [
        146.83f32, 164.81, 174.61, 196.0, 220.0, 246.94, 261.63, 293.66,
    ];
    let drone = Arc::new(DroneParams::default());
    drone.root_hz.set(146.83);
    let pad = Arc::new(PadParams::default());
    for (v, hz) in pad.voices.iter().zip([146.83f32, 220.0, 293.66]) {
        v.set(hz);
    }
    let noise = Arc::new(NoiseParams::default());
    let wind = Arc::new(WindParams::default());
    let texture = Arc::new(TextureParams::default());
    texture.pitches.set(&scale);
    let bells = Arc::new(BellParams::default());
    bells.pitches.set(&scale[4..]);
    bells.density.set(20.0);
    let pluck = Arc::new(PluckParams::default());
    pluck.pitches.set(&scale);
    pluck.density.set(30.0);
    let lp = Arc::new(LoopParams::default());
    lp.crossfade_frames.store(4800, Ordering::Relaxed);
    let loop_buf = Arc::new(AudioBuffer::sine(SR, 98.0, 2.0, 0.3));

    graph.set_source(0, Box::new(DroneSource::new(Arc::clone(&drone))));
    graph.set_source(
        1,
        Box::new(PadSource::new(Arc::clone(&pad)).with_sample_rate(SR)),
    );
    graph.set_source(2, Box::new(NoiseSource::new(Arc::clone(&noise))));
    graph.set_source(3, Box::new(WindSource::new(Arc::clone(&wind))));
    graph.set_source(4, Box::new(TextureSource::new(Arc::clone(&texture))));
    graph.set_source(5, Box::new(BellSource::new(Arc::clone(&bells))));
    graph.set_source(6, Box::new(PluckSource::new(Arc::clone(&pluck), SR)));
    graph.set_source(7, Box::new(LoopSource::new(lp, loop_buf)));
    for (i, db) in [-12.0f32, -14.0, -24.0, -18.0, -16.0, -12.0, -12.0, -18.0]
        .iter()
        .enumerate()
    {
        params.layers[i].gain_db.set(*db);
    }
    Rack {
        graph,
        params,
        noise,
        wind,
        texture,
        bells,
        pluck,
    }
}

fn run(rack: &mut Rack, seconds: f64) -> (usize, f32, f32) {
    // Render in device-sized callbacks; count silent blocks after the ramps settle.
    let mut buf = vec![0.0f32; 480 * 2];
    let total = (seconds * SR as f64) as usize;
    let mut done = 0usize;
    let mut silent_blocks = 0usize;
    let mut max_step = 0.0f32;
    let mut prev = 0.0f32;
    let mut peak = 0.0f32;
    while done < total {
        let n = (total - done).min(480);
        rack.graph.render(&mut buf[..n * 2], 2);
        let mut block_peak = 0.0f32;
        for f in buf[..n * 2].chunks(2) {
            assert!(f[0].is_finite() && f[1].is_finite(), "NaN at frame {done}");
            block_peak = block_peak.max(f[0].abs());
            if done > SR as usize {
                max_step = max_step.max((f[0] - prev).abs());
            }
            prev = f[0];
        }
        peak = peak.max(block_peak);
        if done > SR as usize && block_peak < 1e-4 {
            silent_blocks += 1;
        }
        done += n;
    }
    (silent_blocks, max_step, peak)
}

#[test]
fn eight_layers_render_a_minute_clean() {
    let mut r = rack();
    let (silent, max_step, peak) = run(&mut r, 60.0);
    assert_eq!(silent, 0, "silent 10 ms blocks while eight layers are on");
    assert!(max_step < 0.5, "click-sized step {max_step}");
    assert!(
        peak <= 10f32.powf(-0.3 / 20.0) + 1e-4,
        "limiter ceiling {peak}"
    );
    let m = r.params.meters(8);
    assert!(
        m.layers.iter().all(|&p| p > 0.0),
        "every layer meters: {:?}",
        m.layers
    );
}

/// The spec's "10 minutes without click/dropout": run with
/// `cargo test -p ethnoflow-audio-engine --release --test ambient_rack -- --ignored`.
#[test]
#[ignore]
fn eight_layers_render_ten_minutes_clean() {
    let mut r = rack();
    let started = std::time::Instant::now();
    let (silent, max_step, _) = run(&mut r, 600.0);
    eprintln!("10 min of 8 layers rendered in {:.1?}", started.elapsed());
    assert_eq!(silent, 0);
    assert!(max_step < 0.5, "{max_step}");
}

/// Every slider changes the sound: RMS or spectral tilt moves when a parameter does.
#[test]
fn sliders_change_the_sound_measurably() {
    fn low_share(b: &AudioBuffer) -> f32 {
        let a = 1.0 - (-2.0 * std::f32::consts::PI * 300.0 / SR as f32).exp();
        let mut lp = 0.0f32;
        let (mut low, mut all) = (0.0f32, 0.0f32);
        for &x in &b.samples[SR as usize..] {
            lp += a * (x - lp);
            low += lp * lp;
            all += x * x;
        }
        low / all.max(1e-9)
    }
    fn solo(r: &mut Rack, slot: usize) -> AudioBuffer {
        for (i, l) in r.params.layers.iter().enumerate().take(8) {
            l.solo.store(i == slot, Ordering::Relaxed);
        }
        r.graph.render_offline(4.0, 1)
    }
    let mut r = rack();
    // Noise colour tilts the spectrum.
    r.noise.color.set(0.0);
    let white = low_share(&solo(&mut r, 2));
    r.noise.color.set(1.0);
    let brown = low_share(&solo(&mut r, 2));
    assert!(brown > white * 1.5, "noise colour: {white} → {brown}");
    // Wind centre moves the whistle.
    r.wind.center_hz.set(150.0);
    let low = low_share(&solo(&mut r, 3));
    r.wind.center_hz.set(3000.0);
    let high = low_share(&solo(&mut r, 3));
    assert!(low > high * 1.5, "wind centre: {low} vs {high}");
    // Texture resonance changes the level/tone, brightness adds highs.
    r.texture.brightness.set(0.0);
    let dark = low_share(&solo(&mut r, 4));
    r.texture.brightness.set(1.0);
    let bright = low_share(&solo(&mut r, 4));
    assert!(dark > bright, "texture brightness: {dark} vs {bright}");
    // Bell density changes how much sound there is.
    r.bells.density.set(2.0);
    let sparse = solo(&mut r, 5).rms();
    r.bells.density.set(120.0);
    let dense = solo(&mut r, 5).rms();
    assert!(dense > sparse * 1.5, "bell density: {sparse} vs {dense}");
    // Pluck damping shortens the strings → less energy.
    r.pluck.damping.set(0.0);
    let ringing = solo(&mut r, 6).rms();
    r.pluck.damping.set(1.0);
    let damped = solo(&mut r, 6).rms();
    assert!(
        ringing > damped * 1.3,
        "pluck damping: {ringing} vs {damped}"
    );
}
