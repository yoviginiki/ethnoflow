//! Sound sources for the graph. Each is a `Source` with its own `Arc` of smoothed
//! parameters that the UI sets; rendering is allocation-free.

pub mod drone;

pub use drone::{DroneParams, DroneSource};
