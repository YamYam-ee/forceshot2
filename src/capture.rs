//! Markeringsöverlägget: ett ramlöst fönster per skärm som visar den frusna bilden,
//! dimmar den och låter användaren dra upp ett område.

use crate::icons::{self, Icon};
use crate::platform::Shot;
use crate::settings::{AfterCapture, Settings};
use crate::theme;
use egui::{Align2, Color32, FontId, Key, Pos2, Rect, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2, pos2, vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Then {
    Editor,
    Copy,
    Save,
}

pub enum Outcome {
    Cancel,
    /// Skärmens index och området i bildens egna bildpunkter (x, y, b, h).
    Take { monitor: usize, rect: [u32; 4], then: Then },
}

pub struct Overlay {
    pub shots: Vec<Shot>,
    textures: Vec<Option<TextureHandle>>,
    sel: Option<(usize, Rect)>,
    drag_from: Option<(usize, Pos2)>,
    magnifier: bool,
    after: AfterCapture,
    pub outcome: Option<Outcome>,
}

impl Overlay {
    pub fn new(shots: Vec<Shot>, settings: &Settings) -> Self {
        let n = shots.len();
        Self {
            shots,
            textures: vec![None; n],
            sel: None,
            drag_from: None,
            magnifier: settings.show_magnifier,
            after: settings.after_capture,
            outcome: None,
        }
    }

    fn take(&mut self, then: Then) {
        if let Some((m, r)) = self.sel {
            if r.width() >= 2.0 && r.height() >= 2.0 {
                self.outcome = Some(Outcome::Take {
                    monitor: m,
                    rect: [r.min.x as u32, r.min.y as u32, r.width() as u32, r.height() as u32],
                    then,
                });
            }
        }
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
        // Bildpunkt i den frusna bilden <-> punkt i fönstret.
        let sx = iw / full.width();
        let sy = ih / full.height();
        let to_img = |p: Pos2| pos2(((p.x - full.min.x) * sx).round().clamp(0.0, iw), ((p.y - full.min.y) * sy).round().clamp(0.0, ih));
        let to_scr = |p: Pos2| pos2(full.min.x + p.x / sx, full.min.y + p.y / sy);

        let resp = ui.allocate_rect(full, Sense::click_and_drag());
        let painter = ui.painter_at(full);
        painter.image(tex_id, full, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);

        // ----- Mus -----
        let pointer = ctx.input(|inp| inp.pointer.latest_pos());
        if resp.drag_started_by(egui::PointerButton::Primary) {
            if let Some(p) = ctx.input(|inp| inp.pointer.press_origin()) {
                self.drag_from = Some((i, to_img(p)));
                self.sel = None;
            }
        }
        if let (Some((m, from)), Some(p)) = (self.drag_from, pointer) {
            if m == i && resp.dragged_by(egui::PointerButton::Primary) {
                self.sel = Some((i, Rect::from_two_pos(from, to_img(p))));
            }
        }
        if resp.drag_stopped() && self.drag_from.is_some() {
            self.drag_from = None;
            match self.sel {
                Some((_, r)) if r.width() >= 3.0 && r.height() >= 3.0 => match self.after {
                    AfterCapture::Meny => {}
                    AfterCapture::Redigera => self.take(Then::Editor),
                    AfterCapture::Kopiera => self.take(Then::Copy),
                    AfterCapture::Spara => self.take(Then::Save),
                },
                _ => self.sel = None,
            }
        }
        if resp.secondary_clicked() {
            if self.sel.is_some() {
                self.sel = None;
            } else {
                self.outcome = Some(Outcome::Cancel);
            }
        }
        if resp.double_clicked() && self.sel.is_none() {
            self.sel = Some((i, Rect::from_min_size(Pos2::ZERO, vec2(iw, ih))));
            self.take(Then::Editor);
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
            self.sel = Some((i, Rect::from_min_size(Pos2::ZERO, vec2(iw, ih))));
        }
        if nudge != Vec2::ZERO {
            if let Some((m, r)) = &mut self.sel {
                if *m == i {
                    let step = if shift { 10.0 } else { 1.0 };
                    if ctrl {
                        r.max += nudge * step;
                        r.max = r.max.max(r.min + vec2(2.0, 2.0));
                    } else {
                        *r = r.translate(nudge * step);
                    }
                    *r = r.intersect(Rect::from_min_size(Pos2::ZERO, vec2(iw, ih)));
                }
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
        let dim = Color32::from_black_alpha(120);
        let own_sel = self.sel.filter(|(m, _)| *m == i).map(|(_, r)| r);
        if let Some(r) = own_sel {
            let s = Rect::from_min_max(to_scr(r.min), to_scr(r.max));
            painter.rect_filled(Rect::from_min_max(full.min, pos2(full.max.x, s.min.y)), 0.0, dim);
            painter.rect_filled(Rect::from_min_max(pos2(full.min.x, s.max.y), full.max), 0.0, dim);
            painter.rect_filled(Rect::from_min_max(pos2(full.min.x, s.min.y), pos2(s.min.x, s.max.y)), 0.0, dim);
            painter.rect_filled(Rect::from_min_max(pos2(s.max.x, s.min.y), pos2(full.max.x, s.max.y)), 0.0, dim);
            painter.rect_stroke(s, 0.0, Stroke::new(1.5, theme::ACCENT), StrokeKind::Outside);
            // Måttetikett ovanför markeringen.
            let label = format!("{} × {}", r.width() as u32, r.height() as u32);
            let g = painter.layout_no_wrap(label, FontId::new(12.5, theme::bold()), theme::TEXT);
            let mut lp = pos2(s.min.x, s.min.y - g.size().y - 12.0);
            if lp.y < full.min.y + 4.0 {
                lp.y = s.min.y + 6.0;
                lp.x += 6.0;
            }
            let chip = Rect::from_min_size(lp, g.size() + vec2(14.0, 6.0));
            painter.rect_filled(chip, 6.0, Color32::from_rgba_unmultiplied(0x12, 0x14, 0x18, 0xE6));
            painter.galley(chip.min + vec2(7.0, 3.0), g, theme::TEXT);
        } else {
            painter.rect_filled(full, 0.0, if self.sel.is_some() { dim } else { Color32::from_black_alpha(70) });
        }

        let dragging = self.drag_from.is_some();
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
                let ip = to_img(p);
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

        if self.sel.is_none() && !dragging && pointer.is_some_and(|p| full.contains(p)) {
            let hint = "Dra för att markera  ·  dubbelklick tar hela skärmen  ·  Esc avbryter";
            let g = painter.layout_no_wrap(hint.into(), FontId::proportional(13.0), theme::TEXT);
            let chip = Rect::from_center_size(pos2(full.center().x, full.min.y + 34.0), g.size() + vec2(24.0, 12.0));
            painter.rect_filled(chip, 8.0, Color32::from_rgba_unmultiplied(0x12, 0x14, 0x18, 0xE0));
            painter.galley(chip.min + vec2(12.0, 6.0), g, theme::TEXT);
        }

        // ----- Åtgärdsrad under markeringen -----
        if let (Some(r), false) = (own_sel, dragging) {
            let s = Rect::from_min_max(to_scr(r.min), to_scr(r.max));
            let bar = vec2(318.0, 44.0);
            let mut pos = pos2((s.max.x - bar.x).max(full.min.x + 6.0), s.max.y + 10.0);
            if pos.y + bar.y > full.max.y - 6.0 {
                pos.y = (s.max.y - bar.y - 10.0).max(full.min.y + 6.0);
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
        let _ = Align2::CENTER_CENTER;
        ctx.set_cursor_icon(egui::CursorIcon::Crosshair);
    }
}
