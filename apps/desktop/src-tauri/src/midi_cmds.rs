//! Phase 2.7a: MIDI takes (imported `.mid` rendered by the built-in synth), render
//! options (tempo, muted tracks, voice) and "play the notes" for any note list.

use std::path::Path;
use std::sync::Arc;

use audio_engine::{
    render_notes, AudioBuffer, Player, PlayerStatus, SynthConfig, SynthNote, Voice,
};
use music_core::{parse_smf, MidiClip, Note, TrackInfo};
use serde::Serialize;
use tauri::State;

use crate::analysis_cmds::{current_notes, DEFAULT_A4_HZ};
use crate::commands::{err, CmdResult};
use crate::project_cmds::{project_view, ProjectView};
use crate::state::{AppState, Inner, LoadedMidi, LoadedTake, MidiRender};

/// Sample rate for synth renderings (device adaptation happens in the player).
pub const RENDER_SAMPLE_RATE: u32 = 48_000;

pub fn synth_notes(notes: &[Note]) -> Vec<SynthNote> {
    notes
        .iter()
        .map(|n| SynthNote {
            start: n.start,
            end: n.end,
            midi: n.midi,
            cents: n.cents,
            velocity: n.velocity,
            bend: n.bend.clone(),
        })
        .collect()
}

/// Render a clip with the given options (tempo scales time, muted tracks are dropped).
pub fn render_clip(clip: &MidiClip, r: &MidiRender, a4: f64) -> AudioBuffer {
    let f = 1.0 / r.tempo_factor.clamp(0.25, 2.0);
    let notes: Vec<SynthNote> = clip
        .notes
        .iter()
        .filter(|n| !r.muted_tracks.contains(&n.track))
        .map(|n| SynthNote {
            start: n.start * f,
            end: n.end * f,
            midi: n.pitch as i32,
            cents: 0.0,
            velocity: n.velocity as f64 / 127.0,
            bend: Vec::new(),
        })
        .collect();
    let cfg = SynthConfig {
        a4_hz: a4,
        voice: r.voice,
        ..SynthConfig::new(RENDER_SAMPLE_RATE)
    };
    render_notes(&notes, &cfg)
}

/// Notes of a clip in playback time (tempo factor applied, muted tracks removed).
pub fn clip_notes(clip: &MidiClip, r: &MidiRender) -> Vec<Note> {
    let f = 1.0 / r.tempo_factor.clamp(0.25, 2.0);
    clip.notes
        .iter()
        .filter(|n| !r.muted_tracks.contains(&n.track))
        .map(|n| Note {
            start: n.start * f,
            end: n.end * f,
            midi: n.pitch as i32,
            cents: 0.0,
            confidence: 1.0,
            velocity: n.velocity as f64 / 127.0,
            bend: Vec::new(),
        })
        .collect()
}

pub fn parse_midi_file(path: &Path) -> CmdResult<MidiClip> {
    let bytes = std::fs::read(path).map_err(|e| err("io", e.to_string()))?;
    parse_smf(&bytes).map_err(|e| err("midi", e.to_string()))
}

#[derive(Debug, Clone, Serialize)]
pub struct MidiTakeView {
    pub tracks: Vec<TrackInfo>,
    pub tempo_from_file: bool,
    pub bpm: f64,
    pub time_signature: Option<(u8, u8)>,
    pub render: MidiRender,
    pub duration_seconds: f64,
}

pub fn midi_view(s: &Inner) -> Option<MidiTakeView> {
    let l = s.loaded.as_ref()?;
    let m = l.midi.as_ref()?;
    Some(MidiTakeView {
        tracks: m.clip.tracks.clone(),
        tempo_from_file: m.clip.tempo_from_file,
        bpm: m.clip.tempo_map.first().map(|t| t.bpm).unwrap_or(120.0),
        time_signature: m
            .clip
            .time_signatures
            .first()
            .map(|t| (t.numerator, t.denominator)),
        render: m.render.clone(),
        duration_seconds: l.buffer.duration_seconds(),
    })
}

