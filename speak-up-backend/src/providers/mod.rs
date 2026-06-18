use std::sync::Arc;
use tokio::sync::RwLock;

use speak_up_core::ProviderConfig;

use crate::cleaner::{self, AICleaner};

pub struct ProviderManager {
    cleaner: Arc<RwLock<Box<dyn AICleaner>>>,
}

impl ProviderManager {
    pub fn new_with_defaults() -> Self {
        let cleaner: Box<dyn AICleaner> = Box::new(cleaner::LocalLLM::new());
        Self { cleaner: Arc::new(RwLock::new(cleaner)) }
    }

    pub fn new(cleaner_config: &Option<ProviderConfig>) -> Self {
        let cleaner = cleaner::build_cleaner(cleaner_config);
        Self { cleaner: Arc::new(RwLock::new(cleaner)) }
    }

    pub async fn get_cleaner(&self) -> tokio::sync::RwLockWriteGuard<'_, Box<dyn AICleaner>> {
        self.cleaner.write().await
    }

    pub async fn switch_cleaner(&self, config: &ProviderConfig) -> Result<(), String> {
        let config_str = serde_json::to_string(config).map_err(|e| e.to_string())?;
        let parsed: ProviderConfig =
            serde_json::from_str(&config_str).map_err(|e| e.to_string())?;
        let new_cleaner = cleaner::build_cleaner(&Some(parsed));

        let mut current = self.cleaner.write().await;
        current.shutdown();
        *current = new_cleaner;
        tracing::info!("Switched cleaner to {:?}", config.provider_type);
        Ok(())
    }

    pub async fn switch_cleaner_from_config(&self, config: &Option<ProviderConfig>) {
        if let Some(cfg) = config {
            let new_cleaner = cleaner::build_cleaner(&Some(cfg.clone()));
            let mut current = self.cleaner.write().await;
            current.shutdown();
            *current = new_cleaner;
        }
    }
}
