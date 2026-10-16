//! Standard MIDI File (format 0) writer for extracted notes, with pitch bend.
//!
//! MIDI needs a tempo to turn seconds into ticks. The caller passes one: the project's
//! tempo if the user set it, otherwise a clearly labelled *technical* tempo (see
//! `TECHNICAL_TEMPO_BPM`). Nothing here assumes a musical tempo, key or meter.

use crate::notes::Note;

/// 60 BPM: one quarter note per second, so ticks are plain time. Used only when the
/// project has no tempo, and the export tells the user so.
pub const TECHNICAL_TEMPO_BPM: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmfOptions {
    pub ticks_per_quarter: u16,
    pub tempo_bpm: f64,
    /// Pitch-bend range declared via RPN 0, in semitones (±).
    pub bend_range_semitones: u8,
    pub channel: u8,
    /// Time signature meta event, if the project has one.
    pub time_signature: Option<(u8, u8)>,
}

impl Default for SmfOptions {
    fn default() -> Self {
        Self {
            ticks_per_quarter: 480,
            tempo_bpm: TECHNICAL_TEMPO_BPM,
            bend_range_semitones: 2,
            channel: 0,
            time_signature: None,
        }
    }
}

fn vlq(mut v: u32, out: &mut Vec<u8>) {
    let mut buf = [0u8; 5];
    let mut i = 4;
    buf[i] = (v & 0x7f) as u8;
    v >>= 7;
    while v > 0 {
        i -= 1;
        buf[i] = (v & 0x7f) as u8 | 0x80;
        v >>= 7;
    }
    out.extend_from_slice(&buf[i..]);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Bend,
    Off,
    On,
}

/// Events sorted by tick; at equal ticks: bend, then note off, then note on, so a
/// repeated pitch re-triggers cleanly and the bend is in place before the attack.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Event {
    tick: u32,
    kind: Kind,
    data: [u8; 3],
}

