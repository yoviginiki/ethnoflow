//! MIDI clips: note events with a tempo map, read from Standard MIDI Files.
//!
//! Times are kept both in ticks (the file's truth) and in seconds (through the tempo
//! map), so views and the synth never re-derive timing. The file's own tempo and
//! meter are used; nothing is assumed when they are absent (SMF default: 120 BPM, as
//! the standard says — reported as `tempo_from_file = false`).

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoteEvent {
    pub pitch: u8,
    pub velocity: u8,
    pub channel: u8,
    pub track: u16,
    pub start_tick: u64,
    pub duration_ticks: u64,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempoPointTicks {
    pub tick: u64,
    pub bpm: f64,
    /// Seconds at `tick` (prefix sum), for O(log n) conversion.
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeSignatureTicks {
    pub tick: u64,
    pub numerator: u8,
    pub denominator: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackInfo {
    pub index: u16,
    pub name: Option<String>,
    /// First program change on the track (GM instrument), if any.
    pub program: Option<u8>,
    pub channels: Vec<u8>,
    pub note_count: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MidiClip {
    pub ticks_per_quarter: u16,
    pub tempo_map: Vec<TempoPointTicks>,
    /// False when the file has no tempo event and the SMF default (120 BPM) applies.
    pub tempo_from_file: bool,
    pub time_signatures: Vec<TimeSignatureTicks>,
    pub tracks: Vec<TrackInfo>,
    /// Sorted by start, then pitch.
    pub notes: Vec<NoteEvent>,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MidiError {
    Parse(String),
    Smpte,
    Empty,
}

impl std::fmt::Display for MidiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiError::Parse(m) => write!(f, "not a Standard MIDI File: {m}"),
            MidiError::Smpte => write!(f, "SMPTE timing is not supported"),
            MidiError::Empty => write!(f, "the file contains no notes"),
        }
    }
}
impl std::error::Error for MidiError {}

/// Seconds at `tick` given a tempo map (sorted by tick, first point at tick 0).
pub fn tick_to_seconds(map: &[TempoPointTicks], tpq: u16, tick: u64) -> f64 {
    let i = map.partition_point(|p| p.tick <= tick).saturating_sub(1);
    let p = &map[i];
    p.seconds + (tick - p.tick) as f64 * 60.0 / (p.bpm * tpq as f64)
}

/// Parse SMF bytes into a clip with absolute times.
pub fn parse_smf(bytes: &[u8]) -> Result<MidiClip, MidiError> {
    let smf = Smf::parse(bytes).map_err(|e| MidiError::Parse(e.to_string()))?;
    let tpq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int(),
        Timing::Timecode(..) => return Err(MidiError::Smpte),
    };

    // Pass 1: tempo and meter events from every track (format 1 keeps them in track 0,
    // but files in the wild do not always obey).
    let mut tempos: Vec<(u64, f64)> = Vec::new();
    let mut sigs: Vec<TimeSignatureTicks> = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0u64;
        for ev in track {
            tick += ev.delta.as_int() as u64;
            if let TrackEventKind::Meta(m) = ev.kind {
                match m {
                    MetaMessage::Tempo(us) => {
                        let bpm = 60_000_000.0 / us.as_int().max(1) as f64;
                        tempos.push((tick, bpm));
                    }
                    MetaMessage::TimeSignature(n, d, _, _) => sigs.push(TimeSignatureTicks {
                        tick,
                        numerator: n,
                        denominator: 1u8 << d.min(7),
                    }),
                    _ => {}
                }
            }
        }
    }
    let tempo_from_file = !tempos.is_empty();
    tempos.sort_by_key(|t| t.0);
    tempos.dedup_by_key(|t| t.0);
    if tempos.first().map(|t| t.0) != Some(0) {
        tempos.insert(0, (0, 120.0));
    }
    let mut tempo_map: Vec<TempoPointTicks> = Vec::with_capacity(tempos.len());
    let mut seconds = 0.0;
    for (i, &(tick, bpm)) in tempos.iter().enumerate() {
        if i > 0 {
            let prev = &tempo_map[i - 1];
            seconds = prev.seconds + (tick - prev.tick) as f64 * 60.0 / (prev.bpm * tpq as f64);
        }
        tempo_map.push(TempoPointTicks { tick, bpm, seconds });
    }
    sigs.sort_by_key(|s| s.tick);

    // Pass 2: notes.
    let mut notes: Vec<NoteEvent> = Vec::new();
    let mut tracks: Vec<TrackInfo> = Vec::new();
    for (ti, track) in smf.tracks.iter().enumerate() {
        let mut tick = 0u64;
        let mut open: Vec<Option<(u64, u8)>> = vec![None; 16 * 128]; // (start, velocity)
        let mut info = TrackInfo {
            index: ti as u16,
            name: None,
            program: None,
            channels: Vec::new(),
            note_count: 0,
        };
        for ev in track {
            tick += ev.delta.as_int() as u64;
            match ev.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(n)) if info.name.is_none() => {
                    info.name = Some(String::from_utf8_lossy(n).trim().to_string())
                        .filter(|s| !s.is_empty());
                }
                TrackEventKind::Midi { channel, message } => {
                    let ch = channel.as_int();
                    let slot = |key: u8| ch as usize * 128 + key as usize;
                    match message {
                        MidiMessage::ProgramChange { program } if info.program.is_none() => {
                            info.program = Some(program.as_int());
                        }
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            let k = key.as_int();
                            // A second NoteOn on a sounding key ends the first (common in files).
                            if let Some((start, v)) = open[slot(k)].take() {
                                push_note(
                                    &mut notes, &tempo_map, tpq, ti as u16, ch, k, v, start, tick,
                                );
                            }
                            open[slot(k)] = Some((tick, vel.as_int()));
                        }
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                            let k = key.as_int();
                            if let Some((start, v)) = open[slot(k)].take() {
                                push_note(
                                    &mut notes, &tempo_map, tpq, ti as u16, ch, k, v, start, tick,
                                );
                            }
                        }
                        _ => {}
                    }
                    if !info.channels.contains(&ch) {
                        info.channels.push(ch);
                    }
                }
                _ => {}
            }
        }
        // Notes left open at the end of the track end there.
        for (i, o) in open.iter_mut().enumerate() {
            if let Some((start, v)) = o.take() {
                push_note(
                    &mut notes,
                    &tempo_map,
                    tpq,
                    ti as u16,
                    (i / 128) as u8,
                    (i % 128) as u8,
                    v,
                    start,
                    tick,
                );
            }
        }
        info.note_count = notes.iter().filter(|n| n.track == ti as u16).count();
        tracks.push(info);
    }
    if notes.is_empty() {
        return Err(MidiError::Empty);
    }
    notes.sort_by(|a, b| a.start_tick.cmp(&b.start_tick).then(a.pitch.cmp(&b.pitch)));
    let duration_seconds = notes.iter().map(|n| n.end).fold(0.0, f64::max);
    Ok(MidiClip {
        ticks_per_quarter: tpq,
        tempo_map,
        tempo_from_file,
        time_signatures: sigs,
        tracks,
        notes,
        duration_seconds,
    })
}

