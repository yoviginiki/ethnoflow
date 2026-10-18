//! Song transport (Program P3, Phase 1 slice): **one position for the whole song**.
//! The arrangement timeline (seconds, `clips::Transport`) is the master when it runs;
//! the rhythm section's `BeatClock` is re-anchored to the same musical position whenever
//! the song starts, seeks or the rhythm joins while the song plays, so clips, rhythm and
//! the song map agree on the bar. Positions are converted through the project's song map
//! (ticks ↔ seconds ↔ quarter-note beats), so tempo and meter maps are honoured on the
//! timeline; the beat clock itself runs at the project tempo (one value) — a tempo change
//! inside the map re-aligns at the next play/seek, which is recorded as a limitation.

use std::sync::atomic::Ordering;
use std::sync::Arc;

use audio_engine::ClipsParams;
use project::SongMap;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::arrangement_cmds::{arrangement_mut, frames, play_arrangement, rebuild_source};
use crate::commands::{err, CmdResult};
use crate::state::{AppState, Inner};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransportStatus {
    pub playing: bool,
    /// "arrangement" while the timeline runs or is paused with a position, "rhythm" when
    /// only the rhythm clock runs, "idle" otherwise.
    pub source: String,
    pub seconds: f64,
    pub tick: u64,
    /// 1-based bar and beat (walking the meter map), and the meter at that bar.
    pub bar: u64,
    pub beat: u64,
    pub beats_in_bar: u32,
    pub bpm: f64,
    pub numerator: u32,
    pub unit: u32,
    pub duration_seconds: f64,
    /// Loop region in bars (start, end exclusive) when the timeline loops.
    pub loop_bars: Option<(u64, u64)>,
    /// Name of the song-map section at the position (when following).
    #[serde(default)]
    pub section: Option<String>,
}

/// The project's song map (sections in ticks with tempo/meter maps), or None without a project.
pub(crate) fn song_map(s: &Inner) -> Option<SongMap> {
    s.project
        .as_ref()
        .map(|p| crate::project_cmds::derived_song_map(p.manifest()))
}

/// Quarter-note beats at a timeline position, through the tempo map.
pub fn beats_at_seconds(map: &SongMap, seconds: f64) -> f64 {
    map.seconds_to_tick(seconds.max(0.0)) as f64 / map.ppq as f64
}

/// Timeline seconds for a quarter-note beat position.
pub fn seconds_at_beats(map: &SongMap, beats: f64) -> f64 {
    map.tick_to_seconds((beats.max(0.0) * map.ppq as f64).round() as u64)
}

/// Where the song is right now: the arrangement timeline if it exists, else the rhythm
/// clock's beat position mapped to seconds, else 0. Returns (seconds, source).
pub(crate) fn current_seconds(s: &Inner) -> (f64, &'static str) {
    let rate = s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000);
    let timeline = s.clips.as_ref().map(|p| {
        (
            p.transport.position() as f64 / rate.max(1) as f64,
            p.transport.playing.load(Ordering::Relaxed),
        )
    });
    // A playing timeline is the master; a running rhythm beats a paused timeline (it is
    // the thing actually moving); a paused timeline still reports where it stands.
    if let Some((secs, true)) = timeline {
        return (secs, "arrangement");
    }
    if let (Some(live), Some(g), Some(map)) = (s.rhythm.as_ref(), s.graph.as_ref(), song_map(s)) {
        let frame = g.params().frame.load(Ordering::Relaxed);
        if let Some(beat) = live.clock.beat_at(frame, g.sample_rate()) {
            return (seconds_at_beats(&map, beat), "rhythm");
        }
    }
    if let Some((secs, false)) = timeline {
        if secs > 0.0 {
            return (secs, "arrangement");
        }
    }
    (0.0, "idle")
}

/// The beat the rhythm should start on so it lands where the song is (only while the
/// arrangement timeline plays; otherwise the rhythm starts at bar 1).
pub(crate) fn song_beat_for_rhythm_start(s: &Inner) -> f64 {
    let playing = s
        .clips
        .as_ref()
        .is_some_and(|p| p.transport.playing.load(Ordering::Relaxed));
    if !playing {
        return 0.0;
    }
    match song_map(s) {
        Some(map) => beats_at_seconds(&map, current_seconds(s).0),
        None => 0.0,
    }
}

/// Re-anchor a running rhythm clock to the timeline position (after play/seek).
pub(crate) fn follow_rhythm(s: &Inner) {
    let (secs, source) = current_seconds(s);
    if source != "arrangement" {
        return;
    }
    anchor_rhythm(s, secs);
}

