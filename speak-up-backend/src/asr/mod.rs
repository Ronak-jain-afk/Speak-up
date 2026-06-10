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
