//! CLAP instrument host (ADR-0018, spike). Loads a `.clap` library (or uses an entry
//! linked in-process), lists its instruments, instantiates one, activates it at the
//! graph's sample rate, feeds CLAP note events and reads its audio — and wraps all of
//! that as an `audio_engine::Source` (`ClapSource`) that plays a `NotePatternData` on
//! the shared `BeatClock`, so a CLAP instrument can sit in any graph slot like the
//! built-in voices.
//!
//! Realtime rules (CLAP + ours): `process` is called from the graph's render only; the
//! host preallocates its event and channel buffers at activation (sized to the graph
//! block), so the audio path never allocates. Main-thread calls (create, activate,
//! destroy) happen off the audio thread. Threading beyond that (parameters, GUI, state)
//! is out of the spike's scope and listed in the ADR.
//!
//! Crash isolation: a plugin runs in-process; a misbehaving library can take the app
//! down. Out-of-process hosting stays a later option (ADR-0018).

use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::Arc;

use audio_engine::graph::sources::rhythm::{BeatClock, Cursor};
use audio_engine::graph::{Source, BLOCK};
use audio_engine::NotePatternData;
use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::{
    clap_event_header, clap_event_note, clap_input_events, clap_output_events,
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON,
};
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_EXT_AUDIO_PORTS,
};
use clap_sys::ext::note_ports::{clap_plugin_note_ports, CLAP_EXT_NOTE_PORTS};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::plugin::clap_plugin;
use clap_sys::process::{clap_process, CLAP_PROCESS_ERROR};
use clap_sys::version::{clap_version_is_compatible, CLAP_VERSION};
use serde::{Deserialize, Serialize};

pub const MAX_EVENTS_PER_BLOCK: usize = 256;
pub const MAX_CHANNELS: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub features: Vec<String>,
}

impl PluginDescriptor {
    pub fn is_instrument(&self) -> bool {
        self.features.iter().any(|f| f == "instrument")
    }
}

#[derive(Debug)]
pub enum HostError {
    Load(String),
    NoEntry,
    IncompatibleVersion(u32, u32, u32),
    InitFailed,
    NoFactory,
    NoSuchPlugin(String),
    PluginInitFailed,
    ActivateFailed,
    NoAudioOutput,
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostError::Load(e) => write!(f, "cannot load library: {e}"),
            HostError::NoEntry => write!(f, "no clap_entry symbol"),
            HostError::IncompatibleVersion(a, b, c) => {
                write!(f, "incompatible CLAP version {a}.{b}.{c}")
            }
            HostError::InitFailed => write!(f, "entry init failed"),
            HostError::NoFactory => write!(f, "no plugin factory"),
            HostError::NoSuchPlugin(id) => write!(f, "no plugin {id}"),
            HostError::PluginInitFailed => write!(f, "plugin init failed"),
            HostError::ActivateFailed => write!(f, "plugin activate failed"),
            HostError::NoAudioOutput => write!(f, "plugin has no audio output port"),
        }
    }
}

impl std::error::Error for HostError {}

