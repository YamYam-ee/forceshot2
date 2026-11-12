//! Markeringsöverlägget: ett ramlöst fönster per skärm som visar den frusna bilden,
//! dimmar den och låter användaren dra upp ett område. Markeringen räknas i skrivbordets
//! enheter och får gå över flera skärmar; ett klick tar fönstret under pekaren.

use crate::icons::{self, Icon};
use crate::platform::Shot;
use crate::settings::{AfterCapture, Settings};
use crate::theme;
use egui::{Color32, FontId, Key, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2, pos2, vec2};
use image::RgbaImage;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Then {
    Editor,
    Copy,
    Save,
}

pub enum Outcome {
    Cancel,
    /// Området i skrivbordets enheter (x, y, b, h). Det kan gå över flera skärmar.
    Take { rect: [i32; 4], then: Then },
}

fn shot_rect(s: &Shot) -> Rect {
    Rect::from_min_size(pos2(s.x as f32, s.y as f32), vec2(s.w as f32, s.h as f32))
}

/// Sätter ihop bilden för ett område på skrivbordet av de skärmar det rör vid. Ytor som
/// inte ligger på någon skärm blir svarta. Har skärmarna olika täthet (Mac) bestämmer
/// den tätaste.
pub fn compose(shots: &[Shot], rect: [i32; 4]) -> RgbaImage {
    let [x, y, w, h] = rect;
    let (w, h) = (w.max(1), h.max(1));
    let density = |s: &Shot| s.image.width() as f32 / s.w.max(1) as f32;
    let hits: Vec<&Shot> = shots.iter().filter(|s| x < s.x + s.w as i32 && x + w > s.x && y < s.y + s.h as i32 && y + h > s.y).collect();
    let k = hits.iter().map(|s| density(s)).fold(0.0f32, f32::max);
    let k = if k > 0.0 { k } else { 1.0 };
    let px = |v: i32, k: f32| (v as f32 * k).round() as i32;
    let mut out = RgbaImage::from_pixel(px(w, k).max(1) as u32, px(h, k).max(1) as u32, image::Rgba([0, 0, 0, 255]));
    for s in hits {
        let ks = density(s);
        let (x0, y0) = (x.max(s.x), y.max(s.y));
        let (x1, y1) = ((x + w).min(s.x + s.w as i32), (y + h).min(s.y + s.h as i32));
        let part = crate::imaging::crop(&s.image, px(x0 - s.x, ks), px(y0 - s.y, ks), px(x1 - x0, ks).max(1), px(y1 - y0, ks).max(1));
        let (tw, th) = (px(x1 - x0, k).max(1) as u32, px(y1 - y0, k).max(1) as u32);
        let part = if part.dimensions() == (tw, th) { part } else { image::imageops::resize(&part, tw, th, image::imageops::FilterType::Triangle) };
        image::imageops::replace(&mut out, &part, px(x0 - x, k) as i64, px(y0 - y, k) as i64);
    }
    out
}

pub struct Overlay {
    pub shots: Vec<Shot>,
    textures: Vec<Option<TextureHandle>>,
    /// Fönstren på skrivbordet när bilden togs, det översta först.
    windows: Vec<[i32; 4]>,
    /// Alla skärmar tillsammans.
    bounds: Rect,
    /// Markeringen i skrivbordets enheter.
    sel: Option<Rect>,
    /// Skärmen dragningen började på, och var.
    drag_from: Option<(usize, Pos2)>,
    /// Pekarens läge under en dragning, så att alla skärmar kan rita den.
    cursor: Option<Pos2>,
    /// Markeringen kom från ett klick på ett fönster.
    from_click: bool,
    magnifier: bool,
    after: AfterCapture,
    pub outcome: Option<Outcome>,
    /// Sant när något ändrats som de andra skärmarna också ska rita.
    pub changed: bool,
}

