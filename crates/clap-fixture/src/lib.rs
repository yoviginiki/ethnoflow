//! A minimal CLAP instrument written straight against `clap-sys`: eight sine voices with
//! a short release, one stereo output port, one CLAP-dialect note input port. It exists
//! to prove EthnoFlow's host end to end (ADR-0018 spike) — the same `clap_entry` that a
//! C++ plugin built with the official helpers exports. Built as a cdylib (load it as a
//! `.clap`) and as an rlib (`entry()` for in-process tests).
//!
//! Realtime rules: `process` never allocates or locks; the voice table is fixed.

#![allow(clippy::missing_safety_doc)]

use std::ffi::{c_char, c_void, CStr};
use std::ptr;

use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::{
    clap_event_note, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_NOTE_CHOKE, CLAP_EVENT_NOTE_OFF,
    CLAP_EVENT_NOTE_ON,
};
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_AUDIO_PORT_IS_MAIN, CLAP_EXT_AUDIO_PORTS,
    CLAP_PORT_STEREO,
};
use clap_sys::ext::note_ports::{
    clap_note_port_info, clap_plugin_note_ports, CLAP_EXT_NOTE_PORTS, CLAP_NOTE_DIALECT_CLAP,
};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::plugin_features::{CLAP_PLUGIN_FEATURE_INSTRUMENT, CLAP_PLUGIN_FEATURE_SYNTHESIZER};
use clap_sys::process::{
    clap_process, clap_process_status, CLAP_PROCESS_CONTINUE, CLAP_PROCESS_ERROR,
};
use clap_sys::version::CLAP_VERSION;

pub const PLUGIN_ID: &CStr = c"eu.ensodo.ethnoflow.fixture-sine";
const VOICES: usize = 8;
const RELEASE_S: f32 = 0.08;

/// Null-terminated feature list; the pointers are to static C strings, hence `Sync`.
struct Features([*const c_char; 3]);
unsafe impl Sync for Features {}
static FEATURES: Features = Features([
    CLAP_PLUGIN_FEATURE_INSTRUMENT.as_ptr(),
    CLAP_PLUGIN_FEATURE_SYNTHESIZER.as_ptr(),
    ptr::null(),
]);

static DESCRIPTOR: clap_plugin_descriptor = clap_plugin_descriptor {
    clap_version: CLAP_VERSION,
    id: PLUGIN_ID.as_ptr(),
    name: c"EthnoFlow Fixture Sine".as_ptr(),
    vendor: c"EthnoFlow".as_ptr(),
    url: c"https://github.com/yoviginiki/ethnoflow".as_ptr(),
    manual_url: c"".as_ptr(),
    support_url: c"".as_ptr(),
    version: c"0.1.0".as_ptr(),
    description: c"Eight sine voices with a short release; the host test fixture.".as_ptr(),
    features: FEATURES.0.as_ptr(),
};

#[derive(Clone, Copy, Default)]
struct Voice {
    key: i16,
    phase: f32,
    env: f32,
    gate: bool,
    active: bool,
}

struct State {
    plugin: clap_plugin,
    sample_rate: f32,
    voices: [Voice; VOICES],
}

unsafe fn state<'a>(plugin: *const clap_plugin) -> &'a mut State {
    &mut *((*plugin).plugin_data as *mut State)
}

unsafe extern "C" fn plugin_init(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn plugin_destroy(plugin: *const clap_plugin) {
    if plugin.is_null() {
        return;
    }
    let data = (*plugin).plugin_data as *mut State;
    if !data.is_null() {
        drop(Box::from_raw(data));
    }
}

unsafe extern "C" fn plugin_activate(
    plugin: *const clap_plugin,
    sample_rate: f64,
    _min: u32,
    _max: u32,
) -> bool {
    let s = state(plugin);
    s.sample_rate = sample_rate as f32;
    s.voices = [Voice::default(); VOICES];
    true
}

unsafe extern "C" fn plugin_deactivate(_plugin: *const clap_plugin) {}
unsafe extern "C" fn plugin_start_processing(_plugin: *const clap_plugin) -> bool {
    true
}
unsafe extern "C" fn plugin_stop_processing(_plugin: *const clap_plugin) {}
unsafe extern "C" fn plugin_reset(plugin: *const clap_plugin) {
    state(plugin).voices = [Voice::default(); VOICES];
}
unsafe extern "C" fn plugin_on_main_thread(_plugin: *const clap_plugin) {}

fn note_on(s: &mut State, key: i16) {
    let slot = s.voices.iter().position(|v| !v.active).unwrap_or_else(|| {
        s.voices
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.env.partial_cmp(&b.1.env).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0)
    });
    s.voices[slot] = Voice {
        key,
        phase: 0.0,
        env: 0.0,
        gate: true,
        active: true,
    };
}

