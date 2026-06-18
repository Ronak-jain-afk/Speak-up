use speak_up_core::AudioChunk;
use tokio::sync::mpsc;

use super::*;

pub struct CloudWhisper {
    api_key: String,
    model: String,
    language: Option<String>,
}

impl CloudWhisper {
    pub fn new(api_key: String, model: Option<String>, language: Option<String>) -> Self {
        Self {
            api_key,
            model: model.unwrap_or_else(|| "whisper-1".into()),
            language,
        }
    }
}

impl ASREngine for CloudWhisper {
    fn initialize(&mut self, _config: &ASRConfig) -> Result<(), ASRError> {
        if self.api_key.is_empty() || self.api_key == "sk-..." {
            return Err(ASRError::Initialization(
                "No valid OpenAI API key configured".into(),
            ));
        }
        tracing::info!("CloudWhisper initialized with model {}", self.model);
        Ok(())
    }

    fn transcribe_stream(
        &mut self,
        mut rx: mpsc::Receiver<AudioChunk>,
    ) -> (mpsc::Receiver<TranscriptEvent>, TranscriptionTask) {
        let (tx, events_rx) = mpsc::channel(64);
        let api_key = self.api_key.clone();
        let model = self.model.clone();
        let language = self.language.clone();

        let task = tokio::spawn(async move {
            let mut all_pcm: Vec<u8> = Vec::new();
            let mut sample_rate: u32 = 16000;
            let mut channels: u16 = 1;

            while let Some(chunk) = rx.recv().await {
                all_pcm.extend_from_slice(&chunk.data);
                sample_rate = chunk.sample_rate;
                channels = chunk.channels;
            }

            if all_pcm.is_empty() {
                return TranscriptResult {
                    segments: Vec::new(),
                    full_text: String::new(),
                };
            }

            let wav_data = build_wav(&all_pcm, sample_rate, channels);

            let client = reqwest::Client::new();
            let part = reqwest::multipart::Part::bytes(wav_data)
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .unwrap();

            let mut form = reqwest::multipart::Form::new()
                .part("file", part)
                .text("model", model.clone());

            if let Some(ref lang) = language {
                form = form.text("language", lang.clone());
            }

            let response = match client
                .post("https://api.openai.com/v1/audio/transcriptions")
                .header("Authorization", format!("Bearer {}", api_key))
                .multipart(form)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("OpenAI API request failed: {}", e);
                    return TranscriptResult {
                        segments: Vec::new(),
                        full_text: String::new(),
                    };
                }
            };

            let status = response.status();
            let body: serde_json::Value = match response.json().await {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!("Failed to parse OpenAI response: {}", e);
                    return TranscriptResult {
                        segments: Vec::new(),
                        full_text: String::new(),
                    };
                }
            };

            if !status.is_success() {
                tracing::error!(
                    "OpenAI API error ({}): {:?}",
                    status,
                    body.get("error").and_then(|e| e.get("message"))
                );
                return TranscriptResult {
                    segments: Vec::new(),
                    full_text: String::new(),
                };
            }

            let text = body
                .get("text")
                .and_then(|t| t.as_str())
                .unwrap_or("")
                .to_string();

            if !text.is_empty() {
                let segment = TranscriptSegment {
                    text: text.clone(),
                    is_final: true,
                    confidence: None,
                    timestamp: chrono::Utc::now(),
                };
                let _ = tx
                    .send(TranscriptEvent {
                        segment,
                        is_final: true,
                    })
                    .await;
            }

            TranscriptResult {
                segments: Vec::new(),
                full_text: text,
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
        tracing::info!("CloudWhisper shutdown");
    }
}

fn build_wav(pcm_data: &[u8], sample_rate: u32, channels: u16) -> Vec<u8> {
    let bytes_per_sample: u16 = 2;
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;
    let block_align = channels * bytes_per_sample;
    let data_size = pcm_data.len() as u32;
    let file_size = 36 + data_size;

    let mut wav = Vec::with_capacity(44 + pcm_data.len());

    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&file_size.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bytes_per_sample.to_le_bytes());

    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    wav.extend_from_slice(pcm_data);

    wav
}
