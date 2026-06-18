use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use speak_up_core::{AppContext, AudioChunk, DictationSession, SessionStatus};

use crate::asr::{ASREngine, ASRConfig};
use crate::profiles::ProfileManager;
use crate::dictionary::DictionaryManager;
use crate::providers::ProviderManager;

pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, SessionState>>>,
    asr_engine: Arc<RwLock<Box<dyn ASREngine + Send + Sync>>>,
    provider_mgr: Arc<ProviderManager>,
    profile_mgr: Arc<RwLock<ProfileManager>>,
    dict_mgr: Arc<DictionaryManager>,
}

struct SessionState {
    session: DictationSession,
    _app_context: AppContext,
    audio_buffer: Vec<AudioChunk>,
    audio_tx: mpsc::Sender<AudioChunk>,
    audio_rx: mpsc::Receiver<AudioChunk>,
}

impl SessionManager {
    pub fn new(
        asr_engine: Box<dyn ASREngine + Send + Sync>,
        provider_mgr: Arc<ProviderManager>,
    ) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            asr_engine: Arc::new(RwLock::new(asr_engine)),
            provider_mgr,
            profile_mgr: Arc::new(RwLock::new(ProfileManager::new())),
            dict_mgr: Arc::new(DictionaryManager::new()),
        }
    }

    pub fn provider_manager(&self) -> &Arc<ProviderManager> {
        &self.provider_mgr
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
    ) -> Option<(
        mpsc::Receiver<super::asr::TranscriptEvent>,
        tokio::task::JoinHandle<super::asr::TranscriptResult>,
    )> {
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

    pub async fn clean_transcript(
        &self,
        raw_text: &str,
    ) -> String {
        let profile = {
            let mgr = self.profile_mgr.read().await;
            mgr.get_fallback()
        };
        let dictionary = self.dict_mgr.get_all().await;

        let cleaner = self.provider_mgr.get_cleaner().await;
        match cleaner.clean(raw_text, &profile, &dictionary) {
            Ok(cleaned) => {
                tracing::debug!("Transcript cleaned: {} -> {}", raw_text, cleaned);
                cleaned
            }
            Err(e) => {
                tracing::error!("Cleaner failed: {}, using raw transcript", e);
                raw_text.to_string()
            }
        }
    }
}