/// Put a running rhythm clock at a song position (seconds through the song map).
pub(crate) fn anchor_rhythm(s: &Inner, seconds: f64) {
    let (Some(live), Some(g), Some(map)) = (s.rhythm.as_ref(), s.graph.as_ref(), song_map(s))
    else {
        return;
    };
    let frame = g.params().frame.load(Ordering::Relaxed);
    live.clock.start(frame, beats_at_seconds(&map, seconds));
}

fn status(s: &Inner) -> TransportStatus {
    let map = song_map(s);
    let (seconds, source) = current_seconds(s);
    let playing = s
        .clips
        .as_ref()
        .is_some_and(|p| p.transport.playing.load(Ordering::Relaxed));
    let rate = s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000);
    let duration = s.arrangement.as_ref().map(|a| a.duration()).unwrap_or(0.0);
    let loop_bars = match (s.clips.as_ref(), map.as_ref()) {
        (Some(p), Some(m)) if p.transport.loop_on.load(Ordering::Relaxed) => {
            let a = p.transport.loop_start.load(Ordering::Relaxed) as f64 / rate as f64;
            let b = p.transport.loop_end.load(Ordering::Relaxed) as f64 / rate as f64;
            Some((
                m.tick_to_bar_beat(m.seconds_to_tick(a)).0,
                m.tick_to_bar_beat(m.seconds_to_tick(b)).0,
            ))
        }
        _ => None,
    };
    match map {
        Some(m) => {
            let tick = m.seconds_to_tick(seconds);
            let (bar, beat, _) = m.tick_to_bar_beat(tick);
            let meter = m.meter_at(tick);
            TransportStatus {
                playing,
                source: source.into(),
                seconds,
                tick,
                bar,
                beat,
                beats_in_bar: meter.numerator,
                bpm: m.tempo_at(tick),
                numerator: meter.numerator,
                unit: meter.unit,
                duration_seconds: duration,
                loop_bars,
                section: None,
            }
        }
        None => TransportStatus {
            playing,
            source: source.into(),
            seconds,
            tick: 0,
            bar: 1,
            beat: 1,
            beats_in_bar: 4,
            bpm: 0.0,
            numerator: 4,
            unit: 4,
            duration_seconds: duration,
            loop_bars,
            section: None,
        },
    }
}

/// Switch the section follower (the rhythm prefs carry the same flag; this keeps the
/// buses following when the rhythm section is off).
#[tauri::command]
pub async fn follow_map_set(state: State<'_, AppState>, on: bool) -> CmdResult<TransportStatus> {
    let mut s = state.lock();
    s.follow_map = on;
    if let Some(l) = s.rhythm.as_mut() {
        l.base.follow_map = on;
    }
    let sec = crate::section_follow::tick(&mut s);
    let mut st = status(&s);
    st.section = sec.map(|x| x.name);
    Ok(st)
}

// ---------- commands ----------

/// Play the song from `from_seconds` (or where it stands): the arrangement timeline
/// starts and a running rhythm section is re-anchored to the same bar.
#[tauri::command]
pub async fn transport_play(
    state: State<'_, AppState>,
    device: Option<String>,
    from_seconds: Option<f64>,
) -> CmdResult<TransportStatus> {
    let mut s = state.lock();
    if s.project.is_none() {
        return Err(err("no_project", "No project is open"));
    }
    play_arrangement(&mut s, device.as_deref(), from_seconds)?;
    follow_rhythm(&s);
    Ok(status(&s))
}

/// Pause the timeline where it is (the background layers keep their own switches).
#[tauri::command]
pub async fn transport_stop(state: State<'_, AppState>) -> CmdResult<TransportStatus> {
    let s = state.lock();
    if let Some(p) = s.clips.as_ref() {
        p.transport.playing.store(false, Ordering::Relaxed);
    }
    Ok(status(&s))
}

#[tauri::command]
pub async fn transport_seek(
    state: State<'_, AppState>,
    seconds: f64,
) -> CmdResult<TransportStatus> {
    let mut s = state.lock();
    let rate = s.graph.as_ref().map(|g| g.sample_rate()).unwrap_or(48_000);
    if s.clips.is_none() {
        // A position needs a transport even before the first play.
        s.clips = Some(Arc::new(ClipsParams::default()));
    }
    if let Some(p) = s.clips.as_ref() {
        p.transport.seek(frames(seconds.max(0.0), rate));
        // A paused transport reports the seek target immediately.
        if !p.transport.playing.load(Ordering::Relaxed) {
            p.transport
                .position_frames
                .store(frames(seconds.max(0.0), rate), Ordering::Relaxed);
        }
    }
    // A seek moves the song wherever it is heard from: the timeline (paused or playing)
    // and a running rhythm section alike; the section follower re-reads the position.
    anchor_rhythm(&s, seconds.max(0.0));
    let sec = crate::section_follow::tick(&mut s);
    let mut st = status(&s);
    st.section = sec.map(|x| x.name);
    Ok(st)
}