unsafe fn cstr_string(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

// ---------- the host object every plugin gets ----------

unsafe extern "C" fn host_get_extension(
    _host: *const clap_host,
    _id: *const c_char,
) -> *const c_void {
    ptr::null()
}
unsafe extern "C" fn host_request_restart(_host: *const clap_host) {}
unsafe extern "C" fn host_request_process(_host: *const clap_host) {}
unsafe extern "C" fn host_request_callback(_host: *const clap_host) {}

static HOST: clap_host = clap_host {
    clap_version: CLAP_VERSION,
    host_data: ptr::null_mut(),
    name: c"EthnoFlow Studio".as_ptr(),
    vendor: c"EthnoFlow".as_ptr(),
    url: c"https://github.com/yoviginiki/ethnoflow".as_ptr(),
    version: c"0.4.0".as_ptr(),
    get_extension: Some(host_get_extension),
    request_restart: Some(host_request_restart),
    request_process: Some(host_request_process),
    request_callback: Some(host_request_callback),
};

// ---------- library ----------

enum EntrySource {
    Dynamic {
        _lib: libloading::Library,
        path: PathBuf,
    },
    InProcess,
}

/// A loaded CLAP library: its entry (initialised) and factory.
pub struct ClapLibrary {
    entry: *const clap_plugin_entry,
    factory: *const clap_plugin_factory,
    source: EntrySource,
}

unsafe impl Send for ClapLibrary {}
unsafe impl Sync for ClapLibrary {}

impl ClapLibrary {
    /// Load a `.clap` (a dynamic library exporting `clap_entry`).
    pub fn load(path: &Path) -> Result<Self, HostError> {
        let lib = unsafe { libloading::Library::new(path) }
            .map_err(|e| HostError::Load(e.to_string()))?;
        let entry: *const clap_plugin_entry = unsafe {
            let sym: libloading::Symbol<*const clap_plugin_entry> =
                lib.get(b"clap_entry\0").map_err(|_| HostError::NoEntry)?;
            *sym
        };
        let mut me = Self {
            entry,
            factory: ptr::null(),
            source: EntrySource::Dynamic {
                _lib: lib,
                path: path.to_path_buf(),
            },
        };
        me.init()?;
        Ok(me)
    }

    /// Use an entry linked into this process (tests, static instruments).
    pub fn from_entry(entry: &'static clap_plugin_entry) -> Result<Self, HostError> {
        let mut me = Self {
            entry,
            factory: ptr::null(),
            source: EntrySource::InProcess,
        };
        me.init()?;
        Ok(me)
    }

    fn init(&mut self) -> Result<(), HostError> {
        let e = unsafe { &*self.entry };
        if !clap_version_is_compatible(e.clap_version) {
            return Err(HostError::IncompatibleVersion(
                e.clap_version.major,
                e.clap_version.minor,
                e.clap_version.revision,
            ));
        }
        let path = match &self.source {
            EntrySource::Dynamic { path, .. } => {
                CString::new(path.to_string_lossy().as_bytes()).unwrap_or_default()
            }
            EntrySource::InProcess => CString::default(),
        };
        let ok = unsafe { e.init.map(|f| f(path.as_ptr())).unwrap_or(true) };
        if !ok {
            return Err(HostError::InitFailed);
        }
        let factory = unsafe {
            e.get_factory
                .map(|f| f(CLAP_PLUGIN_FACTORY_ID.as_ptr()))
                .unwrap_or(ptr::null())
        };
        if factory.is_null() {
            return Err(HostError::NoFactory);
        }
        self.factory = factory as *const clap_plugin_factory;
        Ok(())
    }

    pub fn descriptors(&self) -> Vec<PluginDescriptor> {
        let f = unsafe { &*self.factory };
        let n = unsafe { f.get_plugin_count.map(|g| g(self.factory)).unwrap_or(0) };
        let mut out = Vec::new();
        for i in 0..n {
            let d = unsafe {
                f.get_plugin_descriptor
                    .map(|g| g(self.factory, i))
                    .unwrap_or(ptr::null())
            };
            if d.is_null() {
                continue;
            }
            let d = unsafe { &*d };
            let mut features = Vec::new();
            if !d.features.is_null() {
                let mut p = d.features;
                unsafe {
                    while !(*p).is_null() {
                        features.push(cstr_string(*p));
                        p = p.add(1);
                    }
                }
            }
            out.push(PluginDescriptor {
                id: unsafe { cstr_string(d.id) },
                name: unsafe { cstr_string(d.name) },
                vendor: unsafe { cstr_string(d.vendor) },
                version: unsafe { cstr_string(d.version) },
                features,
            });
        }
        out
    }

    /// Instantiate a plugin by id (main thread).
    pub fn instantiate(self: &Arc<Self>, id: &str) -> Result<ClapInstrument, HostError> {
        let f = unsafe { &*self.factory };
        let cid = CString::new(id).map_err(|_| HostError::NoSuchPlugin(id.into()))?;
        let plugin = unsafe {
            f.create_plugin
                .map(|c| c(self.factory, &HOST, cid.as_ptr()))
                .unwrap_or(ptr::null())
        };
        if plugin.is_null() {
            return Err(HostError::NoSuchPlugin(id.into()));
        }
        let ok = unsafe { (*plugin).init.map(|i| i(plugin)).unwrap_or(false) };
        if !ok {
            unsafe {
                if let Some(d) = (*plugin).destroy {
                    d(plugin);
                }
            }
            return Err(HostError::PluginInitFailed);
        }
        // Output port 0 channel count (falls back to stereo when the extension is absent).
        let mut channels = 2usize;
        unsafe {
            let ext = (*plugin)
                .get_extension
                .map(|g| g(plugin, CLAP_EXT_AUDIO_PORTS.as_ptr()))
                .unwrap_or(ptr::null());
            if !ext.is_null() {
                let ap = &*(ext as *const clap_plugin_audio_ports);
                let n = ap.count.map(|c| c(plugin, false)).unwrap_or(0);
                if n == 0 {
                    if let Some(d) = (*plugin).destroy {
                        d(plugin);
                    }
                    return Err(HostError::NoAudioOutput);
                }
                let mut info: clap_audio_port_info = std::mem::zeroed();
                if ap
                    .get
                    .map(|g| g(plugin, 0, false, &mut info))
                    .unwrap_or(false)
                {
                    channels = (info.channel_count as usize).clamp(1, MAX_CHANNELS);
                }
            }
        }
        let has_note_port = unsafe {
            let ext = (*plugin)
                .get_extension
                .map(|g| g(plugin, CLAP_EXT_NOTE_PORTS.as_ptr()))
                .unwrap_or(ptr::null());
            if ext.is_null() {
                false
            } else {
                let np = &*(ext as *const clap_plugin_note_ports);
                np.count.map(|c| c(plugin, true)).unwrap_or(0) > 0
            }
        };
        Ok(ClapInstrument {
            _lib: Arc::clone(self),
            plugin,
            channels,
            has_note_port,
            active: false,
            processing: false,
            sample_rate: 0,
            max_frames: 0,
            chan_bufs: Vec::new(),
            chan_ptrs: Vec::new(),
            events: Vec::new(),
            steady: 0,
        })
    }
}

impl Drop for ClapLibrary {
    fn drop(&mut self) {
        unsafe {
            if let Some(d) = (*self.entry).deinit {
                d();
            }
        }
    }
}

// ---------- instrument ----------

/// A note the host sends to the plugin at a frame offset inside the block.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoteEvent {
    pub frame: u32,
    pub key: i16,
    pub velocity: f64,
    pub on: bool,
}

