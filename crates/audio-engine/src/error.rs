use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioError {
    #[error("no audio host available")]
    NoHost,
    #[error("audio device not found: {0}")]
    DeviceNotFound(String),
    #[error("no default {0} device")]
    NoDefaultDevice(&'static str),
    #[error("unsupported sample format: {0}")]
    UnsupportedFormat(String),
    #[error("device configuration error: {0}")]
    DeviceConfig(String),
    #[error("stream error: {0}")]
    Stream(String),
    #[error("recorder is not running")]
    NotRecording,
    #[error("player is not running")]
    NotPlaying,
    #[error("wav error: {0}")]
    Wav(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<hound::Error> for AudioError {
    fn from(e: hound::Error) -> Self {
        AudioError::Wav(e.to_string())
    }
}

impl From<cpal::DevicesError> for AudioError {
    fn from(e: cpal::DevicesError) -> Self {
        AudioError::DeviceConfig(e.to_string())
    }
}

impl From<cpal::DefaultStreamConfigError> for AudioError {
    fn from(e: cpal::DefaultStreamConfigError) -> Self {
        AudioError::DeviceConfig(e.to_string())
    }
}

impl From<cpal::BuildStreamError> for AudioError {
    fn from(e: cpal::BuildStreamError) -> Self {
        AudioError::Stream(e.to_string())
    }
}

impl From<cpal::PlayStreamError> for AudioError {
    fn from(e: cpal::PlayStreamError) -> Self {
        AudioError::Stream(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AudioError>;
