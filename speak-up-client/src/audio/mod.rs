use speak_up_core::AudioChunk;

pub struct DeviceInfo {
    pub id: String,
    pub name: String,
}

pub trait AudioCapture: Send {
    fn enumerate_devices() -> Vec<DeviceInfo>;
    fn start_stream(&mut self, device_id: &str) -> Result<(), AudioError>;
    fn stop_stream(&mut self);
    fn set_audio_rx(&mut self, tx: tokio::sync::mpsc::Sender<AudioChunk>);
    fn current_level(&self) -> f32;
}

#[derive(Debug)]
pub enum AudioError {
    DeviceNotFound(String),
    StreamError(String),
    PermissionDenied,
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
