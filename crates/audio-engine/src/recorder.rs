//! Microphone recorder that streams to disk.
//!
//! Data path: host audio callback → lock-free ring buffer → drain thread → 32-bit float
//! WAV on disk ([`TakeWriter`], header checkpointed every second) + live waveform peaks.
//! The audio callback never allocates, never locks and never touches the file system.
//! Memory use is independent of take length except for the peaks (~800 B/s).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use cpal::traits::DeviceTrait;
use cpal::{FromSample, SampleFormat, SizedSample};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use serde::{Deserialize, Serialize};

use crate::devices::{find_device, Direction};
use crate::error::{AudioError, Result};
use crate::peaks::{LivePeaks, WaveformPeaks};
use crate::stream_thread::{StreamInfo, StreamThread};
use crate::take_writer::TakeWriter;

/// Ring buffer capacity in samples (interleaved). 2 s at 96 kHz stereo — generous so a
/// briefly stalled drain thread (e.g. a slow disk flush) does not cause overruns.
const RING_CAPACITY_SAMPLES: usize = 96_000 * 2 * 2;
const CHUNK: usize = 1024;
const CLIP_THRESHOLD: f32 = 0.999;

#[derive(Debug, Default)]
struct Shared {
    /// Max |sample| since the last `status`, stored as f32 bits (non-negative floats
    /// order correctly as u32).
    peak_bits: AtomicU32,
    frames_captured: AtomicU64,
    overrun_samples: AtomicU64,
    clipped_samples: AtomicU64,
    stream_errors: AtomicU64,
    drain_stop: AtomicBool,
    /// Set when the WAV size limit was reached; further samples are discarded.
    capped: AtomicBool,
    /// Set when writing to disk failed; further samples are discarded.
    write_failed: AtomicBool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderStatus {
    pub sample_rate: u32,
    pub channels: u16,
    pub frames_captured: u64,
    pub seconds_captured: f64,
    /// Peak level since the previous status query (linear, 0..1).
    pub peak: f32,
    pub overrun_samples: u64,
    pub clipped_samples: u64,
    pub stream_errors: u64,
    /// True once the file size limit was hit; the caller should stop the recorder.
    pub capped: bool,
    pub max_seconds: f64,
    /// True if writing to disk failed; the caller should stop the recorder. Everything
    /// up to the last checkpoint is on disk.
    pub write_failed: bool,
}

/// Result of a finished recording. The audio itself is the file at `path`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordedTake {
    pub path: PathBuf,
    pub sample_rate: u32,
    pub channels: u16,
    pub frames: u64,
    pub duration_seconds: f64,
    pub overrun_samples: u64,
    pub clipped_samples: u64,
    pub stream_errors: u64,
    pub capped: bool,
    /// Present if writing failed mid-take; the file holds the audio up to that point.
    pub write_error: Option<String>,
}

pub struct Recorder {
    stream: StreamThread,
    drain: Option<JoinHandle<DrainResult>>,
    shared: Arc<Shared>,
    peaks: Arc<Mutex<LivePeaks>>,
    info: StreamInfo,
    device_name: String,
    path: PathBuf,
    max_seconds: f64,
}

struct DrainResult {
    frames: u64,
    error: Option<String>,
}

impl Recorder {
    /// Start capturing from `device_name` (or the default input device) into a new WAV
    /// file at `path`. The file must not exist yet.
    pub fn start(device_name: Option<&str>, path: &Path) -> Result<Self> {
        let device = find_device(Direction::Input, device_name)?;
        let resolved_name = device.name().unwrap_or_else(|_| "unknown".into());
        let supported = device.default_input_config()?;
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let info = StreamInfo {
            sample_rate: config.sample_rate.0,
            channels: config.channels,
        };

        let writer = TakeWriter::create(path, info.sample_rate, info.channels)?;
        let max_seconds = writer.max_seconds();
        let shared = Arc::new(Shared::default());
        let peaks = Arc::new(Mutex::new(LivePeaks::new(info.sample_rate, info.channels)));
        let rb = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES);
        let (prod, cons) = rb.split();

        let drain_shared = Arc::clone(&shared);
        let drain_peaks = Arc::clone(&peaks);
        let drain = thread::Builder::new()
            .name("ethnoflow-record-drain".into())
            .spawn(move || drain_loop(cons, writer, drain_shared, drain_peaks))
            .map_err(|e| AudioError::Stream(format!("cannot spawn drain thread: {e}")))?;

