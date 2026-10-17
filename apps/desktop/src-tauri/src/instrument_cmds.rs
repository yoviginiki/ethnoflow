//! Instrument profiles (F2.5): list/create/delete, attach to the project, calibrate a
//! note from a held tone, play a calibrated sample (pitch-shifted to any note).
//!
//! Calibration records a short take with the normal recorder into the profile's folder,
//! finds the longest stable tone, keeps only that part as the note's sample and stores
//! the measurement. The recording is the user's; nothing is uploaded.

use std::path::PathBuf;

use std::time::Duration;

use audio_engine::{
    contour, held_tone, read_wav, write_wav, AudioBuffer, PitchConfig, Player, Recorder,
    StreamRequest, WavFormat,
};
use music_core::Reference;
use project::{
    InstrumentProfile, InstrumentType, ProfileDiagnosis, ProfileNote, ProfileStore, ProfileSummary,
};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::commands::{err, CmdResult};
use crate::project_cmds::{project_view, ProjectView};
use crate::state::{AppState, Inner};

/// Seconds recorded per calibration attempt; the user holds the note inside it.
const CALIBRATION_SECONDS: f64 = 3.5;
const HELD_MIN_SECONDS: f64 = 1.5;
const HELD_MAX_DEVIATION_CENTS: f64 = 50.0;

