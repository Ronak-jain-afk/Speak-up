use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    StartSession { app_context: AppContext },
    AudioChunk { session_id: Uuid, chunk: AudioChunk },
    EndSession { session_id: Uuid },
    ReconfigureProvider { provider_type: ProviderType, config: ProviderConfig },
    ReloadSettings,
    QueryHistory { limit: usize, offset: usize, search_term: Option<String> },
    QueryLastDictation,
    DownloadModel { model_name: String },
    ListModels,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub filename: String,
    pub size_mb: u64,
    pub downloaded: bool,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BackendMessage {
    SessionStarted { session_id: Uuid },
    InterimTranscript { session_id: Uuid, segment: TranscriptSegment },
    FinalTranscript { session_id: Uuid, raw_text: String, cleaned_text: String },
    ProcessingStatus { session_id: Uuid, stage: ProcessingStage },
    ProviderSwitched { provider_type: ProviderType, success: bool, error: Option<String> },
    HistoryResult { entries: Vec<DictationEntry>, total_count: usize },
    LastDictationResult { entry: Option<DictationEntry> },
    ModelList { models: Vec<ModelInfo> },
    ModelDownloadProgress { model_name: String, bytes_downloaded: u64, total_bytes: u64 },
    ModelDownloaded { model_name: String, success: bool, error: Option<String> },
    Error { code: ErrorCode, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessingStage {
    Transcribing,
    Cleaning,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DictationEntry {
    pub id: i64,
    pub timestamp: String,
    pub raw_text: String,
    pub cleaned_text: String,
    pub app_context: Option<String>,
    pub profile_used: Option<String>,
    pub asr_provider: Option<String>,
    pub cleaner_provider: Option<String>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    SessionNotFound,
    ASRError,
    CleanerError,
    ProviderNotFound,
    InvalidConfig,
    Internal,
}
