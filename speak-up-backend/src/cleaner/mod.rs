use speak_up_core::{DictionaryEntry, Profile};

pub trait AICleaner: Send + Sync {
    fn clean(
        &self,
        transcript: &str,
        profile: &Profile,
        dictionary: &[DictionaryEntry],
    ) -> Result<String, CleanerError>;
    fn shutdown(&mut self);
}

#[derive(Debug)]
pub enum CleanerError {
    Initialization(String),
    Inference(String),
    ModelNotFound(String),
    Http(String),
    Auth(String),
}

impl std::fmt::Display for CleanerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CleanerError::Initialization(msg) => write!(f, "Initialization error: {}", msg),
            CleanerError::Inference(msg) => write!(f, "Inference error: {}", msg),
            CleanerError::ModelNotFound(msg) => write!(f, "Model not found: {}", msg),
            CleanerError::Http(msg) => write!(f, "HTTP error: {}", msg),
            CleanerError::Auth(msg) => write!(f, "Auth error: {}", msg),
        }
    }
}

impl std::error::Error for CleanerError {}

pub mod cloud;
pub mod local;

pub use local::LocalLLM;
pub use cloud::{AnthropicCleaner, OpenAICleaner};

pub fn build_cleaner(
    config: &Option<speak_up_core::ProviderConfig>,
) -> Box<dyn AICleaner> {
    match config {
        Some(cfg) => match cfg.provider_type {
            speak_up_core::ProviderType::OpenAICleaner => {
                let api_key = cfg.settings.get("api_key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Box::new(OpenAICleaner::new(api_key))
            }
            speak_up_core::ProviderType::AnthropicCleaner => {
                let api_key = cfg.settings.get("api_key").and_then(|v| v.as_str()).unwrap_or("").to_string();
                Box::new(AnthropicCleaner::new(api_key))
            }
            _ => {
                Box::new(LocalLLM::new())
            }
        },
        None => Box::new(LocalLLM::new()),
    }
}
