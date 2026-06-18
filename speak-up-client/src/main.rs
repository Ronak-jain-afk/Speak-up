#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            speak_up_client::setup_tauri(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            speak_up_client::settings::load_settings_cmd,
            speak_up_client::settings::save_settings_cmd,
            speak_up_client::settings::get_audio_devices_cmd,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
