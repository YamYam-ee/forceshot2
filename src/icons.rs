//! Egna vektorikoner, ritade direkt med egui i en 20×20-ruta. Inga ikonfonter.

use crate::theme;
use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Select,
    Text,
    Rect,
    Ellipse,
    Line,
    Arrow,
    Highlight,
    Free,
    Weld,
    Film,
    Blur,
    Mosaic,
    Crop,
    Copy,
    Save,
    Undo,
    Redo,
    RotateLeft,
    RotateRight,
    Flip,
    Plus,
    Close,
    Camera,
    Folder,
    Paste,
    Gear,
    Sketch,
    Fit,
    Trash,
    Erase,
    Ocr,
}

pub fn paint(p: &egui::Painter, rect: Rect, icon: Icon, color: Color32) {
    let s = rect.width().min(rect.height()) / 20.0;
    let o = rect.center() - vec2(10.0, 10.0) * s;
    let pt = |x: f32, y: f32| -> Pos2 { o + vec2(x, y) * s };
    let st = Stroke::new(1.6 * s, color);
    let line = |pts: &[(f32, f32)]| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| pt(x, y)).collect();
        p.add(Shape::line(v, st));
    };
    let closed = |pts: &[(f32, f32)]| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| pt(x, y)).collect();
        p.add(Shape::closed_line(v, st));
    };
    let filled = |pts: &[(f32, f32)]| {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| pt(x, y)).collect();
        p.add(Shape::convex_polygon(v, color, Stroke::NONE));
    };
    let rrect = |x0: f32, y0: f32, x1: f32, y1: f32, r: f32| {
        p.rect_stroke(Rect::from_min_max(pt(x0, y0), pt(x1, y1)), r * s, st, egui::StrokeKind::Middle);
    };
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| {
        let n = 16;
        let v: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                pt(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        p.add(Shape::line(v, st));
    };
    match icon {
        Icon::Select => {
            filled(&[(5.0, 3.0), (15.0, 10.5), (10.2, 11.2)]);
            filled(&[(5.0, 3.0), (10.2, 11.2), (7.2, 15.5)]);
            line(&[(10.0, 11.5), (12.8, 16.5)]);
        }
        Icon::Text => {
            line(&[(4.5, 5.0), (15.5, 5.0)]);
            line(&[(10.0, 5.0), (10.0, 16.0)]);
            line(&[(7.8, 16.0), (12.2, 16.0)]);
        }
        Icon::Rect => rrect(3.5, 5.0, 16.5, 15.0, 1.5),
        Icon::Ellipse => {
            p.circle_stroke(pt(10.0, 10.0), 6.2 * s, st);
        }
        Icon::Line => line(&[(4.0, 16.0), (16.0, 4.0)]),
        Icon::Arrow => {
            line(&[(4.0, 16.0), (14.0, 6.0)]);
            filled(&[(16.5, 3.5), (15.6, 9.6), (10.4, 4.4)]);
        }
        Icon::Highlight => {
            closed(&[(11.5, 3.5), (16.5, 8.5), (9.5, 15.5), (4.5, 10.5)]);
            line(&[(3.5, 17.0), (9.0, 17.0)]);
        }
        Icon::Free => {
            let v: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let t = i as f32 / 24.0;
                    pt(3.5 + 13.0 * t, 10.0 + 4.5 * (t * std::f32::consts::TAU * 1.25).sin())
                })
                .collect();
            p.add(Shape::line(v, st));
        }
        Icon::Weld => {
            rrect(3.0, 3.5, 17.0, 10.0, 3.2);
            line(&[(10.0, 10.0), (10.0, 14.0)]);
            p.circle_filled(pt(10.0, 15.6), 1.9 * s, color);
        }
        Icon::Film => {
            rrect(3.0, 6.0, 17.0, 14.0, 1.0);
            line(&[(6.5, 6.0), (6.5, 14.0)]);
            line(&[(13.5, 6.0), (13.5, 14.0)]);
        }
        Icon::Blur => {
            for (i, y) in [5.5f32, 10.0, 14.5].iter().enumerate() {
                let a = [0.95f32, 0.6, 0.3][i];
                p.line_segment([pt(4.0, *y), pt(16.0, *y)], Stroke::new(2.4 * s, color.gamma_multiply(a)));
            }
        }
        Icon::Mosaic => {
            for (i, (x, y)) in [(4.0f32, 4.0f32), (10.5, 4.0), (4.0, 10.5), (10.5, 10.5)].iter().enumerate() {
                let r = Rect::from_min_size(pt(*x, *y), vec2(5.5, 5.5) * s);
                let a = if i == 0 || i == 3 { 1.0 } else { 0.4 };
                p.rect_filled(r, 1.0 * s, color.gamma_multiply(a));
            }
        }
        Icon::Crop => {
            line(&[(6.0, 2.5), (6.0, 14.0), (17.5, 14.0)]);
            line(&[(2.5, 6.0), (14.0, 6.0), (14.0, 17.5)]);
        }
        Icon::Copy => {
            rrect(7.0, 7.0, 16.5, 16.5, 1.8);
            line(&[(4.0, 12.5), (4.0, 4.8), (4.8, 4.0), (12.5, 4.0)]);
        }
        Icon::Save => {
            closed(&[(4.0, 4.0), (13.5, 4.0), (16.0, 6.5), (16.0, 16.0), (4.0, 16.0)]);
            line(&[(7.0, 16.0), (7.0, 11.5), (13.0, 11.5), (13.0, 16.0)]);
            line(&[(7.0, 4.0), (7.0, 7.0), (11.5, 7.0)]);
        }
        Icon::Undo | Icon::Redo => {
            let m = |x: f32| if icon == Icon::Redo { 20.0 - x } else { x };
            line(&[(m(7.5), 4.0), (m(4.0), 7.5), (m(7.5), 11.0)]);
            line(&[(m(4.0), 7.5), (m(12.0), 7.5), (m(15.0), 9.0), (m(16.0), 12.0), (m(15.0), 14.5), (m(12.0), 16.0), (m(8.0), 16.0)]);
        }
        Icon::RotateLeft | Icon::RotateRight => {
            let right = icon == Icon::RotateRight;
            if right {
                arc(10.0, 10.5, 5.8, -60.0, 200.0);
                filled(&[(11.0, 2.2), (16.0, 5.2), (11.2, 8.0)]);
            } else {
                arc(10.0, 10.5, 5.8, -20.0, 240.0);
                filled(&[(9.0, 2.2), (4.0, 5.2), (8.8, 8.0)]);
            }
        }
        Icon::Flip => {
            line(&[(10.0, 3.0), (10.0, 17.0)]);
            closed(&[(7.5, 6.0), (7.5, 14.0), (3.0, 14.0)]);
            filled(&[(12.5, 6.0), (12.5, 14.0), (17.0, 14.0)]);
        }
        Icon::Plus => {
            line(&[(10.0, 4.5), (10.0, 15.5)]);
            line(&[(4.5, 10.0), (15.5, 10.0)]);
        }
        Icon::Close => {
            line(&[(5.5, 5.5), (14.5, 14.5)]);
            line(&[(14.5, 5.5), (5.5, 14.5)]);
        }
        Icon::Camera => {
            line(&[(3.0, 7.0), (3.0, 3.0), (7.0, 3.0)]);
            line(&[(13.0, 3.0), (17.0, 3.0), (17.0, 7.0)]);
            line(&[(17.0, 13.0), (17.0, 17.0), (13.0, 17.0)]);
            line(&[(7.0, 17.0), (3.0, 17.0), (3.0, 13.0)]);
            p.circle_filled(pt(10.0, 10.0), 2.2 * s, color);
        }
        Icon::Folder => {
            closed(&[(3.0, 5.0), (8.0, 5.0), (9.8, 7.0), (17.0, 7.0), (17.0, 15.5), (3.0, 15.5)]);
        }
        Icon::Paste => {
            rrect(4.5, 4.5, 15.5, 17.0, 1.8);
            rrect(7.5, 3.0, 12.5, 6.0, 1.0);
            line(&[(7.5, 10.0), (12.5, 10.0)]);
            line(&[(7.5, 13.0), (11.0, 13.0)]);
        }
        Icon::Gear => {
            p.circle_stroke(pt(10.0, 10.0), 3.0 * s, st);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let (c, sn) = (a.cos(), a.sin());
                p.line_segment([pt(10.0 + 5.0 * c, 10.0 + 5.0 * sn), pt(10.0 + 7.2 * c, 10.0 + 7.2 * sn)], Stroke::new(2.2 * s, color));
            }
        }
        Icon::Sketch => {
            p.circle_stroke(pt(10.0, 10.0), 6.5 * s, st);
            line(&[(3.5, 10.0), (16.5, 10.0)]);
            line(&[(10.0, 3.5), (10.0, 10.0)]);
            line(&[(6.5, 10.0), (6.5, 15.5)]);
            line(&[(13.5, 10.0), (13.5, 15.5)]);
        }
        Icon::Fit => {
            line(&[(3.5, 7.5), (3.5, 3.5), (7.5, 3.5)]);
            line(&[(12.5, 3.5), (16.5, 3.5), (16.5, 7.5)]);
            line(&[(16.5, 12.5), (16.5, 16.5), (12.5, 16.5)]);
            line(&[(7.5, 16.5), (3.5, 16.5), (3.5, 12.5)]);
        }
        Icon::Erase => {
            closed(&[(11.0, 3.5), (16.5, 9.0), (10.0, 15.5), (6.5, 15.5), (3.5, 12.5), (3.5, 11.0)]);
            line(&[(7.2, 7.3), (12.7, 12.8)]);
            line(&[(10.0, 15.5), (17.0, 15.5)]);
        }
        Icon::Ocr => {
            line(&[(3.0, 6.5), (3.0, 3.0), (6.5, 3.0)]);
            line(&[(13.5, 3.0), (17.0, 3.0), (17.0, 6.5)]);
            line(&[(17.0, 13.5), (17.0, 17.0), (13.5, 17.0)]);
            line(&[(6.5, 17.0), (3.0, 17.0), (3.0, 13.5)]);
            line(&[(6.5, 7.0), (13.5, 7.0)]);
            line(&[(10.0, 7.0), (10.0, 13.5)]);
        }
        Icon::Trash => {
            line(&[(4.0, 6.0), (16.0, 6.0)]);
            line(&[(8.0, 6.0), (8.0, 3.8), (12.0, 3.8), (12.0, 6.0)]);
            line(&[(5.5, 6.0), (6.3, 16.5), (13.7, 16.5), (14.5, 6.0)]);
        }
    }
}

