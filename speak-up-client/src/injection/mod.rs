use speak_up_core::PostProcessRule;

pub trait TextInjector: Send {
    fn inject_text(&mut self, text: &str) -> Result<(), InjectionError>;
    fn inject_with_post_process(
        &mut self,
        text: &str,
        rules: &[PostProcessRule],
    ) -> Result<(), InjectionError>;
    fn retype_last(&mut self) -> Result<(), InjectionError>;
    fn save_clipboard(&mut self);
    fn restore_clipboard(&mut self);
}

#[derive(Debug)]
pub enum InjectionError {
    Clipboard(String),
    Keystroke(String),
    PermissionDenied,
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
