use super::*;

#[derive(Default)]
pub struct LocalLLM;

impl LocalLLM {
    pub fn new() -> Self {
        Self
    }
}

impl AICleaner for LocalLLM {
    fn clean(
        &self,
        _transcript: &str,
        _profile: &Profile,
        _dictionary: &[DictionaryEntry],
    ) -> Result<String, CleanerError> {
        unimplemented!("Phase 8")
    }

    fn shutdown(&mut self) {}
}
