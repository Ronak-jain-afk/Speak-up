use speak_up_core::AppContext;

use super::{ContextDetector, ContextDetectorState, ContextError};

pub struct WindowsContextDetector {
    state: ContextDetectorState,
}

impl WindowsContextDetector {
    pub fn new() -> Self {
        Self { state: ContextDetectorState::new() }
    }
}

impl ContextDetector for WindowsContextDetector {
    fn get_active_window(&self) -> Result<AppContext, ContextError> {
        unimplemented!("Phase 10")
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
