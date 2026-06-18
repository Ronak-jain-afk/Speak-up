use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, RwLock};
use uuid::Uuid;

use speak_up_core::{AppContext, AudioChunk, DictationSession, SessionStatus};

use crate::asr::{ASREngine, ASRConfig, build_asr_engine};
use crate::history::HistoryStore;
use crate::profiles::ProfileManager;
use crate::dictionary::DictionaryManager;
use crate::providers::ProviderManager;

pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, SessionState>>>,
    asr_engine: Arc<RwLock<Box<dyn ASREngine + Send + Sync>>>,
    provider_mgr: Arc<ProviderManager>,
    profile_mgr: Arc<RwLock<ProfileManager>>,
    dict_mgr: Arc<DictionaryManager>,
    history: Arc<Mutex<Option<HistoryStore>>>,
}

struct SessionState {
    session: DictationSession,
    app_context: AppContext,
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
            history: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_history_store(&self, store: HistoryStore) {
        if let Ok(mut h) = self.history.lock() {
            *h = Some(store);
        }
    }

    pub fn provider_manager(&self) -> &Arc<ProviderManager> {
        &self.provider_mgr
    }

    pub fn profile_manager(&self) -> &Arc<RwLock<ProfileManager>> {
        &self.profile_mgr
    }

    pub fn dictionary_manager(&self) -> &Arc<DictionaryManager> {
        &self.dict_mgr
    }

    pub async fn reload_settings(&self) {
        let settings = crate::load_settings();
        {
            let mut mgr = self.profile_mgr.write().await;
            mgr.load_rules(&settings.profiles);
        }
        self.dict_mgr.set_entries(settings.dictionary).await;

        {
            let mut engine = self.asr_engine.write().await;
            engine.shutdown();
            *engine = build_asr_engine(&settings.asr_provider);
            tracing::info!("ASR engine reloaded from settings");
        }

        tracing::info!("Settings reloaded: profiles, dictionary, and ASR engine updated");
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
            app_context: context,
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
        chrono::DateTime<chrono::Utc>,
        AppContext,
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

        let start_time = state.session.start_time;
        let app_context = state.app_context.clone();

        let default_config = ASRConfig {
            model_path: None,
            language: Some("en".into()),
            hot_words: Vec::new(),
        };

        let mut engine = self.asr_engine.write().await;
        let _ = engine.initialize(&default_config);

        let (events_rx, task) = engine.transcribe_stream(state.audio_rx);

        drop(state.audio_tx);

        Some((events_rx, task, start_time, app_context))
    }

    pub async fn clean_transcript(
        &self,
        raw_text: &str,
        app_context: Option<&AppContext>,
    ) -> String {
        let (profile, matched_name) = {
            let mgr = self.profile_mgr.read().await;
            let matched = app_context.and_then(|ctx| mgr.match_profile(ctx));
            match matched {
                Some(p) => (p.clone(), Some(p.name.clone())),
                None => (mgr.get_fallback(), None),
            }
        };
        if let Some(ref name) = matched_name {
            tracing::debug!("Using profile '{}' for cleaning", name);
        }

        let dictionary = self.dict_mgr.get_all().await;

        let cleaner = self.provider_mgr.get_cleaner().await;
        let cleaned = match cleaner.clean(raw_text, &profile, &dictionary) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Cleaner failed: {}, using raw transcript", e);
                raw_text.to_string()
            }
        };

        let mgr = self.profile_mgr.read().await;
        let result = mgr.apply_post_process(&cleaned, &profile);
        tracing::debug!("Transcript cleaned: {} -> {} (post-processed: {})", raw_text, cleaned, result);
        result
    }

    pub async fn write_history(
        &self,
        raw_text: &str,
        cleaned_text: &str,
        start_time: chrono::DateTime<chrono::Utc>,
        app_context: Option<&str>,
    ) {
        let timestamp = chrono::Utc::now().to_rfc3339();
        let duration_ms = (chrono::Utc::now() - start_time)
            .num_milliseconds()
            .max(0);

        if let Ok(history) = self.history.lock() {
            if let Some(ref h) = *history {
                if let Err(e) = h.insert_entry(
                    &timestamp,
                    raw_text,
                    cleaned_text,
                    app_context,
                    None,
                    None,
                    None,
                    Some(duration_ms),
                ) {
                    tracing::error!("Failed to write history: {}", e);
                }
            }
        }
    }

    pub async fn query_history(
        &self,
        limit: usize,
        offset: usize,
        search_term: Option<String>,
    ) -> (Vec<speak_up_core::ipc::DictationEntry>, usize) {
        if let Ok(history) = self.history.lock() {
            if let Some(ref h) = *history {
                return h.query_recent(limit, offset, search_term.as_deref());
            }
        }
        (Vec::new(), 0)
    }

    pub async fn get_last_dictation(
        &self,
    ) -> Option<speak_up_core::ipc::DictationEntry> {
        if let Ok(history) = self.history.lock() {
            if let Some(ref h) = *history {
                return h.get_last_dictation();
            }
        }
        None
    }
}