impl Overlay {
    pub fn new(shots: Vec<Shot>, windows: Vec<[i32; 4]>, settings: &Settings) -> Self {
        let n = shots.len();
        let bounds = shots.iter().map(shot_rect).reduce(|a, b| a.union(b)).unwrap_or(Rect::ZERO);
        Self {
            shots,
            textures: vec![None; n],
            windows,
            bounds,
            // Utvecklingsläge: en färdig markering (x,y,b,h), för att prova ritningen utan mus.
            sel: std::env::var("FORCESHOT_MARKERING").ok().and_then(|v| {
                let n: Vec<f32> = v.split(',').filter_map(|n| n.trim().parse().ok()).collect();
                (n.len() == 4).then(|| Rect::from_min_size(pos2(n[0], n[1]), vec2(n[2], n[3])))
            }),
            drag_from: None,
            cursor: None,
            from_click: false,
            magnifier: settings.show_magnifier,
            after: settings.after_capture,
            outcome: None,
            changed: false,
        }
    }

    fn take(&mut self, then: Then) {
        if let Some(r) = self.sel {
            if r.width() >= 2.0 && r.height() >= 2.0 {
                self.outcome = Some(Outcome::Take { rect: [r.min.x.round() as i32, r.min.y.round() as i32, r.width().round() as i32, r.height().round() as i32], then });
            }
        }
    }

    /// Det som händer när en markering är klar: meny, eller direkt vidare.
    fn finish(&mut self) {
        match self.after {
            AfterCapture::Meny => {}
            AfterCapture::Redigera => self.take(Then::Editor),
            AfterCapture::Kopiera => self.take(Then::Copy),
            AfterCapture::Spara => self.take(Then::Save),
        }
    }

    fn monitor_at(&self, p: Pos2) -> Option<usize> {
        self.shots.iter().position(|s| shot_rect(s).contains(p))
    }

    /// Fönstret under en punkt, klippt till skärmarna.
    fn window_at(&self, p: Pos2) -> Option<Rect> {
        self.windows
            .iter()
            .map(|w| Rect::from_min_size(pos2(w[0] as f32, w[1] as f32), vec2(w[2] as f32, w[3] as f32)))
            .find(|r| r.contains(p))
            .map(|r| r.intersect(self.bounds))
            .filter(|r| r.width() >= 8.0 && r.height() >= 8.0)
    }

