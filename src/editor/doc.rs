//! Dokumentet: en underlagsbild plus annoteringar som vektorobjekt i bildens eget
//! koordinatsystem. Ångra/gör om sparar bara objektlistan och en delad pekare till bilden.

use crate::settings::{MarkerStyle, Numbering};
use egui::{Color32, Pos2, Rect, Vec2, pos2, vec2};
use image::RgbaImage;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct TextData {
    pub text: String,
    pub size: f32,
    pub box_fill: Option<Color32>,
    pub box_border: Option<Color32>,
}

#[derive(Clone, Debug)]
pub struct WeldData {
    pub methods: Vec<String>,
    pub colors: Vec<Color32>,
    pub number: String,
    pub show_method: bool,
    pub style: MarkerStyle,
    pub scale: f32,
    /// Egen text i stället för metodkod.
    pub free_text: Option<String>,
}

impl WeldData {
    pub fn label(&self) -> String {
        let head = match &self.free_text {
            Some(t) => t.clone(),
            None if self.show_method => self.methods.join("+"),
            None => String::new(),
        };
        match (head.is_empty(), self.number.is_empty()) {
            (true, _) => self.number.clone(),
            (false, true) => head,
            (false, false) => format!("{head}-{}", self.number),
        }
    }
}

#[derive(Clone, Debug)]
pub struct FilmData {
    pub w_cm: f32,
    pub h_cm: f32,
    pub number: u32,
}

