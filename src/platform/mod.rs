//! Allt som skiljer sig mellan Windows och Mac: skärmdump, globala snabbtangenter
//! och fönsterknep för markeringsöverlägget.

use image::RgbaImage;
use std::sync::Arc;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

/// En frusen bild av en skärm.
#[derive(Clone)]
pub struct Shot {
    /// Skärmens läge och storlek i skrivbordets enheter: fysiska bildpunkter på
    /// Windows, punkter på Mac (se `LOGICAL_POSITIONS`).
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub image: Arc<RgbaImage>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Region,
    FullScreen,
    Window,
    LastRegion,
    OpenClipboard,
}

/// En tangentkombination som "Ctrl+Alt+Shift+P" eller "PrintScreen".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Combo {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
    /// Tangentens namn i versaler: "P", "PRINTSCREEN", "F5" …
    pub key: String,
}

impl Combo {
    pub fn parse(s: &str) -> Option<Self> {
        let mut c = Combo { ctrl: false, alt: false, shift: false, meta: false, key: String::new() };
        for part in s.split('+').map(|p| p.trim()).filter(|p| !p.is_empty()) {
            match part.to_ascii_uppercase().as_str() {
                "CTRL" | "CONTROL" => c.ctrl = true,
                "ALT" | "OPTION" => c.alt = true,
                "SHIFT" | "SKIFT" => c.shift = true,
                "WIN" | "CMD" | "META" | "SUPER" => c.meta = true,
                k => c.key = k.to_string(),
            }
        }
        (!c.key.is_empty()).then_some(c)
    }
}

pub fn bindings(h: &crate::settings::Hotkeys) -> Vec<(Combo, Action)> {
    let mut out = Vec::new();
    let mut add = |list: &Vec<String>, a: Action| {
        for s in list {
            if let Some(c) = Combo::parse(s) {
                out.push((c, a));
            }
        }
    };
    add(&h.region, Action::Region);
    add(&h.full_screen, Action::FullScreen);
    add(&h.window, Action::Window);
    add(&h.last_region, Action::LastRegion);
    add(&h.open_clipboard, Action::OpenClipboard);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_combos() {
        let c = Combo::parse("Ctrl+Alt+Shift+P").unwrap();
        assert!(c.ctrl && c.alt && c.shift && !c.meta);
        assert_eq!(c.key, "P");
        assert_eq!(Combo::parse("PrintScreen").unwrap().key, "PRINTSCREEN");
        assert!(Combo::parse("Ctrl+").is_none());
    }
}
