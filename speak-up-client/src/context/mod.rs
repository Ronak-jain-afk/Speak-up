use speak_up_core::AppContext;

pub trait ContextDetector: Send {
    fn get_active_window(&self) -> Result<AppContext, ContextError>;
    fn poll(&mut self);
    fn last_context(&self) -> Option<AppContext>;
}

#[derive(Debug)]
pub enum ContextError {
    PlatformError(String),
    PermissionDenied,
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
