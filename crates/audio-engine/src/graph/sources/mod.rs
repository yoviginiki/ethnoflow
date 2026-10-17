//! Sound sources for the graph. Each is a `Source` with its own `Arc` of smoothed
//! parameters that the UI sets; rendering is allocation-free.

pub mod drone;
pub mod pad;
pub mod reverb;

pub use drone::{root_hz, DroneParams, DroneSource};
pub use pad::{Emotion, PadMacro, PadParams, PadSource, PAD_VOICES};
pub use reverb::Reverb;
