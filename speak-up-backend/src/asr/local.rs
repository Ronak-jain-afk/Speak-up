use speak_up_core::AudioChunk;
use tokio::sync::mpsc;

use super::*;

pub struct MockWhisper;

impl MockWhisper {
    pub fn new() -> Self {
        Self
    }
}

impl ASREngine for MockWhisper {
    fn initialize(&mut self, _config: &ASRConfig) -> Result<(), ASRError> {
        tracing::info!("MockWhisper initialized");
        Ok(())
    }

    fn transcribe_stream(
        &mut self,
        mut rx: mpsc::Receiver<AudioChunk>,
    ) -> (mpsc::Receiver<TranscriptEvent>, TranscriptionTask) {
        let (tx, events_rx) = mpsc::channel(64);

        let task = tokio::spawn(async move {
            let mut total_samples: usize = 0;
            let mut sample_rate: u32 = 16000;

            while let Some(chunk) = rx.recv().await {
                total_samples += chunk.data.len() / 2;
                sample_rate = chunk.sample_rate;
            }

            let duration_ms = if sample_rate > 0 {
                (total_samples as u64 * 1000) / sample_rate as u64
            } else {
                0
            };

            let full_text = format!(
                "[mock transcript: {} samples at {} Hz ({}.{}s)]",
                total_samples,
                sample_rate,
                duration_ms / 1000,
                duration_ms % 1000 / 100,
            );

            let segment = TranscriptSegment {
                text: full_text.clone(),
                is_final: true,
                confidence: Some(0.95),
                timestamp: chrono::Utc::now(),
            };

            let _ = tx.send(TranscriptEvent {
                segment,
                is_final: true,
            }).await;

            TranscriptResult {
                segments: Vec::new(),
                full_text,
            }
        });

        (events_rx, task)
    }

    fn finalize(&mut self) -> TranscriptResult {
        TranscriptResult {
            segments: Vec::new(),
            full_text: String::new(),
        }
    }

    fn shutdown(&mut self) {
        tracing::info!("MockWhisper shutdown");
    }
}

impl Default for MockWhisper {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// LocalWhisper — whisper.cpp via whisper-rs, runs entirely on-device
// Requires the "local-asr" feature (enabled by default in release builds)
// ---------------------------------------------------------------------------

#[cfg(feature = "local-asr")]
pub use local_whisper_impl::LocalWhisper;

#[cfg(not(feature = "local-asr"))]
pub struct LocalWhisper;

#[cfg(not(feature = "local-asr"))]
impl LocalWhisper {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(not(feature = "local-asr"))]
impl ASREngine for LocalWhisper {
    fn initialize(&mut self, _config: &ASRConfig) -> Result<(), ASRError> {
        Err(ASRError::Initialization(
            "LocalWhisper requires building with --features local-asr (needs whisper-rs + clang)".into(),
        ))
    }

    fn transcribe_stream(
        &mut self,
        _rx: mpsc::Receiver<AudioChunk>,
    ) -> (mpsc::Receiver<TranscriptEvent>, TranscriptionTask) {
        let (_tx, rx) = mpsc::channel(64);
        let task = tokio::spawn(async move {
            TranscriptResult { segments: Vec::new(), full_text: String::new() }
        });
        (rx, task)
    }

    fn finalize(&mut self) -> TranscriptResult {
        TranscriptResult { segments: Vec::new(), full_text: String::new() }
    }

    fn shutdown(&mut self) {}
}

#[cfg(feature = "local-asr")]
mod local_whisper_impl {
    use std::sync::Mutex;

    use speak_up_core::AudioChunk;
    use tokio::sync::mpsc;
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    use super::*;
    use crate::models;

    pub struct LocalWhisper {
        ctx: Option<Mutex<WhisperContext>>,
        model_path: Option<String>,
        language: Option<String>,
        n_threads: i32,
    }

    impl LocalWhisper {
        pub fn new() -> Self {
            Self {
                ctx: None,
                model_path: None,
                language: None,
                n_threads: 4,
            }
        }

        fn find_model(model_path: &Option<String>) -> Result<std::path::PathBuf, ASRError> {
            if let Some(path) = model_path {
                let p = std::path::PathBuf::from(path);
                if p.exists() {
                    return Ok(p);
                }
                return Err(ASRError::ModelNotFound(format!(
                    "Specified model not found: {}",
                    path
                )));
            }

            let dir = models::ModelDownloader::ensure_models_dir();
            for spec in models::MODELS {
                let p = dir.join(spec.filename);
                if p.exists() {
                    return Ok(p);
                }
            }

            Err(ASRError::ModelNotFound(
                "No whisper model found in models directory. \
                 Please place ggml-*.bin in the models directory, \
                 or configure a model_path in settings."
                    .into(),
            ))
        }
    }

