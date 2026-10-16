mod commands;
mod state;

use state::AppState;

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
            commands::start_recording,
            commands::recording_status,
            commands::stop_recording,
            commands::recording_waveform,
            commands::take_info,
            commands::get_waveform,
            commands::clear_take,
            commands::start_playback,
            commands::playback_status,
            commands::stop_playback,
            commands::export_wav,
            commands::import_wav,
        ])
        .run(tauri::generate_context!())
        .expect("error while running EthnoFlow Studio");
}
