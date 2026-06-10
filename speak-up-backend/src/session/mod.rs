use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use speak_up_core::{AppContext, AudioChunk, DictationSession};

#[derive(Default)]
pub struct SessionManager {
    #[allow(dead_code)]
    sessions: Arc<RwLock<HashMap<Uuid, SessionState>>>,
}

struct SessionState {
    _session: DictationSession,
    _app_context: AppContext,
    _audio_buffer: Vec<AudioChunk>,
    _audio_rx: Option<mpsc::Receiver<AudioChunk>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create_session(&self, _context: AppContext) -> DictationSession {
        unimplemented!("Phase 3")
    }

    pub async fn append_audio(&self, _session_id: Uuid, _chunk: AudioChunk) {
        unimplemented!("Phase 3")
    }

    pub async fn finalize_session(&self, _session_id: Uuid) {
        unimplemented!("Phase 3")
    }
}
