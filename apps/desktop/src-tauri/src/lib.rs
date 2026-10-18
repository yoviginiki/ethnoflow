mod ambient_cmds;
mod analysis_cmds;
mod arrangement_cmds;
mod assistant_cmds;
mod bus_cmds;
mod clap_cmds;
mod commands;
mod forge_cmds;
mod graph_cmds;
mod harmony_cmds;
mod instrument_cmds;
mod intent_cmds;
mod library_cmds;
mod midi_cmds;
mod midi_in_cmds;
mod pad_cmds;
mod preset_cmds;
mod project_cmds;
mod reference_cmds;
mod reference_phrases;
mod rhythm_cmds;
mod samples_cmds;
mod settings;
mod song_export_cmds;
mod state;
mod synth_cmds;
mod trainer_cmds;
mod transport_cmds;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .build(),
        )
        .manage(AppState::default())
        .setup(|app| {
            let config = app.path().app_config_dir()?;
            let state = app.state::<AppState>();
            let mut s = state.lock();
            s.profiles_dir = Some(config.join("instruments"));
            s.presets_dir = Some(config.join("ambient-presets"));
            s.assistant_dir = Some(config.join("assistant"));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::audio_host,
            commands::list_input_devices,
            commands::list_output_devices,
            commands::play_test_tone,
            commands::start_recording,
            commands::recording_status,
            commands::recording_waveform,
            commands::stop_recording,
            commands::start_playback,
            commands::playback_status,
            commands::set_playback_loop,
            commands::set_playback_click,
            commands::stop_playback,
            commands::start_metronome,
            commands::stop_metronome,
            commands::metronome_status,
            project_cmds::default_projects_dir,
            project_cmds::recent_projects,
            project_cmds::create_project,
            project_cmds::open_project,
            project_cmds::close_project,
            project_cmds::project_state,
            project_cmds::set_timing,
            project_cmds::load_take,
            project_cmds::get_waveform,
            project_cmds::rename_take,
            project_cmds::delete_take,
            project_cmds::set_trim,
            project_cmds::set_markers,
            project_cmds::set_take_recipe,
            project_cmds::set_take_meta,
            project_cmds::analyze_silence,
            project_cmds::snap_trim_to_zero,
            project_cmds::relink_take,
            project_cmds::verify_takes,
            project_cmds::import_audio,
            project_cmds::save_selection_as_take,
            analysis_cmds::analyze_take,
            analysis_cmds::get_analysis,
            analysis_cmds::update_notes,
            analysis_cmds::reset_notes,
            analysis_cmds::set_scale,
            analysis_cmds::set_reference,
            analysis_cmds::export_midi,
            analysis_cmds::start_monitor,
            analysis_cmds::stop_monitor,
            analysis_cmds::monitor_status,
            midi_cmds::midi_take,
            midi_cmds::set_midi_render,
            midi_cmds::play_notes,
            midi_cmds::notes_playback_status,
            midi_cmds::stop_notes,
            midi_cmds::send_notes_to_player,
            instrument_cmds::list_profiles,
            instrument_cmds::get_profile,
            instrument_cmds::create_profile,
            instrument_cmds::delete_profile,
            instrument_cmds::set_project_instrument,
            instrument_cmds::calibrate_note,
            instrument_cmds::remove_profile_note,
            instrument_cmds::play_profile_note,
            trainer_cmds::evaluate_performance,
            synth_cmds::start_live_synth,
            synth_cmds::stop_live_synth,
            synth_cmds::synth_note,
            synth_cmds::synth_all_off,
            synth_cmds::live_synth_status,
            midi_in_cmds::list_midi_ports,
            midi_in_cmds::open_midi_input,
            midi_in_cmds::close_midi_input,
            midi_in_cmds::poll_midi_events,
            project_cmds::export_wav,
            graph_cmds::start_drone,
            graph_cmds::set_drone,
            graph_cmds::stop_drone,
            graph_cmds::drone_status,
            graph_cmds::set_graph_master,
            pad_cmds::start_pad,
            pad_cmds::set_pad,
            pad_cmds::stop_pad,
            pad_cmds::pad_status,
            pad_cmds::pad_emotions,
            intent_cmds::performance_analysis,
            intent_cmds::annotate_note,
            intent_cmds::intent_annotations,
            harmony_cmds::harmony_proposals,
            harmony_cmds::export_harmony_midi,
            arrangement_cmds::arrangement_get,
            arrangement_cmds::set_arrangement,
            arrangement_cmds::arrangement_play,
            arrangement_cmds::arrangement_pause,
            arrangement_cmds::arrangement_seek,
            arrangement_cmds::arrangement_release,
            arrangement_cmds::arrangement_status,
            arrangement_cmds::take_peaks,
            arrangement_cmds::export_arrangement_mix,
            ambient_cmds::ambient_get,
            ambient_cmds::set_ambient,
            ambient_cmds::ambient_play,
            ambient_cmds::ambient_stop,
            ambient_cmds::ambient_status,
            ambient_cmds::background_fade_out,
            ambient_cmds::ambient_program_set,
            ambient_cmds::export_ambient,
            preset_cmds::ambient_presets,
            preset_cmds::ambient_preset_load,
            preset_cmds::ambient_preset_save,
            preset_cmds::ambient_preset_delete,
            rhythm_cmds::start_rhythm,
            rhythm_cmds::set_rhythm,
            rhythm_cmds::stop_rhythm,
            rhythm_cmds::rhythm_status,
            rhythm_cmds::rhythm_restart,
            rhythm_cmds::rhythm_regenerate,
            rhythm_cmds::estimate_take_tempo,
            rhythm_cmds::rhythm_to_takes,
            harmony_cmds::countermelody_to_take,
            library_cmds::library_list,
            library_cmds::library_save,
            library_cmds::library_get,
            library_cmds::library_update,
            library_cmds::library_replace_payload,
            library_cmds::library_delete,
            library_cmds::library_facets,
            project_cmds::set_song_map,
            project_cmds::session_get,
            project_cmds::session_set,
            assistant_cmds::assistant_dir,
            assistant_cmds::assistant_poll,
            assistant_cmds::assistant_reply,
            assistant_cmds::assistant_write_state,
            bus_cmds::bus_get,
            bus_cmds::bus_set,
            bus_cmds::bus_fade,
            bus_cmds::bus_status,
            rhythm_cmds::rhythm_render,
            forge_cmds::instrument_list,
            forge_cmds::instrument_get,
            forge_cmds::instrument_save,
            forge_cmds::instrument_update,
            forge_cmds::instrument_delete,
            forge_cmds::instrument_audition_start,
            forge_cmds::instrument_audition_set,
            forge_cmds::instrument_audition_stop,
            forge_cmds::instrument_render,
            samples_cmds::samples_scan,
            samples_cmds::samples_default_folders,
            samples_cmds::sample_preview,
            samples_cmds::sample_preview_stop,
            reference_cmds::reference_add,
            reference_cmds::reference_list,
            reference_cmds::reference_set_meta,
            reference_cmds::reference_remove,
            reference_cmds::reference_peaks,
            reference_cmds::reference_report,
            reference_cmds::reference_analyze,
            reference_cmds::reference_play,
            reference_phrases::reference_phrases,
            reference_phrases::reference_phrases_get,
            reference_phrases::reference_phrase_play,
            reference_phrases::reference_phrase_to_take,
            transport_cmds::transport_play,
            transport_cmds::transport_stop,
            transport_cmds::transport_seek,
            transport_cmds::transport_status,
            transport_cmds::transport_loop,
            clap_cmds::clap_default_folders,
            clap_cmds::clap_scan,
            clap_cmds::clap_render,
            song_export_cmds::export_song,
        ])
        .build(tauri::generate_context!())
        .expect("error while building EthnoFlow Studio")
        .run(|app, event| {
            // Window closed while recording: finalize the take so it is `complete`.
            if let tauri::RunEvent::Exit = event {
                let state = app.state::<AppState>();
                project_cmds::close_inner(&mut state.lock());
            }
        });
}
