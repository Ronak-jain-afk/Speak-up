use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictationSession {
    pub id: Uuid,
    pub start_time: DateTime<Utc>,
    pub status: SessionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionStatus {
    Recording,
    Processing,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub text: String,
    pub is_final: bool,
    pub confidence: Option<f32>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioChunk {
    pub data: Vec<u8>,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppContext {
    pub window_title: String,
    pub executable_name: String,
    pub window_class: String,
    pub profile_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub provider_type: ProviderType,
    pub name: String,
    pub settings: serde_json::Value,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ProviderType {
    LocalWhisper,
    OpenAIWhisper,
    Deepgram,
    LocalLLM,
    OpenAICleaner,
    AnthropicCleaner,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub system_prompt_template: String,
    pub client_post_process: Vec<PostProcessRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PostProcessRule {
    PrefixSpace,
    TrimWhitespace,
    CapitalizeFirst,
    PreserveLineBreaks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictionaryEntry {
    pub spoken_form: String,
    pub written_form: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub version: String,
    pub microphone: MicrophoneSettings,
    pub hotkeys: HotkeySettings,
    pub asr_provider: Option<ProviderConfig>,
    pub cleaner_provider: Option<ProviderConfig>,
    pub profiles: Vec<ProfileMapping>,
    pub dictionary: Vec<DictionaryEntry>,
    pub general: GeneralSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: "0.1.0".into(),
            microphone: MicrophoneSettings::default(),
            hotkeys: HotkeySettings::default(),
            asr_provider: None,
            cleaner_provider: None,
            profiles: Vec::new(),
            dictionary: Vec::new(),
            general: GeneralSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrophoneSettings {
    pub device_id: Option<String>,
    pub noise_gate_threshold: f32,
}

impl Default for MicrophoneSettings {
    fn default() -> Self {
        Self { device_id: None, noise_gate_threshold: 0.02 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeySettings {
    pub hold_to_record: String,
    pub toggle_mic: String,
    pub retype_last: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            hold_to_record: "Ctrl+Shift+Space".into(),
            toggle_mic: "Ctrl+Shift+M".into(),
            retype_last: "Ctrl+Shift+V".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileMapping {
    pub app_pattern: String,
    pub profile_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    pub launch_at_startup: bool,
    pub sound_feedback: bool,
    pub auto_mute: bool,
    pub overlay_position: OverlayPosition,
    pub history_retention_days: u32,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            sound_feedback: true,
            auto_mute: false,
            overlay_position: OverlayPosition::default(),
            history_retention_days: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum OverlayPosition {
    #[default]
    NearCursor,
    BottomRight,
    TopRight,
}
