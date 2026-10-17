//! Pure music theory for EthnoFlow Studio (no audio I/O, no UI).
//!
//! Rules: no default key, tempo or meter anywhere; every "what key is this" answer is
//! a ranked list with probabilities; non-tempered (custom) scales are first-class.

pub mod events;
pub mod intent;
pub mod key;
pub mod midi;
pub mod notes;
pub mod phrase;
pub mod pitch;
pub mod quantize;
pub mod scale;
pub mod theory;
pub mod tracker;
pub mod trainer;

pub use events::{parse_smf, tick_to_seconds, MidiClip, MidiError, NoteEvent, TrackInfo};
pub use intent::{
    analyze_performance, classify, estimate_intent, AnalysisInput, Annotation, Calibration,
    Classified, ClassifierParams, ClassifyInput, Expected, IntentEstimate, Mode, ModeParams,
    NoteClass, PerformanceAnalysis, Reason,
};
pub use key::{
    explain, rank_hypotheses, Decisive, Evidence, Hypothesis, PcShare, PitchClassHistogram,
};
pub use midi::{write_smf, SmfOptions, TECHNICAL_TEMPO_BPM};
pub use notes::{extract_notes, Note, NoteParams, PitchSample};
pub use phrase::{
    segment_phrases, stable_tones, BoundaryCause, ContourShape, Phrase, PhraseParams, StableTone,
};
pub use pitch::{cents_between, pitch_class_name, Pitch, Reference, PITCH_CLASS_NAMES};
pub use quantize::{quantize, Grid, QuantizeParams};
pub use scale::{DegreeMatch, Scale, ScaleKind};
pub use theory::{detect_chords, interval, transpose, ChordKind, ChordMatch, Interval};
pub use tracker::{ChangeEvidence, Locks, TonalContext, TonalTracker, TrackerParams};
pub use trainer::{evaluate, NoteResult, Report, Summary, Tolerance, Verdict};
