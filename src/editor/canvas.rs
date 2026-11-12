//! Ritytan: zoom, panorering, verktyg, markering, handtag och textinmatning.

use super::doc::{Doc, Element, FilmData, Handle, Kind, TextData, WeldData, snap45};
use super::draw::{self, Res, TEXT_PAD, Xf};
use crate::settings::{FILM_FORMATS, Settings, col};
use crate::{imaging, theme};
use egui::{Color32, CursorIcon, FontId, Key, PointerButton, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, TextureHandle, TextureId, Ui, Vec2, pos2, vec2};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
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
    Erase,
    Blur,
    Mosaic,
    Crop,
}

impl Tool {
    /// Verktyget griper bara tag i objekt av samma sort som det själv ritar.
    fn matches(self, k: &Kind) -> bool {
        matches!(
            (self, k),
            (Tool::Text, Kind::Text(_))
                | (Tool::Rect, Kind::Rect)
                | (Tool::Ellipse, Kind::Ellipse)
                | (Tool::Line, Kind::Line)
                | (Tool::Arrow, Kind::Arrow { .. })
                | (Tool::Highlight, Kind::Highlight)
                | (Tool::Free, Kind::Free(_))
                | (Tool::Weld, Kind::Weld(_))
                | (Tool::Film, Kind::Film(_))
                | (Tool::Erase, Kind::Erase)
                | (Tool::Blur, Kind::Blur(_))
                | (Tool::Mosaic, Kind::Mosaic(_))
        )
    }
}

enum Drag {
    None,
    Pan,
    Move { last: Pos2, started: bool },
    Handle { id: u64, h: Handle, orig: Element },
    Create { id: u64, from: Pos2 },
    Marquee { from: Pos2, to: Pos2 },
    Crop { edge: (i8, i8), orig: Rect, from: Pos2 },
}

pub struct Canvas {
    pub tool: Tool,
    drag: Drag,
    pub editing: Option<u64>,
    focus_edit: bool,
    tex: HashMap<(u64, u64), (u64, TextureHandle)>,
    base_tex: HashMap<u64, (usize, TextureHandle)>,
    pub crop: Option<Rect>,
    /// Svetsmarkör eller film som dubbelklickats och ska visas i ändringsrutan.
    pub edit_dialog: Option<u64>,
}

fn chaikin(pts: &[Pos2]) -> Vec<Pos2> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(pts.len() * 2);
    out.push(pts[0]);
    for w in pts.windows(2) {
        out.push(w[0] + (w[1] - w[0]) * 0.25);
        out.push(w[0] + (w[1] - w[0]) * 0.75);
    }
    out.push(*pts.last().unwrap());
    out
}

fn color_image(img: &image::RgbaImage) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw())
}

impl Canvas {
    pub fn new() -> Self {
        Self { tool: Tool::Select, drag: Drag::None, editing: None, focus_edit: false, tex: HashMap::new(), base_tex: HashMap::new(), crop: None, edit_dialog: None }
    }

    pub fn set_tool(&mut self, tool: Tool, doc: Option<&mut Doc>) {
        if let Some(doc) = doc {
            self.finish_edit(doc);
            self.crop = (tool == Tool::Crop).then(|| doc.image_rect());
            if tool != Tool::Select {
                doc.sel.clear();
            }
        }
        self.tool = tool;
    }

    pub fn forget(&mut self, uid: u64) {
        self.base_tex.remove(&uid);
        self.tex.retain(|(d, _), _| *d != uid);
    }

    pub fn base_texture(&self, uid: u64) -> Option<TextureId> {
        self.base_tex.get(&uid).map(|(_, t)| t.id())
    }

    pub fn textures(&self, uid: u64) -> HashMap<u64, TextureId> {
        self.tex.iter().filter(|((d, _), _)| *d == uid).map(|((_, e), (_, t))| (*e, t.id())).collect()
    }

    fn busy_with(&self, doc: &Doc, id: u64) -> bool {
        match &self.drag {
            Drag::Move { started, .. } => *started && doc.sel.contains(&id),
            Drag::Handle { id: h, .. } | Drag::Create { id: h, .. } => *h == id,
            _ => false,
        }
    }

