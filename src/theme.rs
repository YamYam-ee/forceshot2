//! Färgpalett och egui-stil: grafitgrått med turkos accent, samma som loggan.

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke};

pub const BASE: Color32 = Color32::from_rgb(0x12, 0x14, 0x18);
pub const SURFACE: Color32 = Color32::from_rgb(0x1A, 0x1D, 0x23);
pub const ELEVATED: Color32 = Color32::from_rgb(0x23, 0x27, 0x2F);
pub const HOVER: Color32 = Color32::from_rgb(0x2C, 0x31, 0x3B);
pub const ACTIVE: Color32 = Color32::from_rgb(0x36, 0x3C, 0x48);
pub const BORDER: Color32 = Color32::from_rgb(0x2B, 0x30, 0x3A);
pub const TEXT: Color32 = Color32::from_rgb(0xE9, 0xEC, 0xF2);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x98, 0xA1, 0xB2);
pub const TEXT_FAINT: Color32 = Color32::from_rgb(0x6B, 0x73, 0x83);
pub const ACCENT: Color32 = Color32::from_rgb(0x2D, 0xD4, 0xBF);
pub const ACCENT_DEEP: Color32 = Color32::from_rgb(0x0E, 0xA5, 0xA4);
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x06, 0x1F, 0x1D);
pub const DANGER: Color32 = Color32::from_rgb(0xEF, 0x44, 0x44);

pub fn accent_soft() -> Color32 {
    Color32::from_rgba_unmultiplied(0x2D, 0xD4, 0xBF, 0x30)
}

/// Namnet på fetstilsfamiljen. Finns alltid; saknas en fet systemfont pekar den på standardfonten.
pub fn bold() -> FontFamily {
    FontFamily::Name("fet".into())
}

fn first_existing(paths: &[&str]) -> Option<Vec<u8>> {
    paths.iter().find_map(|p| std::fs::read(p).ok())
}

pub fn apply(ctx: &egui::Context) {
    // Systemets egna gränssnittsfont ger skarpare text än den inbyggda.
    let mut fonts = FontDefinitions::default();
    let regular = first_existing(&[
        "C:\\Windows\\Fonts\\segoeui.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/Library/Fonts/Arial.ttf",
    ]);
    let heavy = first_existing(&[
        "C:\\Windows\\Fonts\\seguisb.ttf",
        "C:\\Windows\\Fonts\\segoeuib.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/Library/Fonts/Arial Bold.ttf",
    ]);
    if let Some(data) = regular {
        fonts.font_data.insert("ui".into(), FontData::from_owned(data).into());
        fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "ui".into());
    }
    let mut bold_stack = fonts.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    if let Some(data) = heavy {
        fonts.font_data.insert("ui-fet".into(), FontData::from_owned(data).into());
        bold_stack.insert(0, "ui-fet".into());
    }
    fonts.families.insert(bold(), bold_stack);
    ctx.set_fonts(fonts);

    let mut style = (*ctx.global_style()).clone();
    let v = &mut style.visuals;
    *v = egui::Visuals::dark();
    v.panel_fill = SURFACE;
    v.window_fill = ELEVATED;
    v.extreme_bg_color = BASE;
    v.faint_bg_color = ELEVATED;
    v.override_text_color = Some(TEXT);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.selection.bg_fill = accent_soft();
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.hyperlink_color = ACCENT;
    let r = CornerRadius::same(6);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT_DIM);
    v.widgets.noninteractive.corner_radius = r;
    v.widgets.inactive.bg_fill = ELEVATED;
    v.widgets.inactive.weak_bg_fill = ELEVATED;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = r;
    v.widgets.hovered.bg_fill = HOVER;
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.corner_radius = r;
    v.widgets.hovered.expansion = 0.0;
    v.widgets.active.bg_fill = ACTIVE;
    v.widgets.active.weak_bg_fill = ACTIVE;
    v.widgets.active.bg_stroke = Stroke::NONE;
    v.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.active.corner_radius = r;
    v.widgets.active.expansion = 0.0;
    v.widgets.open.bg_fill = ACTIVE;
    v.widgets.open.weak_bg_fill = ACTIVE;
    v.widgets.open.corner_radius = r;
    style.spacing.item_spacing = egui::vec2(6.0, 6.0);
    style.spacing.button_padding = egui::vec2(10.0, 5.0);
    style.spacing.interact_size.y = 28.0;
    ctx.set_global_style(style);
}
