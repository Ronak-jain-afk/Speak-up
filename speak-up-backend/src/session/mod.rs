use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use speak_up_core::{AppContext, AudioChunk, DictationSession, SessionStatus};

use crate::asr::{ASREngine, ASRConfig};

#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, SessionState>>>,
    asr_engine: Arc<RwLock<Box<dyn ASREngine + Send + Sync>>>,
}

struct SessionState {
    session: DictationSession,
    _app_context: AppContext,
    audio_buffer: Vec<AudioChunk>,
    audio_tx: mpsc::Sender<AudioChunk>,
    audio_rx: mpsc::Receiver<AudioChunk>,
}

impl SessionManager {
    pub fn new(asr_engine: Box<dyn ASREngine + Send + Sync>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            asr_engine: Arc::new(RwLock::new(asr_engine)),
        }
    }

    pub async fn create_session(&self, context: AppContext) -> Uuid {
        let id = Uuid::new_v4();
        let (audio_tx, audio_rx) = mpsc::channel(1024);

        let session = DictationSession {
            id,
            start_time: chrono::Utc::now(),
            status: SessionStatus::Recording,
        };

        let state = SessionState {
            session,
            _app_context: context,
            audio_buffer: Vec::new(),
            audio_tx,
            audio_rx,
        };

        {
            let mut sessions = self.sessions.write().await;
            sessions.insert(id, state);
        }

        tracing::info!("Session created: {}", id);
        id
    }

    pub async fn append_audio(&self, session_id: Uuid, chunk: AudioChunk) -> bool {
        let mut sessions = self.sessions.write().await;
        if let Some(state) = sessions.get_mut(&session_id) {
            let _ = state.audio_tx.try_send(chunk.clone());
            state.audio_buffer.push(chunk);
            true
        } else {
            false
        }
    }

    pub async fn finalize_session(
        &self,
        session_id: Uuid,
    ) -> Option<(mpsc::Receiver<super::asr::TranscriptEvent>, tokio::task::JoinHandle<super::asr::TranscriptResult>)> {
        let session_state = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(&session_id)
        };

        let state = session_state?;

        {
            let mut sessions = self.sessions.write().await;
            if let Some(s) = sessions.get_mut(&session_id) {
                s.session.status = SessionStatus::Processing;
            }
        }

        let default_config = ASRConfig {
            model_path: None,
            language: Some("en".into()),
            hot_words: Vec::new(),
        };

        let mut engine = self.asr_engine.write().await;
        let _ = engine.initialize(&default_config);

        let (events_rx, task) = engine.transcribe_stream(state.audio_rx);

        drop(state.audio_tx);

        Some((events_rx, task))
    }

    pub async fn handle_transcript_result(
        &self,
        session_id: Uuid,
        result: super::asr::TranscriptResult,
    ) {
        let mut sessions = self.sessions.write().await;
        if let Some(state) = sessions.get_mut(&session_id) {
            state.session.status = SessionStatus::Done;
        }
        tracing::info!(
            "Session {} completed: {}",
            session_id,
            result.full_text
        );
    }
}
