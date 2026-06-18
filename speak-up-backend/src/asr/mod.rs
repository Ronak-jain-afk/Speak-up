use speak_up_core::{AudioChunk, TranscriptSegment};
use tokio::sync::mpsc;

pub type TranscriptionTask = tokio::task::JoinHandle<TranscriptResult>;

#[derive(Debug, Clone)]
pub struct TranscriptEvent {
    pub segment: TranscriptSegment,
    pub is_final: bool,
}

#[derive(Debug, Clone)]
pub struct TranscriptResult {
    pub segments: Vec<TranscriptSegment>,
    pub full_text: String,
}

pub trait ASREngine: Send + Sync {
    fn initialize(&mut self, config: &ASRConfig) -> Result<(), ASRError>;
    fn transcribe_stream(
        &mut self,
        rx: mpsc::Receiver<AudioChunk>,
    ) -> (mpsc::Receiver<TranscriptEvent>, TranscriptionTask);
    fn finalize(&mut self) -> TranscriptResult;
    fn shutdown(&mut self);
}

#[derive(Debug, Clone)]
pub struct ASRConfig {
    pub model_path: Option<String>,
    pub language: Option<String>,
    pub hot_words: Vec<String>,
}

#[derive(Debug)]
pub enum ASRError {
    Initialization(String),
    Inference(String),
    ModelNotFound(String),
}

pub mod cloud;
pub mod local;

use speak_up_core::ProviderConfig;

pub fn build_asr_engine(config: &Option<ProviderConfig>) -> Box<dyn ASREngine + Send + Sync> {
    match config {
        Some(cfg) if cfg.provider_type == speak_up_core::ProviderType::OpenAIWhisper => {
            let api_key = cfg
                .settings
                .get("api_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let model = cfg
                .settings
                .get("model")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let language = cfg
                .settings
                .get("language")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            Box::new(cloud::CloudWhisper::new(api_key, model, language))
        }
        Some(cfg) if cfg.provider_type == speak_up_core::ProviderType::Deepgram => {
            tracing::warn!("Deepgram ASR not yet implemented, falling back to MockWhisper");
            Box::new(local::MockWhisper::new())
        }
        _ => Box::new(local::MockWhisper::new()),
    }
}
