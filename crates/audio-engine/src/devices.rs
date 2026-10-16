//! Audio device enumeration. Devices are addressed by their host-reported name,
//! which is stable enough for Phase 0 and what WASAPI/CoreAudio/ALSA expose consistently.

use cpal::traits::{DeviceTrait, HostTrait};
use serde::{Deserialize, Serialize};

use crate::error::{AudioError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub default_sample_rate: Option<u32>,
    pub default_channels: Option<u16>,
    pub default_sample_format: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Input,
    Output,
}

pub fn host_name() -> String {
    cpal::default_host().id().name().to_string()
}

/// List devices of the given direction with their default configuration.
/// Devices whose default config cannot be queried are still listed (with `None` fields).
pub fn list_devices(direction: Direction) -> Result<Vec<DeviceInfo>> {
    let host = cpal::default_host();
    let default_name = match direction {
        Direction::Input => host.default_input_device(),
        Direction::Output => host.default_output_device(),
    }
    .and_then(|d| d.name().ok());

    let devices: Vec<cpal::Device> = match direction {
        Direction::Input => host.input_devices()?.collect(),
        Direction::Output => host.output_devices()?.collect(),
    };

    let mut out = Vec::with_capacity(devices.len());
    for d in devices {
        let name = match d.name() {
            Ok(n) => n,
            Err(_) => continue,
        };
        let cfg = match direction {
            Direction::Input => d.default_input_config().ok(),
            Direction::Output => d.default_output_config().ok(),
        };
        out.push(DeviceInfo {
            is_default: default_name.as_deref() == Some(name.as_str()),
            default_sample_rate: cfg.as_ref().map(|c| c.sample_rate().0),
            default_channels: cfg.as_ref().map(|c| c.channels()),
            default_sample_format: cfg.as_ref().map(|c| format!("{:?}", c.sample_format())),
            name,
        });
    }
    Ok(out)
}

/// Resolve a device by name, or the host default when `name` is `None`.
pub fn find_device(direction: Direction, name: Option<&str>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    match name {
        None | Some("") => match direction {
            Direction::Input => host
                .default_input_device()
                .ok_or(AudioError::NoDefaultDevice("input")),
            Direction::Output => host
                .default_output_device()
                .ok_or(AudioError::NoDefaultDevice("output")),
        },
        Some(wanted) => {
            let mut iter: Box<dyn Iterator<Item = cpal::Device>> = match direction {
                Direction::Input => Box::new(host.input_devices()?),
                Direction::Output => Box::new(host.output_devices()?),
            };
            iter.find(|d| d.name().map(|n| n == wanted).unwrap_or(false))
                .ok_or_else(|| AudioError::DeviceNotFound(wanted.to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Enumeration must never panic, even on machines with no audio hardware (CI).
    #[test]
    fn enumeration_does_not_panic() {
        let _ = host_name();
        let ins = list_devices(Direction::Input);
        let outs = list_devices(Direction::Output);
        // Either Ok (possibly empty) or a well-formed error.
        assert!(ins.is_ok() || ins.is_err());
        assert!(outs.is_ok() || outs.is_err());
    }

    #[test]
    fn unknown_device_name_is_an_error() {
        let r = find_device(
            Direction::Input,
            Some("definitely-not-a-real-device-ethnoflow"),
        );
        assert!(r.is_err());
    }
}
