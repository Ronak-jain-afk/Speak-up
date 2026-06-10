pub mod asr;
pub mod cleaner;
pub mod dictionary;
pub mod history;
pub mod profiles;
pub mod providers;
pub mod server;
pub mod session;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Speak Up backend v{} starting", env!("CARGO_PKG_VERSION"));
}
