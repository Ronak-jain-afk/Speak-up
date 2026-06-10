use speak_up_core::Settings;

#[allow(dead_code)]
const SETTINGS_PATH: &str = "~/.config/speak-up/settings.json";

pub fn load_settings() -> Settings {
    unimplemented!("Phase 7")
}

pub fn save_settings(settings: &Settings) -> Result<(), SettingsError> {
    let _ = settings;
    unimplemented!("Phase 7")
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

#[derive(Debug)]
pub enum SettingsError {
    Read(String),
    Write(String),
    Parse(String),
}