fn store(s: &Inner) -> CmdResult<ProfileStore> {
    s.profiles_dir
        .as_ref()
        .map(|d| ProfileStore::new(d))
        .ok_or_else(|| err("no_profiles_dir", "Profile folder is not available"))
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileView {
    #[serde(flatten)]
    pub profile: InstrumentProfile,
    pub diagnosis: Option<ProfileDiagnosis>,
    /// Notes proposed by the instrument's default range, with whether each is measured.
    pub range: Vec<RangeNote>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RangeNote {
    pub midi: u8,
    pub measured: bool,
}

pub fn profile_view(p: InstrumentProfile) -> ProfileView {
    let (lo, hi) = p.instrument.default_range();
    let mut midis: Vec<u8> = (lo..=hi).collect();
    for n in &p.notes {
        if !midis.contains(&n.midi) {
            midis.push(n.midi);
        }
    }
    midis.sort_unstable();
    ProfileView {
        diagnosis: p.diagnosis(),
        range: midis
            .into_iter()
            .map(|m| RangeNote {
                midi: m,
                measured: p.note(m).is_some(),
            })
            .collect(),
        profile: p,
    }
}

/// The project's instrument profile, if any and still readable.
pub fn project_profile(s: &Inner) -> Option<InstrumentProfile> {
    let id = s
        .project
        .as_ref()?
        .manifest()
        .instrument
        .as_ref()?
        .id
        .clone();
    store(s).ok()?.load(&id).ok()
}

#[tauri::command]
pub async fn list_profiles(state: State<'_, AppState>) -> CmdResult<Vec<ProfileSummary>> {
    Ok(store(&state.lock())?.list()?)
}

#[tauri::command]
pub async fn get_profile(state: State<'_, AppState>, id: String) -> CmdResult<ProfileView> {
    Ok(profile_view(store(&state.lock())?.load(&id)?))
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewProfile {
    pub instrument: InstrumentType,
    pub name: String,
    pub tuning: String,
    #[serde(default)]
    pub transpose_semitones: i32,
    pub a4_hz: Option<f64>,
}

#[tauri::command]
pub async fn create_profile(state: State<'_, AppState>, req: NewProfile) -> CmdResult<ProfileView> {
    if req.name.trim().is_empty() {
        return Err(err("invalid_name", "Give the instrument a name"));
    }
    if !(-24..=24).contains(&req.transpose_semitones) {
        return Err(err(
            "invalid_argument",
            "transposition outside ±24 semitones",
        ));
    }
    let s = state.lock();
    let a4 = req
        .a4_hz
        .or_else(|| s.project.as_ref().and_then(|p| p.manifest().a4_hz))
        .unwrap_or(crate::analysis_cmds::DEFAULT_A4_HZ);
    let mut p = InstrumentProfile::new(req.instrument, &req.name, &req.tuning, a4);
    p.transpose_semitones = req.transpose_semitones;
    store(&s)?.save(&p)?;
    Ok(profile_view(p))
}

#[tauri::command]
pub async fn delete_profile(
    state: State<'_, AppState>,
    id: String,
) -> CmdResult<Vec<ProfileSummary>> {
    let mut s = state.lock();
    let st = store(&s)?;
    st.delete(&id)?;
    // Detach from the open project if it pointed here.
    if let Some(p) = s.project.as_mut() {
        if p.manifest().instrument.as_ref().is_some_and(|r| r.id == id) {
            p.update_manifest(|m| m.instrument = None)?;
        }
    }
    Ok(st.list()?)
}

#[tauri::command]
pub async fn set_project_instrument(
    state: State<'_, AppState>,
    id: Option<String>,
) -> CmdResult<ProjectView> {
    let mut s = state.lock();
    let r = match id {
        Some(id) => {
            let p = store(&s)?.load(&id)?;
            Some(project::manifest::InstrumentRef {
                id: p.id,
                version: p.version,
            })
        }
        None => None,
    };
    s.project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?
        .update_manifest(|m| m.instrument = r)?;
    project_view(&s)
}

#[derive(Debug, Clone, Serialize)]
pub struct CalibrationResult {
    pub note: ProfileNote,
    /// Nearest tempered note to what was actually played (may differ from the target).
    pub detected_midi: i32,
    pub detected_cents: f64,
    pub held_seconds: f64,
    pub profile: ProfileView,
}

/// Record a few seconds, find the held tone and store it as the measurement of `midi`.
/// Blocks for `CALIBRATION_SECONDS` (runs off the main thread).
#[tauri::command]
pub async fn calibrate_note(
    state: State<'_, AppState>,
    id: String,
    midi: u8,
    device: Option<String>,
    sample_rate: Option<u32>,
) -> CmdResult<CalibrationResult> {
    if midi > 127 {
        return Err(err("invalid_argument", "midi outside 0..127"));
    }
    // Phase 1: start the recorder (no monitor at the same time), release the lock.
    let (st, sample_path, tmp_path) = {
        let mut s = state.lock();
        if s.mode_busy() {
            return Err(err("busy", "Stop recording or playback first"));
        }
        s.stop_monitor();
        let st = store(&s)?;
        let sample_path = st.sample_path(&id, midi);
        let tmp_path = sample_path.with_extension("rec.wav");
        std::fs::create_dir_all(st.profile_dir(&id)).map_err(|e| err("io", e.to_string()))?;
        let _ = std::fs::remove_file(&tmp_path);
        (st, sample_path, tmp_path)
    };
    let rec = Recorder::start_with(
        device.as_deref(),
        &tmp_path,
        StreamRequest {
            sample_rate,
            channels: None,
        },
    )?;
    std::thread::sleep(Duration::from_secs_f64(CALIBRATION_SECONDS));
    let recorded = rec.stop()?;
    let full = read_wav(&recorded.path)?;
    let _ = std::fs::remove_file(&tmp_path);
    let c = contour(&full, &PitchConfig::new(full.sample_rate), 0.01);
    let tone = held_tone(&c, HELD_MIN_SECONDS, HELD_MAX_DEVIATION_CENTS)
        .ok_or_else(|| err("no_held_tone", "No steady tone of at least 1.5 s was heard"))?;

    // Keep only the held part as the sample (mono, float32).
    let sr = full.sample_rate as f64;
    let piece = full
        .to_mono()
        .slice_frames((tone.start * sr) as usize, (tone.end * sr) as usize);
    write_wav(&sample_path, &piece, WavFormat::Float32)?;

    let mut p = st.load(&id)?;
    let r = Reference::new(p.a4_hz);
    let detected = r
        .pitch_of(tone.median_hz)
        .ok_or_else(|| err("pitch", "invalid pitch"))?;
    let cents_offset = (r.hz_to_midi(tone.median_hz) - midi as f64) * 100.0;
    let note = ProfileNote {
        midi,
        measured_hz: tone.median_hz,
        cents_offset,
        stability_cents: tone.stability_cents,
        sample: sample_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned()),
        measured_at_ms: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
    };
    p.set_note(note.clone());
    st.save(&p)?;
    // Keep the project's reference current.
    {
        let mut s = state.lock();
        if let Some(pr) = s.project.as_mut() {
            if pr
                .manifest()
                .instrument
                .as_ref()
                .is_some_and(|x| x.id == id)
            {
                let v = p.version;
                pr.update_manifest(|m| {
                    if let Some(x) = m.instrument.as_mut() {
                        x.version = v;
                    }
                })?;
            }
        }
    }
    Ok(CalibrationResult {
        note,
        detected_midi: detected.midi,
        detected_cents: detected.cents,
        held_seconds: tone.end - tone.start,
        profile: profile_view(p),
    })
}

#[tauri::command]
pub async fn remove_profile_note(
    state: State<'_, AppState>,
    id: String,
    midi: u8,
) -> CmdResult<ProfileView> {
    let s = state.lock();
    let st = store(&s)?;
    let mut p = st.load(&id)?;
    if p.remove_note(midi) {
        let _ = std::fs::remove_file(st.sample_path(&id, midi));
        st.save(&p)?;
    }
    Ok(profile_view(p))
}

/// Play the sample nearest to `midi`, pitch-shifted to the tempered target (by
/// changing the playback rate; good enough for a few semitones).
#[tauri::command]
pub async fn play_profile_note(
    state: State<'_, AppState>,
    id: String,
    midi: u8,
    device: Option<String>,
    exact: bool,
) -> CmdResult<()> {
    let mut s = state.lock();
    let st = store(&s)?;
    let p = st.load(&id)?;
    let (n, _) = p
        .nearest(midi as f64)
        .ok_or_else(|| err("no_notes", "This profile has no calibrated notes yet"))?;
    let path: PathBuf = st.profile_dir(&id).join(
        n.sample
            .clone()
            .ok_or_else(|| err("no_sample", "No sample for this note"))?,
    );
    let buf = read_wav(&path)?;
    let target_hz = Reference::new(p.a4_hz).midi_to_hz(midi as f64);
    // `exact` = play the sample as recorded (its own pitch); else shift to the target.
    let ratio = if exact || n.midi == midi {
        1.0
    } else {
        target_hz / n.measured_hz
    };
    let shifted = AudioBuffer::new(
        ((buf.sample_rate as f64) * ratio).round().max(8_000.0) as u32,
        buf.channels,
        buf.samples,
    );
    if let Some(t) = s.tone.take() {
        t.stop();
    }
    s.tone = Some(Player::start(&shifted, device.as_deref(), 0)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range_marks_measured_notes_and_includes_extras() {
        let mut p = InstrumentProfile::new(InstrumentType::Kaval, "k", "D", 440.0);
        p.set_note(ProfileNote {
            midi: 100, // outside the default range: still listed
            measured_hz: 2637.0,
            cents_offset: 0.0,
            stability_cents: 1.0,
            sample: None,
            measured_at_ms: 0,
        });
        let v = profile_view(p);
        let (lo, hi) = InstrumentType::Kaval.default_range();
        assert_eq!(v.range.first().unwrap().midi, lo);
        assert!(v.range.iter().any(|r| r.midi == hi));
        assert!(v.range.iter().any(|r| r.midi == 100 && r.measured));
        assert!(v.range.iter().filter(|r| r.measured).count() == 1);
        assert_eq!(v.diagnosis.unwrap().notes, 1);
    }
}