        let cb_shared = Arc::clone(&shared);
        let channels = info.channels as u64;
        let started = StreamThread::spawn("ethnoflow-record-stream", move || {
            let err_shared = Arc::clone(&cb_shared);
            let err_fn = move |e: cpal::StreamError| {
                log::error!("input stream error: {e}");
                err_shared.stream_errors.fetch_add(1, Ordering::Relaxed);
            };
            let stream = match sample_format {
                SampleFormat::F32 => {
                    build_input::<f32, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::I16 => {
                    build_input::<i16, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::U16 => {
                    build_input::<u16, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::I32 => {
                    build_input::<i32, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::I8 => {
                    build_input::<i8, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::U8 => {
                    build_input::<u8, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                SampleFormat::F64 => {
                    build_input::<f64, _>(&device, &config, prod, cb_shared, channels, err_fn)?
                }
                other => return Err(AudioError::UnsupportedFormat(format!("{other:?}"))),
            };
            Ok((stream, info))
        });
        let (stream, info) = match started {
            Ok(v) => v,
            Err(e) => {
                // Stream never started: stop the drain thread and remove the empty file.
                shared.drain_stop.store(true, Ordering::Release);
                let _ = drain.join();
                let _ = std::fs::remove_file(path);
                return Err(e);
            }
        };

        Ok(Self {
            stream,
            drain: Some(drain),
            shared,
            peaks,
            info,
            device_name: resolved_name,
            path: path.to_path_buf(),
            max_seconds,
        })
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn sample_rate(&self) -> u32 {
        self.info.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.info.channels
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current status. Resets the peak meter.
    pub fn status(&self) -> RecorderStatus {
        let frames = self.shared.frames_captured.load(Ordering::Relaxed);
        RecorderStatus {
            sample_rate: self.info.sample_rate,
            channels: self.info.channels,
            frames_captured: frames,
            seconds_captured: frames as f64 / self.info.sample_rate as f64,
            peak: f32::from_bits(self.shared.peak_bits.swap(0, Ordering::AcqRel)),
            overrun_samples: self.shared.overrun_samples.load(Ordering::Relaxed),
            clipped_samples: self.shared.clipped_samples.load(Ordering::Relaxed),
            stream_errors: self.shared.stream_errors.load(Ordering::Relaxed),
            capped: self.shared.capped.load(Ordering::Acquire),
            max_seconds: self.max_seconds,
            write_failed: self.shared.write_failed.load(Ordering::Acquire),
        }
    }

    /// Waveform of what has been written so far, reduced to at most `bins` pairs.
    pub fn live_peaks(&self, bins: usize) -> WaveformPeaks {
        self.peaks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .snapshot(bins)
    }

    /// Stop capturing, finalize the file and describe the take.
    pub fn stop(mut self) -> Result<RecordedTake> {
        self.stream.stop();
        self.shared.drain_stop.store(true, Ordering::Release);
        let result = match self.drain.take() {
            Some(j) => j
                .join()
                .map_err(|_| AudioError::Stream("drain thread panicked".into()))?,
            None => DrainResult {
                frames: 0,
                error: None,
            },
        };
        let st = self.status();
        Ok(RecordedTake {
            path: self.path.clone(),
            sample_rate: self.info.sample_rate,
            channels: self.info.channels,
            frames: result.frames,
            duration_seconds: result.frames as f64 / self.info.sample_rate as f64,
            overrun_samples: st.overrun_samples,
            clipped_samples: st.clipped_samples,
            stream_errors: st.stream_errors,
            capped: st.capped,
            write_error: result.error,
        })
    }
}

impl Drop for Recorder {
    fn drop(&mut self) {
        // Dropped without stop() (e.g. app shutdown): still finalize the file.
        self.stream.stop();
        self.shared.drain_stop.store(true, Ordering::Release);
        if let Some(j) = self.drain.take() {
            let _ = j.join();
        }
    }
}

/// Moves samples from the ring buffer to disk and into the live peaks until stopped.
fn drain_loop<C: Consumer<Item = f32>>(
    mut cons: C,
    writer: TakeWriter,
    shared: Arc<Shared>,
    peaks: Arc<Mutex<LivePeaks>>,
) -> DrainResult {
    let mut writer = Some(writer);
    let mut error: Option<String> = None;
    let mut tmp = vec![0.0_f32; CHUNK * 8];
    loop {
        let n = cons.pop_slice(&mut tmp);
        if n == 0 {
            if shared.drain_stop.load(Ordering::Acquire) {
                // The stream is already stopped; one final sweep happened above.
                break;
            }
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        let Some(w) = writer.as_mut() else {
            continue; // write failed earlier: keep draining so the callback never stalls
        };
        match w.write(&tmp[..n]) {
            Ok(accepted) => {
                peaks
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(&tmp[..accepted]);
                if accepted < n {
                    shared.capped.store(true, Ordering::Release);
                }
            }
            Err(e) => {
                log::error!("recording write failed: {e}");
                error = Some(e.to_string());
                shared.write_failed.store(true, Ordering::Release);
                writer = None; // drop finalizes what is there
            }
        }
    }
    let frames = match writer {
        Some(w) => match w.finish() {
            Ok(f) => f,
            Err(e) => {
                error.get_or_insert(e.to_string());
                0
            }
        },
        None => 0,
    };
    let frames = if frames == 0 {
        peaks.lock().unwrap_or_else(|e| e.into_inner()).frames() as u64
    } else {
        frames
    };
    DrainResult { frames, error }
}

/// Body of the input callback: convert, meter, push to the ring buffer.
/// No allocation, no locks, no I/O. `scratch` is preallocated by the caller.
fn capture_block<T, P>(
    data: &[T],
    prod: &mut P,
    scratch: &mut [f32; CHUNK],
    shared: &Shared,
    channels: u64,
) where
    T: SizedSample,
    f32: FromSample<T>,
    P: Producer<Item = f32>,
{
    let mut block_peak = 0.0_f32;
    let mut clipped = 0u64;
    let mut lost = 0u64;
    for chunk in data.chunks(CHUNK) {
        let n = chunk.len();
        for (i, &s) in chunk.iter().enumerate() {
            let v: f32 = s.to_sample();
            let a = v.abs();
            if a > block_peak {
                block_peak = a;
            }
            if a >= CLIP_THRESHOLD {
                clipped += 1;
            }
            scratch[i] = v;
        }
        let pushed = prod.push_slice(&scratch[..n]);
        lost += (n - pushed) as u64;
    }
    shared
        .frames_captured
        .fetch_add(data.len() as u64 / channels.max(1), Ordering::Relaxed);
    shared
        .peak_bits
        .fetch_max(block_peak.to_bits(), Ordering::AcqRel);
    if clipped > 0 {
        shared.clipped_samples.fetch_add(clipped, Ordering::Relaxed);
    }
    if lost > 0 {
        shared.overrun_samples.fetch_add(lost, Ordering::Relaxed);
    }
}

fn build_input<T, E>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    mut prod: ringbuf::HeapProd<f32>,
    shared: Arc<Shared>,
    channels: u64,
    err_fn: E,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
    E: FnMut(cpal::StreamError) + Send + 'static,
{
    let mut scratch = [0.0_f32; CHUNK];
    let stream = device.build_input_stream(
        config,
        move |data: &[T], _: &cpal::InputCallbackInfo| {
            capture_block(data, &mut prod, &mut scratch, &shared, channels);
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wav::read_wav;
    use crate::AudioBuffer;

    #[test]
    fn peak_bits_fetch_max_orders_floats() {
        let a = AtomicU32::new(0);
        a.fetch_max(0.25_f32.to_bits(), Ordering::AcqRel);
        a.fetch_max(0.75_f32.to_bits(), Ordering::AcqRel);
        a.fetch_max(0.5_f32.to_bits(), Ordering::AcqRel);
        assert_eq!(f32::from_bits(a.swap(0, Ordering::AcqRel)), 0.75);
        assert_eq!(f32::from_bits(a.load(Ordering::Relaxed)), 0.0);
    }

    /// Overrun injection: the consumer is stalled, the "callback" keeps delivering.
    /// Every sample that does not fit must be counted, none silently lost.
    #[test]
    fn stalled_drain_counts_every_lost_sample() {
        let shared = Shared::default();
        let rb = HeapRb::<f32>::new(1000);
        let (mut prod, mut cons) = rb.split();
        let mut scratch = [0.0_f32; CHUNK];
        let block: Vec<i16> = (0..480).map(|i| (i * 50) as i16).collect(); // 240 stereo frames
        for _ in 0..5 {
            capture_block(&block, &mut prod, &mut scratch, &shared, 2);
        }
        assert_eq!(shared.frames_captured.load(Ordering::Relaxed), 1200);
        assert_eq!(
            shared.overrun_samples.load(Ordering::Relaxed),
            5 * 480 - 1000
        );
        let mut out = vec![0.0_f32; 2000];
        assert_eq!(cons.pop_slice(&mut out), 1000);
        assert_eq!(out[1], <f32 as FromSample<i16>>::from_sample_(50_i16));
    }

    #[test]
    fn clipping_and_peak_are_metered() {
        let shared = Shared::default();
        let (mut prod, _cons) = HeapRb::<f32>::new(64).split();
        let mut scratch = [0.0_f32; CHUNK];
        capture_block(
            &[0.1_f32, -1.0, 0.5, 1.0],
            &mut prod,
            &mut scratch,
            &shared,
            1,
        );
        assert_eq!(shared.clipped_samples.load(Ordering::Relaxed), 2);
        assert_eq!(
            f32::from_bits(shared.peak_bits.load(Ordering::Relaxed)),
            1.0
        );
    }

    /// The drain thread path end to end with a synthetic source: ring buffer → disk.
    fn run_drain(
        src: &AudioBuffer,
        push_chunk: usize,
        dir: &Path,
    ) -> (DrainResult, PathBuf, usize) {
        let path = dir.join("drain.wav");
        let writer = TakeWriter::create(&path, src.sample_rate, src.channels).unwrap();
        let shared = Arc::new(Shared::default());
        let peaks = Arc::new(Mutex::new(LivePeaks::new(src.sample_rate, src.channels)));
        let (mut prod, cons) = HeapRb::<f32>::new(RING_CAPACITY_SAMPLES).split();
        let (s2, p2) = (Arc::clone(&shared), Arc::clone(&peaks));
        let j = thread::spawn(move || drain_loop(cons, writer, s2, p2));
        for c in src.samples.chunks(push_chunk) {
            let mut off = 0;
            while off < c.len() {
                off += prod.push_slice(&c[off..]);
                if off < c.len() {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
        shared.drain_stop.store(true, Ordering::Release);
        let r = j.join().unwrap();
        let live_frames = peaks.lock().unwrap().frames();
        (r, path, live_frames)
    }

    #[test]
    fn drain_writes_identical_audio_to_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mono = AudioBuffer::sine(48_000, 220.0, 4.0, 0.6);
        let src = AudioBuffer::new(
            48_000,
            2,
            mono.samples.iter().flat_map(|&s| [s, -s]).collect(),
        );
        let (r, path, live_frames) = run_drain(&src, 960, dir.path());
        assert!(r.error.is_none());
        assert_eq!(r.frames, src.frames() as u64);
        assert_eq!(live_frames, src.frames());
        assert_eq!(read_wav(&path).unwrap(), src);
    }

    /// Long-take simulation: 30 minutes of audio through the real drain path.
    /// 8 kHz mono keeps the file at ~58 MB. Run with `cargo test -- --ignored`.
    #[test]
    #[ignore = "long: 30-minute take simulation (~58 MB file)"]
    fn thirty_minute_take_streams_without_loss() {
        let dir = tempfile::tempdir().unwrap();
        let sr = 8_000u32;
        let src = AudioBuffer::sine(sr, 97.0, 30.0 * 60.0, 0.5);
        let (r, path, live_frames) = run_drain(&src, 256, dir.path());
        assert!(r.error.is_none());
        assert_eq!(r.frames, src.frames() as u64);
        assert_eq!(live_frames, src.frames());
        let back = read_wav(&path).unwrap();
        assert_eq!(back.frames(), src.frames());
        assert_eq!(back.samples[..10_000], src.samples[..10_000]);
        assert_eq!(
            back.samples[back.samples.len() - 10_000..],
            src.samples[src.samples.len() - 10_000..]
        );
    }

    /// Real device: ~150 ms to a file (skips with a message when there is no input).
    #[test]
    fn start_stop_writes_a_valid_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("dev.wav");
        match Recorder::start(None, &path) {
            Ok(rec) => {
                std::thread::sleep(Duration::from_millis(150));
                let live = rec.live_peaks(100);
                let take = rec.stop().expect("stop");
                assert!(take.write_error.is_none());
                let back = read_wav(&path).unwrap();
                assert_eq!(back.frames() as u64, take.frames);
                assert_eq!(back.channels, take.channels);
                assert!(live.frames as u64 <= take.frames);
                eprintln!(
                    "recorded {} frames @ {} Hz x{} to disk (overruns {})",
                    take.frames, take.sample_rate, take.channels, take.overrun_samples
                );
            }
            Err(e) => eprintln!("no input device in this environment: {e}"),
        }
    }

    #[test]
    fn recording_never_overwrites_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("exists.wav");
        std::fs::write(&path, b"keep me").unwrap();
        assert!(Recorder::start(None, &path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep me");
    }
}
