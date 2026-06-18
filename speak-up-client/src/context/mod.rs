use std::time::Instant;

use speak_up_core::AppContext;

pub trait ContextDetector: Send {
    fn get_active_window(&self) -> Result<AppContext, ContextError>;
    fn poll(&mut self);
    fn last_context(&self) -> Option<AppContext>;
}

pub struct ContextDetectorState {
    last: Option<AppContext>,
    last_poll: Option<Instant>,
    poll_interval: std::time::Duration,
}

impl Default for ContextDetectorState {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextDetectorState {
    pub fn new() -> Self {
        Self { last: None, last_poll: None, poll_interval: std::time::Duration::from_secs(2) }
    }

    pub fn should_poll(&self) -> bool {
        match self.last_poll {
            None => true,
            Some(t) => t.elapsed() >= self.poll_interval,
        }
    }

    pub fn update(&mut self, context: AppContext) {
        self.last = Some(context);
        self.last_poll = Some(Instant::now());
    }

    pub fn last(&self) -> Option<AppContext> {
        self.last.clone()
    }
}

#[derive(Debug)]
pub enum ContextError {
    PlatformError(String),
    PermissionDenied,
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "linux")]
pub use linux::LinuxContextDetector as DefaultContextDetector;

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos::MacOSContextDetector as DefaultContextDetector;

#[cfg(target_os = "windows")]
pub mod windows;
#[cfg(target_os = "windows")]
pub use windows::WindowsContextDetector as DefaultContextDetector;
