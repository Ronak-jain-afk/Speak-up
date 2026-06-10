pub enum HotkeyAction {
    StartRecording,
    StopRecording,
    ToggleRecording,
    RetypeLast,
}

pub trait HotkeyManager: Send {
    fn register(&mut self, combo: &str, action: HotkeyAction) -> Result<(), HotkeyError>;
    fn unregister_all(&mut self);
    fn poll_event(&mut self) -> Option<HotkeyAction>;
}

#[derive(Debug)]
pub enum HotkeyError {
    RegistrationFailed(String),
    PlatformUnsupported,
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
