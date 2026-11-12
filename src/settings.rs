//! Inställningar. Sparas som JSON i användarens konfigurationsmapp
//! (`%APPDATA%\ForceShot2` på Windows, `~/Library/Application Support/ForceShot2` på Mac).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AfterCapture {
    Meny,
    Redigera,
    Kopiera,
    Spara,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Numbering {
    PerMetod,
    Gemensam,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum MarkerStyle {
    Avlang,
    Rektangel,
}

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Method {
    pub code: String,
    pub name: String,
    pub color: [u8; 3],
}

pub fn default_methods() -> Vec<Method> {
    let m = |code: &str, name: &str, color: [u8; 3]| Method { code: code.into(), name: name.into(), color };
    vec![
        m("RT", "Radiografisk provning", [0x8E, 0x44, 0x57]),
        m("UT", "Ultraljudsprovning", [0x15, 0x65, 0xC0]),
        m("PAUT", "Phased array-ultraljud", [0x00, 0x83, 0x8F]),
        m("MT", "Magnetpulverprovning", [0x7B, 0x1F, 0xA2]),
        m("PT", "Penetrantprovning", [0xE6, 0x51, 0x00]),
        m("VT", "Visuell provning", [0x2E, 0x7D, 0x32]),
    ]
}

/// Filmformat i centimeter, som de står på asken.
pub const FILM_FORMATS: [(f32, f32); 5] = [(10.0, 12.0), (6.0, 24.0), (6.0, 48.0), (10.0, 24.0), (10.0, 48.0)];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    pub region: Vec<String>,
    pub full_screen: Vec<String>,
    pub window: Vec<String>,
    pub last_region: Vec<String>,
    pub open_clipboard: Vec<String>,
}

impl Default for Hotkeys {
    fn default() -> Self {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect();
        // Mac-tangentbord saknar PrintScreen, så där gäller bara den andra kolumnen.
        if cfg!(target_os = "macos") {
            Self {
                region: v(&["Ctrl+Alt+Shift+P"]),
                full_screen: v(&["Ctrl+Alt+Shift+H"]),
                window: v(&["Ctrl+Alt+Shift+F"]),
                last_region: v(&["Ctrl+Alt+Shift+R"]),
                open_clipboard: v(&["Ctrl+Alt+Shift+E"]),
            }
        } else {
            Self {
                region: v(&["PrintScreen", "Ctrl+Alt+Shift+P"]),
                full_screen: v(&["Ctrl+PrintScreen", "Ctrl+Alt+Shift+H"]),
                window: v(&["Alt+PrintScreen", "Ctrl+Alt+Shift+F"]),
                last_region: v(&["Shift+PrintScreen", "Ctrl+Alt+Shift+R"]),
                open_clipboard: v(&["Ctrl+Shift+E"]),
            }
        }
    }
}

/// Ritinställningar som redigeraren kommer ihåg mellan klipp.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolStyle {
    pub stroke: [u8; 4],
    pub fill: [u8; 4],
    pub width: f32,
    pub text_size: f32,
    pub text_color: [u8; 4],
    pub text_box_fill: bool,
    pub text_box_border: bool,
    pub box_fill: [u8; 4],
    pub box_border: [u8; 4],
    pub highlight: [u8; 4],
    pub blur: f32,
    pub mosaic: f32,
    pub arrow_double: bool,
    pub methods: Vec<String>,
    pub show_method: bool,
    pub marker_style: MarkerStyle,
    pub marker_scale: f32,
    pub film_format: usize,
    pub film_landscape: bool,
    pub px_per_cm: f32,
    /// Textjustering: 0 vänster, 1 centrerat, 2 höger.
    pub text_align: u8,
}

