//! Live accompanist (F8.1): the input monitor's pitch frames feed
//! `music_core::LiveFollower`; the UI polls `live_tick`, applies the chord / drone root
//! the follower decided (pad and drone crossfade at phrase boundaries, as with the
//! offline proposals), and shows why the harmony held. Audio never waits for this.

use audio_engine::{ContourPoint, InputMonitor, StreamRequest};
use music_core::{
    HarmonyParams, LiveDecision, LiveFollower, LiveParams, LiveStatus, Locks, Mode, ModeParams,
    PitchSample, Reference, ScaleKind, Strategy,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::harmony_cmds::HarmonyUiParams;
use crate::intent_cmds::IntentSettings;
use crate::state::{AppState, Inner};

pub struct LiveRun {
    pub follower: LiveFollower,
    /// Monitor time of the first frame after start: the live clock starts at 0 there.
    t0: Option<f64>,
    a4_hz: f64,
    scratch: Vec<ContourPoint>,
    /// Decisions not yet handed to the UI.
    pending: Vec<LiveDecision>,
    #[allow(dead_code)]
    pub started_at_ms: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LiveStartParams {
    pub intent: IntentSettings,
    pub harmony: HarmonyUiParams,
    pub strategy: Strategy,
    pub device: Option<String>,
    pub sample_rate: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LiveView {
    pub running: bool,
    pub status: Option<LiveStatus>,
    /// Decisions since the previous tick (the UI applies the last one that changed).
    pub decisions: Vec<LiveDecision>,
    pub monitor_pitch_hz: Option<f64>,
    pub monitor_confidence: f64,
}

fn ensure_monitor(s: &mut Inner, device: Option<&str>, sample_rate: Option<u32>) -> CmdResult<()> {
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording first"));
    }
    if s.monitor.is_none() {
        s.monitor = Some(InputMonitor::start(
            device,
            StreamRequest {
                sample_rate,
                channels: None,
            },
        )?);
    }
    Ok(())
}

/// Start following the microphone. The monitor is started if it is not running (the
/// tuner keeps using it); frames queued before the start are discarded.
#[tauri::command]
pub async fn live_start(
    state: State<'_, AppState>,
    params: LiveStartParams,
) -> CmdResult<LiveView> {
    if !(0.0..=1.0).contains(&params.harmony.tension)
        || !(0.0..=1.0).contains(&params.harmony.movement)
        || !(0.0..=1.0).contains(&params.harmony.richness)
        || params.harmony.register_low < 24
        || params.harmony.register_high > 96
        || params.harmony.register_high - params.harmony.register_low < 12
    {
        return Err(err("invalid_argument", "harmony parameters out of range"));
    }
    let mut s = state.lock();
    ensure_monitor(&mut s, params.device.as_deref(), params.sample_rate)?;
    let mut scratch = Vec::new();
    if let Some(m) = s.monitor.as_mut() {
        m.drain_pitch(&mut scratch);
        scratch.clear();
    }
    let mode = params.intent.mode();
    let mode_params = if mode == Mode::Off {
        mode.default_params()
    } else {
        ModeParams {
            tolerance: params.intent.tolerance.clamp(0.0, 1.0),
            sensitivity: params.intent.sensitivity.clamp(0.0, 1.0),
        }
    };
    let mut harmony = HarmonyParams::for_strategy(params.strategy, params.harmony.seed);
    harmony.tension = params.harmony.tension;
    harmony.movement = params.harmony.movement;
    harmony.richness = params.harmony.richness;
    harmony.register_low = params.harmony.register_low;
    harmony.register_high = params.harmony.register_high;
    harmony.locks = params.intent.locks;
    let mut lp = LiveParams::new(mode, harmony);
    lp.mode_params = mode_params;
    lp.locks = params.intent.locks;
    lp.scale = s.project.as_ref().and_then(|p| p.manifest().scale.clone());
    lp.profile = crate::intent_cmds::profile_offsets(&s);
    lp.phrase_params = crate::intent_cmds::phrase_params(&s);
    lp.calibration =
        music_core::Calibration::from_marks(&crate::intent_cmds::load_calibration(&s).pairs);
    lp.kinds = ScaleKind::ALL.to_vec();
    let a4_hz = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(crate::analysis_cmds::DEFAULT_A4_HZ);
    s.live = Some(LiveRun {
        follower: LiveFollower::new(lp),
        t0: None,
        a4_hz,
        scratch,
        pending: Vec::new(),
        started_at_ms: crate::reference_cmds::now_ms(),
    });
    Ok(view(&mut s))
}

/// Pull new pitch frames into the follower and report its state and new decisions.
#[tauri::command]
pub async fn live_tick(state: State<'_, AppState>) -> CmdResult<LiveView> {
    let mut s = state.lock();
    ingest(&mut s);
    Ok(view(&mut s))
}

#[derive(Debug, Clone, Deserialize)]
pub struct LiveSet {
    pub locks: Option<Locks>,
    pub bypass: Option<bool>,
}

#[tauri::command]
pub async fn live_set(state: State<'_, AppState>, set: LiveSet) -> CmdResult<LiveView> {
    let mut s = state.lock();
    if let Some(run) = s.live.as_mut() {
        if let Some(l) = set.locks {
            run.follower.set_locks(l);
        }
        if let Some(b) = set.bypass {
            run.follower.set_bypass(b);
        }
    }
    Ok(view(&mut s))
}

#[tauri::command]
pub async fn live_stop(state: State<'_, AppState>) -> CmdResult<()> {
    let mut s = state.lock();
    s.live = None;
    Ok(())
}

pub(crate) fn ingest(s: &mut Inner) {
    let a4 = s.live.as_ref().map(|r| r.a4_hz).unwrap_or(440.0);
    let Some(monitor) = s.monitor.as_mut() else {
        return;
    };
    let Some(run) = s.live.as_mut() else {
        return;
    };
    run.scratch.clear();
    monitor.drain_pitch(&mut run.scratch);
    if run.scratch.is_empty() {
        return;
    }
    let reference = Reference::new(a4);
    let t0 = *run.t0.get_or_insert(run.scratch[0].t);
    let samples: Vec<PitchSample> = run
        .scratch
        .iter()
        .map(|p| PitchSample {
            t: p.t - t0,
            midi: p.hz.map(|hz| reference.hz_to_midi(hz)),
            confidence: p.confidence,
            rms: p.rms,
        })
        .collect();
    let new = run.follower.push(&samples);
    run.pending.extend(new);
}

fn view(s: &mut Inner) -> LiveView {
    let (pitch, conf) = s
        .monitor
        .as_ref()
        .map(|m| {
            let st = m.status();
            (st.pitch_hz, st.pitch_confidence)
        })
        .unwrap_or((None, 0.0));
    match s.live.as_mut() {
        Some(run) => LiveView {
            running: true,
            status: Some(run.follower.status()),
            decisions: std::mem::take(&mut run.pending),
            monitor_pitch_hz: pitch,
            monitor_confidence: conf,
        },
        None => LiveView {
            running: false,
            status: None,
            decisions: Vec::new(),
            monitor_pitch_hz: pitch,
            monitor_confidence: conf,
        },
    }
}