fn note_off(s: &mut State, key: i16) {
    for v in s.voices.iter_mut() {
        if v.active && (key < 0 || v.key == key) {
            v.gate = false;
        }
    }
}

unsafe extern "C" fn plugin_process(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    if plugin.is_null() || process.is_null() {
        return CLAP_PROCESS_ERROR;
    }
    let s = state(plugin);
    let p = &*process;
    let frames = p.frames_count as usize;
    if p.audio_outputs_count == 0 || p.audio_outputs.is_null() {
        return CLAP_PROCESS_ERROR;
    }
    let out = &*p.audio_outputs;
    if out.data32.is_null() || out.channel_count == 0 {
        return CLAP_PROCESS_ERROR;
    }
    let channels = out.channel_count as usize;
    // Events: walk the input list once, applying each event at its frame offset.
    let (n_events, list) = match p.in_events.as_ref() {
        Some(l) => (l.size.map(|f| f(l)).unwrap_or(0), Some(l)),
        None => (0, None),
    };
    let mut next_event = 0u32;
    let sr = s.sample_rate.max(1.0);
    let rel = (-6.9078 / (RELEASE_S * sr)).exp();
    let att = 1.0 / (0.004 * sr);
    for i in 0..frames {
        while next_event < n_events {
            let Some(l) = list else { break };
            let h = l.get.map(|g| g(l, next_event)).unwrap_or(ptr::null());
            if h.is_null() {
                next_event += 1;
                continue;
            }
            let hdr = &*h;
            if hdr.time as usize > i {
                break;
            }
            if hdr.space_id == CLAP_CORE_EVENT_SPACE_ID {
                match hdr.type_ {
                    CLAP_EVENT_NOTE_ON => {
                        let e = &*(h as *const clap_event_note);
                        note_on(s, e.key);
                    }
                    CLAP_EVENT_NOTE_OFF | CLAP_EVENT_NOTE_CHOKE => {
                        let e = &*(h as *const clap_event_note);
                        note_off(s, e.key);
                    }
                    _ => {}
                }
            }
            next_event += 1;
        }
        let mut acc = 0.0f32;
        for v in s.voices.iter_mut() {
            if !v.active {
                continue;
            }
            if v.gate {
                v.env = (v.env + att).min(1.0);
            } else {
                v.env *= rel;
                if v.env < 1e-4 {
                    v.active = false;
                    continue;
                }
            }
            let hz = 440.0 * 2f32.powf((v.key as f32 - 69.0) / 12.0);
            acc += (v.phase * std::f32::consts::TAU).sin() * v.env * 0.2;
            v.phase += hz / sr;
            if v.phase >= 1.0 {
                v.phase -= 1.0;
            }
        }
        for c in 0..channels {
            let ch = *out.data32.add(c);
            if !ch.is_null() {
                *ch.add(i) = acc;
            }
        }
    }
    CLAP_PROCESS_CONTINUE
}

unsafe extern "C" fn audio_ports_count(_plugin: *const clap_plugin, is_input: bool) -> u32 {
    if is_input {
        0
    } else {
        1
    }
}

unsafe extern "C" fn audio_ports_get(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_audio_port_info,
) -> bool {
    if is_input || index != 0 || info.is_null() {
        return false;
    }
    let i = &mut *info;
    i.id = 0;
    i.name = [0; 256];
    for (k, b) in b"Main out".iter().enumerate() {
        i.name[k] = *b as c_char;
    }
    i.flags = CLAP_AUDIO_PORT_IS_MAIN;
    i.channel_count = 2;
    i.port_type = CLAP_PORT_STEREO.as_ptr();
    i.in_place_pair = u32::MAX;
    true
}

