use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
use x11rb::rust_connection::RustConnection;

use speak_up_core::AppContext;

use super::{ContextDetector, ContextDetectorState, ContextError};

pub struct LinuxContextDetector {
    conn: Option<RustConnection>,
    state: ContextDetectorState,
}

impl Default for LinuxContextDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl LinuxContextDetector {
    pub fn new() -> Self {
        let conn = x11rb::connect(None).ok().map(|(c, _)| c);
        Self { conn, state: ContextDetectorState::new() }
    }

    fn query_active_window(&self) -> Result<AppContext, ContextError> {
        let conn = self
            .conn
            .as_ref()
            .ok_or_else(|| ContextError::PlatformError("No X11 connection".into()))?;

        let screen = &conn.setup().roots[0];

        let active_cookie = conn
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")
            .map_err(|e| ContextError::PlatformError(e.to_string()))?;
        let active_atom =
            active_cookie.reply().map_err(|e| ContextError::PlatformError(e.to_string()))?;

        let wm_name_cookie = conn
            .intern_atom(false, b"_NET_WM_NAME")
            .map_err(|e| ContextError::PlatformError(e.to_string()))?;
        let wm_name_atom =
            wm_name_cookie.reply().map_err(|e| ContextError::PlatformError(e.to_string()))?.atom;

        let utf8_cookie = conn
            .intern_atom(false, b"UTF8_STRING")
            .map_err(|e| ContextError::PlatformError(e.to_string()))?;
        let utf8_atom =
            utf8_cookie.reply().map_err(|e| ContextError::PlatformError(e.to_string()))?.atom;

        let class_cookie = conn
            .intern_atom(false, b"WM_CLASS")
            .map_err(|e| ContextError::PlatformError(e.to_string()))?;
        let wm_class_atom =
            class_cookie.reply().map_err(|e| ContextError::PlatformError(e.to_string()))?.atom;

        let prop_cookie = conn
            .get_property(false, screen.root, active_atom.atom, AtomEnum::WINDOW, 0, 1)
            .map_err(|e| ContextError::PlatformError(e.to_string()))?;
        let active_prop =
            prop_cookie.reply().map_err(|e| ContextError::PlatformError(e.to_string()))?;

        if active_prop.value.len() < 4 {
            return Ok(default_context());
        }

        let window = u32::from_ne_bytes(active_prop.value[..4].try_into().unwrap_or([0; 4]));

        if window == 0 || window == screen.root {
            return Ok(default_context());
        }

        let title = conn
            .get_property(false, window, wm_name_atom, utf8_atom, 0, 1024)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| if r.value.is_empty() { None } else { String::from_utf8(r.value).ok() })
            .unwrap_or_default();

        let (executable, class) = conn
            .get_property(false, window, wm_class_atom, AtomEnum::STRING, 0, 1024)
            .ok()
            .and_then(|c| c.reply().ok())
            .and_then(|r| {
                let s = String::from_utf8(r.value).ok()?;
                let mut parts = s.split('\0');
                Some((
                    parts.next().unwrap_or("").to_string(),
                    parts.next().unwrap_or("").to_string(),
                ))
            })
            .unwrap_or_default();

        Ok(AppContext {
            window_title: title,
            executable_name: executable,
            window_class: class,
            profile_name: None,
        })
    }
}

fn default_context() -> AppContext {
    AppContext {
        window_title: String::new(),
        executable_name: String::new(),
        window_class: String::new(),
        profile_name: None,
    }
}

impl ContextDetector for LinuxContextDetector {
    fn get_active_window(&self) -> Result<AppContext, ContextError> {
        let ctx = self.query_active_window().unwrap_or_else(|_| default_context());
        Ok(ctx)
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
