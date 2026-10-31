//! MusicXML (uncompressed `.musicxml` / `.xml`, score-partwise) → notes in seconds.
//!
//! This is the "give the app a sheet" path: MuseScore, Audiveris (OMR from a PDF or a
//! photo) and most notation programs export MusicXML. We read what a melody lesson
//! needs: pitches, durations, rests, ties, chords, tempo, time signature, transposing
//! instruments, `<backup>`/`<forward>`. Not read (ignored on purpose): dynamics,
//! articulations, slurs, lyrics, tuplet *notation* (durations already carry the
//! tuplet), repeats and endings (the score is read once, in order). Compressed `.mxl`
//! is a ZIP container and is not handled here; unzip it or export uncompressed.
//!
//! Tempo: the first `<sound tempo>` (or `<metronome>` with a beat unit) wins; without
//! one, the caller's default (the project's tempo) is used, and if there is none,
//! 120 quarter notes per minute, which is the MusicXML convention. `tempo_from_file`
//! tells the caller which happened so the UI can say so.

use roxmltree::{Document, Node};

use crate::notes::Note;

#[derive(Debug, Clone, PartialEq)]
pub struct MusicXmlScore {
    /// Sounding notes of all parts, sorted by start; `cents` 0, `confidence` 1.
    pub notes: Vec<Note>,
    /// Quarter notes per minute used for the seconds.
    pub tempo_bpm: f64,
    /// True when the file declared the tempo; false when a default was used.
    pub tempo_from_file: bool,
    /// First time signature found, if any.
    pub time_signature: Option<(u8, u8)>,
    /// `<work-title>` or `<movement-title>`.
    pub title: Option<String>,
    /// Number of `<part>` elements that contained notes.
    pub parts: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MusicXmlError {
    /// Not well-formed XML.
    Xml(String),
    /// Well-formed, but not a `score-partwise` document.
    NotPartwise,
    /// `score-timewise` is valid MusicXML but not supported here.
    Timewise,
    /// No pitched note in any part.
    NoNotes,
}

impl std::fmt::Display for MusicXmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MusicXmlError::Xml(e) => write!(f, "not valid XML: {e}"),
            MusicXmlError::NotPartwise => write!(f, "not a MusicXML score-partwise document"),
            MusicXmlError::Timewise => {
                write!(f, "score-timewise is not supported; export score-partwise")
            }
            MusicXmlError::NoNotes => write!(f, "the score has no pitched notes"),
        }
    }
}

impl std::error::Error for MusicXmlError {}

const DEFAULT_QPM: f64 = 120.0;
const DEFAULT_DIVISIONS: f64 = 1.0;

