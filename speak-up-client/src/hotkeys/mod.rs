use std::collections::HashMap;

use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyManager as GHKManager,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HotkeyAction {
    StartRecording,
    StopRecording,
    ToggleRecording,
    RetypeLast,
}

pub struct HotkeyManager {
    manager: GHKManager,
    action_map: HashMap<u32, HotkeyAction>,
    receiver: crossbeam_channel::Receiver<global_hotkey::GlobalHotKeyEvent>,
    registered: Vec<(String, HotkeyAction)>,
}

impl HotkeyManager {
    pub fn new() -> Result<Self, HotkeyError> {
        let manager =
            GHKManager::new().map_err(|e| HotkeyError::RegistrationFailed(e.to_string()))?;
        let receiver = global_hotkey::GlobalHotKeyEvent::receiver().clone();
        Ok(Self { manager, action_map: HashMap::new(), receiver, registered: Vec::new() })
    }

    pub fn register(&mut self, combo: &str, action: HotkeyAction) -> Result<(), HotkeyError> {
        let hotkey = parse_hotkey(combo)?;
        let id = hotkey.id();
        self.manager
            .register(hotkey)
            .map_err(|e| HotkeyError::RegistrationFailed(e.to_string()))?;
        self.action_map.insert(id, action);
        self.registered.push((combo.to_string(), action));
        Ok(())
    }

    pub fn unregister_all(&mut self) {
        for (combo, _) in self.registered.drain(..) {
            if let Ok(hotkey) = parse_hotkey(&combo) {
                let _ = self.manager.unregister(hotkey);
            }
        }
        self.action_map.clear();
    }

    pub fn poll_event(&mut self) -> Option<HotkeyAction> {
        while let Ok(event) = self.receiver.try_recv() {
            if let Some(action) = self.action_map.get(&event.id()) {
                return Some(*action);
            }
        }
        None
    }
}

fn parse_hotkey(combo: &str) -> Result<HotKey, HotkeyError> {
    let parts: Vec<&str> = combo.split('+').collect();
    let mut modifiers = Modifiers::empty();
    let mut code = None;

    for part in parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= Modifiers::CONTROL,
            "shift" => modifiers |= Modifiers::SHIFT,
            "alt" | "option" => modifiers |= Modifiers::ALT,
            "meta" | "cmd" | "command" | "win" | "super" => modifiers |= Modifiers::META,
            other => {
                code = Some(key_from_str(other).ok_or_else(|| {
                    HotkeyError::RegistrationFailed(format!("Unknown key: {}", other))
                })?);
            }
        }
    }

    let code =
        code.ok_or_else(|| HotkeyError::RegistrationFailed("No key specified in combo".into()))?;
    Ok(HotKey::new(Some(modifiers), code))
}

fn key_from_str(s: &str) -> Option<Code> {
    Some(match s.to_lowercase().as_str() {
        "space" => Code::Space,
        "enter" | "return" => Code::Enter,
        "tab" => Code::Tab,
        "escape" | "esc" => Code::Escape,
        "backspace" => Code::Backspace,
        "delete" => Code::Delete,
        "home" => Code::Home,
        "end" => Code::End,
        "pageup" => Code::PageUp,
        "pagedown" => Code::PageDown,
        "up" => Code::ArrowUp,
        "down" => Code::ArrowDown,
        "left" => Code::ArrowLeft,
        "right" => Code::ArrowRight,
        "0" => Code::Digit0,
        "1" => Code::Digit1,
        "2" => Code::Digit2,
        "3" => Code::Digit3,
        "4" => Code::Digit4,
        "5" => Code::Digit5,
        "6" => Code::Digit6,
        "7" => Code::Digit7,
        "8" => Code::Digit8,
        "9" => Code::Digit9,
        "a" => Code::KeyA,
        "b" => Code::KeyB,
        "c" => Code::KeyC,
        "d" => Code::KeyD,
        "e" => Code::KeyE,
        "f" => Code::KeyF,
        "g" => Code::KeyG,
        "h" => Code::KeyH,
        "i" => Code::KeyI,
        "j" => Code::KeyJ,
        "k" => Code::KeyK,
        "l" => Code::KeyL,
        "m" => Code::KeyM,
        "n" => Code::KeyN,
        "o" => Code::KeyO,
        "p" => Code::KeyP,
        "q" => Code::KeyQ,
        "r" => Code::KeyR,
        "s" => Code::KeyS,
        "t" => Code::KeyT,
        "u" => Code::KeyU,
        "v" => Code::KeyV,
        "w" => Code::KeyW,
        "x" => Code::KeyX,
        "y" => Code::KeyY,
        "z" => Code::KeyZ,
        "f1" => Code::F1,
        "f2" => Code::F2,
        "f3" => Code::F3,
        "f4" => Code::F4,
        "f5" => Code::F5,
        "f6" => Code::F6,
        "f7" => Code::F7,
        "f8" => Code::F8,
        "f9" => Code::F9,
        "f10" => Code::F10,
        "f11" => Code::F11,
        "f12" => Code::F12,
        "f13" => Code::F13,
        "f14" => Code::F14,
        "f15" => Code::F15,
        "f16" => Code::F16,
        "f17" => Code::F17,
        "f18" => Code::F18,
        "f19" => Code::F19,
        "f20" => Code::F20,
        "f21" => Code::F21,
        "f22" => Code::F22,
        "f23" => Code::F23,
        "f24" => Code::F24,
        "minus" | "-" => Code::Minus,
        "equals" | "=" => Code::Equal,
        "comma" | "," => Code::Comma,
        "period" | "." => Code::Period,
        "semicolon" | ";" => Code::Semicolon,
        "quote" | "'" => Code::Quote,
        "backslash" | "\\" => Code::Backslash,
        "slash" | "/" => Code::Slash,
        "grave" | "`" => Code::Backquote,
        "leftbracket" | "[" => Code::BracketLeft,
        "rightbracket" | "]" => Code::BracketRight,
        _ => return None,
    })
}

#[derive(Debug)]
pub enum HotkeyError {
    RegistrationFailed(String),
}

#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;
