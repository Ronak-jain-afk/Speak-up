pub mod asr;
pub mod cleaner;
pub mod dictionary;
pub mod history;
pub mod profiles;
pub mod providers;
pub mod server;
pub mod session;

use std::sync::Arc;

use speak_up_core::Settings;

pub async fn run_async(port: u16) {
    tracing::info!("Speak Up backend v{} starting", env!("CARGO_PKG_VERSION"));

    let asr_engine: Box<dyn asr::ASREngine + Send + Sync> =
        Box::new(asr::local::MockWhisper::new());

    let settings = load_settings();
    let provider_mgr = Arc::new(providers::ProviderManager::new(
        &settings.cleaner_provider,
    ));

    let session_manager = Arc::new(session::SessionManager::new(asr_engine, provider_mgr));

    server::start_server(port, session_manager).await;
}

pub fn run() {
    run_with_port(9876);
}

pub fn run_with_port(port: u16) {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set tracing subscriber");

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(run_async(port));
}

fn load_settings() -> Settings {
    let config_dir = dirs::config_dir()
        .map(|p| p.join("speak-up"))
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config/speak-up"));
    let path = config_dir.join("settings.json");
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
            tracing::warn!("Failed to parse settings ({}), using defaults", e);
            Settings::default()
        }),
        Err(_) => {
            tracing::info!("No settings file found, using defaults");
            Settings::default()
        }
    }
}