/// Kvadratisk ikonknapp. `selected` ger accentfärgad bakgrund.
pub fn button(ui: &mut Ui, icon: Icon, tip: &str, selected: bool) -> Response {
    sized_button(ui, icon, tip, selected, 34.0)
}

pub fn sized_button(ui: &mut Ui, icon: Icon, tip: &str, selected: bool, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if ui.is_rect_visible(rect) {
        let enabled = ui.is_enabled();
        let bg = if selected {
            theme::accent_soft()
        } else if resp.is_pointer_button_down_on() {
            theme::ACTIVE
        } else if resp.hovered() {
            theme::HOVER
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect.shrink(1.0), 7.0, bg);
        let fg = if !enabled {
            theme::TEXT_FAINT
        } else if selected {
            theme::ACCENT
        } else if resp.hovered() {
            theme::TEXT
        } else {
            theme::TEXT_DIM
        };
        let inner = Rect::from_center_size(rect.center(), Vec2::splat(20.0f32.min(size - 8.0)));
        paint(ui.painter(), inner, icon, fg);
    }
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// Knapp med ikon och text. `primary` ger fylld accentknapp.
pub fn labeled(ui: &mut Ui, icon: Icon, text: &str, primary: bool) -> Response {
    let font = egui::FontId::new(13.5, if primary { theme::bold() } else { egui::FontFamily::Proportional });
    let fg = if primary { theme::ON_ACCENT } else { theme::TEXT };
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font, fg);
    let size = vec2(galley.size().x + 44.0, 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = match (primary, resp.is_pointer_button_down_on(), resp.hovered()) {
            (true, true, _) => theme::ACCENT_DEEP,
            (true, _, true) => theme::ACCENT.gamma_multiply(0.9),
            (true, _, _) => theme::ACCENT,
            (false, true, _) => theme::ACTIVE,
            (false, _, true) => theme::HOVER,
            (false, _, _) => theme::ELEVATED,
        };
        ui.painter().rect_filled(rect, 7.0, bg);
        let ir = Rect::from_min_size(pos2(rect.min.x + 10.0, rect.center().y - 8.0), Vec2::splat(16.0));
        paint(ui.painter(), ir, icon, fg);
        ui.painter().galley(pos2(rect.min.x + 33.0, rect.center().y - galley.size().y / 2.0), galley, fg);
    }
    resp
}
