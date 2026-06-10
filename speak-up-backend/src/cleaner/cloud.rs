use super::*;

#[derive(Default)]
pub struct CloudCleaner;

impl CloudCleaner {
    pub fn new() -> Self {
        Self
    }
}

impl AICleaner for CloudCleaner {
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