/// Serialise notes to SMF bytes. Notes may overlap in time; each is independent.
pub fn write_smf(notes: &[Note], opts: &SmfOptions) -> Vec<u8> {
    let tpq = opts.ticks_per_quarter.max(1) as f64;
    let ticks =
        |seconds: f64| -> u32 { (seconds * opts.tempo_bpm / 60.0 * tpq).round().max(0.0) as u32 };
    let ch = opts.channel & 0x0f;
    let range = opts.bend_range_semitones.max(1) as f64;
    let bend_value = |cents: f64| -> u16 {
        let semis = (cents / 100.0).clamp(-range, range);
        (8192.0 + semis / range * 8192.0)
            .round()
            .clamp(0.0, 16383.0) as u16
    };

    let mut events: Vec<Event> = Vec::new();
    for n in notes {
        let on = ticks(n.start);
        let off = ticks(n.end).max(on + 1);
        let vel = (n.velocity.clamp(0.0, 1.0) * 126.0).round() as u8 + 1;
        let key = n.midi.clamp(0, 127) as u8;
        // Bend curve: one event per sample, de-duplicated by value and tick.
        let mut last: Option<(u32, u16)> = None;
        for &(t_rel, cents) in &n.bend {
            let tick = on + ticks(t_rel);
            let v = bend_value(cents);
            if last.is_some_and(|(lt, lv)| lt == tick || lv == v) {
                continue;
            }
            last = Some((tick, v));
            events.push(Event {
                tick,
                kind: Kind::Bend,
                data: [0xE0 | ch, (v & 0x7f) as u8, (v >> 7) as u8],
            });
        }
        if n.bend.is_empty() {
            events.push(Event {
                tick: on,
                kind: Kind::Bend,
                data: [0xE0 | ch, 0x00, 0x40],
            });
        }
        events.push(Event {
            tick: on,
            kind: Kind::On,
            data: [0x90 | ch, key, vel],
        });
        events.push(Event {
            tick: off,
            kind: Kind::Off,
            data: [0x80 | ch, key, 0],
        });
        // Reset the bend after the note so the next one starts neutral.
        events.push(Event {
            tick: off,
            kind: Kind::Bend,
            data: [0xE0 | ch, 0x00, 0x40],
        });
    }
    events.sort();

    let mut track: Vec<u8> = Vec::new();
    // Tempo.
    let usec_per_quarter = (60_000_000.0 / opts.tempo_bpm).round() as u32;
    track.push(0);
    track.extend_from_slice(&[0xFF, 0x51, 0x03]);
    track.extend_from_slice(&usec_per_quarter.to_be_bytes()[1..]);
    if let Some((num, den)) = opts.time_signature {
        let dd = (den as f64).log2().round() as u8;
        track.push(0);
        track.extend_from_slice(&[0xFF, 0x58, 0x04, num, dd, 24, 8]);
    }
    // RPN 0 (pitch bend range): CC101=0, CC100=0, CC6=semitones, CC38=0.
    for (cc, v) in [
        (101u8, 0u8),
        (100, 0),
        (6, opts.bend_range_semitones),
        (38, 0),
    ] {
        track.push(0);
        track.extend_from_slice(&[0xB0 | ch, cc, v]);
    }
    let mut last_tick = 0u32;
    for e in &events {
        vlq(e.tick - last_tick, &mut track);
        track.extend_from_slice(&e.data);
        last_tick = e.tick;
    }
    track.push(0);
    track.extend_from_slice(&[0xFF, 0x2F, 0x00]);

    let mut out = Vec::with_capacity(track.len() + 22);
    out.extend_from_slice(b"MThd");
    out.extend_from_slice(&6u32.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&opts.ticks_per_quarter.to_be_bytes());
    out.extend_from_slice(b"MTrk");
    out.extend_from_slice(&(track.len() as u32).to_be_bytes());
    out.extend_from_slice(&track);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(start: f64, end: f64, midi: i32, bend: Vec<(f64, f64)>) -> Note {
        Note {
            start,
            end,
            midi,
            cents: 0.0,
            confidence: 1.0,
            velocity: 0.5,
            bend,
        }
    }

    /// (tick, status, data1, data2)
    type ChannelEvent = (u32, u8, u8, u8);
    /// (tick, meta kind, payload)
    type MetaEvent = (u32, u8, Vec<u8>);

    /// Minimal SMF reader for the tests: returns (tpq, channel events, meta events).
    fn parse(bytes: &[u8]) -> (u16, Vec<ChannelEvent>, Vec<MetaEvent>) {
        assert_eq!(&bytes[0..4], b"MThd");
        assert_eq!(u32::from_be_bytes(bytes[4..8].try_into().unwrap()), 6);
        assert_eq!(u16::from_be_bytes(bytes[8..10].try_into().unwrap()), 0);
        assert_eq!(u16::from_be_bytes(bytes[10..12].try_into().unwrap()), 1);
        let tpq = u16::from_be_bytes(bytes[12..14].try_into().unwrap());
        assert_eq!(&bytes[14..18], b"MTrk");
        let len = u32::from_be_bytes(bytes[18..22].try_into().unwrap()) as usize;
        let track = &bytes[22..22 + len];
        assert_eq!(bytes.len(), 22 + len);
        let mut i = 0;
        let mut tick = 0u32;
        let mut events = Vec::new();
        let mut metas = Vec::new();
        while i < track.len() {
            let mut delta = 0u32;
            loop {
                let b = track[i];
                i += 1;
                delta = (delta << 7) | (b & 0x7f) as u32;
                if b & 0x80 == 0 {
                    break;
                }
            }
            tick += delta;
            let status = track[i];
            i += 1;
            if status == 0xFF {
                let kind = track[i];
                let l = track[i + 1] as usize;
                metas.push((tick, kind, track[i + 2..i + 2 + l].to_vec()));
                i += 2 + l;
            } else {
                events.push((tick, status, track[i], track[i + 1]));
                i += 2;
            }
        }
        (tpq, events, metas)
    }

    #[test]
    fn vlq_encoding_matches_the_spec() {
        for (v, expect) in [
            (0u32, vec![0x00u8]),
            (0x7f, vec![0x7f]),
            (0x80, vec![0x81, 0x00]),
            (0x2000, vec![0xC0, 0x00]),
            (0x0FFFFFFF, vec![0xFF, 0xFF, 0xFF, 0x7F]),
        ] {
            let mut out = Vec::new();
            vlq(v, &mut out);
            assert_eq!(out, expect, "{v:#x}");
        }
    }

    #[test]
    fn notes_roundtrip_with_exact_ticks_at_the_technical_tempo() {
        let notes = vec![
            note(0.0, 0.5, 60, vec![]),
            note(0.5, 1.0, 62, vec![]),
            note(1.2, 2.0, 67, vec![]),
        ];
        let bytes = write_smf(&notes, &SmfOptions::default());
        let (tpq, ev, metas) = parse(&bytes);
        assert_eq!(tpq, 480);
        // Tempo meta = 1 000 000 µs per quarter (60 BPM).
        let tempo = metas.iter().find(|m| m.1 == 0x51).unwrap();
        assert_eq!(tempo.2, vec![0x0F, 0x42, 0x40]);
        assert_eq!(metas.last().unwrap().1, 0x2F, "end of track");
        let ons: Vec<_> = ev.iter().filter(|e| e.1 == 0x90).collect();
        let offs: Vec<_> = ev.iter().filter(|e| e.1 == 0x80).collect();
        assert_eq!(ons.len(), 3);
        assert_eq!(offs.len(), 3);
        assert_eq!((ons[0].0, ons[0].2), (0, 60));
        assert_eq!((offs[0].0, offs[0].2), (240, 60));
        assert_eq!((ons[1].0, ons[1].2), (240, 62));
        assert_eq!((ons[2].0, ons[2].2), (576, 67));
        assert_eq!(offs[2].0, 960);
        // Velocity 0.5 → 64.
        assert_eq!(ons[0].3, 64);
        // RPN pitch-bend range of 2 semitones is declared.
        let ccs: Vec<_> = ev
            .iter()
            .filter(|e| e.1 == 0xB0)
            .map(|e| (e.2, e.3))
            .collect();
        assert_eq!(ccs, vec![(101, 0), (100, 0), (6, 2), (38, 0)]);
    }

    #[test]
    fn bend_curve_is_written_before_the_attack_and_clamped() {
        let n = note(
            1.0,
            1.5,
            69,
            vec![(0.0, -700.0), (0.1, -100.0), (0.2, 0.0), (0.3, 50.0)],
        );
        let bytes = write_smf(&[n], &SmfOptions::default());
        let (_, ev, _) = parse(&bytes);
        let bends: Vec<(u32, u16)> = ev
            .iter()
            .filter(|e| e.1 == 0xE0)
            .map(|e| (e.0, e.2 as u16 | ((e.3 as u16) << 7)))
            .collect();
        let on = ev.iter().find(|e| e.1 == 0x90).unwrap();
        assert_eq!(on.0, 480);
        // −700 cents clamps to the bottom of a ±2 semitone range.
        assert_eq!(bends[0], (480, 0));
        assert!(bends[0].0 <= on.0);
        // −100 cents = half way down; 0 = centre; +50 = quarter up.
        assert_eq!(bends[1], (528, 8192 - 4096));
        assert_eq!(bends[2], (576, 8192));
        assert_eq!(bends[3], (624, 8192 + 2048));
        // Reset to centre at note off.
        assert_eq!(*bends.last().unwrap(), (720, 8192));
    }

    #[test]
    fn project_tempo_and_meter_are_honoured() {
        let opts = SmfOptions {
            tempo_bpm: 120.0,
            time_signature: Some((7, 8)),
            ..SmfOptions::default()
        };
        let bytes = write_smf(&[note(0.0, 1.0, 60, vec![])], &opts);
        let (_, ev, metas) = parse(&bytes);
        let ts = metas.iter().find(|m| m.1 == 0x58).unwrap();
        assert_eq!(ts.2[..2], [7, 3]);
        // 1 s at 120 BPM = 2 quarters = 960 ticks.
        assert_eq!(ev.iter().find(|e| e.1 == 0x80).unwrap().0, 960);
    }
}