static AUDIO_PORTS: clap_plugin_audio_ports = clap_plugin_audio_ports {
    count: Some(audio_ports_count),
    get: Some(audio_ports_get),
};

unsafe extern "C" fn note_ports_count(_plugin: *const clap_plugin, is_input: bool) -> u32 {
    if is_input {
        1
    } else {
        0
    }
}

unsafe extern "C" fn note_ports_get(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_note_port_info,
) -> bool {
    if !is_input || index != 0 || info.is_null() {
        return false;
    }
    let i = &mut *info;
    i.id = 0;
    i.supported_dialects = CLAP_NOTE_DIALECT_CLAP;
    i.preferred_dialect = CLAP_NOTE_DIALECT_CLAP;
    i.name = [0; 256];
    for (k, b) in b"Notes".iter().enumerate() {
        i.name[k] = *b as c_char;
    }
    true
}

static NOTE_PORTS: clap_plugin_note_ports = clap_plugin_note_ports {
    count: Some(note_ports_count),
    get: Some(note_ports_get),
};

unsafe extern "C" fn plugin_get_extension(
    _plugin: *const clap_plugin,
    id: *const c_char,
) -> *const c_void {
    if id.is_null() {
        return ptr::null();
    }
    let id = CStr::from_ptr(id);
    if id == CLAP_EXT_AUDIO_PORTS {
        &AUDIO_PORTS as *const _ as *const c_void
    } else if id == CLAP_EXT_NOTE_PORTS {
        &NOTE_PORTS as *const _ as *const c_void
    } else {
        ptr::null()
    }
}

unsafe extern "C" fn factory_get_plugin_count(_factory: *const clap_plugin_factory) -> u32 {
    1
}

unsafe extern "C" fn factory_get_plugin_descriptor(
    _factory: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    if index == 0 {
        &DESCRIPTOR
    } else {
        ptr::null()
    }
}

unsafe extern "C" fn factory_create_plugin(
    _factory: *const clap_plugin_factory,
    _host: *const clap_host,
    plugin_id: *const c_char,
) -> *const clap_plugin {
    if plugin_id.is_null() || CStr::from_ptr(plugin_id) != PLUGIN_ID {
        return ptr::null();
    }
    let state = Box::new(State {
        plugin: clap_plugin {
            desc: &DESCRIPTOR,
            plugin_data: ptr::null_mut(),
            init: Some(plugin_init),
            destroy: Some(plugin_destroy),
            activate: Some(plugin_activate),
            deactivate: Some(plugin_deactivate),
            start_processing: Some(plugin_start_processing),
            stop_processing: Some(plugin_stop_processing),
            reset: Some(plugin_reset),
            process: Some(plugin_process),
            get_extension: Some(plugin_get_extension),
            on_main_thread: Some(plugin_on_main_thread),
        },
        sample_rate: 48_000.0,
        voices: [Voice::default(); VOICES],
    });
    let raw = Box::into_raw(state);
    (*raw).plugin.plugin_data = raw as *mut c_void;
    &(*raw).plugin
}

static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(factory_get_plugin_count),
    get_plugin_descriptor: Some(factory_get_plugin_descriptor),
    create_plugin: Some(factory_create_plugin),
};

unsafe extern "C" fn entry_init(_path: *const c_char) -> bool {
    true
}
unsafe extern "C" fn entry_deinit() {}
unsafe extern "C" fn entry_get_factory(factory_id: *const c_char) -> *const c_void {
    if !factory_id.is_null() && CStr::from_ptr(factory_id) == CLAP_PLUGIN_FACTORY_ID {
        &FACTORY as *const _ as *const c_void
    } else {
        ptr::null()
    }
}

/// The symbol every CLAP library exports.
#[no_mangle]
#[allow(non_upper_case_globals)]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(entry_get_factory),
};

/// The same entry for in-process use (tests, static linking).
pub fn entry() -> &'static clap_plugin_entry {
    &clap_entry
}