/// Parse an uncompressed MusicXML document. `default_bpm` is used only when the file
/// has no tempo (pass the project's tempo, or `None`).
pub fn parse_musicxml(
    text: &str,
    default_bpm: Option<f64>,
) -> Result<MusicXmlScore, MusicXmlError> {
    let doc = Document::parse(text).map_err(|e| MusicXmlError::Xml(e.to_string()))?;
    let root = doc.root_element();
    match root.tag_name().name() {
        "score-partwise" => {}
        "score-timewise" => return Err(MusicXmlError::Timewise),
        _ => return Err(MusicXmlError::NotPartwise),
    }

    let title = child(root, "work")
        .and_then(|w| child(w, "work-title"))
        .or_else(|| child(root, "movement-title"))
        .and_then(|n| n.text())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    // First pass: tempo and time signature are global for the seconds mapping. A
    // tempo change inside the piece is honoured by position (see `TempoMap`).
    let mut tempo_map = TempoMap::default();
    let mut time_signature = None;
    for part in root.children().filter(|n| n.has_tag_name("part")) {
        let mut divisions = DEFAULT_DIVISIONS;
        let mut pos = 0.0; // in quarter notes
        for measure in part.children().filter(|n| n.has_tag_name("measure")) {
            let start = pos;
            let mut cur = start;
            let mut max = start;
            for el in measure.children().filter(|n| n.is_element()) {
                match el.tag_name().name() {
                    "attributes" => {
                        if let Some(d) = child(el, "divisions").and_then(num) {
                            if d > 0.0 {
                                divisions = d;
                            }
                        }
                        if time_signature.is_none() {
                            if let Some(t) = child(el, "time") {
                                let b = child(t, "beats").and_then(num);
                                let bt = child(t, "beat-type").and_then(num);
                                if let (Some(b), Some(bt)) = (b, bt) {
                                    if (1.0..=64.0).contains(&b) && (1.0..=64.0).contains(&bt) {
                                        time_signature = Some((b as u8, bt as u8));
                                    }
                                }
                            }
                        }
                    }
                    "direction" => {
                        if let Some(qpm) = direction_tempo(el) {
                            tempo_map.push(cur, qpm);
                        }
                    }
                    "sound" => {
                        if let Some(qpm) = attr_num(el, "tempo") {
                            tempo_map.push(cur, qpm);
                        }
                    }
                    "note" => {
                        let is_chord = child(el, "chord").is_some();
                        let is_grace = child(el, "grace").is_some();
                        let dur = child(el, "duration").and_then(num).unwrap_or(0.0) / divisions;
                        if !is_chord && !is_grace {
                            cur += dur;
                        }
                    }
                    "backup" => {
                        cur -= child(el, "duration").and_then(num).unwrap_or(0.0) / divisions;
                    }
                    "forward" => {
                        cur += child(el, "duration").and_then(num).unwrap_or(0.0) / divisions;
                    }
                    _ => {}
                }
                if cur > max {
                    max = cur;
                }
            }
            pos = max;
        }
        // Only the first part decides the tempo map (parts share it in valid files).
        if !tempo_map.is_empty() {
            break;
        }
    }
    if tempo_map.is_empty() {
        tempo_map.push(0.0, default_bpm.filter(|b| *b > 0.0).unwrap_or(DEFAULT_QPM));
        tempo_map.from_file = false;
    }

    // Second pass: notes.
    let mut notes: Vec<Note> = Vec::new();
    let mut parts_with_notes = 0;
    for part in root.children().filter(|n| n.has_tag_name("part")) {
        let mut divisions = DEFAULT_DIVISIONS;
        let mut transpose = 0i32;
        let mut pos = 0.0;
        let before = notes.len();
        // Open tie per MIDI number: index into `notes` to extend.
        let mut open_ties: Vec<(i32, usize)> = Vec::new();
        for measure in part.children().filter(|n| n.has_tag_name("measure")) {
            let start = pos;
            let mut cur = start;
            let mut max = start;
            let mut last_start = start; // for `<chord/>`
            for el in measure.children().filter(|n| n.is_element()) {
                match el.tag_name().name() {
                    "attributes" => {
                        if let Some(d) = child(el, "divisions").and_then(num) {
                            if d > 0.0 {
                                divisions = d;
                            }
                        }
                        if let Some(t) = child(el, "transpose") {
                            let chrom = child(t, "chromatic").and_then(num).unwrap_or(0.0) as i32;
                            let oct = child(t, "octave-change").and_then(num).unwrap_or(0.0) as i32;
                            transpose = chrom + 12 * oct;
                        }
                    }
                    "note" => {
                        let is_chord = child(el, "chord").is_some();
                        let is_grace = child(el, "grace").is_some();
                        let dur_q = child(el, "duration").and_then(num).unwrap_or(0.0) / divisions;
                        let note_start = if is_chord { last_start } else { cur };
                        if is_grace {
                            continue;
                        }
                        let pitched = child(el, "pitch")
                            .and_then(|p| pitch_midi(p))
                            .map(|m| m + transpose);
                        let tie_stop = el
                            .children()
                            .any(|c| c.has_tag_name("tie") && c.attribute("type") == Some("stop"));
                        let tie_start = el
                            .children()
                            .any(|c| c.has_tag_name("tie") && c.attribute("type") == Some("start"));
                        if let Some(midi) = pitched {
                            let s = tempo_map.seconds(note_start);
                            let e = tempo_map.seconds(note_start + dur_q);
                            let mut extended = false;
                            if tie_stop {
                                if let Some(i) = open_ties.iter().position(|(m, _)| *m == midi) {
                                    let (_, idx) = open_ties.remove(i);
                                    notes[idx].end = notes[idx].end.max(e);
                                    if tie_start {
                                        open_ties.push((midi, idx));
                                    }
                                    extended = true;
                                }
                            }
                            if !extended && dur_q > 0.0 {
                                notes.push(Note {
                                    start: s,
                                    end: e,
                                    midi,
                                    cents: 0.0,
                                    confidence: 1.0,
                                    velocity: 0.8,
                                    bend: Vec::new(),
                                });
                                if tie_start {
                                    open_ties.push((midi, notes.len() - 1));
                                }
                            }
                        }
                        if !is_chord {
                            last_start = cur;
                            cur += dur_q;
                        }
                    }
                    "backup" => {
                        cur -= child(el, "duration").and_then(num).unwrap_or(0.0) / divisions;
                    }
                    "forward" => {
                        cur += child(el, "duration").and_then(num).unwrap_or(0.0) / divisions;
                    }
                    _ => {}
                }
                if cur > max {
                    max = cur;
                }
            }
            pos = max;
        }
        if notes.len() > before {
            parts_with_notes += 1;
        }
    }
    if notes.is_empty() {
        return Err(MusicXmlError::NoNotes);
    }
    notes.sort_by(|a, b| {
        a.start
            .partial_cmp(&b.start)
            .unwrap()
            .then(a.midi.cmp(&b.midi))
    });
    Ok(MusicXmlScore {
        notes,
        tempo_bpm: tempo_map.first_qpm(),
        tempo_from_file: tempo_map.from_file,
        time_signature,
        title,
        parts: parts_with_notes,
    })
}