    pub fn ui(&mut self, ui: &mut Ui, i: usize) {
        let ctx = ui.ctx().clone();
        let ppp = ctx.pixels_per_point();
        let full = ctx.content_rect();
        let Some(shot) = self.shots.get(i) else { return };
        let (iw, ih) = (shot.image.width() as f32, shot.image.height() as f32);
        let tex = self.textures[i].get_or_insert_with(|| {
            let img = egui::ColorImage::from_rgba_unmultiplied([iw as usize, ih as usize], shot.image.as_raw());
            let opts = egui::TextureOptions { magnification: egui::TextureFilter::Nearest, ..egui::TextureOptions::LINEAR };
            ctx.load_texture(format!("skarm-{i}"), img, opts)
        });
        let tex_id = tex.id();
        let home = shot_rect(shot);
        // Skrivbordets enheter per punkt i fönstret, och bildpunkter per skrivbordsenhet.
        let (ux, uy) = (home.width() / full.width(), home.height() / full.height());
        let (kx, ky) = (iw / home.width(), ih / home.height());
        let to_desk = |p: Pos2| pos2((home.min.x + (p.x - full.min.x) * ux).round(), (home.min.y + (p.y - full.min.y) * uy).round());
        let to_scr = |p: Pos2| pos2(full.min.x + (p.x - home.min.x) / ux, full.min.y + (p.y - home.min.y) / uy);
        let bounds = self.bounds;
        let clamp = |p: Pos2| pos2(p.x.clamp(bounds.min.x, bounds.max.x), p.y.clamp(bounds.min.y, bounds.max.y));

        let resp = ui.allocate_rect(full, Sense::click_and_drag());
        // Utvecklingsläge: ta den förvalda markeringen direkt, som ett tryck på Redigera.
        if std::env::var_os("FORCESHOT_AUTOTA").is_some() && self.sel.is_some() {
            self.take(Then::Editor);
        }
        let painter = ui.painter_at(full);
        painter.image(tex_id, full, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);

        // ----- Mus -----
        let own = ctx.input(|inp| inp.pointer.latest_pos());
        if resp.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = ctx.input(|inp| inp.pointer.press_origin()) {
                self.drag_from = Some((i, to_desk(p)));
                self.sel = None;
                self.from_click = false;
            }
        }
        if let Some((m, from)) = self.drag_from.filter(|(m, _)| *m == i) {
            if resp.dragged_by(egui::PointerButton::Primary) {
                // Pekaren kan vara på en annan skärm än fönstret som äger dragningen.
                #[cfg(windows)]
                let p = {
                    let (x, y) = crate::platform::cursor_pos();
                    Some(pos2(x as f32, y as f32))
                };
                #[cfg(not(windows))]
                let p = own.map(to_desk);
                if let Some(p) = p.map(clamp) {
                    self.cursor = Some(p);
                    self.sel = Some(Rect::from_two_pos(from, p));
                    self.changed = true;
                }
                ctx.request_repaint();
            }
            if resp.drag_stopped() {
                let _ = m;
                self.drag_from = None;
                self.cursor = None;
                self.changed = true;
                match self.sel {
                    Some(r) if r.width() >= 3.0 && r.height() >= 3.0 => self.finish(),
                    _ => self.sel = None,
                }
            }
        }
        if resp.secondary_clicked() {
            if self.sel.is_some() {
                self.sel = None;
                self.changed = true;
            } else {
                self.outcome = Some(Outcome::Cancel);
            }
        }
        if resp.double_clicked() && (self.sel.is_none() || self.from_click) {
            self.sel = Some(home);
            self.take(Then::Editor);
        } else if resp.clicked() && self.sel.is_none() {
            // Ett klick tar fönstret under pekaren.
            if let Some(r) = own.and_then(|p| self.window_at(to_desk(p))) {
                self.sel = Some(r);
                self.from_click = true;
                self.changed = true;
                self.finish();
            }
        }

        // ----- Tangenter -----
        let (mut copy, mut save, mut enter, mut esc, mut all, mut toggle) = (false, false, false, false, false, false);
        let mut nudge = Vec2::ZERO;
        let (mut shift, mut ctrl) = (false, false);
        ctx.input(|inp| {
            shift = inp.modifiers.shift;
            ctrl = inp.modifiers.command || inp.modifiers.ctrl;
            copy = inp.events.iter().any(|e| matches!(e, egui::Event::Copy)) || (ctrl && inp.key_pressed(Key::C));
            save = ctrl && inp.key_pressed(Key::S);
            enter = inp.key_pressed(Key::Enter);
            esc = inp.key_pressed(Key::Escape);
            all = ctrl && inp.key_pressed(Key::A);
            toggle = !ctrl && inp.key_pressed(Key::M);
            for (k, d) in [(Key::ArrowLeft, vec2(-1.0, 0.0)), (Key::ArrowRight, vec2(1.0, 0.0)), (Key::ArrowUp, vec2(0.0, -1.0)), (Key::ArrowDown, vec2(0.0, 1.0))] {
                if inp.key_pressed(k) {
                    nudge += d;
                }
            }
        });
        if esc {
            self.outcome = Some(Outcome::Cancel);
        }
        if toggle {
            self.magnifier = !self.magnifier;
        }
        if all {
            // Första gången den här skärmen, andra gången alla skärmar.
            self.sel = Some(if self.sel == Some(home) { bounds } else { home });
            self.from_click = false;
            self.changed = true;
        }
        if nudge != Vec2::ZERO {
            if let Some(r) = &mut self.sel {
                let step = if shift { 10.0 } else { 1.0 };
                if ctrl {
                    r.max += nudge * step;
                    r.max = r.max.max(r.min + vec2(2.0, 2.0));
                } else {
                    *r = r.translate(nudge * step);
                }
                *r = r.intersect(bounds);
                self.changed = true;
            }
        }
        if enter {
            self.take(Then::Editor);
        }
        if copy {
            self.take(Then::Copy);
        }
        if save {
            self.take(Then::Save);
        }

