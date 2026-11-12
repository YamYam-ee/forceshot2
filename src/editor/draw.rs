//! Gör om annoteringarna till egui-former. Samma kod ritar på skärmen och i den
//! exporterade bilden — det enda som skiljer är transformen `Xf`.

use super::doc::{Element, FilmData, Kind, TextData, WeldData, rotate};
use crate::settings::MarkerStyle;
use crate::theme;
use egui::epaint::{Mesh, PathShape, PathStroke, RectShape, TextShape, Vertex};
use egui::{Color32, CornerRadius, FontId, Pos2, Rect, Shape, Stroke, StrokeKind, TextureId, Vec2, pos2, vec2};
use std::collections::HashMap;
use std::sync::Arc;

/// Bildens koordinater -> målytans punkter.
#[derive(Clone, Copy)]
pub struct Xf {
    pub s: f32,
    pub o: Vec2,
}

impl Xf {
    pub fn p(&self, p: Pos2) -> Pos2 {
        pos2(p.x * self.s + self.o.x, p.y * self.s + self.o.y)
    }
    pub fn inv(&self, p: Pos2) -> Pos2 {
        pos2((p.x - self.o.x) / self.s, (p.y - self.o.y) / self.s)
    }
    pub fn r(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.p(r.min), self.p(r.max))
    }
}

pub struct Res<'a> {
    pub ctx: &'a egui::Context,
    /// Texturer per objekt: inklistrade bilder och bearbetade utsnitt.
    pub tex: &'a HashMap<u64, TextureId>,
    /// Textobjektet som redigeras just nu ritas av textfältet i stället.
    pub editing: Option<u64>,
    pub image_rect: Rect,
}

/// Luft mellan textrutans kant och texten, i bildpunkter.
pub const TEXT_PAD: f32 = 6.0;

pub fn text_galley(ctx: &egui::Context, el: &Element, td: &TextData, s: f32) -> Arc<egui::Galley> {
    let wrap = ((el.rect().width() - 2.0 * TEXT_PAD) * s).max(8.0);
    ctx.fonts_mut(|f| f.layout(td.text.clone(), FontId::new(td.size * s, theme::bold()), el.stroke, wrap))
}

/// Textrutans höjd i bildpunkter, så som texten radbryts just nu.
pub fn text_height(ctx: &egui::Context, el: &Element, td: &TextData, s: f32) -> f32 {
    let g = text_galley(ctx, el, td, s);
    (g.size().y / s).max(td.size * 1.25) + 2.0 * TEXT_PAD
}

fn quad(el: &Element, xf: &Xf) -> [Pos2; 4] {
    let r = el.rect();
    [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].map(|p| xf.p(el.to_world(p)))
}

fn textured_quad(pts: [Pos2; 4], tex: TextureId, tint: Color32) -> Shape {
    let mut m = Mesh::with_texture(tex);
    let uv = [pos2(0.0, 0.0), pos2(1.0, 0.0), pos2(1.0, 1.0), pos2(0.0, 1.0)];
    for i in 0..4 {
        m.vertices.push(Vertex { pos: pts[i], uv: uv[i], color: tint });
    }
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    Shape::mesh(m)
}