    /// Laddar upp underlaget och räknar om bearbetade utsnitt som har ändrats.
    pub fn sync_textures(&mut self, ctx: &egui::Context, doc: &Doc) {
        let ptr = Arc::as_ptr(&doc.base) as usize;
        if self.base_tex.get(&doc.uid).map(|(p, _)| *p) != Some(ptr) {
            let opts = egui::TextureOptions { magnification: egui::TextureFilter::Nearest, ..egui::TextureOptions::LINEAR };
            let t = ctx.load_texture(format!("bild-{}", doc.uid), color_image(&doc.base), opts);
            self.base_tex.insert(doc.uid, (ptr, t));
        }
        let live: Vec<u64> = doc.els.iter().map(|e| e.id).collect();
        self.tex.retain(|(d, e), _| *d != doc.uid || live.contains(e));
        for el in &doc.els {
            let key = match &el.kind {
                Kind::Image(img) => Arc::as_ptr(img) as u64,
                Kind::Highlight | Kind::Erase | Kind::Blur(_) | Kind::Mosaic(_) => {
                    let r = el.rect().intersect(doc.image_rect());
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    (r.min.x as i32, r.min.y as i32, r.max.x as i32, r.max.y as i32, ptr, el.stroke.to_array()).hash(&mut h);
                    match el.kind {
                        Kind::Blur(v) => (1u8, v.to_bits()).hash(&mut h),
                        Kind::Mosaic(v) => (2u8, v.to_bits()).hash(&mut h),
                        Kind::Erase => 4u8.hash(&mut h),
                        _ => 3u8.hash(&mut h),
                    }
                    h.finish()
                }
                _ => continue,
            };
            let slot = (doc.uid, el.id);
            if self.tex.get(&slot).map(|(k, _)| *k) == Some(key) || self.busy_with(doc, el.id) {
                continue;
            }
            let image = match &el.kind {
                Kind::Image(img) => Some((**img).clone()),
                _ => {
                    let r = el.rect().intersect(doc.image_rect());
                    if r.width() < 1.0 || r.height() < 1.0 {
                        None
                    } else {
                        let (x, y, w, h) = (r.min.x as i32, r.min.y as i32, r.width() as i32, r.height() as i32);
                        Some(match el.kind {
                            Kind::Blur(v) => {
                                // Oskärpan behöver omgivande bildpunkter som underlag, annars blir kanten platt.
                                let m = (v * 3.0) as i32;
                                let big = imaging::crop(&doc.base, x - m, y - m, w + 2 * m, h + 2 * m);
                                let ox = x - (x - m).max(0);
                                let oy = y - (y - m).max(0);
                                imaging::crop(&imaging::blur(&big, v), ox, oy, w, h)
                            }
                            Kind::Mosaic(v) => imaging::mosaic(&imaging::crop(&doc.base, x, y, w, h), v as u32),
                            Kind::Erase => imaging::inpaint(&doc.base, x, y, w, h),
                            _ => {
                                let c = el.stroke.to_array();
                                imaging::multiply(&imaging::crop(&doc.base, x, y, w, h), [c[0], c[1], c[2]])
                            }
                        })
                    }
                }
            };
            match image {
                Some(img) => {
                    let t = ctx.load_texture(format!("obj-{}-{}", doc.uid, el.id), color_image(&img), egui::TextureOptions::LINEAR);
                    self.tex.insert(slot, (key, t));
                }
                None => {
                    self.tex.remove(&slot);
                }
            }
        }
    }

    fn new_element(&self, tool: Tool, a: Pos2, b: Pos2, settings: &Settings, doc: &Doc) -> Element {
        let st = &settings.style;
        let mut el = Element { id: 0, kind: Kind::Rect, a, b, rot: 0.0, stroke: col(st.stroke), fill: col(st.fill), width: st.width };
        el.kind = match tool {
            Tool::Ellipse => Kind::Ellipse,
            Tool::Line => Kind::Line,
            Tool::Arrow => Kind::Arrow { double: st.arrow_double },
            Tool::Free => Kind::Free(vec![a]),
            Tool::Highlight => {
                el.stroke = col(st.highlight);
                Kind::Highlight
            }
            Tool::Erase => Kind::Erase,
            Tool::Blur => Kind::Blur(st.blur),
            Tool::Mosaic => Kind::Mosaic(st.mosaic),
            Tool::Text => {
                el.stroke = col(st.text_color);
                el.width = 2.0;
                Kind::Text(TextData {
                    text: String::new(),
                    size: st.text_size,
                    box_fill: st.text_box_fill.then(|| col(st.box_fill)),
                    box_border: st.text_box_border.then(|| col(st.box_border)),
                    align: st.text_align,
                })
            }
            Tool::Weld => {
                let methods = if st.methods.is_empty() { vec!["RT".to_string()] } else { st.methods.clone() };
                let colors = methods.iter().map(|m| settings.method_color(m)).collect();
                let number = doc.next_weld_number(&methods, settings.numbering).to_string();
                Kind::Weld(WeldData { methods, colors, number, show_method: st.show_method, style: st.marker_style, scale: st.marker_scale, free_text: None })
            }
            Tool::Film => {
                let (w_cm, h_cm) = FILM_FORMATS[st.film_format.min(FILM_FORMATS.len() - 1)];
                let scale = if doc.px_per_cm > 0.0 {
                    doc.px_per_cm
                } else if st.px_per_cm > 0.0 {
                    st.px_per_cm
                } else {
                    // Okalibrerad skiss: gissa att bilden är ungefär tre meter bred.
                    (doc.size().x / 300.0).max(1.5)
                };
                let r = Rect::from_center_size(a, vec2(w_cm, h_cm) * scale);
                el.a = r.min;
                el.b = r.max;
                el.width = 2.0;
                el.stroke = Color32::from_rgb(0x15, 0x65, 0xC0);
                if st.film_landscape {
                    el.rot = std::f32::consts::FRAC_PI_2;
                }
                Kind::Film(FilmData { w_cm, h_cm, number: doc.next_film_number() })
            }
            _ => Kind::Rect,
        };
        el
    }

