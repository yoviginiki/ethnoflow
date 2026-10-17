//! Sound sources for the graph. Each is a `Source` with its own `Arc` of smoothed
//! parameters that the UI sets; rendering is allocation-free.

pub mod bells;
pub mod clips;
pub mod drone;
pub mod gen;
pub mod loop_player;
pub mod noise;
pub mod pad;
pub mod pluck;
pub mod reverb;
pub mod texture;
pub mod wind;

pub use bells::{BellParams, BellSource, BELL_VOICES};
pub use clips::{
    ClipData, ClipSource, ClipsParams, TrackParams, Transport, MAX_TRACKS as MAX_CLIP_TRACKS,
};
pub use drone::{root_hz, DroneParams, DroneSource};
pub use gen::{PitchSet, Rng, Trigger, MAX_PITCHES};
pub use loop_player::{LoopParams, LoopSource};
pub use noise::{NoiseParams, NoiseSource};
pub use pad::{Emotion, PadMacro, PadParams, PadSource, PAD_VOICES};
pub use pluck::{PluckParams, PluckSource, PLUCK_VOICES};
pub use reverb::Reverb;
pub use texture::{TextureParams, TextureSource};
pub use wind::{WindParams, WindSource};