pub struct ClapInstrument {
    _lib: Arc<ClapLibrary>,
    plugin: *const clap_plugin,
    channels: usize,
    has_note_port: bool,
    active: bool,
    processing: bool,
    sample_rate: u32,
    max_frames: usize,
    chan_bufs: Vec<Vec<f32>>,
    chan_ptrs: Vec<*mut f32>,
    events: Vec<clap_event_note>,
    steady: i64,
}

unsafe impl Send for ClapInstrument {}

unsafe extern "C" fn events_size(list: *const clap_input_events) -> u32 {
    let v = &*((*list).ctx as *const Vec<clap_event_note>);
    v.len() as u32
}

unsafe extern "C" fn events_get(
    list: *const clap_input_events,
    index: u32,
) -> *const clap_event_header {
    let v = &*((*list).ctx as *const Vec<clap_event_note>);
    match v.get(index as usize) {
        Some(e) => &e.header,
        None => ptr::null(),
    }
}

unsafe extern "C" fn out_events_try_push(
    _list: *const clap_output_events,
    _event: *const clap_event_header,
) -> bool {
    false
}

impl ClapInstrument {
    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn has_note_port(&self) -> bool {
        self.has_note_port
    }

    /// Activate at a sample rate with a maximum block size; allocates the host buffers
    /// (the only allocation on this object's audio path, done here, off the audio thread).
    pub fn activate(&mut self, sample_rate: u32, max_frames: usize) -> Result<(), HostError> {
        if self.active {
            self.deactivate();
        }
        let ok = unsafe {
            (*self.plugin)
                .activate
                .map(|a| a(self.plugin, sample_rate as f64, 1, max_frames as u32))
                .unwrap_or(false)
        };
        if !ok {
            return Err(HostError::ActivateFailed);
        }
        self.sample_rate = sample_rate;
        self.max_frames = max_frames;
        self.chan_bufs = (0..self.channels)
            .map(|_| vec![0.0f32; max_frames])
            .collect();
        self.chan_ptrs = self.chan_bufs.iter_mut().map(|b| b.as_mut_ptr()).collect();
        self.events = Vec::with_capacity(MAX_EVENTS_PER_BLOCK);
        self.active = true;
        let started = unsafe {
            (*self.plugin)
                .start_processing
                .map(|s| s(self.plugin))
                .unwrap_or(true)
        };
        self.processing = started;
        Ok(())
    }

