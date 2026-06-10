use speak_up_core::DictionaryEntry;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Default)]
pub struct DictionaryManager {
    entries: Arc<RwLock<Vec<DictionaryEntry>>>,
}

impl DictionaryManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn get_all(&self) -> Vec<DictionaryEntry> {
        self.entries.read().await.clone()
    }

    pub async fn set_entries(&self, entries: Vec<DictionaryEntry>) {
        let mut w = self.entries.write().await;
        *w = entries;
    }

    pub fn format_for_prompt(&self, entries: &[DictionaryEntry]) -> String {
        entries
            .iter()
            .map(|e| {
                format!("Always write '{}' when you hear '{}'.", e.written_form, e.spoken_form)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