impl Default for ToolStyle {
    fn default() -> Self {
        Self {
            stroke: [0xE5, 0x39, 0x35, 0xFF],
            fill: [0, 0, 0, 0],
            width: 3.0,
            text_size: 18.0,
            text_color: [0, 0, 0, 0xFF],
            text_box_fill: true,
            text_box_border: true,
            box_fill: [0xFF, 0xFF, 0xFF, 0xF2],
            box_border: [0x00, 0xAC, 0xC1, 0xFF],
            highlight: [0xFF, 0xEB, 0x3B, 0xFF],
            blur: 8.0,
            mosaic: 12.0,
            arrow_double: false,
            methods: vec!["RT".into()],
            show_method: true,
            marker_style: MarkerStyle::Avlang,
            marker_scale: 1.0,
            film_format: 3,
            film_landscape: false,
            px_per_cm: 0.0,
            text_align: 0,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub hotkeys: Hotkeys,
    pub after_capture: AfterCapture,
    pub include_cursor: bool,
    pub show_magnifier: bool,
    pub save_folder: PathBuf,
    pub filename_pattern: String,
    pub jpeg: bool,
    pub jpeg_quality: u8,
    pub ask_filename: bool,
    pub numbering: Numbering,
    pub methods: Vec<Method>,
    pub style: ToolStyle,
    pub last_region: Option<(usize, [u32; 4])>,
    /// GitHub-repot som utgåvorna hämtas från, som `ägare/repo`. Tomt = ingen uppdateringskoll.
    pub update_repo: String,
    pub check_updates: bool,
    /// En version användaren valt att hoppa över.
    pub skipped_update: String,
    /// Bladexporten för fiberskisser.
    pub sheet_jpeg: bool,
    pub sheet_quality: u8,
    pub sheet_max_width: u32,
}

impl Default for Settings {
    fn default() -> Self {
        let pictures = dirs::picture_dir().or_else(dirs::home_dir).unwrap_or_default();
        Self {
            hotkeys: Hotkeys::default(),
            after_capture: AfterCapture::Meny,
            include_cursor: false,
            show_magnifier: true,
            save_folder: pictures.join("ForceShot"),
            filename_pattern: "ForceShot_{yyyy}-{MM}-{dd}_{HH}{mm}{ss}".into(),
            jpeg: false,
            jpeg_quality: 92,
            ask_filename: true,
            numbering: Numbering::PerMetod,
            methods: default_methods(),
            style: ToolStyle::default(),
            last_region: None,
            update_repo: DEFAULT_UPDATE_REPO.into(),
            check_updates: true,
            skipped_update: String::new(),
            sheet_jpeg: true,
            sheet_quality: 82,
            sheet_max_width: 1200,
        }
    }
}

/// Version 2 har ett eget repo för utgåvor, skilt från version 1:s.
pub const DEFAULT_UPDATE_REPO: &str = "YamYam-ee/forceshot2";
/// Förvalet i de allra första byggena av version 2; pekade på version 1:s repo.
const OLD_UPDATE_REPO: &str = "YamYam-ee/forceshot";

pub fn config_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("ForceShot2")
}

impl Settings {
    fn path() -> PathBuf {
        config_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let mut s: Self = std::fs::read_to_string(Self::path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        if s.update_repo.trim().eq_ignore_ascii_case(OLD_UPDATE_REPO) {
            s.update_repo = DEFAULT_UPDATE_REPO.into();
        }
        s
    }

    pub fn save(&self) {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }

    pub fn method_color(&self, code: &str) -> egui::Color32 {
        self.methods
            .iter()
            .find(|m| m.code == code)
            .map(|m| egui::Color32::from_rgb(m.color[0], m.color[1], m.color[2]))
            .unwrap_or(egui::Color32::from_rgb(0x79, 0x55, 0x48))
    }

    /// Filnamn utan ändelse enligt mönstret.
    pub fn filename(&self, w: u32, h: u32) -> String {
        let now = chrono::Local::now();
        let f = |s: &str| now.format(s).to_string();
        self.filename_pattern
            .replace("{yyyy}", &f("%Y"))
            .replace("{MM}", &f("%m"))
            .replace("{dd}", &f("%d"))
            .replace("{HH}", &f("%H"))
            .replace("{mm}", &f("%M"))
            .replace("{ss}", &f("%S"))
            .replace("{bredd}", &w.to_string())
            .replace("{hojd}", &h.to_string())
            .replace("{titel}", "")
            .replace("{nr}", "")
    }
}

pub fn col(c: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

pub fn arr(c: egui::Color32) -> [u8; 4] {
    c.to_srgba_unmultiplied()
}