/// Piecewise-constant tempo in quarter notes per minute, by position in quarters.
#[derive(Debug, Default)]
struct TempoMap {
    /// (position in quarters, qpm), sorted by position.
    changes: Vec<(f64, f64)>,
    from_file: bool,
}

impl TempoMap {
    fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
    fn push(&mut self, pos: f64, qpm: f64) {
        if qpm <= 0.0 || !qpm.is_finite() {
            return;
        }
        if let Some(last) = self.changes.last_mut() {
            if (last.0 - pos).abs() < 1e-9 {
                last.1 = qpm;
                return;
            }
        }
        self.changes.push((pos, qpm));
        self.from_file = true;
    }
    fn first_qpm(&self) -> f64 {
        self.changes.first().map(|c| c.1).unwrap_or(DEFAULT_QPM)
    }
    /// Seconds at a position in quarter notes, walking the tempo changes.
    fn seconds(&self, pos: f64) -> f64 {
        let mut secs = 0.0;
        let mut at = 0.0;
        let mut qpm = self.first_qpm();
        for &(p, q) in &self.changes {
            if p <= 0.0 {
                qpm = q;
                continue;
            }
            if p >= pos {
                break;
            }
            secs += (p - at) * 60.0 / qpm;
            at = p;
            qpm = q;
        }
        secs + (pos - at).max(0.0) * 60.0 / qpm
    }
}

fn child<'a>(n: Node<'a, 'a>, name: &str) -> Option<Node<'a, 'a>> {
    n.children().find(|c| c.has_tag_name(name))
}

fn num(n: Node) -> Option<f64> {
    n.text().and_then(|t| t.trim().parse::<f64>().ok())
}

fn attr_num(n: Node, name: &str) -> Option<f64> {
    n.attribute(name).and_then(|t| t.trim().parse::<f64>().ok())
}

/// `<direction>`: `<sound tempo="…">` or `<metronome><beat-unit>…<per-minute>`.
fn direction_tempo(d: Node) -> Option<f64> {
    if let Some(s) = child(d, "sound").and_then(|s| attr_num(s, "tempo")) {
        return Some(s);
    }
    let dt = child(d, "direction-type")?;
    let m = child(dt, "metronome")?;
    let per_minute = child(m, "per-minute").and_then(num)?;
    let unit = child(m, "beat-unit")
        .and_then(|u| u.text())
        .unwrap_or("quarter")
        .trim();
    let dots = m
        .children()
        .filter(|c| c.has_tag_name("beat-unit-dot"))
        .count() as i32;
    let mut quarters = match unit {
        "whole" => 4.0,
        "half" => 2.0,
        "quarter" => 1.0,
        "eighth" => 0.5,
        "16th" => 0.25,
        "32nd" => 0.125,
        _ => 1.0,
    };
    for _ in 0..dots {
        quarters *= 1.5;
    }
    Some(per_minute * quarters)
}

fn pitch_midi(p: Node) -> Option<i32> {
    let step = child(p, "step")?.text()?.trim().chars().next()?;
    let base = match step.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let alter = child(p, "alter").and_then(num).unwrap_or(0.0).round() as i32;
    let octave = child(p, "octave").and_then(num)? as i32;
    Some((octave + 1) * 12 + base + alter)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(body: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<score-partwise version="4.0"><work><work-title>Тест</work-title></work>
<part-list><score-part id="P1"><part-name>Kaval</part-name></score-part></part-list>
<part id="P1">{body}</part></score-partwise>"#
        )
    }

    const M1: &str = r#"<measure number="1">
  <attributes><divisions>2</divisions><time><beats>7</beats><beat-type>8</beat-type></time></attributes>
  <direction><direction-type><metronome><beat-unit>quarter</beat-unit><per-minute>60</per-minute></metronome></direction-type></direction>
  <note><pitch><step>D</step><octave>4</octave></pitch><duration>2</duration></note>
  <note><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration></note>
  <note><rest/><duration>1</duration></note>
  <note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>2</duration><tie type="start"/></note>