        // ----- Ritning -----
        let dragging = self.drag_from.is_some();
        // Dimmar allt utom ett hål.
        let dim_around = |hole: Option<Rect>, color: Color32| match hole.map(|h| h.intersect(full)).filter(|h| h.width() > 0.0 && h.height() > 0.0) {
            Some(s) => {
                painter.rect_filled(Rect::from_min_max(full.min, pos2(full.max.x, s.min.y)), 0.0, color);
                painter.rect_filled(Rect::from_min_max(pos2(full.min.x, s.max.y), full.max), 0.0, color);
                painter.rect_filled(Rect::from_min_max(pos2(full.min.x, s.min.y), pos2(s.min.x, s.max.y)), 0.0, color);
                painter.rect_filled(Rect::from_min_max(pos2(s.max.x, s.min.y), pos2(full.max.x, s.max.y)), 0.0, color);
            }
            None => {
                painter.rect_filled(full, 0.0, color);
            }
        };
        let sel_scr = self.sel.map(|r| Rect::from_min_max(to_scr(r.min), to_scr(r.max)));
        if let (Some(r), Some(s)) = (self.sel, sel_scr) {
            dim_around(Some(s), Color32::from_black_alpha(120));
            painter.rect_stroke(s, 0.0, Stroke::new(1.5, theme::ACCENT), StrokeKind::Outside);
            // Måttetikett ovanför markeringen, på skärmen där markeringen börjar.
            if full.expand(1.0).contains(s.min) {
                let label = format!("{} × {}", (r.width() * kx).round() as u32, (r.height() * ky).round() as u32);
                let g = painter.layout_no_wrap(label, FontId::new(12.5, theme::bold()), theme::TEXT);
                let mut lp = pos2(s.min.x, s.min.y - g.size().y - 12.0);
                if lp.y < full.min.y + 4.0 {
                    lp.y = s.min.y + 6.0;
                    lp.x += 6.0;
                }
                let chip = Rect::from_min_size(lp, g.size() + vec2(14.0, 6.0));
                painter.rect_filled(chip, 6.0, Color32::from_rgba_unmultiplied(0x12, 0x14, 0x18, 0xE6));
                painter.galley(chip.min + vec2(7.0, 3.0), g, theme::TEXT);
            }
        } else {
            // Fönstret under pekaren lyfts fram: det är det ett klick tar.
            let hover = own.filter(|p| full.contains(*p) && !dragging).and_then(|p| self.window_at(to_desk(p))).map(|r| Rect::from_min_max(to_scr(r.min), to_scr(r.max)));
            dim_around(hover, Color32::from_black_alpha(70));
            if let Some(h) = hover {
                painter.rect_stroke(h.intersect(full), 0.0, Stroke::new(2.0, Color32::from_rgba_unmultiplied(0x2D, 0xD4, 0xBF, 0xC8)), StrokeKind::Inside);
            }
        }