    pub fn start_edit(&mut self, doc: &mut Doc, id: u64) {
        doc.checkpoint();
        doc.sel = vec![id];
        self.editing = Some(id);
        self.focus_edit = true;
    }

    pub fn finish_edit(&mut self, doc: &mut Doc) {
        if let Some(id) = self.editing.take() {
            let empty = matches!(doc.get(id).map(|e| &e.kind), Some(Kind::Text(t)) if t.text.trim().is_empty());
            if empty {
                doc.remove(id);
            }
        }
    }

    fn grab_same_sort(&mut self, doc: &mut Doc, p: Pos2, tol: f32) -> bool {
        let tool = self.tool;
        let hit = doc.els.iter().rev().find(|e| tool.matches(&e.kind) && e.hit(p, tol)).map(|e| e.id);
        if let Some(id) = hit {
            doc.sel = vec![id];
            self.drag = Drag::Move { last: p, started: false };
            true
        } else {
            false
        }
    }

    fn begin_drag(&mut self, doc: &mut Doc, settings: &Settings, p: Pos2, space: bool, shift: bool, tol: f32) {
        if space {
            self.drag = Drag::Pan;
            return;
        }
        if self.tool == Tool::Crop {
            if let Some(c) = self.crop {
                let near = |a: f32, b: f32| (a - b).abs() <= tol * 2.5;
                let ex = if near(p.x, c.min.x) { -1 } else if near(p.x, c.max.x) { 1 } else { 0 };
                let ey = if near(p.y, c.min.y) { -1 } else if near(p.y, c.max.y) { 1 } else { 0 };
                if (ex != 0 || ey != 0) && c.expand(tol * 2.5).contains(p) || c.contains(p) {
                    self.drag = Drag::Crop { edge: (ex, ey), orig: c, from: p };
                }
            }
            return;
        }
        if doc.sel.len() == 1 {
            let id = doc.sel[0];
            if let Some(el) = doc.get(id).cloned() {
                for (h, hp) in el.handles(26.0 / doc.zoom) {
                    if hp.distance(p) <= 9.0 / doc.zoom {
                        doc.checkpoint();
                        self.drag = Drag::Handle { id, h, orig: el };
                        return;
                    }
                }
            }
        }
        match self.tool {
            Tool::Select => {
                if let Some(id) = doc.hit(p, tol) {
                    if !doc.sel.contains(&id) {
                        if !shift {
                            doc.sel.clear();
                        }
                        doc.sel.push(id);
                    }
                    self.drag = Drag::Move { last: p, started: false };
                } else {
                    if !shift {
                        doc.sel.clear();
                    }
                    self.drag = Drag::Marquee { from: p, to: p };
                }
            }
            Tool::Film => {
                self.grab_same_sort(doc, p, tol);
            }
            tool => {
                if !self.grab_same_sort(doc, p, tol) {
                    doc.checkpoint();
                    let el = self.new_element(tool, p, p, settings, doc);
                    let id = doc.add(el);
                    doc.sel = vec![id];
                    self.drag = Drag::Create { id, from: p };
                }
            }
        }
    }

