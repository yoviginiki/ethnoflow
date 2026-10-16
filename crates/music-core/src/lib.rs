//! Pure music theory for EthnoFlow Studio (no audio I/O, no UI).
//!
//! Rules: no default key, tempo or meter anywhere; every "what key is this" answer is
//! a ranked list with probabilities; non-tempered (custom) scales are first-class.

pub mod key;
pub mod pitch;
pub mod scale;

pub use key::{rank_hypotheses, Hypothesis, PitchClassHistogram};
pub use pitch::{cents_between, pitch_class_name, Pitch, Reference, PITCH_CLASS_NAMES};
pub use scale::{DegreeMatch, Scale, ScaleKind};
