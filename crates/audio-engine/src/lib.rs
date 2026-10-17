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
pub mod graph;
pub mod live_synth;
pub mod metronome;
pub mod midi_in;
pub mod monitor;
pub mod peaks;
pub mod pitch;
pub mod player;
pub mod process;
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
pub use graph::player::{GraphCmd, GraphPlayer, GraphStatus};
pub use graph::sources::{
    root_hz as drone_root_hz, BellParams, BellSource, ClipData, ClipSource, ClipsParams,
    DroneParams, DroneSource, Emotion, LoopParams, LoopSource, NoiseParams, NoiseSource, PadMacro,
    PadParams, PadSource, PitchSet, PluckParams, PluckSource, TextureParams, TextureSource,
    TrackParams, Transport, WindParams, WindSource, MAX_CLIP_TRACKS, MAX_PITCHES, PAD_VOICES,
};
pub use graph::{
    Graph, GraphMeters, GraphParams, LayerParams, MasterParams, SineSource, Source, MAX_LAYERS,
};
pub use live_synth::{Cmd as SynthCmd, LiveEngine, LiveSynth, LiveSynthStatus};
pub use metronome::{
    mix_click, ClickConfig, ClickKind, ClickSynth, GridPosition, Metronome, MetronomeStatus,
};
pub use midi_in::{
    list_inputs as list_midi_inputs, parse_message as parse_midi_message, MidiEvent, MidiIn,
    MidiPortInfo,
};
pub use monitor::{InputMonitor, MonitorStatus};
pub use peaks::{compute_peaks, LivePeaks, WaveformPeaks};
pub use pitch::{
    contour, estimate as estimate_pitch, held_tone, ContourPoint, HeldTone, PitchConfig,
    PitchEstimate, PitchTracker,
};
pub use player::{adapt_for_device, PlayOptions, Player, PlayerStatus};
pub use process::{
    apply_recipe, detect_silence, loop_crossfade, nearest_zero_crossing, normalize, sound_bounds,
    Recipe,
};
pub use recorder::{RecordedTake, Recorder, RecorderStatus};
pub use resample::resample_linear;
pub use synth::{render_notes, SynthConfig, SynthNote, Voice};
pub use take_writer::TakeWriter;
pub use wav::{read_wav, repair_wav_header, wav_info, write_wav, WavFormat, WavInfo};
