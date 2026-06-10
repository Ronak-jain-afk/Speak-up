pub struct OverlayConfig {
    pub position: OverlayPosition,
    pub opacity: f32,
}

pub enum OverlayPosition {
    NearCursor,
    BottomRight,
    TopRight,
}

pub struct OverlayState {
    pub is_visible: bool,
    pub is_recording: bool,
    pub is_processing: bool,
    pub audio_level: f32,
    pub transcript: String,
}

pub fn run_overlay_loop(_config: OverlayConfig) {
    unimplemented!("Phase 4")
}
