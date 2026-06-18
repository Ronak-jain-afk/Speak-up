use std::time::Instant;

use arboard::Clipboard;
use enigo::{Direction, Enigo, Key, Keyboard, Settings};

use speak_up_core::PostProcessRule;

use super::{InjectionError, TextInjector};

pub struct MacOSTextInjector {
    enigo: Enigo,
    clipboard: Option<Clipboard>,
    saved_clipboard: Option<String>,
    saved_at: Option<Instant>,
    last_injected: Option<String>,
}

impl MacOSTextInjector {
    pub fn new() -> Result<Self, InjectionError> {
        let enigo = Enigo::new(&Settings::default())
            .map_err(|e| InjectionError::Keystroke(format!("enigo init: {}", e)))?;
        let clipboard = Clipboard::new().ok();
        Ok(Self {
            enigo,
            clipboard,
            saved_clipboard: None,
            saved_at: None,
            last_injected: None,
        })
    }

    fn send_paste_shortcut(&mut self) -> Result<(), InjectionError> {
        self.enigo
            .key(Key::Meta, Direction::Press)
            .map_err(|e| InjectionError::Keystroke(format!("Cmd press: {}", e)))?;
        self.enigo
            .key(Key::Unicode('v'), Direction::Click)
            .map_err(|e| InjectionError::Keystroke(format!("v click: {}", e)))?;
        self.enigo
            .key(Key::Meta, Direction::Release)
            .map_err(|e| InjectionError::Keystroke(format!("Cmd release: {}", e)))?;
        Ok(())
    }
}

impl TextInjector for MacOSTextInjector {
    fn inject_text(&mut self, text: &str) -> Result<(), InjectionError> {
        if self.clipboard.is_some() {
            let cb = self.clipboard.as_mut().unwrap();
            if cb.set_text(text).is_ok() && self.send_paste_shortcut().is_ok() {
                self.last_injected = Some(text.to_string());
                return Ok(());
            }
        }
        self.enigo
            .text(text)
            .map_err(|e| InjectionError::Keystroke(format!("enigo text: {}", e)))?;
        self.last_injected = Some(text.to_string());
        Ok(())
    }

    fn inject_with_post_process(
        &mut self,
        text: &str,
        rules: &[PostProcessRule],
    ) -> Result<(), InjectionError> {
        let processed = apply_rules(text, rules);
        self.inject_text(&processed)
    }

    fn retype_last(&mut self) -> Result<(), InjectionError> {
        let text = self
            .last_injected
            .clone()
            .ok_or_else(|| InjectionError::Keystroke("nothing to retype".into()))?;
        self.inject_text(&text)
    }

    fn save_clipboard(&mut self) {
        if let Some(ref mut cb) = self.clipboard {
            self.saved_clipboard = cb.get_text().ok();
            self.saved_at = Some(Instant::now());
        }
    }

    fn restore_clipboard(&mut self) {
        if let Some(saved) = self.saved_at {
            if saved.elapsed() > std::time::Duration::from_secs(1) {
                return;
            }
        }
        if let Some(ref mut cb) = self.clipboard {
            if let Some(ref saved) = self.saved_clipboard {
                let _ = cb.set_text(saved);
            }
        }
        self.saved_clipboard = None;
        self.saved_at = None;
    }
}

fn apply_rules(text: &str, rules: &[PostProcessRule]) -> String {
    let mut result = text.to_string();
    for rule in rules {
        match rule {
            PostProcessRule::PrefixSpace => {
                if !result.starts_with(' ') {
                    result.insert(0, ' ');
                }
            }
            PostProcessRule::TrimWhitespace => {
                result = result.trim().to_string();
            }
            PostProcessRule::CapitalizeFirst => {
                if let Some(c) = result.chars().next() {
                    if c.is_lowercase() {
                        let capitalized: String = c.to_uppercase().collect();
                        result = capitalized + &result[c.len_utf8()..];
                    }
                }
            }
            PostProcessRule::PreserveLineBreaks => {}
        }
    }
    result
}
