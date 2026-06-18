use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use speak_up_core::*;

fn config_dir() -> PathBuf {
    if let Some(dir) = dirs::config_dir() {
        dir.join("speak-up")
    } else {
        PathBuf::from("~/.config/speak-up")
    }
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.json")
}

fn ensure_config_dir() -> std::io::Result<()> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir)
}

pub fn load_settings_from_disk() -> Settings {
    let path = settings_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to parse settings ({}), using defaults", e);
                default_settings()
            }
        },
        Err(_) => {
            tracing::info!("No settings file found, using defaults");
            default_settings()
        }
    }
}

pub fn save_settings_to_disk(settings: &Settings) -> Result<(), String> {
    let path = settings_path();
    ensure_config_dir().map_err(|e| format!("Failed to create config dir: {}", e))?;

    let tmp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    let mut file =
        std::fs::File::create(&tmp_path).map_err(|e| format!("Failed to write temp file: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write content: {}", e))?;
    file.sync_all()
        .map_err(|e| format!("Failed to sync: {}", e))?;

    std::fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Failed to rename temp file: {}", e))?;

    tracing::info!("Settings saved to {}", path.display());
    Ok(())
}

pub fn default_settings() -> Settings {
    use speak_up_core::*;
    Settings {
        version: env!("CARGO_PKG_VERSION").to_string(),
        microphone: MicrophoneSettings { device_id: None, noise_gate_threshold: 0.02 },
        hotkeys: HotkeySettings {
            hold_to_record: "Ctrl+Shift+Space".into(),
            toggle_mic: "Ctrl+Shift+M".into(),
            retype_last: "Ctrl+Shift+V".into(),
        },
        asr_provider: None,
        cleaner_provider: None,
        profiles: vec![
            ProfileMapping { app_pattern: ".*Outlook.*".into(), profile_name: "email".into() },
            ProfileMapping { app_pattern: ".*Slack.*".into(), profile_name: "chat".into() },
            ProfileMapping { app_pattern: ".*Code.*".into(), profile_name: "code".into() },
            ProfileMapping { app_pattern: ".*Terminal.*".into(), profile_name: "terminal".into() },
        ],
        dictionary: vec![DictionaryEntry {
            spoken_form: "speak up".into(),
            written_form: "Speak Up".into(),
        }],
        general: GeneralSettings {
            launch_at_startup: false,
            sound_feedback: true,
            auto_mute: false,
            overlay_position: OverlayPosition::NearCursor,
            history_retention_days: 30,
        },
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

#[tauri::command]
pub fn load_settings_cmd() -> Result<Settings, String> {
    Ok(load_settings_from_disk())
}

#[tauri::command]
pub fn save_settings_cmd(settings: Settings) -> Result<(), String> {
    save_settings_to_disk(&settings)
}

#[tauri::command]
pub fn get_audio_devices_cmd() -> Vec<DeviceInfo> {
    crate::audio::AudioCapture::enumerate_devices()
        .into_iter()
        .map(|d| DeviceInfo { id: d.id, name: d.name })
        .collect()
}
