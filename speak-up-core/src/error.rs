use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("ASR error: {0}")]
    ASR(String),

    #[error("Cleaner error: {0}")]
    Cleaner(String),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("Settings error: {0}")]
    Settings(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Audio error: {0}")]
    Audio(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}
