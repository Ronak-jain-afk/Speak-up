use speak_up_core::AppContext;

use super::{ContextDetector, ContextDetectorState, ContextError};

pub struct MacOSContextDetector {
    state: ContextDetectorState,
}

impl MacOSContextDetector {
    pub fn new() -> Self {
        Self { state: ContextDetectorState::new() }
    }
}

impl ContextDetector for MacOSContextDetector {
    fn get_active_window(&self) -> Result<AppContext, ContextError> {
        tracing::warn!("macOS context detection not implemented, using empty context");
        Ok(AppContext {
            window_title: String::new(),
            executable_name: String::new(),
            window_class: String::new(),
            profile_name: None,
        })
    }

    fn poll(&mut self) {
        if self.state.should_poll() {
            if let Ok(ctx) = self.get_active_window() {
                self.state.update(ctx);
            }
        }
    }

    fn last_context(&self) -> Option<AppContext> {
        self.state.last()
    }
}