pub fn element(el: &Element, xf: &Xf, res: &Res, out: &mut Vec<Shape>) {
    let s = xf.s;
    let stroke = Stroke::new((el.width * s).max(0.6), el.stroke);
    match &el.kind {
        Kind::Rect => {
            let pts = quad(el, xf).to_vec();
            out.push(Shape::Path(PathShape { points: pts, closed: true, fill: el.fill, stroke: PathStroke::new(stroke.width, stroke.color) }));
        }
        Kind::Ellipse => {
            let r = el.rect();
            let c = r.center();
            let n = 72;
            let pts: Vec<Pos2> = (0..n)
                .map(|i| {
                    let a = i as f32 / n as f32 * std::f32::consts::TAU;
                    xf.p(el.to_world(c + vec2(a.cos() * r.width() / 2.0, a.sin() * r.height() / 2.0)))
                })
                .collect();
            out.push(Shape::Path(PathShape { points: pts, closed: true, fill: el.fill, stroke: PathStroke::new(stroke.width, stroke.color) }));
        }
        Kind::Line => out.push(Shape::line_segment([xf.p(el.a), xf.p(el.b)], stroke)),
        Kind::Arrow { double } => arrow(xf.p(el.a), xf.p(el.b), el.width * s, el.stroke, *double, out),
        Kind::Free(pts) => {
            if pts.len() == 1 {
                out.push(Shape::circle_filled(xf.p(pts[0]), stroke.width / 2.0, el.stroke));
            } else {
                out.push(Shape::line(pts.iter().map(|p| xf.p(*p)).collect(), stroke));
            }
        }
        Kind::Highlight | Kind::Erase | Kind::Blur(_) | Kind::Mosaic(_) => {
            let r = el.rect().intersect(res.image_rect);
            if r.width() <= 0.0 || r.height() <= 0.0 {
                return;
            }
            let sr = xf.r(r);
            match res.tex.get(&el.id) {
                Some(tex) => out.push(Shape::Rect(RectShape::filled(sr, 0.0, Color32::WHITE).with_texture(*tex, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))))),
                None => {
                    let c = if matches!(el.kind, Kind::Highlight) { el.stroke.gamma_multiply(0.35) } else { Color32::from_rgba_unmultiplied(120, 126, 138, 150) };
                    out.push(Shape::rect_filled(sr, 0.0, c));
                }
            }
        }
        Kind::Image(_) => {
            if let Some(tex) = res.tex.get(&el.id) {
                out.push(textured_quad(quad(el, xf), *tex, Color32::WHITE));
            }
        }
        Kind::Text(td) => text(el, td, xf, res, out),
        Kind::Weld(w) => weld(el, w, xf, res, out),
        Kind::Film(f) => film(el, f, xf, res, out),
    }
}

fn arrow(a: Pos2, b: Pos2, w: f32, color: Color32, double: bool, out: &mut Vec<Shape>) {
    let v = b - a;
    let len = v.length();
    if len < 0.5 {
        return;
    }
    let dir = v / len;
    let n = vec2(-dir.y, dir.x);
    let head = (w * 4.2).max(9.0).min(len * 0.6);
    let half = head * 0.42;
    let mut tip = |at: Pos2, d: Vec2| {
        let base = at - d * head;
        out.push(Shape::convex_polygon(vec![at, base + n * half, base - n * half], color, Stroke::NONE));
    };
    tip(b, dir);
    let mut start = a;
    if double {
        tip(a, -dir);
        start = a + dir * head * 0.85;
    }
    // Skaftet kortas så att det inte sticker ut genom spetsen.
    out.push(Shape::line_segment([start, b - dir * head * 0.85], Stroke::new(w.max(0.6), color)));
}

fn text(el: &Element, td: &TextData, xf: &Xf, res: &Res, out: &mut Vec<Shape>) {
    let s = xf.s;
    if td.box_fill.is_some() || td.box_border.is_some() {
        let stroke = td.box_border.map(|c| PathStroke::new((el.width * s).max(0.6), c)).unwrap_or(PathStroke::NONE);
        out.push(Shape::Path(PathShape { points: quad(el, xf).to_vec(), closed: true, fill: td.box_fill.unwrap_or(Color32::TRANSPARENT), stroke }));
    }
    if res.editing == Some(el.id) || td.text.is_empty() {
        return;
    }
    let g = text_galley(res.ctx, el, td, s);
    let origin = xf.p(el.to_world(el.rect().min + vec2(TEXT_PAD, TEXT_PAD)));
    if td.box_fill.is_none() && td.box_border.is_none() {
        // Utan ruta: en mörk eller ljus kontrastkant runt bokstäverna.
        let lum = el.stroke.r() as u32 * 3 + el.stroke.g() as u32 * 6 + el.stroke.b() as u32;
        let halo = if lum > 1275 { Color32::from_black_alpha(190) } else { Color32::from_white_alpha(210) };
        let d = (td.size * s * 0.06).max(0.8);
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-0.7, -0.7), (0.7, -0.7), (-0.7, 0.7), (0.7, 0.7)] {
            out.push(Shape::Text(TextShape::new(origin + vec2(dx, dy) * d, g.clone(), halo).with_override_text_color(halo).with_angle(el.rot)));
        }
    }
    out.push(Shape::Text(TextShape::new(origin, g, el.stroke).with_angle(el.rot)));
}

