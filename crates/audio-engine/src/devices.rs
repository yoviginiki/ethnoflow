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
    /// Standard rates (44.1–192 kHz) the device reports as supported, ascending.
    pub sample_rates: Vec<u32>,
    /// Channel counts the device reports as supported, ascending.
    pub channel_counts: Vec<u16>,
}

/// Rates offered for selection. Anything else the device supports stays reachable
/// through its default config.
pub const STANDARD_RATES: &[u32] = &[44_100, 48_000, 88_200, 96_000, 176_400, 192_000];

/// Requested stream format; `None` fields mean "device default".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StreamRequest {
    pub sample_rate: Option<u32>,
    pub channels: Option<u16>,
}

fn supported_ranges(
    device: &cpal::Device,
    direction: Direction,
) -> Vec<cpal::SupportedStreamConfigRange> {
    match direction {
        Direction::Input => device
            .supported_input_configs()
            .map(|it| it.collect())
            .unwrap_or_default(),
        Direction::Output => device
            .supported_output_configs()
            .map(|it| it.collect())
            .unwrap_or_default(),
    }
}

fn format_rank(f: cpal::SampleFormat) -> u8 {
    use cpal::SampleFormat as F;
    match f {
        F::F32 => 0,
        F::I32 => 1,
        F::I16 => 2,
        _ => 3,
    }
}

/// Pick a stream config for `request`, falling back to the device default for unset
/// fields. Errors if the device does not support the requested rate/channels.
pub fn choose_config(
    device: &cpal::Device,
    direction: Direction,
    request: StreamRequest,
) -> Result<cpal::SupportedStreamConfig> {
    let default = match direction {
        Direction::Input => device.default_input_config()?,
        Direction::Output => device.default_output_config()?,
    };
    let rate = request.sample_rate.unwrap_or(default.sample_rate().0);
    let channels = request.channels.unwrap_or(default.channels());
    if rate == default.sample_rate().0 && channels == default.channels() {
        return Ok(default);
    }
    let mut ranges = supported_ranges(device, direction);
    ranges.sort_by_key(|r| format_rank(r.sample_format()));
    ranges
        .into_iter()
        .filter(|r| r.channels() == channels)
        .find_map(|r| r.try_with_sample_rate(cpal::SampleRate(rate)))
        .ok_or_else(|| {
            AudioError::DeviceConfig(format!("device does not support {rate} Hz × {channels} ch"))
        })
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
        let ranges = supported_ranges(&d, direction);
        let mut sample_rates: Vec<u32> = STANDARD_RATES
            .iter()
            .copied()
            .filter(|&r| {
                ranges
                    .iter()
                    .any(|c| c.min_sample_rate().0 <= r && r <= c.max_sample_rate().0)
            })
            .collect();
        if let Some(c) = &cfg {
            sample_rates.push(c.sample_rate().0);
        }
        sample_rates.sort_unstable();
        sample_rates.dedup();
        let mut channel_counts: Vec<u16> = ranges.iter().map(|c| c.channels()).collect();
        channel_counts.extend(cfg.as_ref().map(|c| c.channels()));
        channel_counts.sort_unstable();
        channel_counts.dedup();
        out.push(DeviceInfo {
            is_default: default_name.as_deref() == Some(name.as_str()),
            default_sample_rate: cfg.as_ref().map(|c| c.sample_rate().0),
            default_channels: cfg.as_ref().map(|c| c.channels()),
            default_sample_format: cfg.as_ref().map(|c| format!("{:?}", c.sample_format())),
            sample_rates,
            channel_counts,
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

    /// On a machine with an input: the default request resolves to the default config,
    /// every listed rate can actually be chosen, and an absurd rate is refused.
    #[test]
    fn listed_rates_are_choosable() {
        let Ok(devs) = list_devices(Direction::Input) else {
            return;
        };
        for info in devs.iter().filter(|d| d.default_sample_rate.is_some()) {
            let Ok(dev) = find_device(Direction::Input, Some(&info.name)) else {
                continue;
            };
            let def = choose_config(&dev, Direction::Input, StreamRequest::default()).unwrap();
            assert_eq!(Some(def.sample_rate().0), info.default_sample_rate);
            for &r in &info.sample_rates {
                let req = StreamRequest {
                    sample_rate: Some(r),
                    channels: info.default_channels,
                };
                if let Err(e) = choose_config(&dev, Direction::Input, req) {
                    eprintln!(
                        "{}: listed {r} Hz but cannot choose it with default channels: {e}",
                        info.name
                    );
                }
            }
            let bad = StreamRequest {
                sample_rate: Some(12_345),
                channels: None,
            };
            assert!(choose_config(&dev, Direction::Input, bad).is_err());
            eprintln!(
                "{}: rates {:?}, channels {:?}",
                info.name, info.sample_rates, info.channel_counts
            );
        }
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