#[tauri::command]
pub async fn transport_status(state: State<'_, AppState>) -> CmdResult<TransportStatus> {
    let mut s = state.lock();
    let sec = crate::section_follow::tick(&mut s);
    let mut st = status(&s);
    st.section = sec.map(|x| x.name);
    Ok(st)
}

/// Loop `bars` bars from `start_bar` (1-based) on the timeline; `None` clears the loop.
/// The region is stored in the arrangement so the studio shows the same loop.
#[tauri::command]
pub async fn transport_loop(
    state: State<'_, AppState>,
    start_bar: Option<u64>,
    bars: Option<u64>,
) -> CmdResult<TransportStatus> {
    let mut s = state.lock();
    let region = match (start_bar, bars, song_map(&s)) {
        (Some(b), Some(n), Some(map)) if n > 0 => {
            let a = map.tick_to_seconds(map.bar_to_tick(b.max(1)));
            let z = map.tick_to_seconds(map.bar_to_tick(b.max(1) + n));
            Some((a, z))
        }
        _ => None,
    };
    {
        let root = s
            .project
            .as_ref()
            .map(|p| p.root().to_path_buf())
            .ok_or_else(|| err("no_project", "No project is open"))?;
        let a = arrangement_mut(&mut s)?;
        a.loop_region = region;
        a.save(&root)?;
    }
    if s.clips.is_none() {
        s.clips = Some(Arc::new(ClipsParams::default()));
    }
    if s.graph.is_some() {
        rebuild_source(&mut s)?;
    } else if let Some(p) = s.clips.as_ref() {
        // Nothing plays yet: keep the loop on the transport so the status shows it; the
        // first play rebuilds the source from the arrangement (seconds → frames at the
        // stream rate).
        match region {
            Some((a, z)) => {
                p.transport
                    .loop_start
                    .store(frames(a, 48_000), Ordering::Relaxed);
                p.transport
                    .loop_end
                    .store(frames(z, 48_000), Ordering::Relaxed);
                p.transport.loop_on.store(true, Ordering::Relaxed);
            }
            None => p.transport.loop_on.store(false, Ordering::Relaxed),
        }
    }
    Ok(status(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_and_beats_follow_the_tempo_map() {
        // 120 BPM for 4 bars of 4/4 (8 s), then 60 BPM.
        let mut map = SongMap::new(120.0, 4, 4, Vec::new());
        map.tempo_map.push(project::TempoSegment {
            tick: map.ppq * 16,
            bpm: 60.0,
        });
        assert!((beats_at_seconds(&map, 4.0) - 8.0).abs() < 1e-6);
        // 8 s = 16 beats at 120; 2 more seconds at 60 BPM = 2 beats.
        assert!((beats_at_seconds(&map, 10.0) - 18.0).abs() < 1e-6);
        assert!((seconds_at_beats(&map, 18.0) - 10.0).abs() < 1e-6);
        // Round trip at an arbitrary position.
        let b = beats_at_seconds(&map, 13.37);
        assert!((seconds_at_beats(&map, b) - 13.37).abs() < 1e-3);
    }

    #[test]
    fn bar_and_beat_are_right_in_seven_eight() {
        let map = SongMap::new(120.0, 7, 8, vec![2, 2, 3]);
        // A bar of 7/8 at 120 BPM (quarter = 0.5 s) lasts 1.75 s.
        let tick = map.seconds_to_tick(1.75 * 2.0 + 0.25);
        let (bar, beat, _) = map.tick_to_bar_beat(tick);
        assert_eq!(bar, 3);
        assert_eq!(beat, 2, "0.25 s into the bar = second eighth");
        assert_eq!(map.meter_at(tick).numerator, 7);
    }

    #[test]
    fn idle_status_without_a_project_is_safe() {
        let s = Inner::default();
        let st = status(&s);
        assert!(!st.playing);
        assert_eq!(st.source, "idle");
        assert_eq!((st.bar, st.beat), (1, 1));
        assert_eq!(song_beat_for_rhythm_start(&s), 0.0);
    }
}