</measure>
<measure number="2">
  <note><pitch><step>F</step><alter>1</alter><octave>4</octave></pitch><duration>2</duration><tie type="stop"/></note>
  <note><pitch><step>G</step><octave>4</octave></pitch><duration>2</duration></note>
  <note><chord/><pitch><step>B</step><octave>4</octave></pitch><duration>2</duration></note>
  <note><grace/><pitch><step>A</step><octave>4</octave></pitch></note>
  <note><pitch><step>A</step><octave>4</octave></pitch><duration>2</duration></note>
</measure>"#;

    #[test]
    fn melody_with_tie_rest_chord_grace_and_metronome() {
        let s = parse_musicxml(&score(M1), None).unwrap();
        assert_eq!(s.title.as_deref(), Some("Тест"));
        assert_eq!(s.tempo_bpm, 60.0);
        assert!(s.tempo_from_file);
        assert_eq!(s.time_signature, Some((7, 8)));
        assert_eq!(s.parts, 1);
        // D4 (1 s), E4 (0.5 s), rest, F#4 tied across the bar (2 s), G4+B4 chord, A4.
        let v: Vec<(i32, f64, f64)> = s.notes.iter().map(|n| (n.midi, n.start, n.end)).collect();
        assert_eq!(v[0], (62, 0.0, 1.0));
        assert_eq!(v[1], (64, 1.0, 1.5));
        assert_eq!(v[2], (66, 2.0, 4.0)); // tie merged
        assert_eq!(v[3], (67, 4.0, 5.0));
        assert_eq!(v[4], (71, 4.0, 5.0)); // chord: same start
        assert_eq!(v[5], (69, 5.0, 6.0)); // grace skipped
        assert_eq!(v.len(), 6);
    }

    #[test]
    fn default_tempo_comes_from_the_caller_then_120() {
        let body = r#"<measure number="1"><attributes><divisions>1</divisions></attributes>
            <note><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration></note></measure>"#;
        let s = parse_musicxml(&score(body), Some(90.0)).unwrap();
        assert_eq!(s.tempo_bpm, 90.0);
        assert!(!s.tempo_from_file);
        assert!((s.notes[0].end - 60.0 / 90.0).abs() < 1e-9);
        let s2 = parse_musicxml(&score(body), None).unwrap();
        assert_eq!(s2.tempo_bpm, 120.0);
        assert!((s2.notes[0].end - 0.5).abs() < 1e-9);
    }

    #[test]
    fn transposing_part_backup_and_tempo_change_are_honoured() {
        let body = r#"<measure number="1">
            <attributes><divisions>1</divisions><transpose><chromatic>-2</chromatic></transpose></attributes>
            <sound tempo="120"/>
            <note><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration></note>
            <backup><duration>2</duration></backup>
            <note><pitch><step>E</step><octave>3</octave></pitch><duration>2</duration></note>
            <sound tempo="60"/>
            <note><pitch><step>D</step><octave>4</octave></pitch><duration>1</duration></note>
          </measure>"#;
        let s = parse_musicxml(&score(body), None).unwrap();
        let v: Vec<(i32, f64, f64)> = s.notes.iter().map(|n| (n.midi, n.start, n.end)).collect();
        // Written C4 sounds Bb3 (58); E3 → D3 (50); both at 0..1 s at 120.
        assert_eq!(v[0], (50, 0.0, 1.0));
        assert_eq!(v[1], (58, 0.0, 1.0));
        // D4 → C4 (60), after the tempo change: one quarter at 60 = 1 s, starting at 1 s.
        assert_eq!(v[2], (60, 1.0, 2.0));
    }

    #[test]
    fn errors_are_named() {
        assert!(matches!(
            parse_musicxml("<nope>", None),
            Err(MusicXmlError::Xml(_))
        ));
        assert_eq!(
            parse_musicxml("<score-timewise/>", None),
            Err(MusicXmlError::Timewise)
        );
        assert_eq!(
            parse_musicxml("<html/>", None),
            Err(MusicXmlError::NotPartwise)
        );
        let body = r#"<measure number="1"><note><rest/><duration>4</duration></note></measure>"#;
        assert_eq!(
            parse_musicxml(&score(body), None),
            Err(MusicXmlError::NoNotes)
        );
    }
}
