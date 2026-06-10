use speak_up_core::{ProviderConfig, ProviderType};

#[derive(Default)]
pub struct ProviderManager;

impl ProviderManager {
    pub fn new() -> Self {
        Self
    }

    pub fn switch_provider(&mut self, _provider_type: ProviderType, _config: ProviderConfig) {
        unimplemented!("Phase 8")
    }
}