    pub fn deactivate(&mut self) {
        unsafe {
            if self.processing {
                if let Some(s) = (*self.plugin).stop_processing {
                    s(self.plugin);
                }
                self.processing = false;
            }
            if self.active {
                if let Some(d) = (*self.plugin).deactivate {
                    d(self.plugin);
                }
                self.active = false;
            }
        }
    }

    /// Process one block: `events` land at their frame offsets, `out` receives the
    /// plugin's first output port as interleaved `channels()` samples (frames × channels).
    /// No allocation when `frames ≤ max_frames` and `events.len() ≤ MAX_EVENTS_PER_BLOCK`.
    pub fn process(&mut self, frames: usize, events: &[NoteEvent], out: &mut [f32]) -> bool {
        if !self.active || !self.processing || frames == 0 || frames > self.max_frames {
            out.fill(0.0);
            return false;
        }
        self.events.clear();
        for e in events.iter().take(self.events.capacity()) {
            self.events.push(clap_event_note {
                header: clap_event_header {
                    size: std::mem::size_of::<clap_event_note>() as u32,
                    time: e.frame.min(frames as u32 - 1),
                    space_id: CLAP_CORE_EVENT_SPACE_ID,
                    type_: if e.on {
                        CLAP_EVENT_NOTE_ON
                    } else {
                        CLAP_EVENT_NOTE_OFF
                    },
                    flags: 0,
                },
                note_id: -1,
                port_index: 0,
                channel: 0,
                key: e.key,
                velocity: e.velocity.clamp(0.0, 1.0),
            });
        }
        for b in self.chan_bufs.iter_mut() {
            b[..frames].fill(0.0);
        }
        let in_events = clap_input_events {
            ctx: &self.events as *const Vec<clap_event_note> as *mut c_void,
            size: Some(events_size),
            get: Some(events_get),
        };
        let out_events = clap_output_events {
            ctx: ptr::null_mut(),
            try_push: Some(out_events_try_push),
        };
        let mut audio_out = clap_audio_buffer {
            data32: self.chan_ptrs.as_mut_ptr(),
            data64: ptr::null_mut(),
            channel_count: self.channels as u32,
            latency: 0,
            constant_mask: 0,
        };
        let p = clap_process {
            steady_time: self.steady,
            frames_count: frames as u32,
            transport: ptr::null(),
            audio_inputs: ptr::null(),
            audio_outputs: &mut audio_out,
            audio_inputs_count: 0,
            audio_outputs_count: 1,
            in_events: &in_events,
            out_events: &out_events,
        };
        let status = unsafe {
            (*self.plugin)
                .process
                .map(|f| f(self.plugin, &p))
                .unwrap_or(CLAP_PROCESS_ERROR)
        };
        self.steady += frames as i64;
        if status == CLAP_PROCESS_ERROR {
            out.fill(0.0);
            return false;
        }
        let ch = self.channels;
        for (i, frame) in out.chunks_mut(ch).take(frames).enumerate() {
            for (c, s) in frame.iter_mut().enumerate() {
                *s = self.chan_bufs[c][i];
            }
        }
        true
    }
}

