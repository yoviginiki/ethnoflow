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
pub mod decode;
pub mod devices;
pub mod error;
pub mod metronome;
pub mod monitor;
pub mod peaks;
pub mod pitch;
pub mod player;
pub mod recorder;
pub mod resample;
mod stream_thread;
pub mod synth;
pub mod take_writer;
pub mod wav;

pub use buffer::{linear_to_dbfs, AudioBuffer};
pub use decode::{decode_file, IMPORT_EXTENSIONS};
pub use devices::{
    choose_config, find_device, host_name, list_devices, DeviceInfo, Direction, StreamRequest,
    STANDARD_RATES,
};
pub use error::{AudioError, Result};
pub use metronome::{
    mix_click, ClickConfig, ClickKind, ClickSynth, GridPosition, Metronome, MetronomeStatus,
};
pub use monitor::{InputMonitor, MonitorStatus};
pub use peaks::{compute_peaks, LivePeaks, WaveformPeaks};
pub use pitch::{
    contour, estimate as estimate_pitch, ContourPoint, PitchConfig, PitchEstimate, PitchTracker,
};
pub use player::{adapt_for_device, PlayOptions, Player, PlayerStatus};
pub use recorder::{RecordedTake, Recorder, RecorderStatus};
pub use resample::resample_linear;
pub use synth::{render_notes, SynthConfig, SynthNote, Voice};
pub use take_writer::TakeWriter;
pub use wav::{read_wav, repair_wav_header, wav_info, write_wav, WavFormat, WavInfo};