    impl ASREngine for LocalWhisper {
        fn initialize(&mut self, config: &ASRConfig) -> Result<(), ASRError> {
            let model_path = Self::find_model(&config.model_path)?;
            self.model_path = Some(model_path.to_string_lossy().to_string());
            self.language = config.language.clone();

            tracing::info!("Loading whisper model from {} ...", model_path.display());

            let ctx = WhisperContext::new_with_params(
                model_path.to_str().unwrap(),
                WhisperContextParameters::default(),
            )
            .map_err(|e| {
                ASRError::Initialization(format!("Failed to load whisper model: {}", e))
            })?;

            self.ctx = Some(Mutex::new(ctx));
            tracing::info!("LocalWhisper initialized successfully");
            Ok(())
        }

        fn transcribe_stream(
            &mut self,
            mut rx: mpsc::Receiver<AudioChunk>,
        ) -> (mpsc::Receiver<TranscriptEvent>, TranscriptionTask) {
            let (tx, events_rx) = mpsc::channel(64);
            let model_path = self.model_path.clone();
            let language = self.language.clone();
            let n_threads = self.n_threads;

            let task = tokio::spawn(async move {
                let mut all_pcm_i16: Vec<i16> = Vec::new();

                while let Some(chunk) = rx.recv().await {
                    if chunk.data.len() >= 2 {
                        let samples: Vec<i16> = chunk
                            .data
                            .chunks_exact(2)
                            .map(|b| i16::from_le_bytes([b[0], b[1]]))
                            .collect();
                        all_pcm_i16.extend(samples);
                    }
                }

                if all_pcm_i16.is_empty() {
                    tracing::warn!("LocalWhisper: no audio data received");
                    return TranscriptResult {
                        segments: Vec::new(),
                        full_text: String::new(),
                    };
                }

                let audio_f32: Vec<f32> = all_pcm_i16
                    .iter()
                    .map(|&s| s as f32 / 32768.0)
                    .collect();

                let path = match &model_path {
                    Some(p) => p.clone(),
                    None => {
                        tracing::error!("LocalWhisper: no model path, was initialize() called?");
                        return TranscriptResult {
                            segments: Vec::new(),
                            full_text: String::new(),
                        };
                    }
                };

                tracing::info!(
                    "LocalWhisper: running inference on {} samples ({:.1}s)",
                    audio_f32.len(),
                    audio_f32.len() as f64 / 16000.0
                );

                let ctx = match WhisperContext::new_with_params(
                    &path,
                    WhisperContextParameters::default(),
                ) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::error!("LocalWhisper: failed to load model: {}", e);
                        return TranscriptResult {
                            segments: Vec::new(),
                            full_text: String::new(),
                        };
                    }
                };

                let result = tokio::task::block_in_place(|| {
                    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                    params.set_n_threads(n_threads);
                    params.set_language(language.as_deref().unwrap_or("en"));
                    params.set_translate(false);
                    params.set_print_progress(false);
                    params.set_print_realtime(false);
                    params.set_print_timestamps(false);
                    params.set_no_context(true);

                    let mut state = match ctx.create_state() {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!("LocalWhisper: failed to create state: {}", e);
                            return TranscriptResult {
                                segments: Vec::new(),
                                full_text: String::new(),
                            };
                        }
                    };

                    if let Err(e) = state.full(params, &audio_f32) {
                        tracing::error!("LocalWhisper: inference failed: {}", e);
                        return TranscriptResult {
                            segments: Vec::new(),
                            full_text: String::new(),
                        };
                    }

                    let num_segments = state.full_n_segments();
                    tracing::info!("LocalWhisper: got {} segments", num_segments);

                    let mut full_text = String::new();
                    for i in 0..num_segments {
                        if let Ok(text) = state.full_get_segment_text(i) {
                            if !full_text.is_empty() {
                                full_text.push(' ');
                            }
                            full_text.push_str(text.trim());
                        }
                    }

                    if !full_text.is_empty() {
                        let _ = tx.try_send(TranscriptEvent {
                            segment: TranscriptSegment {
                                text: full_text.clone(),
                                is_final: true,
                                confidence: Some(1.0),
                                timestamp: chrono::Utc::now(),
                            },
                            is_final: true,
                        });
                    }

                    TranscriptResult {
                        segments: Vec::new(),
                        full_text,
                    }
                });

                result
            });

            (events_rx, task)
        }

        fn finalize(&mut self) -> TranscriptResult {
            TranscriptResult {
                segments: Vec::new(),
                full_text: String::new(),
            }
        }

        fn shutdown(&mut self) {
            self.ctx = None;
            tracing::info!("LocalWhisper shutdown");
        }
    }
}