    fn update_drag(&mut self, doc: &mut Doc, p: Pos2, shift: bool, screen_delta: Vec2) {
        match &mut self.drag {
            Drag::None => {}
            Drag::Pan => doc.pan += screen_delta,
            Drag::Move { last, started } => {
                let d = p - *last;
                if d == Vec2::ZERO {
                    return;
                }
                if !*started {
                    doc.checkpoint();
                    *started = true;
                }
                *last = p;
                let sel = doc.sel.clone();
                for e in doc.els.iter_mut().filter(|e| sel.contains(&e.id)) {
                    e.translate(d);
                }
            }
            Drag::Handle { id, h, orig } => {
                if let Some(el) = doc.get_mut(*id) {
                    el.drag_handle(orig, *h, p, shift);
                }
            }
            Drag::Create { id, from } => {
                let from = *from;
                let min_step = 1.5 / doc.zoom;
                if let Some(el) = doc.get_mut(*id) {
                    match &mut el.kind {
                        Kind::Line | Kind::Arrow { .. } => el.b = if shift { from + snap45(p - from) } else { p },
                        Kind::Free(pts) => {
                            if pts.last().is_none_or(|l| l.distance(p) >= min_step) {
                                pts.push(p);
                            }
                            let r = Rect::from_points(pts);
                            el.a = r.min;
                            el.b = r.max;
                        }
                        Kind::Rect | Kind::Ellipse if shift => {
                            let d = p - from;
                            let side = d.x.abs().max(d.y.abs());
                            el.b = from + vec2(side * d.x.signum(), side * d.y.signum());
                        }
                        _ => el.b = p,
                    }
                }
            }
            Drag::Marquee { to, .. } => *to = p,
            Drag::Crop { edge, orig, from } => {
                let lim = doc.image_rect();
                let mut c = *orig;
                if *edge == (0, 0) {
                    let mut d = p - *from;
                    d.x = d.x.clamp(lim.min.x - orig.min.x, lim.max.x - orig.max.x);
                    d.y = d.y.clamp(lim.min.y - orig.min.y, lim.max.y - orig.max.y);
                    c = orig.translate(d);
                } else {
                    let q = pos2(p.x.clamp(lim.min.x, lim.max.x), p.y.clamp(lim.min.y, lim.max.y));
                    if edge.0 < 0 { c.min.x = q.x.min(c.max.x - 4.0) }
                    if edge.0 > 0 { c.max.x = q.x.max(c.min.x + 4.0) }
                    if edge.1 < 0 { c.min.y = q.y.min(c.max.y - 4.0) }
                    if edge.1 > 0 { c.max.y = q.y.max(c.min.y + 4.0) }
                }
                self.crop = Some(c);
            }
        }
    }

