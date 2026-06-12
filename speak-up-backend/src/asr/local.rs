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
