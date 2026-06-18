use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use speak_up_core::*;
use tauri::Manager;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

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
    let mut settings: Settings = match std::fs::read_to_string(&path) {
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
    };

    if let Some(ref mut provider) = settings.asr_provider {
        if let Some(key) = crate::keychain::get_api_key("asr") {
            if let Some(obj) = provider.settings.as_object_mut() {
                obj.insert("api_key".into(), serde_json::Value::String(key));
            }
        }
    }
    if let Some(ref mut provider) = settings.cleaner_provider {
        if let Some(key) = crate::keychain::get_api_key("cleaner") {
            if let Some(obj) = provider.settings.as_object_mut() {
                obj.insert("api_key".into(), serde_json::Value::String(key));
            }
        }
    }

    settings
}

pub fn save_settings_to_disk(settings: &Settings) -> Result<(), String> {
    let path = settings_path();
    ensure_config_dir().map_err(|e| format!("Failed to create config dir: {}", e))?;

    if let Some(ref provider) = settings.asr_provider {
        if let Some(key) = provider.settings.get("api_key").and_then(|v| v.as_str()) {
            let _ = crate::keychain::store_api_key("asr", key);
        }
    }
    if let Some(ref provider) = settings.cleaner_provider {
        if let Some(key) = provider.settings.get("api_key").and_then(|v| v.as_str()) {
            let _ = crate::keychain::store_api_key("cleaner", key);
        }
    }

    let mut stripped = settings.clone();
    if let Some(ref mut provider) = stripped.asr_provider {
        if let Some(obj) = provider.settings.as_object_mut() {
            obj.remove("api_key");
        }
    }
    if let Some(ref mut provider) = stripped.cleaner_provider {
        if let Some(obj) = provider.settings.as_object_mut() {
            obj.remove("api_key");
        }
    }

    let tmp_path = path.with_extension("json.tmp");
    let content = serde_json::to_string_pretty(&stripped)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    let mut file =
        std::fs::File::create(&tmp_path).map_err(|e| format!("Failed to write temp file: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write content: {}", e))?;
    file.sync_all()
        .map_err(|e| format!("Failed to sync: {}", e))?;

    std::fs::rename(&tmp_path, &path)
        .map_err(|e| format!("Failed to rename temp file: {}", e))?;

    tracing::info!("Settings saved to {} (API keys stored in keychain)", path.display());
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
    save_settings_to_disk(&settings)?;
    crate::notify_settings_changed();
    Ok(())
}

#[tauri::command]
pub fn get_audio_devices_cmd() -> Vec<DeviceInfo> {
    crate::audio::AudioCapture::enumerate_devices()
        .into_iter()
        .map(|d| DeviceInfo { id: d.id, name: d.name })
        .collect()
}

#[tauri::command]
pub fn query_history_cmd(
    limit: usize,
    offset: usize,
    search_term: Option<String>,
) -> Result<(Vec<speak_up_core::ipc::DictationEntry>, usize), String> {
    let req_tx = crate::get_backend_request_tx().ok_or("Backend not connected")?;
    let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
    let req = crate::BackendRequest::QueryHistory {
        limit,
        offset,
        search_term,
        response_tx: resp_tx,
    };
    req_tx.send(req).map_err(|e| format!("Failed to send request: {}", e))?;
    resp_rx.recv().map_err(|e| format!("Failed to receive response: {}", e))?
}

#[tauri::command]
pub fn query_last_dictation_cmd() -> Result<Option<speak_up_core::ipc::DictationEntry>, String> {
    let req_tx = crate::get_backend_request_tx().ok_or("Backend not connected")?;
    let (resp_tx, resp_rx) = crossbeam_channel::bounded(1);
    let req = crate::BackendRequest::QueryLastDictation {
        response_tx: resp_tx,
    };
    req_tx.send(req).map_err(|e| format!("Failed to send request: {}", e))?;
    resp_rx.recv().map_err(|e| format!("Failed to receive response: {}", e))?
}

#[tauri::command]
pub fn close_wizard_cmd(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("wizard") {
        let _ = window.close();
    }
    Ok(())
}

#[tauri::command]
pub fn inject_text_cmd(text: String) -> Result<(), String> {
    let req_tx = crate::get_backend_request_tx().ok_or("Backend not connected")?;
    let req = crate::BackendRequest::InjectText { text };
    req_tx.send(req).map_err(|e| format!("Failed to send request: {}", e))
}

#[tauri::command]
pub fn is_first_run_cmd() -> bool {
    !settings_path().exists()
}

pub fn is_first_run() -> bool {
    !settings_path().exists()
}

#[tauri::command]
pub fn test_microphone_cmd(device_id: String) -> Result<f32, String> {
    let host = cpal::default_host();
    let device = if device_id.is_empty() {
        host.default_input_device().ok_or("No default input device found")?
    } else {
        host.input_devices()
            .map_err(|e| format!("Failed to list devices: {}", e))?
            .find(|d| d.name().map(|n| n == device_id).unwrap_or(false))
            .ok_or_else(|| format!("Device '{}' not found", device_id))?
    };

    let supported = device.default_input_config().map_err(|e| format!("No config: {}", e))?;
    let config: cpal::StreamConfig = supported.into();

    let level = std::sync::Arc::new(std::sync::Mutex::new(0.0f32));
    let level_clone = level.clone();
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done_clone = done.clone();

    let stream = device
        .build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let sum_sq: f32 = data.iter().map(|s| s * s).sum();
                let rms = (sum_sq / data.len() as f32).sqrt();
                if let Ok(mut l) = level_clone.lock() {
                    if rms > *l { *l = rms; }
                }
                done_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            },
            move |err| tracing::error!("Mic test error: {}", err),
            None,
        )
        .map_err(|e| format!("Failed to open stream: {}", e))?;

    stream.play().map_err(|e| format!("Failed to play stream: {}", e))?;

    // Capture for ~1.5 seconds
    std::thread::sleep(Duration::from_millis(1500));

    drop(stream);

    let peak = *level.lock().map_err(|e| format!("Lock error: {}", e))?;
    Ok(peak)
}