fn weld(el: &Element, w: &WeldData, xf: &Xf, res: &Res, out: &mut Vec<Shape>) {
    let k = xf.s * w.scale;
    let first = w.colors.first().copied().unwrap_or(el.stroke);
    let (a, b) = (xf.p(el.a), xf.p(el.b));
    let hair = Color32::from_white_alpha(170);

    let label = w.label();
    let g = res.ctx.fonts_mut(|f| f.layout_no_wrap(label, FontId::new(13.5 * k, theme::bold()), Color32::WHITE));
    let size = vec2(g.size().x + 18.0 * k, g.size().y + 8.0 * k);
    let badge = Rect::from_center_size(b, size);
    let radius = match w.style {
        MarkerStyle::Avlang => size.y / 2.0,
        MarkerStyle::Rektangel => 3.0 * k,
    };

    // Hänvisningslinjen går från skarvpunkten till brickan, under brickan.
    if !badge.contains(a) {
        out.push(Shape::line_segment([a, b], Stroke::new(3.4 * k, Color32::from_black_alpha(60))));
        out.push(Shape::line_segment([a, b], Stroke::new(2.0 * k, first)));
    }
    out.push(Shape::circle_filled(a, 5.6 * k, hair));
    out.push(Shape::circle_filled(a, 4.6 * k, first));

    // Två förskjutna kopior med låg täckning ger en mjuk skugga.
    out.push(Shape::rect_filled(badge.translate(vec2(0.0, 2.6 * k)).expand(0.8 * k), radius, Color32::from_black_alpha(34)));
    out.push(Shape::rect_filled(badge.translate(vec2(0.0, 1.2 * k)), radius, Color32::from_black_alpha(60)));

    let n = w.colors.len().max(1);
    let rr = radius.round().clamp(0.0, 255.0) as u8;
    for i in 0..n {
        let x0 = badge.min.x + size.x * i as f32 / n as f32;
        let x1 = badge.min.x + size.x * (i + 1) as f32 / n as f32;
        let (l, r) = (if i == 0 { rr } else { 0 }, if i + 1 == n { rr } else { 0 });
        let cr = CornerRadius { nw: l, sw: l, ne: r, se: r };
        let c = w.colors.get(i).copied().unwrap_or(first);
        out.push(Shape::rect_filled(Rect::from_min_max(pos2(x0, badge.min.y), pos2(x1, badge.max.y)), cr, c));
    }
    out.push(Shape::rect_stroke(badge, radius, Stroke::new((0.9 * k).max(0.6), hair), StrokeKind::Inside));
    out.push(Shape::galley(badge.center() - g.size() / 2.0, g, Color32::WHITE));
}

fn film(el: &Element, f: &FilmData, xf: &Xf, res: &Res, out: &mut Vec<Shape>) {
    let s = xf.s;
    let pts = quad(el, xf).to_vec();
    out.push(Shape::Path(PathShape {
        points: pts,
        closed: true,
        fill: el.stroke.gamma_multiply(0.22),
        stroke: PathStroke::new((el.width * s).max(0.8), el.stroke),
    }));
    // Numret står alltid upprätt, även när filmen är vriden.
    let c = xf.p(el.center());
    let short = el.rect().width().min(el.rect().height());
    let num_size = (short * 0.42).clamp(11.0, 44.0) * s;
    let num = res.ctx.fonts_mut(|fo| fo.layout_no_wrap(f.number.to_string(), FontId::new(num_size, theme::bold()), Color32::WHITE));
    let fmt = format!("{}×{}", f.w_cm as u32, f.h_cm as u32);
    let sub = res.ctx.fonts_mut(|fo| fo.layout_no_wrap(fmt, FontId::new((num_size * 0.42).max(7.0 * s), theme::bold()), Color32::WHITE));
    let total = num.size().y + sub.size().y;
    let chip = Rect::from_center_size(c, vec2(num.size().x.max(sub.size().x) + 10.0 * s, total + 6.0 * s));
    out.push(Shape::rect_filled(chip, 4.0 * s, el.stroke));
    out.push(Shape::galley(pos2(c.x - num.size().x / 2.0, chip.min.y + 3.0 * s), num.clone(), Color32::WHITE));
    out.push(Shape::galley(pos2(c.x - sub.size().x / 2.0, chip.min.y + 3.0 * s + num.size().y), sub, Color32::WHITE));
    let _ = rotate;
}