impl Drop for ClapInstrument {
    fn drop(&mut self) {
        self.deactivate();
        unsafe {
            if let Some(d) = (*self.plugin).destroy {
                d(self.plugin);
            }
        }
    }
}

// ---------- graph source ----------

/// A CLAP instrument playing a note pattern on the shared beat clock, as a graph source
/// (mono mix of the plugin's output; note keys from the pattern's frequencies, cents
/// dropped — note expressions are a later step).
pub struct ClapSource {
    instrument: ClapInstrument,
    clock: Arc<BeatClock>,
    pattern: Arc<NotePatternData>,
    cursor: Cursor,
    /// Notes currently held: (key, release beat).
    held: Vec<(i16, f64)>,
    events: Vec<NoteEvent>,
    block: Vec<f32>,
    frame: u64,
}

impl ClapSource {
    /// Activates the instrument for the graph block size.
    pub fn new(
        mut instrument: ClapInstrument,
        clock: Arc<BeatClock>,
        pattern: Arc<NotePatternData>,
        sample_rate: u32,
    ) -> Result<Self, HostError> {
        instrument.activate(sample_rate, BLOCK)?;
        let ch = instrument.channels();
        Ok(Self {
            instrument,
            clock,
            pattern,
            cursor: Cursor::new(),
            held: Vec::with_capacity(64),
            events: Vec::with_capacity(MAX_EVENTS_PER_BLOCK),
            block: vec![0.0; BLOCK * ch],
            frame: 0,
        })
    }

    fn key_of(hz: f32) -> i16 {
        (69.0 + 12.0 * (hz / 440.0).log2())
            .round()
            .clamp(0.0, 127.0) as i16
    }
}

impl Source for ClapSource {
    fn render(&mut self, out: &mut [f32], sample_rate: u32) {
        let f = self.frame;
        self.render_at(out, sample_rate, f);
        self.frame += out.len() as u64;
    }

    fn render_at(&mut self, out: &mut [f32], sample_rate: u32, frame: u64) {
        let beat0 = self.clock.beat_at(frame, sample_rate);
        let bpf = self.clock.beats_per_frame(sample_rate);
        let generation = self
            .clock
            .generation
            .load(std::sync::atomic::Ordering::Relaxed);
        if let Some(b) = beat0 {
            if self.cursor.needs_seek(generation) {
                self.cursor
                    .seek(&self.pattern.beats, self.pattern.length_beats, b);
                self.cursor.mark(generation);
            }
        }
        let ch = self.instrument.channels();
        let mut done = 0;
        while done < out.len() {
            let n = (out.len() - done).min(BLOCK);
            self.events.clear();
            for i in 0..n {
                let Some(b0) = beat0 else { break };
                let beat = b0 + (done + i) as f64 * bpf;
                // Releases due at this frame.
                let mut k = 0;
                while k < self.held.len() {
                    if beat >= self.held[k].1 {
                        let (key, _) = self.held.swap_remove(k);
                        if self.events.len() < self.events.capacity() {
                            self.events.push(NoteEvent {
                                frame: i as u32,
                                key,
                                velocity: 0.0,
                                on: false,
                            });
                        }
                    } else {
                        k += 1;
                    }
                }
                while let Some(idx) =
                    self.cursor
                        .due(&self.pattern.beats, self.pattern.length_beats, beat)
                {
                    let note = self.pattern.events[idx];
                    let key = Self::key_of(note.hz);
                    let loops = (beat / self.pattern.length_beats).floor();
                    let until = note.beat + loops * self.pattern.length_beats + note.length_beats;
                    if self.events.len() < self.events.capacity() {
                        self.events.push(NoteEvent {
                            frame: i as u32,
                            key,
                            velocity: note.velocity as f64,
                            on: true,
                        });
                    }
                    if self.held.len() < self.held.capacity() {
                        self.held.push((key, until));
                    }
                }
            }
            if beat0.is_none() && !self.held.is_empty() {
                for (key, _) in self.held.drain(..) {
                    if self.events.len() < self.events.capacity() {
                        self.events.push(NoteEvent {
                            frame: 0,
                            key,
                            velocity: 0.0,
                            on: false,
                        });
                    }
                }
            }
            let block = &mut self.block[..n * ch];
            self.instrument.process(n, &self.events, block);
            for (i, s) in out[done..done + n].iter_mut().enumerate() {
                let mut acc = 0.0f32;
                for c in 0..ch {
                    acc += block[i * ch + c];
                }
                *s = acc / ch as f32;
            }
            done += n;
        }
    }