#[allow(clippy::too_many_arguments)]
fn push_note(
    notes: &mut Vec<NoteEvent>,
    map: &[TempoPointTicks],
    tpq: u16,
    track: u16,
    channel: u8,
    pitch: u8,
    velocity: u8,
    start_tick: u64,
    end_tick: u64,
) {
    let end_tick = end_tick.max(start_tick + 1);
    notes.push(NoteEvent {
        pitch,
        velocity,
        channel,
        track,
        start_tick,
        duration_ticks: end_tick - start_tick,
        start: tick_to_seconds(map, tpq, start_tick),
        end: tick_to_seconds(map, tpq, end_tick),
    });
}

impl MidiClip {
    /// Convert to the analysis `Note` shape (no bend data) for the piano roll / synth.
    pub fn to_notes(&self) -> Vec<crate::notes::Note> {
        self.notes
            .iter()
            .map(|n| crate::notes::Note {
                start: n.start,
                end: n.end,
                midi: n.pitch as i32,
                cents: 0.0,
                confidence: 1.0,
                velocity: n.velocity as f64 / 127.0,
                bend: Vec::new(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::midi::{write_smf, SmfOptions};
    use crate::notes::Note;

    fn note(start: f64, end: f64, midi: i32) -> Note {
        Note {
            start,
            end,
            midi,
            cents: 0.0,
            confidence: 1.0,
            velocity: 0.5,
            bend: vec![],
        }
    }

    #[test]
    fn roundtrips_our_own_writer() {
        let src = vec![note(0.0, 0.5, 60), note(0.5, 1.0, 62), note(1.2, 2.0, 67)];
        let bytes = write_smf(
            &src,
            &SmfOptions {
                tempo_bpm: 90.0,
                time_signature: Some((7, 8)),
                ..SmfOptions::default()
            },
        );
        let clip = parse_smf(&bytes).unwrap();
        assert_eq!(clip.ticks_per_quarter, 480);
        assert!(clip.tempo_from_file);
        // Tempo is stored as whole microseconds per quarter: 90 BPM → 666 667 µs.
        assert!((clip.tempo_map[0].bpm - 90.0).abs() < 1e-3);
        assert_eq!(clip.time_signatures[0].numerator, 7);
        assert_eq!(clip.time_signatures[0].denominator, 8);
        assert_eq!(clip.notes.len(), 3);
        for (a, b) in clip.notes.iter().zip(&src) {
            assert_eq!(a.pitch as i32, b.midi);
            assert!(
                (a.start - b.start).abs() < 2e-3,
                "{} vs {}",
                a.start,
                b.start
            );
            assert!((a.end - b.end).abs() < 2e-3);
            assert_eq!(a.velocity, 64);
        }
        assert!((clip.duration_seconds - 2.0).abs() < 2e-3);
        assert_eq!(clip.tracks.len(), 1);
        assert_eq!(clip.tracks[0].note_count, 3);
    }

    /// Hand-built format 1 file: tempo track with a tempo change mid-way, two
    /// instrument tracks, running status, NoteOn with velocity 0 as NoteOff.
    #[test]
    fn format1_with_tempo_change_and_running_status() {
        let mut f = Vec::new();
        f.extend_from_slice(b"MThd\0\0\0\x06\0\x01\0\x03\x01\xE0"); // format 1, 3 tracks, 480 tpq
                                                                    // Track 0: tempo 120 at 0, tempo 60 at tick 960 (= 1 s), then EOT.
        let t0: Vec<u8> = vec![
            0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20, // 500000 µs = 120 BPM
            0x00, 0xFF, 0x58, 0x04, 0x05, 0x03, 0x18, 0x08, // 5/8
            0x87, 0x40, 0xFF, 0x51, 0x03, 0x0F, 0x42, 0x40, // +960 ticks: 60 BPM
            0x00, 0xFF, 0x2F, 0x00,
        ];
        // Track 1 (channel 0): name, program 73 (flute), C4 for 480 ticks, then D4
        // from tick 960 for 480 ticks (= 1 s at 60 BPM), using running status.
        let t1: Vec<u8> = vec![
            0x00, 0xFF, 0x03, 0x05, b'F', b'l', b'u', b't', b'e', 0x00, 0xC0, 0x49, 0x00, 0x90,
            0x3C, 0x50, 0x83, 0x60, 0x3C, 0x00, // +480, running status: NoteOn vel 0 = off
            0x83, 0x60, 0x3E, 0x60, // +480 → tick 960
            0x83, 0x60, 0x3E, 0x00, // +480 → tick 1440
            0x00, 0xFF, 0x2F, 0x00,
        ];
        // Track 2 (channel 1): a bass note left open until end of track.
        let t2: Vec<u8> = vec![0x00, 0x91, 0x30, 0x40, 0x83, 0x60, 0xFF, 0x2F, 0x00];
        for t in [&t0, &t1, &t2] {
            f.extend_from_slice(b"MTrk");
            f.extend_from_slice(&(t.len() as u32).to_be_bytes());
            f.extend_from_slice(t);
        }
        let clip = parse_smf(&f).unwrap();
        assert_eq!(clip.tempo_map.len(), 2);
        assert!(
            (clip.tempo_map[1].seconds - 1.0).abs() < 1e-9,
            "tempo change at 1 s"
        );
        assert_eq!(clip.time_signatures[0].numerator, 5);
        assert_eq!(clip.time_signatures[0].denominator, 8);
        assert_eq!(clip.tracks[1].name.as_deref(), Some("Flute"));
        assert_eq!(clip.tracks[1].program, Some(73));
        let c4 = clip.notes.iter().find(|n| n.pitch == 60).unwrap();
        assert!((c4.start, c4.end) == (0.0, 0.5), "{c4:?}");
        let d4 = clip.notes.iter().find(|n| n.pitch == 62).unwrap();
        assert!(
            (d4.start - 1.0).abs() < 1e-9 && (d4.end - 2.0).abs() < 1e-9,
            "{d4:?}"
        );
        let bass = clip.notes.iter().find(|n| n.pitch == 48).unwrap();
        assert_eq!((bass.channel, bass.track), (1, 2));
        assert!((bass.end - 0.5).abs() < 1e-9, "closed at end of track");
        assert!((clip.duration_seconds - 2.0).abs() < 1e-9);
        assert_eq!(clip.to_notes().len(), 3);
    }

    #[test]
    fn garbage_empty_and_smpte_are_errors() {
        assert!(matches!(parse_smf(b"not midi"), Err(MidiError::Parse(_))));
        let empty = write_smf(&[], &SmfOptions::default());
        assert!(matches!(parse_smf(&empty), Err(MidiError::Empty)));
        let mut smpte = Vec::new();
        smpte
            .extend_from_slice(b"MThd\0\0\0\x06\0\x00\0\x01\xE7\x28MTrk\0\0\0\x04\x00\xFF\x2F\x00");
        assert!(matches!(parse_smf(&smpte), Err(MidiError::Smpte)));
    }

    #[test]
    fn tick_conversion_follows_the_tempo_map() {
        let map = vec![
            TempoPointTicks {
                tick: 0,
                bpm: 120.0,
                seconds: 0.0,
            },
            TempoPointTicks {
                tick: 960,
                bpm: 60.0,
                seconds: 1.0,
            },
        ];
        assert_eq!(tick_to_seconds(&map, 480, 480), 0.5);
        assert_eq!(tick_to_seconds(&map, 480, 960), 1.0);
        assert_eq!(tick_to_seconds(&map, 480, 1440), 2.0);
    }
}
