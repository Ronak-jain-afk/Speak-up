use speak_up_core::{DictionaryEntry, Profile};

pub trait AICleaner: Send + Sync {
    fn clean(
        &self,
        transcript: &str,
        profile: &Profile,
        dictionary: &[DictionaryEntry],
    ) -> Result<String, CleanerError>;
    fn shutdown(&mut self);
}

#[derive(Debug)]
pub enum CleanerError {
    Initialization(String),
    Inference(String),
    ModelNotFound(String),
}

pub mod cloud;
pub mod local;
