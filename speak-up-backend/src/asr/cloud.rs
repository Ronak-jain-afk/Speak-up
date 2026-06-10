use speak_up_core::AudioChunk;
use tokio::sync::mpsc;

use super::*;

#[derive(Default)]
pub struct CloudWhisper;

impl CloudWhisper {
    pub fn new() -> Self {
        Self
    }
}

impl ASREngine for CloudWhisper {
    fn initialize(&mut self, _config: &ASRConfig) -> Result<(), ASRError> {
        Ok(())
    }

    fn transcribe_stream(
        &mut self,
        _rx: mpsc::Receiver<AudioChunk>,
    ) -> (mpsc::Receiver<TranscriptEvent>, TranscriptionTask) {
        unimplemented!("Phase 8")
    }

    fn finalize(&mut self) -> TranscriptResult {
        unimplemented!("Phase 8")
    }

    fn shutdown(&mut self) {}
}
