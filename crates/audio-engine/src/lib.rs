//! EthnoFlow Studio audio engine.
//!
//! Scope: device enumeration, microphone recording streamed to disk (crash-safe),
//! buffer playback, WAV import/export and waveform peak reduction. Transport, mixing,
//! metronome and DSP modules arrive in later phases (see docs/ROADMAP.md).
//!
//! Design rules (docs/ARCHITECTURE.md):
//! * audio callbacks never allocate, lock or do I/O;
//! * raw frames never cross into the UI, only reduced data (peaks, status);
//! * the original recording is immutable once captured; edits produce new buffers.

pub mod buffer;
pub mod devices;
pub mod error;
pub mod metronome;
pub mod peaks;
pub mod player;
pub mod recorder;
pub mod resample;
mod stream_thread;
pub mod take_writer;
pub mod wav;

pub use buffer::{linear_to_dbfs, AudioBuffer};
pub use devices::{find_device, host_name, list_devices, DeviceInfo, Direction};
pub use error::{AudioError, Result};
pub use metronome::{
    mix_click, ClickConfig, ClickKind, ClickSynth, GridPosition, Metronome, MetronomeStatus,
};
pub use peaks::{compute_peaks, LivePeaks, WaveformPeaks};
pub use player::{adapt_for_device, PlayOptions, Player, PlayerStatus};
pub use recorder::{RecordedTake, Recorder, RecorderStatus};
pub use resample::resample_linear;
pub use take_writer::TakeWriter;
pub use wav::{read_wav, repair_wav_header, wav_info, write_wav, WavFormat, WavInfo};