#[derive(Clone, Debug)]
pub enum Kind {
    Rect,
    Ellipse,
    Line,
    Arrow { double: bool },
    Free(Vec<Pos2>),
    Highlight,
    /// Radera element: ytan fylls i från omgivningen.
    Erase,
    Blur(f32),
    Mosaic(f32),
    Text(TextData),
    Weld(WeldData),
    Film(FilmData),
    Image(Arc<RgbaImage>),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Handle {
    /// Hörn och sidor: (-1|0|1, -1|0|1) anger vilken kant som flyttas.
    Edge(i8, i8),
    A,
    B,
    Rotate,
}

#[derive(Clone, Debug)]
pub struct Element {
    pub id: u64,
    pub kind: Kind,
    /// Två definierande punkter: motstående hörn för rutor, ändpunkter för linjer,
    /// skarvpunkt och brickans mitt för svetsmarkörer.
    pub a: Pos2,
    pub b: Pos2,
    /// Vridning i radianer kring mitten.
    pub rot: f32,
    pub stroke: Color32,
    pub fill: Color32,
    pub width: f32,
}

pub fn rotate(v: Vec2, angle: f32) -> Vec2 {
    let (s, c) = angle.sin_cos();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_sq();
    if len2 < 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

impl Element {
    pub fn is_box(&self) -> bool {
        matches!(
            self.kind,
            Kind::Rect | Kind::Ellipse | Kind::Highlight | Kind::Erase | Kind::Blur(_) | Kind::Mosaic(_) | Kind::Text(_) | Kind::Film(_) | Kind::Image(_)
        )
    }

    pub fn can_rotate(&self) -> bool {
        matches!(self.kind, Kind::Rect | Kind::Ellipse | Kind::Text(_) | Kind::Film(_) | Kind::Image(_))
    }

    /// Objekt som hämtar bildpunkter ur underlaget och därför måste räknas om när de flyttas.
    pub fn is_patch(&self) -> bool {
        matches!(self.kind, Kind::Highlight | Kind::Erase | Kind::Blur(_) | Kind::Mosaic(_))
    }

    pub fn same_sort(&self, other: &Kind) -> bool {
        std::mem::discriminant(&self.kind) == std::mem::discriminant(other)
    }

    pub fn rect(&self) -> Rect {
        Rect::from_two_pos(self.a, self.b)
    }

    pub fn center(&self) -> Pos2 {
        self.rect().center()
    }

    /// Punkten uttryckt i objektets ovridna koordinater.
    pub fn to_local(&self, p: Pos2) -> Pos2 {
        if self.rot == 0.0 {
            return p;
        }
        let c = self.center();
        c + rotate(p - c, -self.rot)
    }

    pub fn to_world(&self, p: Pos2) -> Pos2 {
        if self.rot == 0.0 {
            return p;
        }
        let c = self.center();
        c + rotate(p - c, self.rot)
    }

    /// Ungefärlig storlek på svetsbrickan, för träfftest.
    pub fn badge_rect(&self) -> Rect {
        if let Kind::Weld(w) = &self.kind {
            let chars = w.label().chars().count().max(1) as f32;
            let size = vec2(chars * 8.6 + 20.0, 25.0) * w.scale;
            Rect::from_center_size(self.b, size)
        } else {
            Rect::NOTHING
        }
    }

    pub fn hit(&self, p: Pos2, tol: f32) -> bool {
        match &self.kind {
            Kind::Line | Kind::Arrow { .. } => dist_to_segment(p, self.a, self.b) <= tol + self.width / 2.0,
            Kind::Free(pts) => pts.windows(2).any(|w| dist_to_segment(p, w[0], w[1]) <= tol + self.width / 2.0),
            Kind::Weld(w) => {
                self.badge_rect().expand(tol).contains(p)
                    || p.distance(self.a) <= 6.0 * w.scale + tol
                    || dist_to_segment(p, self.a, self.b) <= tol + 1.5
            }
            _ => self.rect().expand(tol).contains(self.to_local(p)),
        }
    }

    pub fn translate(&mut self, d: Vec2) {
        self.a += d;
        self.b += d;
        if let Kind::Free(pts) = &mut self.kind {
            for p in pts {
                *p += d;
            }
        }
    }

    /// Omslutande ruta i bildens koordinater, med hänsyn till vridning.
    pub fn bounds(&self) -> Rect {
        match &self.kind {
            Kind::Weld(_) => Rect::from_two_pos(self.a, self.a).union(self.badge_rect()),
            _ if self.rot != 0.0 => {
                let r = self.rect();
                Rect::from_points(&[r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()].map(|p| self.to_world(p)))
            }
            _ => self.rect(),
        }
    }

    /// Handtagen i bildens koordinater. `gap` är avståndet till vridhandtaget.
    pub fn handles(&self, gap: f32) -> Vec<(Handle, Pos2)> {
        let mut out = Vec::new();
        match &self.kind {
            Kind::Line | Kind::Arrow { .. } | Kind::Weld(_) => {
                out.push((Handle::A, self.a));
                out.push((Handle::B, self.b));
            }
            Kind::Free(_) => {}
            _ => {
                let r = self.rect();
                let c = r.center();
                let half = r.size() / 2.0;
                let edges: &[(i8, i8)] = match self.kind {
                    Kind::Text(_) => &[(-1, 0), (1, 0)],
                    Kind::Film(_) => &[(-1, -1), (1, -1), (1, 1), (-1, 1)],
                    _ => &[(-1, -1), (0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0)],
                };
                for &(ex, ey) in edges {
                    let p = c + vec2(ex as f32 * half.x, ey as f32 * half.y);
                    out.push((Handle::Edge(ex, ey), self.to_world(p)));
                }
                if self.can_rotate() {
                    out.push((Handle::Rotate, self.to_world(pos2(c.x, r.min.y - gap))));
                }
            }
        }
        out
    }

    /// Drar ett handtag till `p`. `orig` är objektet som det såg ut när draget började.
    pub fn drag_handle(&mut self, orig: &Element, h: Handle, p: Pos2, shift: bool) {
        match h {
            Handle::A | Handle::B => {
                let (moving, fixed) = if h == Handle::A { (&mut self.a, orig.b) } else { (&mut self.b, orig.a) };
                let mut q = p;
                if shift && !matches!(orig.kind, Kind::Weld(_)) {
                    q = fixed + snap45(p - fixed);
                }
                *moving = q;
            }
            Handle::Rotate => {
                let c = orig.center();
                let mut ang = (p - c).angle() + std::f32::consts::FRAC_PI_2;
                if shift {
                    let step = 15f32.to_radians();
                    ang = (ang / step).round() * step;
                }
                self.rot = ang;
            }
            Handle::Edge(ex, ey) => {
                let r = orig.rect();
                let c = r.center();
                let lp = orig.to_local(p);
                let (mut min, mut max) = (r.min, r.max);
                if ex < 0 {
                    min.x = lp.x.min(max.x - 2.0);
                } else if ex > 0 {
                    max.x = lp.x.max(min.x + 2.0);
                }
                if ey < 0 {
                    min.y = lp.y.min(max.y - 2.0);
                } else if ey > 0 {
                    max.y = lp.y.max(min.y + 2.0);
                }
                // Proportionerna låses för film alltid, och för inklistrade bilder i hörnen.
                let keep = matches!(orig.kind, Kind::Film(_)) || (matches!(orig.kind, Kind::Image(_)) && ex != 0 && ey != 0 && !shift);
                let square = shift && matches!(orig.kind, Kind::Rect | Kind::Ellipse) && ex != 0 && ey != 0;
                if (keep || square) && r.width() > 0.0 && r.height() > 0.0 {
                    let aspect = if square { 1.0 } else { r.width() / r.height() };
                    let w = (max.x - min.x).max((max.y - min.y) * aspect);
                    let hgt = w / aspect;
                    if ex < 0 { min.x = max.x - w } else { max.x = min.x + w }
                    if ey < 0 { min.y = max.y - hgt } else { max.y = min.y + hgt }
                }
                // Håll motstående kant stilla även när objektet är vridet.
                let nr = Rect::from_min_max(min, max);
                let world_c = c + rotate(nr.center() - c, orig.rot);
                let shift_v = world_c - nr.center();
                self.a = nr.min + shift_v;
                self.b = nr.max + shift_v;
            }
        }
    }
}

/// Låser en riktning till steg om 45 grader.
pub fn snap45(v: Vec2) -> Vec2 {
    let step = std::f32::consts::FRAC_PI_4;
    let ang = (v.angle() / step).round() * step;
    Vec2::angled(ang) * v.length()
}

#[derive(Clone)]
struct Snap {
    base: Arc<RgbaImage>,
    els: Vec<Element>,
}

pub struct Doc {
    pub uid: u64,
    pub title: String,
    pub path: Option<PathBuf>,
    pub base: Arc<RgbaImage>,
    pub els: Vec<Element>,
    pub sel: Vec<u64>,
    undo: Vec<Snap>,
    redo: Vec<Snap>,
    pub dirty: bool,
    pub zoom: f32,
    pub pan: Vec2,
    /// Sant tills vyn har anpassats till fönstret första gången.
    pub fit: bool,
    next_id: u64,
    /// Skissens skala för filmmarkeringar. 0 = inte kalibrerad.
    pub px_per_cm: f32,
}

impl Doc {
    pub fn new(uid: u64, title: String, image: RgbaImage, path: Option<PathBuf>) -> Self {
        Self {
            uid,
            title,
            path,
            base: Arc::new(image),
            els: Vec::new(),
            sel: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            dirty: false,
            zoom: 1.0,
            pan: Vec2::ZERO,
            fit: true,
            next_id: 1,
            px_per_cm: 0.0,
        }
    }

    pub fn size(&self) -> Vec2 {
        vec2(self.base.width() as f32, self.base.height() as f32)
    }

    pub fn image_rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size())
    }

