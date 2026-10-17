mod analysis_cmds;
mod commands;
mod midi_cmds;
mod project_cmds;
mod settings;
mod state;

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
            project_cmds::export_wav,
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