    fn end_drag(&mut self, doc: &mut Doc, settings: &mut Settings) {
        match std::mem::replace(&mut self.drag, Drag::None) {
            Drag::Create { id, .. } => {
                let mut remove = false;
                let mut edit = false;
                let mut ask = false;
                if let Some(el) = doc.get_mut(id) {
                    match &mut el.kind {
                        Kind::Line | Kind::Arrow { .. } => remove = el.a.distance(el.b) < 3.0,
                        Kind::Free(pts) => *pts = chaikin(pts),
                        Kind::Weld(w) => {
                            if el.a.distance(el.b) < 8.0 {
                                el.b = el.a + vec2(0.0, -46.0 * w.scale);
                            }
                            ask = true;
                        }
                        Kind::Text(_) => {
                            let r = el.rect();
                            let width = if r.width() < 40.0 { 260.0 } else { r.width() };
                            el.a = r.min;
                            el.b = r.min + vec2(width, 30.0);
                            edit = true;
                        }
                        _ => {
                            let r = el.rect();
                            el.a = r.min;
                            el.b = r.max;
                            remove = r.width() < 3.0 || r.height() < 3.0;
                        }
                    }
                }
                if remove {
                    doc.undo();
                } else if edit {
                    self.editing = Some(id);
                    self.focus_edit = true;
                } else if ask && !doc.auto_number {
                    self.edit_dialog = Some(id);
                }
            }
            Drag::Handle { id, orig, .. } => {
                if let (Kind::Film(f), Some(el)) = (&orig.kind, doc.get(id)) {
                    let scale = el.rect().width() / f.w_cm;
                    if scale > 0.05 {
                        doc.set_film_scale(scale);
                        settings.style.px_per_cm = scale;
                    }
                }
            }
            Drag::Marquee { from, to } => {
                let r = Rect::from_two_pos(from, to);
                if r.width() > 2.0 || r.height() > 2.0 {
                    let hits: Vec<u64> = doc.els.iter().filter(|e| r.intersects(e.bounds())).map(|e| e.id).collect();
                    for id in hits {
                        if !doc.sel.contains(&id) {
                            doc.sel.push(id);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn click(&mut self, doc: &mut Doc, settings: &Settings, p: Pos2, shift: bool, tol: f32) {
        let tool = self.tool;
        let same = doc.els.iter().rev().find(|e| tool.matches(&e.kind) && e.hit(p, tol)).map(|e| e.id);
        match tool {
            Tool::Crop => {}
            Tool::Select => match doc.hit(p, tol) {
                Some(id) if shift => {
                    if doc.sel.contains(&id) { doc.sel.retain(|s| *s != id) } else { doc.sel.push(id) }
                }
                Some(id) => doc.sel = vec![id],
                None => doc.sel.clear(),
            },
            Tool::Text => match same {
                Some(id) => self.start_edit(doc, id),
                None => {
                    doc.checkpoint();
                    let mut el = self.new_element(Tool::Text, p, p + vec2(260.0, 30.0), settings, doc);
                    el.a = p - vec2(TEXT_PAD, TEXT_PAD);
                    el.b = el.a + vec2(260.0, 30.0);
                    let id = doc.add(el);
                    doc.sel = vec![id];
                    self.editing = Some(id);
                    self.focus_edit = true;
                }
            },
            Tool::Weld | Tool::Film => match same {
                Some(id) => doc.sel = vec![id],
                None => {
                    doc.checkpoint();
                    let mut el = self.new_element(tool, p, p, settings, doc);
                    if let Kind::Weld(w) = &el.kind {
                        el.b = p + vec2(0.0, -46.0 * w.scale);
                    }
                    let id = doc.add(el);
                    doc.sel = vec![id];
                    // Skarvnumret frågas när skarven sätts ut, tills löpnumrering valts för bilden.
                    if tool == Tool::Weld && !doc.auto_number {
                        self.edit_dialog = Some(id);
                    }
                }
            },
            _ => match same {
                Some(id) => doc.sel = vec![id],
                None => doc.sel.clear(),
            },
        }
    }

    fn keyboard(&mut self, ctx: &egui::Context, doc: &mut Doc) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let mut pick: Option<Tool> = None;
        let (mut del, mut undo, mut redo, mut all, mut esc, mut enter) = (false, false, false, false, false, false);
        let mut turn = 0.0f32;
        let mut straighten = false;
        ctx.input(|i| {
            let cmd = i.modifiers.command || i.modifiers.ctrl;
            if !cmd && !i.modifiers.alt {
                for (k, t) in [
                    (Key::V, Tool::Select), (Key::T, Tool::Text), (Key::R, Tool::Rect), (Key::E, Tool::Ellipse), (Key::L, Tool::Line),
                    (Key::A, Tool::Arrow), (Key::H, Tool::Highlight), (Key::F, Tool::Free), (Key::W, Tool::Weld), (Key::G, Tool::Film),
                    (Key::D, Tool::Erase), (Key::O, Tool::Blur), (Key::M, Tool::Mosaic), (Key::C, Tool::Crop),
                ] {
                    if i.key_pressed(k) {
                        pick = Some(t);
                    }
                }
            }
            del = i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace);
            undo = cmd && !i.modifiers.shift && i.key_pressed(Key::Z);
            redo = cmd && (i.key_pressed(Key::Y) || (i.modifiers.shift && i.key_pressed(Key::Z)));
            all = cmd && i.key_pressed(Key::A);
            esc = i.key_pressed(Key::Escape);
            enter = i.key_pressed(Key::Enter);
            if cmd {
                let step = if i.modifiers.shift { 15f32 } else { 1.0 }.to_radians();
                if i.key_pressed(Key::ArrowLeft) { turn -= step }
                if i.key_pressed(Key::ArrowRight) { turn += step }
                straighten = i.key_pressed(Key::ArrowUp) || i.key_pressed(Key::ArrowDown);
            }
        });
        if let Some(t) = pick {
            self.set_tool(t, Some(doc));
        }
        if del && !doc.sel.is_empty() {
            doc.checkpoint();
            let sel = std::mem::take(&mut doc.sel);
            doc.els.retain(|e| !sel.contains(&e.id));
        }
        if undo { doc.undo() }
        if redo { doc.redo() }
        if all {
            self.tool = Tool::Select;
            self.crop = None;
            doc.sel = doc.els.iter().filter(|e| !matches!(e.kind, Kind::SheetFrame(_))).map(|e| e.id).collect();
        }
        if (turn != 0.0 || straighten) && !doc.sel.is_empty() {
            doc.checkpoint();
            let sel = doc.sel.clone();
            for e in doc.els.iter_mut().filter(|e| sel.contains(&e.id) && e.can_rotate()) {
                e.rot = if straighten { 0.0 } else { e.rot + turn };
            }
        }
        if self.tool == Tool::Crop {
            if enter {
                if let Some(c) = self.crop.take() {
                    doc.crop(c);
                }
                self.tool = Tool::Select;
            } else if esc {
                self.set_tool(Tool::Select, Some(doc));
            }
        } else if esc {
            if doc.sel.is_empty() { self.tool = Tool::Select } else { doc.sel.clear() }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui, doc: &mut Doc, settings: &mut Settings) {
        let ctx = ui.ctx().clone();
        let rect = ui.available_rect_before_wrap();
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, theme::BASE);

        let size = doc.size();
        if doc.fit && rect.width() > 10.0 {
            let avail = rect.size() - vec2(56.0, 56.0);
            doc.zoom = (avail.x / size.x).min(avail.y / size.y).clamp(0.02, 1.0);
            doc.pan = (rect.size() - size * doc.zoom) / 2.0;
            doc.fit = false;
        }

        // Rullhjulet zoomar kring pekaren.
        let (scroll, pinch, hover, space, shift) =
            ctx.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.pointer.hover_pos(), i.key_down(Key::Space), i.modifiers.shift));
        if let Some(p) = hover.filter(|p| rect.contains(*p)) {
            let factor = pinch * (scroll.y * 0.0028).exp();
            if (factor - 1.0).abs() > 1e-4 {
                let new = (doc.zoom * factor).clamp(0.03, 40.0);
                let local = p - rect.min - doc.pan;
                doc.pan = (p - rect.min) - local * (new / doc.zoom);
                doc.zoom = new;
            }
        }
        if resp.dragged_by(PointerButton::Middle) {
            doc.pan += resp.drag_delta();
        }

        let xf = Xf { s: doc.zoom, o: rect.min.to_vec2() + doc.pan };
        let tol = 5.0 / doc.zoom;
        let ip = ctx.input(|i| i.pointer.latest_pos()).map(|p| xf.inv(p));
        let space = space && self.editing.is_none();

        // Textrutor växer nedåt med sitt innehåll.
        for el in &mut doc.els {
            if let Kind::Text(td) = &el.kind {
                let h = draw::text_height(&ctx, el, td, xf.s);
                let r = el.rect();
                el.a = r.min;
                el.b = pos2(r.max.x, r.min.y + h);
            }
        }

        // ----- Mus -----
        let was_editing = self.editing.is_some();
        if was_editing && (resp.clicked() || resp.drag_started()) {
            self.finish_edit(doc);
        }
        if resp.drag_started_by(PointerButton::Primary) {
            if let Some(p0) = ctx.input(|i| i.pointer.press_origin()) {
                self.begin_drag(doc, settings, xf.inv(p0), space, shift, tol);
            }
        }
        if resp.dragged_by(PointerButton::Primary) {
            if let Some(p) = ip {
                self.update_drag(doc, p, shift, resp.drag_delta());
            }
        }
        if resp.drag_stopped() {
            self.end_drag(doc, settings);
        }
        if resp.double_clicked() {
            if let Some(id) = ip.and_then(|p| doc.hit(p, tol)) {
                match doc.get(id).map(|e| &e.kind) {
                    Some(Kind::Text(_)) => self.start_edit(doc, id),
                    Some(Kind::Weld(_) | Kind::Film(_)) => {
                        doc.sel = vec![id];
                        self.edit_dialog = Some(id);
                    }
                    _ => {}
                }
            }
        } else if resp.clicked() && !was_editing {
            if let Some(p) = ip {
                self.click(doc, settings, p, shift, tol);
            }
        }
        if resp.clicked() || resp.drag_started() {
            resp.request_focus();
            ctx.memory_mut(|m| m.surrender_focus(resp.id));
        }
        self.keyboard(&ctx, doc);

        // ----- Ritning -----
        self.sync_textures(&ctx, doc);
        let img_rect = xf.r(doc.image_rect());
        if doc.space {
            // Arbetsytan är vit så långt man ser.
            painter.rect_filled(rect, 0.0, Color32::WHITE);
        } else {
            painter.rect_filled(img_rect.translate(vec2(0.0, 5.0)).expand(5.0), 4.0, Color32::from_black_alpha(46));
            painter.rect_filled(img_rect.expand(1.0), 0.0, Color32::from_black_alpha(90));
        }
        if let Some(t) = self.base_texture(doc.uid) {
            painter.image(t, img_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let tex = self.textures(doc.uid);
        let res = Res { ctx: &ctx, tex: &tex, editing: self.editing, image_rect: doc.image_rect() };
        let mut shapes = Vec::new();
        for el in &doc.els {
            draw::element(el, &xf, &res, &mut shapes);
        }
        // I en arbetsyta får innehållet ligga utanför guiden; annars klipps det vid bildkanten.
        let clip = if doc.space { rect } else { img_rect.intersect(rect) };
        painter.with_clip_rect(clip).extend(shapes);
        if doc.space {
            let g = img_rect;
            let pts = [g.left_top(), g.right_top(), g.right_bottom(), g.left_bottom(), g.left_top()];
            painter.extend(Shape::dashed_line(&pts, Stroke::new(1.5, theme::ACCENT_DEEP), 8.0, 6.0));
            // Etiketten är också handtaget som flyttar guiden. Storleken är fast.
            let label = painter.layout_no_wrap("Guide – det här blir bilden · dra här för att flytta".into(), FontId::proportional(11.5), Color32::WHITE);
            let chip = Rect::from_min_size(g.left_top() - vec2(0.0, label.size().y + 10.0), label.size() + vec2(14.0, 6.0));
            let grip = ui.interact(chip.intersect(rect), ui.scope_id().with(("guide", doc.uid)), Sense::drag());
            if grip.drag_started() {
                doc.checkpoint();
            }
            if grip.dragged() {
                doc.move_guide(grip.drag_delta());
            }
            if grip.hovered() || grip.dragged() {
                ctx.set_cursor_icon(CursorIcon::Move);
            }
            painter.rect_filled(chip, 5.0, if grip.hovered() || grip.dragged() { theme::ACCENT } else { theme::ACCENT_DEEP });
            painter.galley(chip.min + vec2(7.0, 3.0), label, Color32::WHITE);
        }

        // Markering och handtag.
        let single = doc.sel.len() == 1;
        let mut cursor = match self.tool {
            Tool::Select => CursorIcon::Default,
            Tool::Text => CursorIcon::Text,
            _ => CursorIcon::Crosshair,
        };
        for el in doc.els.iter().filter(|e| doc.sel.contains(&e.id)) {
            if self.editing == Some(el.id) {
                continue;
            }
            let outline: Vec<Pos2> = match &el.kind {
                Kind::Line | Kind::Arrow { .. } => vec![],
                Kind::Weld(_) | Kind::Free(_) => {
                    let b = xf.r(el.bounds()).expand(4.0);
                    vec![b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom(), b.left_top()]
                }
                _ => {
                    let r = el.rect();
                    [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()].iter().map(|p| xf.p(el.to_world(*p))).collect()
                }
            };
            if !outline.is_empty() {
                painter.extend(Shape::dashed_line(&outline, Stroke::new(1.3, theme::ACCENT), 5.0, 4.0));
            }
            if single {
                for (h, hp) in el.handles(26.0 / doc.zoom) {
                    let sp = xf.p(hp);
                    if h == Handle::Rotate {
                        let top = xf.p(el.to_world(pos2(el.center().x, el.rect().min.y)));
                        painter.line_segment([top, sp], Stroke::new(1.2, theme::ACCENT));
                        painter.circle(sp, 5.0, Color32::WHITE, Stroke::new(1.5, theme::ACCENT));
                    } else {
                        let hr = Rect::from_center_size(sp, Vec2::splat(8.0));
                        painter.rect(hr, 2.0, Color32::WHITE, Stroke::new(1.5, theme::ACCENT), StrokeKind::Middle);
                    }
                    if ip.is_some_and(|p| p.distance(hp) <= 9.0 / doc.zoom) {
                        cursor = if h == Handle::Rotate { CursorIcon::Alias } else { CursorIcon::Grab };
                    }
                }
            }
        }
        if let Drag::Handle { id, h: Handle::Rotate, .. } = &self.drag {
            if let Some(el) = doc.get(*id) {
                let deg = el.rot.to_degrees().rem_euclid(360.0);
                let c = xf.p(el.center());
                let g = painter.layout_no_wrap(format!("{deg:.0}°"), FontId::new(12.0, theme::bold()), theme::TEXT);
                let chip = Rect::from_center_size(c, g.size() + vec2(12.0, 6.0));
                painter.rect_filled(chip, 5.0, Color32::from_rgba_unmultiplied(0x12, 0x14, 0x18, 0xE6));
                painter.galley(chip.min + vec2(6.0, 3.0), g, theme::TEXT);
            }
        }
        if let Drag::Marquee { from, to } = &self.drag {
            let r = xf.r(Rect::from_two_pos(*from, *to));
            painter.rect(r, 0.0, theme::accent_soft(), Stroke::new(1.0, theme::ACCENT), StrokeKind::Middle);
        }
        if let Some(c) = self.crop {
            let sr = xf.r(c);
            let dim = Color32::from_black_alpha(150);
            painter.rect_filled(Rect::from_min_max(img_rect.min, pos2(img_rect.max.x, sr.min.y)), 0.0, dim);
            painter.rect_filled(Rect::from_min_max(pos2(img_rect.min.x, sr.max.y), img_rect.max), 0.0, dim);
            painter.rect_filled(Rect::from_min_max(pos2(img_rect.min.x, sr.min.y), pos2(sr.min.x, sr.max.y)), 0.0, dim);
            painter.rect_filled(Rect::from_min_max(pos2(sr.max.x, sr.min.y), pos2(img_rect.max.x, sr.max.y)), 0.0, dim);
            painter.rect_stroke(sr, 0.0, Stroke::new(1.5, theme::ACCENT), StrokeKind::Outside);
            for t in [1.0 / 3.0, 2.0 / 3.0] {
                let st = Stroke::new(1.0, Color32::from_white_alpha(60));
                painter.line_segment([pos2(sr.min.x + sr.width() * t, sr.min.y), pos2(sr.min.x + sr.width() * t, sr.max.y)], st);
                painter.line_segment([pos2(sr.min.x, sr.min.y + sr.height() * t), pos2(sr.max.x, sr.min.y + sr.height() * t)], st);
            }
            for (x, y) in [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.0, 0.5), (1.0, 1.0), (0.5, 1.0), (0.0, 1.0), (0.0, 0.5)] {
                let hp = pos2(sr.min.x + sr.width() * x, sr.min.y + sr.height() * y);
                painter.rect(Rect::from_center_size(hp, Vec2::splat(9.0)), 2.0, Color32::WHITE, Stroke::new(1.5, theme::ACCENT), StrokeKind::Middle);
            }
            let g = painter.layout_no_wrap(format!("{} × {}   Enter beskär · Esc avbryter", c.width().round() as u32, c.height().round() as u32), FontId::proportional(12.5), theme::TEXT);
            let chip = Rect::from_min_size(pos2(sr.min.x, (sr.min.y - g.size().y - 14.0).max(rect.min.y + 4.0)), g.size() + vec2(14.0, 8.0));
            painter.rect_filled(chip, 6.0, Color32::from_rgba_unmultiplied(0x12, 0x14, 0x18, 0xE6));
            painter.galley(chip.min + vec2(7.0, 4.0), g, theme::TEXT);
        }

        // ----- Textinmatning direkt i rutan -----
        if let Some(id) = self.editing {
            let mut done = false;
            if let Some(el) = doc.get_mut(id) {
                let color = el.stroke;
                let r = xf.r(el.rect()).shrink(TEXT_PAD * xf.s);
                if let Kind::Text(td) = &mut el.kind {
                    let edit = egui::TextEdit::multiline(&mut td.text)
                        .font(FontId::new(td.size * xf.s, theme::bold()))
                        .text_color(color)
                        .frame(egui::Frame::NONE)
                        .margin(0.0)
                        .desired_rows(1)
                        .horizontal_align(match td.align {
                            1 => egui::Align::Center,
                            2 => egui::Align::Max,
                            _ => egui::Align::Min,
                        })
                        .desired_width(r.width());
                    let out = ui.put(Rect::from_min_size(r.min, vec2(r.width(), r.height().max(td.size * xf.s))), edit);
                    if self.focus_edit {
                        out.request_focus();
                        self.focus_edit = false;
                    }
                    if ctx.input(|i| i.key_pressed(Key::Escape)) {
                        done = true;
                    }
                } else {
                    done = true;
                }
            } else {
                done = true;
            }
            if done {
                self.finish_edit(doc);
            }
        }

        if resp.hovered() {
            if matches!(self.drag, Drag::Pan) || resp.dragged_by(PointerButton::Middle) {
                cursor = CursorIcon::Grabbing;
            } else if space {
                cursor = CursorIcon::Grab;
            } else if matches!(self.drag, Drag::Move { .. }) {
                cursor = CursorIcon::Move;
            } else if self.tool == Tool::Select && ip.is_some_and(|p| doc.hit(p, tol).is_some()) && cursor == CursorIcon::Default {
                cursor = CursorIcon::Move;
            }
            ctx.set_cursor_icon(cursor);
        }
    }
}
