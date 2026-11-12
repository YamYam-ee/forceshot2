//! macOS: skärmdump via ScreenCaptureKit (xcap) och globala snabbtangenter via Carbon
//! (global-hotkey). Kräver behörigheten Skärminspelning i Systeminställningar.
//!
//! OBS: den här filen är typkontrollerad men inte körd på en riktig Mac än.

use super::{Action, Combo, Shot};
use std::sync::Arc;

/// Fönster placeras i punkter, inte fysiska bildpunkter.
pub const LOGICAL_POSITIONS: bool = true;

pub fn capture_monitors(_include_cursor: bool) -> Vec<Shot> {
    let Ok(monitors) = xcap::Monitor::all() else { return Vec::new() };
    monitors
        .into_iter()
        .filter_map(|m| {
            let image = m.capture_image().ok()?;
            Some(Shot {
                x: m.x().ok()?,
                y: m.y().ok()?,
                w: m.width().ok()?,
                h: m.height().ok()?,
                image: Arc::new(image),
            })
        })
        .collect()
}

/// Textavläsning finns inte på Mac än; den ska gå via Apples Vision-ramverk.
pub fn ocr(_img: &image::RgbaImage) -> Result<Vec<String>, String> {
    Err("Textavläsning finns inte i Mac-versionen än.".into())
}

/// Öppnar ett nytt mejl i Mail med filen bifogad.
pub fn send_mail(path: &std::path::Path) -> Result<(), String> {
    std::process::Command::new("open").args(["-a", "Mail"]).arg(path).status().map(|_| ()).map_err(|e| e.to_string())
}

pub fn cursor_pos() -> (i32, i32) {
    // Utan pekarläge väljs första skärmen; räcker för "hela skärmen" på en bärbar.
    (0, 0)
}

pub fn active_window_rect() -> Option<(i32, i32, i32, i32)> {
    let windows = xcap::Window::all().ok()?;
    let w = windows.into_iter().find(|w| w.is_focused().unwrap_or(false))?;
    Some((w.x().ok()?, w.y().ok()?, w.width().ok()? as i32, w.height().ok()? as i32))
}

/// De synliga fönstrens ramar i punkter, det översta först.
pub fn window_rects() -> Vec<[i32; 4]> {
    let Ok(windows) = xcap::Window::all() else { return Vec::new() };
    windows
        .into_iter()
        .filter(|w| !w.is_minimized().unwrap_or(true))
        .filter_map(|w| Some([w.x().ok()?, w.y().ok()?, w.width().ok()? as i32, w.height().ok()? as i32]))
        .collect()
}

pub fn force_foreground(window: &winit::window::Window) {
    window.focus_window();
}

/// Lyfter överlägget ovanför menyrad och Dock och låter det följa med till alla skrivbord.
pub fn prepare_overlay_window(window: &winit::window::Window) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let Ok(handle) = window.window_handle() else { return };
    if let RawWindowHandle::AppKit(h) = handle.as_raw() {
        let view = h.ns_view.as_ptr() as *mut AnyObject;
        unsafe {
            let ns_window: *mut AnyObject = msg_send![view, window];
            if !ns_window.is_null() {
                // NSScreenSaverWindowLevel = 1000
                let _: () = msg_send![ns_window, setLevel: 1000isize];
                // canJoinAllSpaces | fullScreenAuxiliary
                let _: () = msg_send![ns_window, setCollectionBehavior: (1usize | 256usize)];
            }
        }
    }
}

fn code_of(key: &str) -> Option<global_hotkey::hotkey::Code> {
    use global_hotkey::hotkey::Code::*;
    let k = key.to_ascii_uppercase();
    Some(match k.as_str() {
        "A" => KeyA, "B" => KeyB, "C" => KeyC, "D" => KeyD, "E" => KeyE, "F" => KeyF, "G" => KeyG,
        "H" => KeyH, "I" => KeyI, "J" => KeyJ, "K" => KeyK, "L" => KeyL, "M" => KeyM, "N" => KeyN,
        "O" => KeyO, "P" => KeyP, "Q" => KeyQ, "R" => KeyR, "S" => KeyS, "T" => KeyT, "U" => KeyU,
        "V" => KeyV, "W" => KeyW, "X" => KeyX, "Y" => KeyY, "Z" => KeyZ,
        "0" => Digit0, "1" => Digit1, "2" => Digit2, "3" => Digit3, "4" => Digit4,
        "5" => Digit5, "6" => Digit6, "7" => Digit7, "8" => Digit8, "9" => Digit9,
        "F1" => F1, "F2" => F2, "F3" => F3, "F4" => F4, "F5" => F5, "F6" => F6,
        "F7" => F7, "F8" => F8, "F9" => F9, "F10" => F10, "F11" => F11, "F12" => F12,
        "SPACE" => Space,
        _ => return None,
    })
}

/// Globala snabbtangenter. Måste skapas på huvudtråden och hållas vid liv.
pub struct Hotkeys {
    _manager: Option<global_hotkey::GlobalHotKeyManager>,
}

impl Hotkeys {
    pub fn start(bindings: Vec<(Combo, Action)>, send: impl Fn(Action) + Send + Sync + 'static) -> Self {
        use global_hotkey::hotkey::{HotKey, Modifiers};
        use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
        let Ok(manager) = GlobalHotKeyManager::new() else { return Self { _manager: None } };
        let mut table: Vec<(u32, Action)> = Vec::new();
        for (c, action) in bindings {
            let Some(code) = code_of(&c.key) else { continue };
            let mut mods = Modifiers::empty();
            mods.set(Modifiers::CONTROL, c.ctrl);
            mods.set(Modifiers::ALT, c.alt);
            mods.set(Modifiers::SHIFT, c.shift);
            mods.set(Modifiers::META, c.meta);
            let hk = HotKey::new(Some(mods), code);
            if manager.register(hk).is_ok() {
                table.push((hk.id(), action));
            }
        }
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.state == HotKeyState::Pressed {
                if let Some((_, a)) = table.iter().find(|(id, _)| *id == e.id) {
                    send(*a);
                }
            }
        }));
        Self { _manager: Some(manager) }
    }
}