#[tauri::command]
pub async fn midi_take(state: State<'_, AppState>) -> CmdResult<Option<MidiTakeView>> {
    Ok(midi_view(&state.lock()))
}

/// Re-render the loaded MIDI take with new options; the file itself never changes.
#[tauri::command]
pub async fn set_midi_render(
    state: State<'_, AppState>,
    render: MidiRender,
) -> CmdResult<MidiTakeView> {
    if !(render.tempo_factor.is_finite() && (0.25..=2.0).contains(&render.tempo_factor)) {
        return Err(err("invalid_argument", "tempo factor outside 0.25..2"));
    }
    let mut s = state.lock();
    if s.mode_busy() {
        return Err(err("busy", "Stop recording or playback first"));
    }
    let a4 = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(DEFAULT_A4_HZ);
    let l = s
        .loaded
        .as_mut()
        .ok_or_else(|| err("no_take", "Nothing is loaded"))?;
    let m = l
        .midi
        .as_mut()
        .ok_or_else(|| err("not_midi", "The loaded take is not a MIDI file"))?;
    m.render = render;
    l.buffer = Arc::new(render_clip(&m.clip, &m.render, a4));
    midi_view(&s).ok_or_else(|| err("not_midi", "no MIDI take"))
}

/// Render the current notes (edited > automatic > MIDI clip) and play them.
#[tauri::command]
pub async fn play_notes(
    state: State<'_, AppState>,
    device: Option<String>,
    voice: Option<Voice>,
) -> CmdResult<PlayerStatus> {
    let mut s = state.lock();
    if s.recording.is_some() {
        return Err(err("recording", "Stop recording first"));
    }
    s.stop_playback();
    let notes = current_notes(&s)?;
    if notes.is_empty() {
        return Err(err("no_notes", "There are no notes to play"));
    }
    let a4 = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(DEFAULT_A4_HZ);
    let cfg = SynthConfig {
        a4_hz: a4,
        voice: voice.unwrap_or(Voice::Flute),
        ..SynthConfig::new(RENDER_SAMPLE_RATE)
    };
    let buf = render_notes(&synth_notes(&notes), &cfg);
    let player = Player::start(&buf, device.as_deref(), 0)?;
    let st = player.status();
    s.notes_player = Some(player);
    Ok(st)
}

#[tauri::command]
pub async fn notes_playback_status(state: State<'_, AppState>) -> CmdResult<Option<PlayerStatus>> {
    let mut s = state.lock();
    let st = s.notes_player.as_ref().map(|p| p.status());
    if st.as_ref().is_some_and(|st| st.finished) {
        if let Some(p) = s.notes_player.take() {
            p.stop();
        }
    }
    Ok(st)
}