        // Hårkors och förstoringsglas följer pekaren, även när dragningen ägs av en annan skärm.
        let pointer = if dragging { self.cursor.map(to_scr) } else { own };
        if let Some(p) = pointer.filter(|p| full.contains(*p)) {
            if self.sel.is_none() {
                let st = Stroke::new(1.0, Color32::from_rgba_unmultiplied(0x2D, 0xD4, 0xBF, 0xB0));
                painter.line_segment([pos2(full.min.x, p.y), pos2(full.max.x, p.y)], st);
                painter.line_segment([pos2(p.x, full.min.y), pos2(p.x, full.max.y)], st);
            }
            if self.magnifier && (self.sel.is_none() || dragging) {
                let size = 132.0;
                let zoom = 6.0;
                let mut c = p + vec2(26.0 + size / 2.0, 26.0 + size / 2.0);
                if c.x + size / 2.0 > full.max.x - 4.0 {
                    c.x = p.x - 26.0 - size / 2.0;
                }
                if c.y + size / 2.0 + 24.0 > full.max.y - 4.0 {
                    c.y = p.y - 26.0 - size / 2.0;
                }
                let box_r = Rect::from_center_size(c, Vec2::splat(size));
                let d = to_desk(p);
                let ip = pos2((d.x - home.min.x) * kx, (d.y - home.min.y) * ky);
                let half = size * ppp / zoom / 2.0;
                let uv = Rect::from_min_max(pos2((ip.x - half) / iw, (ip.y - half) / ih), pos2((ip.x + half) / iw, (ip.y + half) / ih));
                painter.rect_filled(box_r.expand(2.0), 10.0, Color32::from_rgb(0x12, 0x14, 0x18));
                painter.add(egui::epaint::RectShape::filled(box_r, 8.0, Color32::WHITE).with_texture(tex_id, uv));
                painter.rect_stroke(box_r, 8.0, Stroke::new(1.5, theme::ACCENT), StrokeKind::Outside);
                let cs = Stroke::new(1.0, theme::ACCENT);
                painter.line_segment([c - vec2(9.0, 0.0), c + vec2(9.0, 0.0)], cs);
                painter.line_segment([c - vec2(0.0, 9.0), c + vec2(0.0, 9.0)], cs);
                let txt = format!("{}, {}", ip.x as i32, ip.y as i32);
                let g = painter.layout_no_wrap(txt, FontId::proportional(11.5), theme::TEXT);
                let chip = Rect::from_center_size(pos2(c.x, box_r.max.y + 14.0), g.size() + vec2(12.0, 5.0));
                painter.rect_filled(chip, 5.0, Color32::from_rgba_unmultiplied(0x12, 0x14, 0x18, 0xE6));
                painter.galley(chip.min + vec2(6.0, 2.5), g, theme::TEXT);
            }
        }

        if self.sel.is_none() && !dragging && own.is_some_and(|p| full.contains(p)) {
            let hint = "Dra för att markera  ·  klick tar fönstret  ·  dubbelklick tar hela skärmen  ·  Esc avbryter";
            let g = painter.layout_no_wrap(hint.into(), FontId::proportional(13.0), theme::TEXT);
            let chip = Rect::from_center_size(pos2(full.center().x, full.min.y + 34.0), g.size() + vec2(24.0, 12.0));
            painter.rect_filled(chip, 8.0, Color32::from_rgba_unmultiplied(0x12, 0x14, 0x18, 0xE0));
            painter.galley(chip.min + vec2(12.0, 6.0), g, theme::TEXT);
        }