    /// Sparar läget före en ändring.
    pub fn checkpoint(&mut self) {
        self.undo.push(Snap { base: self.base.clone(), els: self.els.clone() });
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(s) = self.undo.pop() {
            self.redo.push(Snap { base: std::mem::replace(&mut self.base, s.base), els: std::mem::replace(&mut self.els, s.els) });
            self.sel.clear();
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        if let Some(s) = self.redo.pop() {
            self.undo.push(Snap { base: std::mem::replace(&mut self.base, s.base), els: std::mem::replace(&mut self.els, s.els) });
            self.sel.clear();
            self.dirty = true;
        }
    }

    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn add(&mut self, mut el: Element) -> u64 {
        el.id = self.new_id();
        let id = el.id;
        self.els.push(el);
        id
    }

    pub fn get(&self, id: u64) -> Option<&Element> {
        self.els.iter().find(|e| e.id == id)
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Element> {
        self.els.iter_mut().find(|e| e.id == id)
    }

    pub fn remove(&mut self, id: u64) {
        self.els.retain(|e| e.id != id);
        self.sel.retain(|s| *s != id);
    }

    /// Översta objektet under punkten.
    pub fn hit(&self, p: Pos2, tol: f32) -> Option<u64> {
        self.els.iter().rev().find(|e| e.hit(p, tol)).map(|e| e.id)
    }

    /// Nästa lediga skarvnummer för de valda metoderna.
    pub fn next_weld_number(&self, methods: &[String], mode: Numbering) -> u32 {
        let mut max = 0;
        for e in &self.els {
            if let Kind::Weld(w) = &e.kind {
                let relevant = match mode {
                    Numbering::Gemensam => true,
                    Numbering::PerMetod => w.methods.first() == methods.first(),
                };
                if relevant {
                    if let Ok(n) = w.number.trim().parse::<u32>() {
                        max = max.max(n);
                    }
                }
            }
        }
        max + 1
    }

    pub fn next_film_number(&self) -> u32 {
        self.els.iter().filter_map(|e| if let Kind::Film(f) = &e.kind { Some(f.number) } else { None }).max().unwrap_or(0) + 1
    }

    /// Sätter skissens skala och ger alla filmer rätt storlek kring sin egen mitt.
    pub fn set_film_scale(&mut self, px_per_cm: f32) {
        self.px_per_cm = px_per_cm;
        for e in &mut self.els {
            if let Kind::Film(f) = &e.kind {
                let size = vec2(f.w_cm, f.h_cm) * px_per_cm;
                let r = Rect::from_center_size(e.center(), size);
                e.a = r.min;
                e.b = r.max;
            }
        }
    }

    fn remap(&mut self, f: impl Fn(Pos2) -> Pos2, quarter_turn: f32, mirror: bool) {
        for e in &mut self.els {
            let keeps_shape = matches!(e.kind, Kind::Text(_) | Kind::Film(_) | Kind::Image(_));
            if keeps_shape {
                let size = e.rect().size();
                let r = Rect::from_center_size(f(e.center()), size);
                e.a = r.min;
                e.b = r.max;
                e.rot = if mirror { -e.rot } else { e.rot + quarter_turn };
            } else {
                e.a = f(e.a);
                e.b = f(e.b);
                if mirror {
                    e.rot = -e.rot;
                }
                if let Kind::Free(pts) = &mut e.kind {
                    for p in pts {
                        *p = f(*p);
                    }
                }
            }
        }
    }

    /// Vrider bilden ett kvarts varv; annoteringarna följer med och förblir redigerbara.
    pub fn rotate90(&mut self, clockwise: bool) {
        self.checkpoint();
        let (w, h) = (self.base.width() as f32, self.base.height() as f32);
        if clockwise {
            self.base = Arc::new(image::imageops::rotate90(&*self.base));
            self.remap(|p| pos2(h - p.y, p.x), std::f32::consts::FRAC_PI_2, false);
        } else {
            self.base = Arc::new(image::imageops::rotate270(&*self.base));
            self.remap(|p| pos2(p.y, w - p.x), -std::f32::consts::FRAC_PI_2, false);
        }
        self.fit = true;
    }

    pub fn flip_horizontal(&mut self) {
        self.checkpoint();
        let w = self.base.width() as f32;
        self.base = Arc::new(image::imageops::flip_horizontal(&*self.base));
        self.remap(|p| pos2(w - p.x, p.y), 0.0, true);
    }

    pub fn crop(&mut self, r: Rect) {
        let r = r.intersect(self.image_rect());
        if r.width() < 2.0 || r.height() < 2.0 {
            return;
        }
        self.checkpoint();
        let (x, y) = (r.min.x.round() as i32, r.min.y.round() as i32);
        self.base = Arc::new(crate::imaging::crop(&self.base, x, y, r.width().round() as i32, r.height().round() as i32));
        let d = vec2(-(x as f32), -(y as f32));
        for e in &mut self.els {
            e.translate(d);
        }
        self.fit = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn el(kind: Kind, a: Pos2, b: Pos2) -> Element {
        Element { id: 0, kind, a, b, rot: 0.0, stroke: Color32::RED, fill: Color32::TRANSPARENT, width: 2.0 }
    }

    fn doc() -> Doc {
        Doc::new(1, "t".into(), RgbaImage::new(200, 100), None)
    }

    #[test]
    fn undo_redo_restores_elements() {
        let mut d = doc();
        d.checkpoint();
        d.add(el(Kind::Rect, pos2(1.0, 1.0), pos2(5.0, 5.0)));
        assert_eq!(d.els.len(), 1);
        d.undo();
        assert_eq!(d.els.len(), 0);
        d.redo();
        assert_eq!(d.els.len(), 1);
    }

    #[test]
    fn rotation_maps_points_and_back() {
        let mut d = doc();
        d.add(el(Kind::Line, pos2(10.0, 20.0), pos2(30.0, 40.0)));
        d.rotate90(true);
        assert_eq!(d.base.dimensions(), (100, 200));
        assert_eq!(d.els[0].a, pos2(80.0, 10.0));
        d.rotate90(false);
        assert_eq!(d.els[0].a, pos2(10.0, 20.0));
        assert_eq!(d.els[0].b, pos2(30.0, 40.0));
    }

    #[test]
    fn crop_moves_annotations() {
        let mut d = doc();
        d.add(el(Kind::Rect, pos2(50.0, 50.0), pos2(60.0, 60.0)));
        d.crop(Rect::from_min_max(pos2(40.0, 30.0), pos2(140.0, 90.0)));
        assert_eq!(d.base.dimensions(), (100, 60));
        assert_eq!(d.els[0].a, pos2(10.0, 20.0));
    }

    #[test]
    fn weld_numbers_count_per_method() {
        let mut d = doc();
        let weld = |m: &str, n: &str| {
            Kind::Weld(WeldData { methods: vec![m.into()], colors: vec![Color32::RED], number: n.into(), show_method: true, style: MarkerStyle::Avlang, scale: 1.0, free_text: None })
        };
        d.add(el(weld("RT", "1"), Pos2::ZERO, Pos2::ZERO));
        d.add(el(weld("RT", "2"), Pos2::ZERO, Pos2::ZERO));
        d.add(el(weld("UT", "7"), Pos2::ZERO, Pos2::ZERO));
        assert_eq!(d.next_weld_number(&["RT".into()], Numbering::PerMetod), 3);
        assert_eq!(d.next_weld_number(&["UT".into()], Numbering::PerMetod), 8);
        assert_eq!(d.next_weld_number(&["MT".into()], Numbering::PerMetod), 1);
        assert_eq!(d.next_weld_number(&["MT".into()], Numbering::Gemensam), 8);
    }

    #[test]
    fn rotated_box_hit_and_resize_keeps_opposite_corner() {
        let mut e = el(Kind::Rect, pos2(0.0, 0.0), pos2(100.0, 20.0));
        e.rot = std::f32::consts::FRAC_PI_2;
        // Vriden ett kvarts varv ligger rutan stående kring (50, 10).
        assert!(e.hit(pos2(50.0, 55.0), 0.0));
        assert!(!e.hit(pos2(95.0, 10.0), 0.0));
        let orig = e.clone();
        let fixed = orig.to_world(pos2(0.0, 0.0));
        let target = orig.to_world(pos2(140.0, 40.0));
        e.drag_handle(&orig, Handle::Edge(1, 1), target, false);
        let now = e.to_world(e.rect().min);
        assert!((now - fixed).length() < 0.01, "{now:?} vs {fixed:?}");
        assert!((e.rect().width() - 140.0).abs() < 0.01);
    }

    #[test]
    fn weld_label_formats() {
        let mut w = WeldData { methods: vec!["RT".into(), "MT".into()], colors: vec![], number: "7".into(), show_method: true, style: MarkerStyle::Avlang, scale: 1.0, free_text: None };
        assert_eq!(w.label(), "RT+MT-7");
        w.show_method = false;
        assert_eq!(w.label(), "7");
        w.free_text = Some("Reparerad".into());
        w.number.clear();
        assert_eq!(w.label(), "Reparerad");
    }
}