    fn reset(&mut self) {
        self.cursor = Cursor::new();
        self.held.clear();
        self.frame = 0;
    }
}

/// `.clap` files under a folder (one level of bundles on macOS is not handled: Windows
/// and Linux ship flat libraries).
pub fn scan_folder(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension()
                .map(|x| x.eq_ignore_ascii_case("clap"))
                .unwrap_or(false)
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use audio_engine::graph::{Graph, GraphParams};
    use audio_engine::PatternNote;

    fn goertzel(x: &[f32], hz: f32, sr: f32) -> f32 {
        let w = std::f32::consts::TAU * hz / sr;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &v in x {
            let s0 = v + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        (s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2).sqrt() / x.len() as f32
    }

    #[test]
    fn fixture_entry_lists_an_instrument_and_renders_a_note_in_tune() {
        let lib = Arc::new(ClapLibrary::from_entry(clap_fixture::entry()).unwrap());
        let descs = lib.descriptors();
        assert_eq!(descs.len(), 1);
        assert!(descs[0].is_instrument(), "{:?}", descs[0]);
        assert_eq!(descs[0].id, "eu.ensodo.ethnoflow.fixture-sine");
        let mut inst = lib.instantiate(&descs[0].id).unwrap();
        assert_eq!(inst.channels(), 2);
        assert!(inst.has_note_port());
        inst.activate(48_000, 512).unwrap();
        let mut out = vec![0.0f32; 512 * 2];
        // Note on at frame 0 of the first block; render 20 blocks (~213 ms).
        let mut mono = Vec::new();
        for b in 0..20 {
            let ev = if b == 0 {
                vec![NoteEvent {
                    frame: 0,
                    key: 69,
                    velocity: 1.0,
                    on: true,
                }]
            } else {
                Vec::new()
            };
            assert!(inst.process(512, &ev, &mut out));
            mono.extend(out.chunks(2).map(|f| (f[0] + f[1]) * 0.5));
        }
        let peak = mono.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(peak > 0.1 && peak <= 1.0, "audible {peak}");
        let on = goertzel(&mono[2048..], 440.0, 48_000.0);
        let off = goertzel(&mono[2048..], 466.16, 48_000.0);
        assert!(on > off * 5.0, "A4 present: {on} vs {off}");
        // Note off → silence after the release.
        let ev = vec![NoteEvent {
            frame: 0,
            key: 69,
            velocity: 0.0,
            on: false,
        }];
        inst.process(512, &ev, &mut out);
        let mut tail = 0.0f32;
        for _ in 0..40 {
            inst.process(512, &[], &mut out);
            tail = out.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        }
        assert!(tail < 1e-3, "released {tail}");
        let unknown = lib.instantiate("no.such.plugin");
        assert!(matches!(unknown, Err(HostError::NoSuchPlugin(_))));
    }

    #[test]
    fn clap_source_plays_a_pattern_on_the_graph_clock() {
        let lib = Arc::new(ClapLibrary::from_entry(clap_fixture::entry()).unwrap());
        let inst = lib.instantiate("eu.ensodo.ethnoflow.fixture-sine").unwrap();
        let clock = Arc::new(BeatClock::new(120.0)); // beat = 0.5 s
        clock.start(0, 0.0);
        let pat = Arc::new(NotePatternData::new(
            vec![
                PatternNote {
                    beat: 0.0,
                    length_beats: 0.8,
                    hz: 440.0,
                    velocity: 0.9,
                },
                PatternNote {
                    beat: 1.0,
                    length_beats: 0.8,
                    hz: 659.26,
                    velocity: 0.9,
                },
            ],
            2.0,
        ));
        let src = ClapSource::new(inst, Arc::clone(&clock), pat, 48_000).unwrap();
        let gp = Arc::new(GraphParams::default());
        gp.master.gain_db.set(0.0);
        gp.layers[0].gain_db.set(0.0);
        let mut g = Graph::new(gp, 48_000);
        g.set_source(0, Box::new(src));
        let b = g.render_offline(1.0, 1);
        let x = &b.samples;
        assert!(x.iter().all(|v| v.is_finite()));
        let first = &x[4800..19_200]; // 0.1–0.4 s: A4
        let second = &x[28_800..43_200]; // 0.6–0.9 s: E5
        assert!(goertzel(first, 440.0, 48_000.0) > goertzel(first, 659.26, 48_000.0) * 4.0);
        assert!(goertzel(second, 659.26, 48_000.0) > goertzel(second, 440.0, 48_000.0) * 4.0);
    }

    #[test]
    fn fixture_loads_as_a_dynamic_library_when_built() {
        // The cdylib lives next to the test binary's profile dir; build it on demand with
        // the same target directory so the test is honest about what it checked.
        let exe = std::env::current_exe().unwrap();
        let profile_dir = exe.parent().unwrap().parent().unwrap().to_path_buf();
        let name = if cfg!(target_os = "windows") {
            "clap_fixture.dll"
        } else if cfg!(target_os = "macos") {
            "libclap_fixture.dylib"
        } else {
            "libclap_fixture.so"
        };
        let dll = profile_dir.join(name);
        if !dll.exists() {
            let target = profile_dir.parent().unwrap();
            let status = std::process::Command::new(env!("CARGO"))
                .args(["build", "-p", "ethnoflow-clap-fixture"])
                .env("CARGO_TARGET_DIR", target)
                .status()
                .expect("cargo build of the fixture");
            assert!(status.success(), "fixture build failed");
        }
        assert!(dll.exists(), "fixture library at {}", dll.display());
        // CLAP libraries carry the .clap extension; a copy with it proves the loader does
        // not care about the name.
        let clap_path = profile_dir.join("ethnoflow-fixture.clap");
        std::fs::copy(&dll, &clap_path).unwrap();
        assert_eq!(scan_folder(&profile_dir), vec![clap_path.clone()]);
        let lib = Arc::new(ClapLibrary::load(&clap_path).unwrap());
        let d = lib.descriptors();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].name, "EthnoFlow Fixture Sine");
        let mut inst = lib.instantiate(&d[0].id).unwrap();
        inst.activate(44_100, 256).unwrap();
        let mut out = vec![0.0f32; 256 * 2];
        assert!(inst.process(
            256,
            &[NoteEvent {
                frame: 10,
                key: 60,
                velocity: 0.8,
                on: true
            }],
            &mut out
        ));
        assert!(out.iter().any(|v| v.abs() > 0.01));
        assert!(ClapLibrary::load(&profile_dir.join("missing.clap")).is_err());
    }
}
