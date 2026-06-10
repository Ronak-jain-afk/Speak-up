pub mod audio;
pub mod context;
pub mod hotkeys;
pub mod injection;
pub mod overlay;
pub mod settings;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    tracing::info!("Speak Up client v{} starting", env!("CARGO_PKG_VERSION"));
}