        // ----- Åtgärdsrad under markeringen, på skärmen där markeringen slutar -----
        let bar_on = self.sel.and_then(|r| self.monitor_at(r.max - vec2(1.0, 1.0)).or_else(|| self.shots.iter().position(|s| shot_rect(s).intersects(r))));
        if let (Some(s), false, true) = (sel_scr, dragging, bar_on == Some(i)) {
            let bar = vec2(318.0, 44.0);
            let mut pos = pos2((s.max.x - bar.x).clamp(full.min.x + 6.0, (full.max.x - bar.x - 6.0).max(full.min.x + 6.0)), s.max.y + 10.0);
            if pos.y + bar.y > full.max.y - 6.0 {
                pos.y = (s.max.y - bar.y - 10.0).clamp(full.min.y + 6.0, (full.max.y - bar.y - 6.0).max(full.min.y + 6.0));
            }
            let mut pick = None;
            egui::Area::new(egui::Id::unique(("klippmeny", i))).fixed_pos(pos).order(egui::Order::Foreground).show(&ctx, |ui| {
                egui::Frame::new().fill(theme::SURFACE).stroke(Stroke::new(1.0, theme::BORDER)).corner_radius(10).inner_margin(7.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if icons::labeled(ui, Icon::Select, "Redigera", true).on_hover_text("Enter").clicked() {
                            pick = Some(Some(Then::Editor));
                        }
                        if icons::labeled(ui, Icon::Copy, "Kopiera", false).on_hover_text("Ctrl+C").clicked() {
                            pick = Some(Some(Then::Copy));
                        }
                        if icons::labeled(ui, Icon::Save, "Spara", false).on_hover_text("Ctrl+S").clicked() {
                            pick = Some(Some(Then::Save));
                        }
                        if icons::sized_button(ui, Icon::Close, "Avbryt (Esc)", false, 30.0).clicked() {
                            pick = Some(None);
                        }
                    });
                });
            });
            match pick {
                Some(Some(t)) => self.take(t),
                Some(None) => self.outcome = Some(Outcome::Cancel),
                None => {}
            }
        }
        ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn shot(x: i32, y: i32, w: u32, h: u32, density: u32, c: [u8; 3]) -> Shot {
        Shot { x, y, w, h, image: Arc::new(RgbaImage::from_pixel(w * density, h * density, image::Rgba([c[0], c[1], c[2], 255]))) }
    }

    #[test]
    fn a_selection_across_two_screens_is_stitched() {
        // Röd skärm till vänster, blå till höger och 40 enheter längre ned.
        let shots = [shot(0, 0, 200, 100, 1, [255, 0, 0]), shot(200, 40, 100, 100, 1, [0, 0, 255])];
        let img = compose(&shots, [150, 20, 100, 60]);
        assert_eq!(img.dimensions(), (100, 60));
        assert_eq!(img.get_pixel(10, 10).0, [255, 0, 0, 255]);
        assert_eq!(img.get_pixel(49, 59).0, [255, 0, 0, 255]);
        // Ovanför den högra skärmen finns ingenting: svart.
        assert_eq!(img.get_pixel(60, 10).0, [0, 0, 0, 255]);
        assert_eq!(img.get_pixel(60, 30).0, [0, 0, 255, 255]);
    }

    #[test]
    fn a_selection_on_one_screen_is_a_plain_crop() {
        let shots = [shot(-300, 0, 300, 200, 1, [10, 20, 30]), shot(0, 0, 400, 300, 1, [200, 100, 50])];
        let img = compose(&shots, [-250, 50, 120, 80]);
        assert_eq!(img.dimensions(), (120, 80));
        assert!(img.pixels().all(|p| p.0 == [10, 20, 30, 255]));
    }

    #[test]
    fn the_densest_screen_decides_the_size() {
        // En tät skärm (2 bildpunkter per enhet) bredvid en vanlig.
        let shots = [shot(0, 0, 100, 100, 2, [255, 255, 255]), shot(100, 0, 100, 100, 1, [0, 255, 0])];
        let img = compose(&shots, [50, 0, 100, 50]);
        assert_eq!(img.dimensions(), (200, 100));
        assert_eq!(img.get_pixel(20, 20).0, [255, 255, 255, 255]);
        assert_eq!(img.get_pixel(180, 80).0, [0, 255, 0, 255]);
    }

    #[test]
    fn a_click_picks_the_topmost_window() {
        let shots = vec![shot(0, 0, 800, 600, 1, [0, 0, 0])];
        let o = Overlay::new(shots, vec![[100, 100, 200, 150], [50, 50, 600, 400], [700, 500, 400, 400]], &Settings::default());
        assert_eq!(o.window_at(pos2(150.0, 150.0)), Some(Rect::from_min_size(pos2(100.0, 100.0), vec2(200.0, 150.0))));
        assert_eq!(o.window_at(pos2(400.0, 300.0)), Some(Rect::from_min_size(pos2(50.0, 50.0), vec2(600.0, 400.0))));
        // Ett fönster som sticker ut utanför skärmen klipps vid kanten.
        assert_eq!(o.window_at(pos2(750.0, 550.0)), Some(Rect::from_min_max(pos2(700.0, 500.0), pos2(800.0, 600.0))));
        assert_eq!(o.window_at(pos2(20.0, 20.0)), None);
    }
}
