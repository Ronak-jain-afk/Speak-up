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

impl std::fmt::Display for ASRError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ASRError::Initialization(msg) => write!(f, "Initialization error: {}", msg),
            ASRError::Inference(msg) => write!(f, "Inference error: {}", msg),
            ASRError::ModelNotFound(msg) => write!(f, "Model not found: {}", msg),
        }
    }
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
        Some(cfg) if cfg.provider_type == speak_up_core::ProviderType::LocalWhisper => {
            let mut engine = local::LocalWhisper::new();
            let language = cfg
                .settings
                .get("language")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let model_path = cfg
                .settings
                .get("model_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let asr_cfg = ASRConfig {
                model_path,
                language,
                hot_words: Vec::new(),
            };
            if let Err(e) = engine.initialize(&asr_cfg) {
                #[cfg(feature = "local-asr")]
                tracing::error!("LocalWhisper init failed: {} — falling back to MockWhisper", e);
                #[cfg(not(feature = "local-asr"))]
                {
                    let _ = e;
                    tracing::warn!(
                        "LocalWhisper not available — build with --features local-asr. Falling back to MockWhisper"
                    );
                }
                Box::new(local::MockWhisper::new())
            } else {
                Box::new(engine)
            }
        }
        Some(cfg) if cfg.provider_type == speak_up_core::ProviderType::Deepgram => {
            tracing::warn!("Deepgram ASR not yet implemented, falling back to MockWhisper");
            Box::new(local::MockWhisper::new())
        }
        _ => Box::new(local::MockWhisper::new()),
    }
}
