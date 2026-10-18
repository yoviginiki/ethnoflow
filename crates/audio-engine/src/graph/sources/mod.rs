//! Sound sources for the graph. Each is a `Source` with its own `Arc` of smoothed
//! parameters that the UI sets; rendering is allocation-free.

pub mod bass;
pub mod bells;
pub mod chordpad;
pub mod clips;
pub mod drone;
pub mod gen;
pub mod grain;
pub mod lead;
pub mod loop_player;
pub mod noise;
pub mod pad;
pub mod patsynth;
pub mod pluck;
pub mod resonator;
pub mod reverb;
pub mod rhythm;
pub mod shimmer;
pub mod tapan;
pub mod texture;
pub mod wind;

pub use bass::{BassNoteEvent, BassParams, BassPatternData, BassSource, BassVoice};
pub use bells::{BellParams, BellSource, BELL_VOICES};
pub use chordpad::{
    ChordEvent, ChordPadParams, ChordPadSource, ChordPatternData, CHORDPAD_VOICES, CHORD_NOTES,
};
pub use clips::{
    ClipData, ClipSource, ClipsParams, TrackParams, Transport, MAX_TRACKS as MAX_CLIP_TRACKS,
};
pub use drone::{root_hz, DroneParams, DroneSource};
pub use gen::{PitchSet, Rng, Trigger, MAX_PITCHES};
pub use grain::{GrainParams, GrainSource, MAX_GRAINS};
pub use lead::{LeadParams, LeadSource};
pub use loop_player::{LoopParams, LoopSource};
pub use noise::{NoiseParams, NoiseSource};
pub use pad::{Emotion, PadMacro, PadParams, PadSource, PAD_VOICES};
pub use patsynth::{NotePatternData, PatternNote, PatternParams, PatternSynthSource};
pub use pluck::{PluckParams, PluckSource, PLUCK_VOICES};
pub use resonator::{ResonatorParams, ResonatorSource, RESONATOR_VOICES};
pub use reverb::Reverb;
pub use rhythm::{
    pattern_channel, BeatClock, DrumEvent, DrumKind, DrumParams, DrumPatternData, DrumSource,
    PatternInbox, PatternSwap, DRUM_KINDS,
};
pub use shimmer::PitchShifter;
pub use tapan::{TapanParams, TapanSource, TAPAN_STROKES};
pub use texture::{TextureParams, TextureSource};
pub use wind::{WindParams, WindSource};