#[tauri::command]
pub async fn stop_notes(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(p) = state.lock().notes_player.take() {
        p.stop();
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
pub struct SentToPlayer {
    pub project: ProjectView,
    pub take_id: String,
}

/// Turn the current notes (extracted, edited or MIDI) into a new MIDI take of the
/// project — no export/import round trip — ready for the falling-notes player.
#[tauri::command]
pub async fn send_notes_to_player(
    state: State<'_, AppState>,
    label: Option<String>,
) -> CmdResult<SentToPlayer> {
    let mut s = state.lock();
    if s.mode_busy() {
        return Err(err("busy", "Stop recording or playback first"));
    }
    let notes = current_notes(&s)?;
    if notes.is_empty() {
        return Err(err("no_notes", "There are no notes to send"));
    }
    let parent = s
        .loaded
        .as_ref()
        .map(|l| l.id.clone())
        .ok_or_else(|| err("no_take", "Nothing is loaded"))?;
    let m = s
        .project
        .as_ref()
        .ok_or_else(|| err("no_project", "No project is open"))?
        .manifest();
    let a4 = m.a4_hz.unwrap_or(DEFAULT_A4_HZ);
    let opts = music_core::SmfOptions {
        tempo_bpm: m
            .tempo_map
            .first()
            .map(|t| t.bpm)
            .unwrap_or(music_core::TECHNICAL_TEMPO_BPM),
        time_signature: m
            .time_signatures
            .first()
            .map(|t| (t.numerator as u8, t.unit as u8)),
        ..music_core::SmfOptions::default()
    };
    let bytes = music_core::write_smf(&notes, &opts);
    let rendered = render_notes(
        &synth_notes(&notes),
        &SynthConfig {
            a4_hz: a4,
            ..SynthConfig::new(RENDER_SAMPLE_RATE)
        },
    );
    let p = s
        .project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    let take = p.create_derived_midi(
        &parent,
        label.as_deref(),
        (
            rendered.sample_rate,
            rendered.channels,
            rendered.frames() as u64,
        ),
        |path| std::fs::write(path, &bytes),
    )?;
    s.health.insert(take.id.clone(), project::TakeHealth::Ok);
    Ok(SentToPlayer {
        project: project_view(&s)?,
        take_id: take.id,
    })
}

/// Build the in-memory take for a `.mid` file: parse, render, keep the clip.
pub fn load_midi_take(id: &str, path: &Path, a4: f64) -> CmdResult<LoadedTake> {
    let clip = parse_midi_file(path)?;
    let render = MidiRender::default();
    let buffer = render_clip(&clip, &render, a4);
    Ok(LoadedTake {
        id: id.to_string(),
        buffer: Arc::new(buffer),
        offset_seconds: 0.0,
        midi: Some(LoadedMidi { clip, render }),
    })
}

/// Import a `.mid` into the project as a take; facts describe the rendering.
pub fn import_midi(s: &mut Inner, src: &Path) -> CmdResult<ProjectView> {
    let clip = parse_midi_file(src)?;
    let a4 = s
        .project
        .as_ref()
        .and_then(|p| p.manifest().a4_hz)
        .unwrap_or(DEFAULT_A4_HZ);
    let render = MidiRender::default();
    let buffer = render_clip(&clip, &render, a4);
    if buffer.is_empty() {
        return Err(err("empty_file", "The MIDI file renders to no audio"));
    }
    let p = s
        .project
        .as_mut()
        .ok_or_else(|| err("no_project", "No project is open"))?;
    let take = p.import_file(
        src,
        project::TakeSource::MidiFile,
        buffer.sample_rate,
        buffer.channels,
        buffer.frames() as u64,
    )?;
    s.health.insert(take.id.clone(), project::TakeHealth::Ok);
    s.stop_playback();
    s.loaded = Some(LoadedTake {
        id: take.id.clone(),
        buffer: Arc::new(buffer),
        offset_seconds: 0.0,
        midi: Some(LoadedMidi { clip, render }),
    });
    project_view(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use music_core::{write_smf, SmfOptions};

    #[test]
    fn tempo_factor_and_mutes_change_the_rendering_not_the_clip() {
        let notes = vec![
            Note {
                start: 0.0,
                end: 1.0,
                midi: 60,
                cents: 0.0,
                confidence: 1.0,
                velocity: 0.5,
                bend: vec![],
            },
            Note {
                start: 1.0,
                end: 2.0,
                midi: 67,
                cents: 0.0,
                confidence: 1.0,
                velocity: 0.5,
                bend: vec![],
            },
        ];
        let bytes = write_smf(&notes, &SmfOptions::default());
        let clip = parse_smf(&bytes).unwrap();
        let normal = render_clip(&clip, &MidiRender::default(), 440.0);
        let fast = render_clip(
            &clip,
            &MidiRender {
                tempo_factor: 2.0,
                ..MidiRender::default()
            },
            440.0,
        );
        assert!((normal.duration_seconds() - 2.06).abs() < 0.01);
        assert!((fast.duration_seconds() - 1.06).abs() < 0.01);
        let muted = render_clip(
            &clip,
            &MidiRender {
                muted_tracks: vec![0],
                ..MidiRender::default()
            },
            440.0,
        );
        assert!(muted.is_empty());
        assert_eq!(clip.notes.len(), 2, "clip untouched");
        assert_eq!(
            clip_notes(
                &clip,
                &MidiRender {
                    tempo_factor: 0.5,
                    ..MidiRender::default()
                }
            )[1]
            .start,
            2.0
        );
    }
}
