//! Redigerarfönstret: flikar, verktygsrad, alternativrad, statusrad, startsida,
//! inställningar och allt som rör öppna, spara och kopiera.

pub mod canvas;
pub mod doc;
pub mod draw;
pub mod render;

use crate::app::UserEvent;
use crate::icons::{self, Icon};
use crate::platform::Action;
use crate::settings::{AfterCapture, FILM_FORMATS, MarkerStyle, Numbering, Settings, col};
use crate::sketch::{self, SketchMessage, SketchTab};
use crate::theme;
use canvas::{Canvas, Tool};
use doc::{Doc, Element, Kind};
use egui::{Align, Color32, FontId, Key, Layout, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, Ui, Vec2, pos2, vec2};
use image::RgbaImage;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use winit::event_loop::EventLoopProxy;

const CLIP_MARKER: &str = "ForceShot-objekt";

pub enum Tab {
    Doc(Box<Doc>),
    Sketch(SketchTab),
}

impl Tab {
    fn title(&self) -> &str {
        match self {
            Tab::Doc(d) => &d.title,
            Tab::Sketch(s) => &s.title,
        }
    }
    fn dirty(&self) -> bool {
        matches!(self, Tab::Doc(d) if d.dirty)
    }
    fn uid(&self) -> u64 {
        match self {
            Tab::Doc(d) => d.uid,
            Tab::Sketch(s) => s.uid,
        }
    }
}

/// Sådant redigeraren ber programmet om.
pub enum Request {
    Capture(Action),
    CloseWindow,
    Quit,
    RebindHotkeys,
    /// Kör den hämtade Setup-filen och avsluta.
    RunInstaller(PathBuf),
}

enum JobKind {
    Copy,
    Save(PathBuf),
}

struct Job {
    uid: u64,
    kind: JobKind,
    warmed: bool,
}

enum Confirm {
    CloseTab(usize),
    CloseWindow,
}

pub struct Env<'a> {
    pub window: &'a Arc<winit::window::Window>,
    pub proxy: &'a EventLoopProxy<UserEvent>,
}

pub struct Editor {
    pub tabs: Vec<Tab>,
    pub active: usize,
    canvas: Canvas,
    pub requests: Vec<Request>,
    show_settings: bool,
    confirm: Option<Confirm>,
    toast: Option<(String, f64)>,
    jobs: Vec<Job>,
    clipboard: Option<arboard::Clipboard>,
    logo: Option<TextureHandle>,
    next_uid: u64,
    el_clip: Vec<Element>,
    style_gesture: bool,
    dialog_saved: Option<u64>,
    capture_count: u32,
    /// PDF-filer som läses in: filnamn och kanalen sidorna kommer på.
    loading: Vec<(String, std::sync::mpsc::Receiver<crate::pdf::PdfMsg>)>,
    ocr: Option<OcrJob>,
    update: Option<UpdateUi>,
    /// Pågående "Sök nu" i inställningarna, och texten som visar hur det gick.
    update_check: Option<std::sync::mpsc::Receiver<Result<Option<crate::update::Release>, String>>>,
    update_status: String,
    /// Autostartens läge, läst ur registret första gången inställningarna visas.
    autostart: Option<bool>,
}

struct UpdateUi {
    release: crate::update::Release,
    state: UpState,
}

enum UpState {
    Offer,
    Downloading { progress: Arc<std::sync::atomic::AtomicU64>, rx: std::sync::mpsc::Receiver<Result<PathBuf, String>> },
    Failed(String),
}

struct OcrJob {
    rx: std::sync::mpsc::Receiver<Result<Vec<String>, String>>,
    result: Option<Result<String, String>>,
}

const PALETTE: [[u8; 3]; 9] = [
    [0xE5, 0x39, 0x35],
    [0xFB, 0x8C, 0x00],
    [0xFD, 0xD8, 0x35],
    [0x43, 0xA0, 0x47],
    [0x00, 0xAC, 0xC1],
    [0x1E, 0x88, 0xE5],
    [0x8E, 0x24, 0xAA],
    [0x00, 0x00, 0x00],
    [0xFF, 0xFF, 0xFF],
];

const MARKERS: [[u8; 3]; 5] = [[0xFF, 0xEB, 0x3B], [0x9C, 0xF0, 0x6A], [0xFF, 0x9E, 0xC4], [0x7F, 0xE3, 0xF5], [0xFF, 0xB7, 0x4D]];

/// En rad runda färgprover plus en fri färgväljare. Sant om färgen ändrades.
fn swatches(ui: &mut Ui, current: &mut [u8; 4], palette: &[[u8; 3]]) -> bool {
    let mut changed = false;
    ui.spacing_mut().item_spacing.x = 3.0;
    for c in palette {
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(22.0), Sense::click());
        let color = Color32::from_rgb(c[0], c[1], c[2]);
        let selected = current[..3] == c[..] && current[3] > 0;
        ui.painter().circle_filled(rect.center(), 8.0, color);
        ui.painter().circle_stroke(rect.center(), 8.0, Stroke::new(1.0, Color32::from_white_alpha(40)));
        if selected {
            ui.painter().circle_stroke(rect.center(), 10.5, Stroke::new(1.6, theme::ACCENT));
        } else if resp.hovered() {
            ui.painter().circle_stroke(rect.center(), 10.5, Stroke::new(1.0, theme::TEXT_DIM));
        }
        if resp.clicked() {
            *current = [c[0], c[1], c[2], 255];
            changed = true;
        }
    }
    ui.spacing_mut().item_spacing.x = 6.0;
    changed |= ui.color_edit_button_srgba_unmultiplied(current).on_hover_text("Egen färg").changed();
    changed
}

fn dim(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(theme::TEXT_DIM).size(12.5));
}

fn divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 22.0), Sense::hover());
    ui.painter().line_segment([rect.center_top(), rect.center_bottom()], Stroke::new(1.0, theme::BORDER));
}

/// En tryckbar bricka, t.ex. en provningsmetod eller ett filmformat.
fn chip(ui: &mut Ui, text: &str, selected: bool, color: Option<Color32>) -> egui::Response {
    let g = ui.painter().layout_no_wrap(text.to_owned(), FontId::new(12.5, theme::bold()), Color32::WHITE);
    let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 20.0, 26.0), Sense::click());
    let (bg, fg) = match (color, selected) {
        (Some(c), true) => (c, Color32::WHITE),
        (Some(c), false) => (c.gamma_multiply(0.28), theme::TEXT_DIM),
        (None, true) => (theme::accent_soft(), theme::ACCENT),
        (None, false) => (if resp.hovered() { theme::HOVER } else { theme::ELEVATED }, theme::TEXT_DIM),
    };
    ui.painter().rect_filled(rect, 13.0, bg);
    if selected && color.is_some() {
        ui.painter().rect_stroke(rect, 13.0, Stroke::new(1.5, Color32::from_white_alpha(200)), StrokeKind::Inside);
    }
    ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
    resp
}

fn write_image(img: &RgbaImage, path: &Path, quality: u8) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if ext == "jpg" || ext == "jpeg" {
        let rgb = image::DynamicImage::ImageRgba8(img.clone()).to_rgb8();
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(std::io::BufWriter::new(file), quality.clamp(30, 100));
        enc.encode_image(&rgb).map_err(|e| e.to_string())
    } else {
        img.save(path).map_err(|e| e.to_string())
    }
}

impl Editor {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active: 0,
            canvas: Canvas::new(),
            requests: Vec::new(),
            show_settings: false,
            confirm: None,
            toast: None,
            jobs: Vec::new(),
            clipboard: None,
            logo: None,
            next_uid: 1,
            el_clip: Vec::new(),
            style_gesture: false,
            dialog_saved: None,
            capture_count: 0,
            loading: Vec::new(),
            ocr: None,
            update: None,
            update_check: None,
            update_status: String::new(),
            autostart: None,
        }
    }

    fn uid(&mut self) -> u64 {
        self.next_uid += 1;
        self.next_uid
    }

    fn say(&mut self, text: impl Into<String>) {
        self.toast = Some((text.into(), -1.0));
    }

    /// Visar rutan om att en nyare version finns.
    pub fn offer_update(&mut self, release: crate::update::Release) {
        self.update = Some(UpdateUi { release, state: UpState::Offer });
    }

    pub fn show_settings(&mut self) {
        self.show_settings = true;
    }

    fn doc(&mut self) -> Option<&mut Doc> {
        match self.tabs.get_mut(self.active) {
            Some(Tab::Doc(d)) => Some(d),
            _ => None,
        }
    }

    pub fn add_image(&mut self, image: RgbaImage, title: Option<String>, path: Option<PathBuf>, px_per_cm: f32) {
        self.add_tab(image, title, path, px_per_cm, true);
    }

    /// Lägger till en bildflik. Med `activate` falskt hamnar den i bakgrunden.
    fn add_tab(&mut self, image: RgbaImage, title: Option<String>, path: Option<PathBuf>, px_per_cm: f32, activate: bool) {
        let uid = self.uid();
        let title = title.unwrap_or_else(|| {
            self.capture_count += 1;
            format!("Klipp {}", self.capture_count)
        });
        let mut d = Doc::new(uid, title, image, path);
        d.px_per_cm = px_per_cm;
        self.tabs.push(Tab::Doc(Box::new(d)));
        if !activate && self.tabs.len() > 1 {
            return;
        }
        if let Some(Tab::Doc(old)) = self.tabs.get_mut(self.active) {
            self.canvas.finish_edit(old);
        }
        self.active = self.tabs.len() - 1;
        self.canvas.set_tool(self.canvas.tool, None);
        if self.canvas.tool == Tool::Crop {
            self.canvas.tool = Tool::Select;
        }
        self.canvas.crop = None;
        self.canvas.editing = None;
    }

    pub fn open_path(&mut self, path: &Path, ctx: &egui::Context) {
        let is_pdf = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        if is_pdf {
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let ctx = ctx.clone();
            self.loading.push((name, crate::pdf::load(path.to_path_buf(), move || ctx.request_repaint())));
            self.say("Läser PDF …");
            return;
        }
        match image::open(path) {
            Ok(img) => {
                let title = path.file_name().map(|n| n.to_string_lossy().to_string());
                self.add_image(img.to_rgba8(), title, Some(path.to_path_buf()), 0.0);
            }
            Err(e) => self.say(format!("Kunde inte öppna {}: {e}", path.display())),
        }
    }

    pub fn open_dialog(&mut self, ctx: &egui::Context) {
        let picked = rfd::FileDialog::new()
            .add_filter("Bilder och PDF", &["png", "jpg", "jpeg", "bmp", "pdf"])
            .add_filter("Bilder", &["png", "jpg", "jpeg", "bmp"])
            .add_filter("PDF", &["pdf"])
            .pick_files();
        for p in picked.unwrap_or_default() {
            self.open_path(&p, ctx);
        }
    }

    /// Läser av texten i den aktiva bildens underlag, i en bakgrundstråd.
    pub fn start_ocr(&mut self, ctx: &egui::Context) {
        let Some(d) = self.doc() else { return };
        let image = d.base.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::platform::ocr(&image));
            ctx.request_repaint();
        });
        self.ocr = Some(OcrJob { rx, result: None });
    }

    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().ok();
        }
        self.clipboard.as_mut()
    }

    fn clipboard_image(&mut self) -> Option<RgbaImage> {
        let img = self.clipboard()?.get_image().ok()?;
        RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.into_owned())
    }

    /// Öppnar bilden i urklippet som en ny flik.
    pub fn open_clipboard(&mut self) {
        match self.clipboard_image() {
            Some(img) => self.add_image(img, Some("Urklipp".into()), None, 0.0),
            None => self.say("Det finns ingen bild i urklippet"),
        }
    }

    pub fn copy_image(&mut self, img: &RgbaImage) -> bool {
        let data = arboard::ImageData { width: img.width() as usize, height: img.height() as usize, bytes: std::borrow::Cow::Borrowed(img.as_raw()) };
        self.clipboard().is_some_and(|c| c.set_image(data).is_ok())
    }

    /// Sparar ett klipp direkt i skärmklippsmappen, utan redigeraren.
    pub fn save_direct(&mut self, img: &RgbaImage, settings: &Settings) -> Result<PathBuf, String> {
        let ext = if settings.jpeg { "jpg" } else { "png" };
        let path = settings.save_folder.join(format!("{}.{ext}", settings.filename(img.width(), img.height())));
        write_image(img, &path, settings.jpeg_quality)?;
        Ok(path)
    }


    /// Provbild med ett objekt av varje sort. Används för att granska ritning och export
    /// utan att klicka runt: `TRK-ForceShot --demo [utfil.png]`.
    pub fn demo(&mut self, settings: &Settings, out: Option<PathBuf>) {
        use doc::{FilmData, TextData, WeldData};
        let mut img = RgbaImage::from_pixel(1100, 640, image::Rgba([0xF4, 0xF5, 0xF7, 255]));
        for (x, y, p) in img.enumerate_pixels_mut() {
            if x % 50 == 0 || y % 50 == 0 {
                *p = image::Rgba([0xE2, 0xE5, 0xEA, 255]);
            }
            if (250..370).contains(&y) && (60..1040).contains(&x) {
                let v = 150 + ((y as i32 - 250) - 60).unsigned_abs().min(60) as u8;
                *p = image::Rgba([v, v, (v as u16 + 8).min(255) as u8, 255]);
            }
            if (250..370).contains(&y) && (x == 340 || x == 620 || x == 900) {
                *p = image::Rgba([0x33, 0x38, 0x42, 255]);
            }
            if (470..476).contains(&y) && (80..520).contains(&x) && (x / 14) % 2 == 0 {
                *p = image::Rgba([0x22, 0x22, 0x22, 255]);
            }
        }
        self.add_image(img, Some("Provbild".into()), None, 0.0);
        let Some(d) = self.doc() else { return };
        let red = Color32::from_rgb(0xE5, 0x39, 0x35);
        let base = Element { id: 0, kind: Kind::Rect, a: pos2(0.0, 0.0), b: pos2(0.0, 0.0), rot: 0.0, stroke: red, fill: Color32::TRANSPARENT, width: 3.0 };
        let mut add = |kind: Kind, a: egui::Pos2, b: egui::Pos2, f: &dyn Fn(&mut Element)| {
            let mut e = Element { kind, a, b, ..base.clone() };
            f(&mut e);
            d.add(e);
        };
        let weld = |methods: &[&str], n: &str, style: MarkerStyle| {
            Kind::Weld(WeldData {
                methods: methods.iter().map(|m| m.to_string()).collect(),
                colors: methods.iter().map(|m| settings.method_color(m)).collect(),
                number: n.into(),
                show_method: true,
                style,
                scale: 1.3,
                free_text: None,
            })
        };
        let film = |w: f32, h: f32, n: u32| Kind::Film(FilmData { w_cm: w, h_cm: h, number: n });
        let blue = Color32::from_rgb(0x15, 0x65, 0xC0);
        add(film(10.0, 24.0, 1), pos2(300.0, 215.0), pos2(380.0, 407.0), &|e| { e.stroke = blue; e.width = 2.0 });
        add(film(10.0, 24.0, 2), pos2(580.0, 215.0), pos2(660.0, 407.0), &|e| { e.stroke = blue; e.width = 2.0 });
        add(film(6.0, 48.0, 3), pos2(736.0, 118.0), pos2(784.0, 502.0), &|e| { e.stroke = blue; e.width = 2.0; e.rot = std::f32::consts::FRAC_PI_2 });
        add(weld(&["RT"], "1", MarkerStyle::Avlang), pos2(340.0, 310.0), pos2(340.0, 150.0), &|_| {});
        add(weld(&["UT"], "2", MarkerStyle::Avlang), pos2(620.0, 310.0), pos2(560.0, 150.0), &|_| {});
        add(weld(&["RT", "MT"], "3", MarkerStyle::Avlang), pos2(900.0, 310.0), pos2(930.0, 150.0), &|_| {});
        add(weld(&["PT"], "4", MarkerStyle::Rektangel), pos2(180.0, 310.0), pos2(150.0, 190.0), &|_| {});
        add(Kind::Arrow { double: false }, pos2(120.0, 560.0), pos2(260.0, 500.0), &|_| {});
        add(Kind::Arrow { double: true }, pos2(300.0, 560.0), pos2(470.0, 560.0), &|e| e.stroke = Color32::from_rgb(0x1E, 0x88, 0xE5));
        add(Kind::Rect, pos2(840.0, 420.0), pos2(1010.0, 520.0), &|e| e.fill = Color32::from_rgba_unmultiplied(0xE5, 0x39, 0x35, 50));
        add(Kind::Ellipse, pos2(860.0, 540.0), pos2(1000.0, 610.0), &|e| { e.stroke = Color32::from_rgb(0x43, 0xA0, 0x47); e.rot = -0.3 });
        add(Kind::Line, pos2(560.0, 600.0), pos2(800.0, 600.0), &|e| { e.stroke = Color32::BLACK; e.width = 2.0 });
        add(Kind::Highlight, pos2(70.0, 455.0), pos2(300.0, 492.0), &|e| e.stroke = Color32::from_rgb(0xFF, 0xEB, 0x3B));
        add(Kind::Blur(8.0), pos2(360.0, 440.0), pos2(530.0, 500.0), &|_| {});
        add(Kind::Mosaic(12.0), pos2(880.0, 250.0), pos2(1040.0, 300.0), &|_| {});
        add(Kind::Erase, pos2(90.0, 330.0), pos2(250.0, 430.0), &|_| {});
        let wave: Vec<egui::Pos2> = (0..60).map(|i| pos2(560.0 + i as f32 * 4.0, 540.0 + (i as f32 * 0.35).sin() * 18.0)).collect();
        add(Kind::Free(wave), pos2(560.0, 520.0), pos2(800.0, 560.0), &|e| e.stroke = Color32::from_rgb(0x8E, 0x24, 0xAA));
        let text = |t: &str, fill: bool| {
            Kind::Text(TextData { text: t.into(), size: 20.0, box_fill: fill.then(|| Color32::from_rgba_unmultiplied(255, 255, 255, 242)), box_border: fill.then(|| Color32::from_rgb(0x00, 0xAC, 0xC1)) })
        };
        add(text("Mantelring 2 – omprovas efter reparation, åäö ÅÄÖ", true), pos2(60.0, 40.0), pos2(360.0, 70.0), &|e| { e.stroke = Color32::BLACK; e.width = 2.0 });
        add(text("Utan ruta", false), pos2(420.0, 40.0), pos2(620.0, 70.0), &|e| e.stroke = Color32::WHITE);
        add(text("Vriden text", true), pos2(700.0, 30.0), pos2(860.0, 60.0), &|e| { e.stroke = Color32::BLACK; e.width = 2.0; e.rot = 0.12 });
        d.dirty = false;
        if let Some(path) = out {
            let uid = d.uid;
            self.jobs.push(Job { uid, kind: JobKind::Save(path), warmed: false });
        }
    }

    pub fn open_sketch(&mut self, id: &str) {
        if let Some(i) = self.tabs.iter().position(|t| matches!(t, Tab::Sketch(s) if s.sketch_id == id)) {
            self.active = i;
            return;
        }
        if let Some(info) = sketch::catalog().into_iter().find(|s| s.id == id) {
            let uid = self.uid();
            self.tabs.push(Tab::Sketch(SketchTab::new(uid, &info)));
            self.active = self.tabs.len() - 1;
        }
    }

    pub fn on_sketch_message(&mut self, uid: u64, json: &str) {
        let Some(msg) = sketch::parse_message(json) else { return };
        let Some(i) = self.tabs.iter().position(|t| t.uid() == uid) else { return };
        match msg {
            SketchMessage::Scale(v) => {
                if let Tab::Sketch(s) = &mut self.tabs[i] {
                    s.px_per_cm = v;
                }
            }
            SketchMessage::File { name, bytes } => {
                let lower = name.to_ascii_lowercase();
                if lower.ends_with(".png") {
                    let scale = if let Tab::Sketch(s) = &self.tabs[i] { s.px_per_cm } else { 0.0 };
                    match image::load_from_memory(&bytes) {
                        // Bilden från skissen blir en ny flik, färdig att märka upp.
                        Ok(img) => self.add_image(img.to_rgba8(), Some(name.trim_end_matches(".png").to_string()), None, scale),
                        Err(e) => self.say(format!("Kunde inte läsa bilden från skissen: {e}")),
                    }
                } else {
                    let ext = lower.rsplit('.').next().unwrap_or("").to_string();
                    let mut dlg = rfd::FileDialog::new().set_file_name(&name);
                    if !ext.is_empty() {
                        dlg = dlg.add_filter(ext.to_uppercase(), &[ext.as_str()]);
                    }
                    if let Some(path) = dlg.save_file() {
                        match std::fs::write(&path, &bytes) {
                            Ok(()) => self.say(format!("Sparade {}", path.display())),
                            Err(e) => self.say(format!("Kunde inte spara: {e}")),
                        }
                    }
                }
            }
        }
    }

    /// Fönstret vill stängas. Frågar först om något är osparat.
    pub fn request_close(&mut self) {
        if self.tabs.iter().any(|t| t.dirty()) {
            self.confirm = Some(Confirm::CloseWindow);
        } else {
            self.close_all();
        }
    }

    fn close_all(&mut self) {
        for t in &self.tabs {
            self.canvas.forget(t.uid());
        }
        self.tabs.clear();
        self.active = 0;
        self.requests.push(Request::CloseWindow);
    }

    fn close_tab(&mut self, i: usize, force: bool) {
        if i >= self.tabs.len() {
            return;
        }
        if !force && self.tabs[i].dirty() {
            self.confirm = Some(Confirm::CloseTab(i));
            return;
        }
        let uid = self.tabs[i].uid();
        self.canvas.forget(uid);
        self.canvas.editing = None;
        self.canvas.crop = None;
        self.tabs.remove(i);
        if self.active >= i && self.active > 0 {
            self.active -= 1;
        }
    }

    fn queue_save(&mut self, settings: &Settings, ask: bool) {
        let Some(d) = self.doc() else { return };
        let (uid, w, h) = (d.uid, d.base.width(), d.base.height());
        let origin = d.path.clone();
        let default_ext = if settings.jpeg { "jpg" } else { "png" };
        let (folder, name) = match &origin {
            Some(p) => (p.parent().map(Path::to_path_buf).unwrap_or_default(), p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()),
            None => (settings.save_folder.clone(), format!("{}.{default_ext}", settings.filename(w, h))),
        };
        let path = if ask || origin.is_some() || settings.ask_filename {
            let _ = std::fs::create_dir_all(&folder);
            rfd::FileDialog::new()
                .set_directory(&folder)
                .set_file_name(&name)
                .add_filter("PNG", &["png"])
                .add_filter("JPEG", &["jpg", "jpeg"])
                .add_filter("BMP", &["bmp"])
                .save_file()
        } else {
            Some(folder.join(name))
        };
        if let Some(mut path) = path {
            if path.extension().is_none() {
                path.set_extension(default_ext);
            }
            self.jobs.push(Job { uid, kind: JobKind::Save(path), warmed: false });
        }
    }

    fn queue_copy(&mut self) {
        if let Some(d) = self.doc() {
            let uid = d.uid;
            self.jobs.push(Job { uid, kind: JobKind::Copy, warmed: false });
        }
    }

    /// Körs efter att bildrutan ritats: då finns texturer och bokstäver på grafikkortet.
    pub fn after_paint(&mut self, ctx: &egui::Context, rs: Option<egui_wgpu::RenderState>, settings: &Settings) {
        if self.jobs.is_empty() {
            return;
        }
        let Some(rs) = rs else { return };
        let mut job = self.jobs.remove(0);
        let Some(i) = self.tabs.iter().position(|t| t.uid() == job.uid) else { return };
        if !job.warmed {
            if let Tab::Doc(d) = &self.tabs[i] {
                let _ = render::shapes(ctx, &mut self.canvas, d, ctx.pixels_per_point());
            }
            job.warmed = true;
            self.jobs.insert(0, job);
            ctx.request_repaint();
            return;
        }
        let rendered = match &self.tabs[i] {
            Tab::Doc(d) => render::render(ctx, &rs, &mut self.canvas, d),
            _ => return,
        };
        let img = match rendered {
            Ok(img) => img,
            Err(e) => {
                self.say(e);
                return;
            }
        };
        match job.kind {
            JobKind::Copy => {
                if self.copy_image(&img) {
                    self.say("Kopierad till urklipp");
                } else {
                    self.say("Kunde inte kopiera till urklipp");
                }
            }
            JobKind::Save(path) => match write_image(&img, &path, settings.jpeg_quality) {
                Ok(()) => {
                    if let Tab::Doc(d) = &mut self.tabs[i] {
                        d.dirty = false;
                        d.title = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                        d.path = Some(path.clone());
                    }
                    self.say(format!("Sparad: {}", path.display()));
                }
                Err(e) => self.say(format!("Kunde inte spara: {e}")),
            },
        }
        ctx.request_repaint();
    }

    fn paste_elements(&mut self) {
        let clip = self.el_clip.clone();
        if clip.is_empty() {
            return;
        }
        if let Some(d) = self.doc() {
            d.checkpoint();
            d.sel.clear();
            for mut e in clip {
                e.translate(vec2(18.0, 18.0));
                let id = d.add(e);
                d.sel.push(id);
            }
            self.canvas.tool = Tool::Select;
        }
        // Nästa inklistring trappas ytterligare ett steg.
        for e in &mut self.el_clip {
            e.translate(vec2(18.0, 18.0));
        }
    }

    fn paste_image(&mut self, img: RgbaImage) {
        match self.tabs.get_mut(self.active) {
            Some(Tab::Doc(d)) => {
                d.checkpoint();
                let size = d.size();
                let (w, h) = (img.width() as f32, img.height() as f32);
                // En bildpunkt på en bildpunkt; krymps bara om den inte får plats.
                let k = (size.x / w).min(size.y / h).min(1.0);
                let r = Rect::from_center_size((size / 2.0).to_pos2(), vec2(w, h) * k);
                let id = d.add(Element { id: 0, kind: Kind::Image(Arc::new(img)), a: r.min, b: r.max, rot: 0.0, stroke: Color32::WHITE, fill: Color32::TRANSPARENT, width: 0.0 });
                d.sel = vec![id];
                self.canvas.tool = Tool::Select;
            }
            _ => self.add_image(img, Some("Urklipp".into()), None, 0.0),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context, settings: &Settings) {
        let typing = ctx.egui_wants_keyboard_input();
        let (mut save, mut save_as, mut close, mut next, mut prev, mut open, mut fit, mut zoom) = (false, false, false, false, false, false, false, 0i32);
        let (mut copy, mut cut) = (false, false);
        let mut paste_text: Option<String> = None;
        let mut paste_img: Option<Arc<egui::ColorImage>> = None;
        ctx.input(|i| {
            let cmd = i.modifiers.command || i.modifiers.ctrl;
            save = cmd && !i.modifiers.shift && i.key_pressed(Key::S);
            save_as = cmd && i.modifiers.shift && i.key_pressed(Key::S);
            close = cmd && i.key_pressed(Key::W);
            next = cmd && !i.modifiers.shift && i.key_pressed(Key::Tab);
            prev = cmd && i.modifiers.shift && i.key_pressed(Key::Tab);
            open = cmd && i.key_pressed(Key::O);
            fit = cmd && i.key_pressed(Key::Num0);
            if cmd && (i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals)) { zoom = 1 }
            if cmd && i.key_pressed(Key::Minus) { zoom = -1 }
            if !typing {
                for e in &i.events {
                    match e {
                        egui::Event::Copy => copy = true,
                        egui::Event::Cut => cut = true,
                        egui::Event::Paste(t) => paste_text = Some(t.clone()),
                        egui::Event::PasteImage(img) => paste_img = Some(img.clone()),
                        _ => {}
                    }
                }
            }
        });
        if save { self.queue_save(settings, false) }
        if save_as { self.queue_save(settings, true) }
        if close && !self.tabs.is_empty() { self.close_tab(self.active, false) }
        if !self.tabs.is_empty() {
            if next { self.active = (self.active + 1) % self.tabs.len() }
            if prev { self.active = (self.active + self.tabs.len() - 1) % self.tabs.len() }
        }
        if open { self.open_dialog(ctx) }
        if let Some(d) = self.doc() {
            if fit { d.fit = true }
            if zoom != 0 {
                let f = if zoom > 0 { 1.25 } else { 0.8 };
                d.zoom = (d.zoom * f).clamp(0.03, 40.0);
            }
        }
        if copy || cut {
            let picked: Vec<Element> = self.doc().map(|d| d.els.iter().filter(|e| d.sel.contains(&e.id)).cloned().collect()).unwrap_or_default();
            if picked.is_empty() {
                self.queue_copy();
            } else {
                let n = picked.len();
                self.el_clip = picked;
                if let Some(c) = self.clipboard() {
                    let _ = c.set_text(CLIP_MARKER);
                }
                if cut {
                    if let Some(d) = self.doc() {
                        d.checkpoint();
                        let sel = std::mem::take(&mut d.sel);
                        d.els.retain(|e| !sel.contains(&e.id));
                    }
                    // Det som klippts ut läggs tillbaka där det låg.
                    for e in &mut self.el_clip {
                        e.translate(vec2(-18.0, -18.0));
                    }
                }
                self.say(format!("{n} objekt kopierade"));
            }
        }
        if let Some(t) = paste_text {
            if t == CLIP_MARKER {
                self.paste_elements();
            }
        }
        if let Some(img) = paste_img {
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_srgba_unmultiplied()).collect();
            if let Some(rgba) = RgbaImage::from_raw(img.width() as u32, img.height() as u32, bytes) {
                self.paste_image(rgba);
            }
        }
    }

    // ------------------------------------------------------------------
    // Gränssnittet
    // ------------------------------------------------------------------

    pub fn ui(&mut self, ui: &mut Ui, settings: &mut Settings, env: &Env) {
        let ctx = ui.ctx().clone();
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        for p in dropped {
            self.open_path(&p, &ctx);
        }
        // Sidor ur PDF-filer som läses in i bakgrunden.
        let mut arrived = Vec::new();
        self.loading.retain(|(name, rx)| loop {
            match rx.try_recv() {
                Ok(m) => arrived.push((name.clone(), m)),
                Err(std::sync::mpsc::TryRecvError::Empty) => break true,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break false,
            }
        });
        for (name, msg) in arrived {
            match msg {
                crate::pdf::PdfMsg::Page { index, total, image } => {
                    let title = if total > 1 { format!("{name} – s. {}", index + 1) } else { name.clone() };
                    // Första sidan visas; resten läggs som flikar utan att störa det man gör.
                    self.add_tab(image, Some(title), None, 0.0, index == 0);
                    if index + 1 == crate::pdf::MAX_PAGES && total > crate::pdf::MAX_PAGES {
                        self.say(format!("Öppnade de första {} av {total} sidor", crate::pdf::MAX_PAGES));
                    }
                }
                crate::pdf::PdfMsg::Error(e) => self.say(e),
            }
        }
        self.shortcuts(&ctx, settings);
        if !ctx.input(|i| i.pointer.any_down()) {
            self.style_gesture = false;
        }
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }

        let bar = egui::Frame::new().fill(theme::SURFACE).inner_margin(egui::Margin::symmetric(10, 6));
        egui::Panel::top("toppen").exact_size(46.0).show_separator_line(false).frame(bar).show(ui, |ui| self.top_bar(ui, settings));

        let is_doc = matches!(self.tabs.get(self.active), Some(Tab::Doc(_)));
        if is_doc {
            let opt = egui::Frame::new().fill(theme::SURFACE).inner_margin(egui::Margin::symmetric(12, 5));
            egui::Panel::top("alternativ").exact_size(42.0).show_separator_line(false).frame(opt).show(ui, |ui| {
                let r = ui.max_rect();
                ui.painter().line_segment([pos2(r.min.x - 12.0, r.min.y - 5.0), pos2(r.max.x + 12.0, r.min.y - 5.0)], Stroke::new(1.0, theme::BORDER));
                ui.horizontal_centered(|ui| self.options_bar(ui, settings));
            });
            let status = egui::Frame::new().fill(theme::SURFACE).inner_margin(egui::Margin::symmetric(12, 3));
            egui::Panel::bottom("status").exact_size(28.0).show_separator_line(false).frame(status).show(ui, |ui| {
                ui.horizontal_centered(|ui| self.status_bar(ui));
            });
            let rail = egui::Frame::new().fill(theme::SURFACE).inner_margin(egui::Margin::symmetric(7, 8));
            egui::Panel::left("verktyg").exact_size(48.0).resizable(false).show_separator_line(false).frame(rail).show(ui, |ui| self.tool_rail(ui));
        }

        let overlay_open = self.show_settings || self.confirm.is_some() || self.ocr.is_some() || self.update.is_some() || ctx.any_popup_open();
        let active = self.active;
        for (i, t) in self.tabs.iter_mut().enumerate() {
            if let Tab::Sketch(s) = t {
                if i != active {
                    s.place(None);
                }
            }
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(theme::BASE)).show(ui, |ui| match self.tabs.get_mut(active) {
            Some(Tab::Doc(d)) => self.canvas.ui(ui, d, settings),
            Some(Tab::Sketch(s)) => {
                let rect = ui.available_rect_before_wrap();
                let proxy = env.proxy.clone();
                s.ensure(env.window, move |uid, body| {
                    let _ = proxy.send_event(UserEvent::Sketch(uid, body));
                });
                // Webbvyn ligger ovanpå allt egui ritar, så den göms medan en meny eller dialog är öppen.
                s.place((!overlay_open).then_some(rect));
                if s.run_test_script() {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
                }
                if let Some(e) = &s.error {
                    ui.centered_and_justified(|ui| ui.label(RichText::new(e).color(theme::DANGER)));
                }
            }
            None => self.home(ui, settings),
        });

        self.dialogs(&ctx, settings);
        self.draw_toast(&ctx);
    }

    fn logo(&mut self, ctx: &egui::Context) -> egui::TextureId {
        self.logo
            .get_or_insert_with(|| {
                let img = image::load_from_memory(include_bytes!("../../assets/icon-256.png")).map(|i| i.to_rgba8()).unwrap_or_else(|_| RgbaImage::new(1, 1));
                let ci = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                ctx.load_texture("logga", ci, egui::TextureOptions::LINEAR)
            })
            .id()
    }

    fn new_menu(&mut self, ui: &mut Ui) {
        ui.set_min_width(230.0);
        let item = |ui: &mut Ui, text: &str, hint: &str| -> bool {
            let r = ui.add(egui::Button::new(text).shortcut_text(hint).min_size(vec2(220.0, 28.0)));
            r.clicked()
        };
        if item(ui, "Klipp ett område", "PrintScreen") {
            self.requests.push(Request::Capture(Action::Region));
            ui.close();
        }
        if item(ui, "Hela skärmen", "Ctrl+PrtSc") {
            self.requests.push(Request::Capture(Action::FullScreen));
            ui.close();
        }
        if item(ui, "Öppna bild eller PDF …", "Ctrl+O") {
            ui.close();
            let ctx = ui.ctx().clone();
            self.open_dialog(&ctx);
        }
        if item(ui, "Klistra in från urklipp", "") {
            ui.close();
            self.open_clipboard();
        }
        ui.separator();
        for s in sketch::catalog() {
            if item(ui, &s.name, "Skiss") {
                ui.close();
                self.open_sketch(&s.id);
            }
        }
    }

    fn top_bar(&mut self, ui: &mut Ui, settings: &mut Settings) {
        let logo = self.logo(ui.ctx());
        ui.horizontal_centered(|ui| {
            ui.add(egui::Image::new((logo, Vec2::splat(24.0))));
            ui.menu_button(RichText::new("ForceShot").font(FontId::new(15.0, theme::bold())), |ui| {
                self.new_menu(ui);
                ui.separator();
                if ui.add(egui::Button::new("Inställningar").min_size(vec2(220.0, 28.0))).clicked() {
                    self.show_settings = true;
                    ui.close();
                }
                if ui.add(egui::Button::new("Avsluta ForceShot").min_size(vec2(220.0, 28.0))).clicked() {
                    self.requests.push(Request::Quit);
                    ui.close();
                }
            });
            divider(ui);

            // Flikar.
            let mut activate = None;
            let mut close = None;
            for (i, t) in self.tabs.iter().enumerate() {
                let mut title: String = t.title().chars().take(24).collect();
                if t.title().chars().count() > 24 {
                    title.push('…');
                }
                let active = i == self.active;
                let g = ui.painter().layout_no_wrap(title, FontId::proportional(13.0), theme::TEXT);
                let (rect, resp) = ui.allocate_exact_size(vec2(g.size().x + 52.0, 32.0), Sense::click());
                let bg = if active { theme::ELEVATED } else if resp.hovered() { theme::HOVER.gamma_multiply(0.6) } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(rect, 8.0, bg);
                if active {
                    ui.painter().line_segment([pos2(rect.min.x + 10.0, rect.max.y - 1.0), pos2(rect.max.x - 10.0, rect.max.y - 1.0)], Stroke::new(2.0, theme::ACCENT));
                }
                let dot = pos2(rect.min.x + 13.0, rect.center().y);
                match t {
                    Tab::Sketch(_) => ui.painter().circle_stroke(dot, 3.6, Stroke::new(1.4, theme::ACCENT)),
                    Tab::Doc(d) if d.dirty => ui.painter().circle_filled(dot, 3.4, theme::ACCENT),
                    Tab::Doc(_) => ui.painter().circle_filled(dot, 3.0, theme::TEXT_FAINT),
                };
                ui.painter().galley(pos2(rect.min.x + 24.0, rect.center().y - g.size().y / 2.0), g, if active { theme::TEXT } else { theme::TEXT_DIM });
                let x_rect = Rect::from_center_size(pos2(rect.max.x - 15.0, rect.center().y), Vec2::splat(18.0));
                let x_resp = ui.interact(x_rect, ui.scope_id().with(("stäng", i)), Sense::click());
                if active || resp.hovered() || x_resp.hovered() {
                    if x_resp.hovered() {
                        ui.painter().rect_filled(x_rect, 5.0, theme::ACTIVE);
                    }
                    icons::paint(ui.painter(), x_rect.shrink(3.0), Icon::Close, if x_resp.hovered() { theme::TEXT } else { theme::TEXT_DIM });
                }
                if x_resp.clicked() || resp.middle_clicked() {
                    close = Some(i);
                } else if resp.clicked() {
                    activate = Some(i);
                }
            }
            if let Some(i) = activate {
                if let Some(Tab::Doc(d)) = self.tabs.get_mut(self.active) {
                    self.canvas.finish_edit(d);
                }
                self.canvas.crop = None;
                if self.canvas.tool == Tool::Crop {
                    self.canvas.tool = Tool::Select;
                }
                self.active = i;
            }
            if let Some(i) = close {
                self.close_tab(i, false);
            }
            let plus = icons::sized_button(ui, Icon::Plus, "Nytt klipp, bild eller skiss", false, 30.0);
            egui::Popup::menu(&plus).show(|ui| self.new_menu(ui));

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let has_doc = matches!(self.tabs.get(self.active), Some(Tab::Doc(_)));
                ui.add_enabled_ui(has_doc, |ui| {
                    if icons::labeled(ui, Icon::Copy, "Kopiera", true).on_hover_text("Kopiera hela bilden (Ctrl+C)").clicked() {
                        self.queue_copy();
                    }
                    if icons::labeled(ui, Icon::Save, "Spara", false).on_hover_text("Ctrl+S").clicked() {
                        self.queue_save(settings, false);
                    }
                });
                if icons::sized_button(ui, Icon::Gear, "Inställningar", false, 30.0).clicked() {
                    self.show_settings = true;
                }
                if icons::sized_button(ui, Icon::Camera, "Klipp ett område", false, 30.0).clicked() {
                    self.requests.push(Request::Capture(Action::Region));
                }
            });
        });
    }

    fn tool_rail(&mut self, ui: &mut Ui) {
        const TOOLS: [(Tool, Icon, &str); 14] = [
            (Tool::Select, Icon::Select, "Markera (V)"),
            (Tool::Weld, Icon::Weld, "Svetsskarv (W)"),
            (Tool::Film, Icon::Film, "Filmmarkering (G)"),
            (Tool::Text, Icon::Text, "Text (T)"),
            (Tool::Arrow, Icon::Arrow, "Pil (A)"),
            (Tool::Line, Icon::Line, "Linje (L)"),
            (Tool::Rect, Icon::Rect, "Rektangel (R)"),
            (Tool::Ellipse, Icon::Ellipse, "Ellips (E)"),
            (Tool::Free, Icon::Free, "Frihand (F)"),
            (Tool::Highlight, Icon::Highlight, "Överstrykning (H)"),
            (Tool::Erase, Icon::Erase, "Radera element (D)"),
            (Tool::Blur, Icon::Blur, "Oskärpa (O)"),
            (Tool::Mosaic, Icon::Mosaic, "Mosaik (M)"),
            (Tool::Crop, Icon::Crop, "Beskär (C)"),
        ];
        ui.spacing_mut().item_spacing.y = 3.0;
        let mut pick = None;
        for (i, (tool, icon, tip)) in TOOLS.iter().enumerate() {
            if i == 1 || i == 3 || i == 9 || i == 13 {
                let (r, _) = ui.allocate_exact_size(vec2(34.0, 7.0), Sense::hover());
                ui.painter().line_segment([pos2(r.min.x + 6.0, r.center().y), pos2(r.max.x - 6.0, r.center().y)], Stroke::new(1.0, theme::BORDER));
            }
            if icons::button(ui, *icon, tip, self.canvas.tool == *tool).clicked() {
                pick = Some(*tool);
            }
        }
        if let Some(t) = pick {
            let d = match self.tabs.get_mut(self.active) {
                Some(Tab::Doc(d)) => Some(&mut **d),
                _ => None,
            };
            self.canvas.set_tool(t, d);
        }
    }

    fn options_bar(&mut self, ui: &mut Ui, settings: &mut Settings) {
        let Some(Tab::Doc(d)) = self.tabs.get_mut(self.active) else { return };
        ui.add_enabled_ui(d.can_undo(), |ui| {
            if icons::sized_button(ui, Icon::Undo, "Ångra (Ctrl+Z)", false, 30.0).clicked() {
                d.undo();
            }
        });
        ui.add_enabled_ui(d.can_redo(), |ui| {
            if icons::sized_button(ui, Icon::Redo, "Gör om (Ctrl+Y)", false, 30.0).clicked() {
                d.redo();
            }
        });
        divider(ui);

        let st = &mut settings.style;
        let mut changed = false;
        // Med markeringsverktyget styr raden det markerade objektets sort.
        let sel_kind = d.sel.first().and_then(|id| d.get(*id)).map(|e| e.kind.clone());
        let tool = match (self.canvas.tool, &sel_kind) {
            (Tool::Select, Some(k)) => match k {
                Kind::Text(_) => Tool::Text,
                Kind::Weld(_) => Tool::Weld,
                Kind::Film(_) => Tool::Film,
                Kind::Highlight => Tool::Highlight,
                Kind::Erase => Tool::Erase,
                Kind::Blur(_) => Tool::Blur,
                Kind::Mosaic(_) => Tool::Mosaic,
                Kind::Rect | Kind::Ellipse => Tool::Rect,
                Kind::Arrow { .. } => Tool::Arrow,
                Kind::Line | Kind::Free(_) => Tool::Line,
                Kind::Image(_) => Tool::Select,
            },
            (t, _) => t,
        };
        match tool {
            Tool::Select => {
                dim(ui, "Klicka på ett objekt för att flytta eller ändra det. Dra en ruta för att markera flera.");
            }
            Tool::Rect | Tool::Ellipse | Tool::Line | Tool::Arrow | Tool::Free => {
                dim(ui, "Färg");
                changed |= swatches(ui, &mut st.stroke, &PALETTE);
                divider(ui);
                dim(ui, "Tjocklek");
                changed |= ui.add(egui::Slider::new(&mut st.width, 1.0..=24.0).fixed_decimals(0)).changed();
                if matches!(tool, Tool::Rect | Tool::Ellipse) {
                    divider(ui);
                    let mut filled = st.fill[3] > 0;
                    if ui.checkbox(&mut filled, "Fyllning").changed() {
                        st.fill = if filled { [st.stroke[0], st.stroke[1], st.stroke[2], 70] } else { [0, 0, 0, 0] };
                        changed = true;
                    }
                    if filled {
                        changed |= ui.color_edit_button_srgba_unmultiplied(&mut st.fill).changed();
                    }
                }
                if tool == Tool::Arrow {
                    divider(ui);
                    changed |= ui.checkbox(&mut st.arrow_double, "Spets i båda ändar").changed();
                }
            }
            Tool::Highlight => {
                dim(ui, "Penna");
                changed |= swatches(ui, &mut st.highlight, &MARKERS);
            }
            Tool::Text => {
                dim(ui, "Storlek");
                changed |= ui.add(egui::DragValue::new(&mut st.text_size).range(8.0..=200.0).speed(0.5)).changed();
                divider(ui);
                dim(ui, "Text");
                changed |= swatches(ui, &mut st.text_color, &PALETTE[5..]);
                divider(ui);
                changed |= ui.checkbox(&mut st.text_box_fill, "Bakgrund").changed();
                if st.text_box_fill {
                    changed |= ui.color_edit_button_srgba_unmultiplied(&mut st.box_fill).changed();
                }
                changed |= ui.checkbox(&mut st.text_box_border, "Ram").changed();
                if st.text_box_border {
                    changed |= ui.color_edit_button_srgba_unmultiplied(&mut st.box_border).changed();
                }
            }
            Tool::Weld => {
                dim(ui, "Metod");
                ui.spacing_mut().item_spacing.x = 4.0;
                let ctrl = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
                for m in &settings.methods {
                    let on = st.methods.contains(&m.code);
                    let c = Color32::from_rgb(m.color[0], m.color[1], m.color[2]);
                    let tip = format!("{} — Ctrl+klick lägger till fler metoder", m.name);
                    if chip(ui, &m.code, on, Some(c)).on_hover_text(tip).clicked() {
                        if ctrl {
                            if on && st.methods.len() > 1 { st.methods.retain(|x| x != &m.code) } else if !on { st.methods.push(m.code.clone()) }
                        } else {
                            st.methods = vec![m.code.clone()];
                        }
                    }
                }
                ui.spacing_mut().item_spacing.x = 6.0;
                divider(ui);
                let next = d.next_weld_number(&st.methods, settings.numbering);
                dim(ui, &format!("Nästa nr {next}"));
                divider(ui);
                changed |= ui.checkbox(&mut st.show_method, "Visa metod").changed();
                egui::ComboBox::from_id_salt("brickform")
                    .selected_text(if st.marker_style == MarkerStyle::Avlang { "Avlång" } else { "Rektangel" })
                    .width(92.0)
                    .show_ui(ui, |ui| {
                        changed |= ui.selectable_value(&mut st.marker_style, MarkerStyle::Avlang, "Avlång").changed();
                        changed |= ui.selectable_value(&mut st.marker_style, MarkerStyle::Rektangel, "Rektangel").changed();
                    });
                dim(ui, "Storlek");
                changed |= ui.add(egui::Slider::new(&mut st.marker_scale, 0.6..=3.0).fixed_decimals(1)).changed();
            }
            Tool::Film => {
                dim(ui, "Format");
                ui.spacing_mut().item_spacing.x = 4.0;
                for (i, (w, h)) in FILM_FORMATS.iter().enumerate() {
                    if chip(ui, &format!("{}×{}", *w as u32, *h as u32), st.film_format == i, None).clicked() {
                        st.film_format = i;
                    }
                }
                ui.spacing_mut().item_spacing.x = 6.0;
                divider(ui);
                ui.checkbox(&mut st.film_landscape, "Liggande");
                divider(ui);
                if d.px_per_cm > 0.0 {
                    dim(ui, &format!("Skala {:.1} px/cm", d.px_per_cm));
                } else {
                    dim(ui, "Dra i första filmens hörn för att ställa in skalan");
                }
            }
            Tool::Erase => {
                dim(ui, "Dra en ruta över det som ska bort. Ytan fylls i med omgivningens innehåll och går att flytta och ändra i efterhand.");
            }
            Tool::Blur => {
                dim(ui, "Styrka");
                changed |= ui.add(egui::Slider::new(&mut st.blur, 2.0..=40.0).fixed_decimals(0)).changed();
            }
            Tool::Mosaic => {
                dim(ui, "Rutstorlek");
                changed |= ui.add(egui::Slider::new(&mut st.mosaic, 4.0..=60.0).fixed_decimals(0)).changed();
            }
            Tool::Crop => {
                if icons::labeled(ui, Icon::Crop, "Beskär", true).clicked() {
                    if let Some(c) = self.canvas.crop.take() {
                        d.crop(c);
                    }
                    self.canvas.tool = Tool::Select;
                }
                if ui.button("Avbryt").clicked() {
                    self.canvas.crop = None;
                    self.canvas.tool = Tool::Select;
                }
            }
        }

        // Ändringar i raden gäller också det som är markerat.
        if changed && !d.sel.is_empty() {
            if !self.style_gesture {
                d.checkpoint();
                self.style_gesture = true;
            }
            let sel = d.sel.clone();
            for e in d.els.iter_mut().filter(|e| sel.contains(&e.id)) {
                match &mut e.kind {
                    Kind::Rect | Kind::Ellipse => {
                        e.stroke = col(st.stroke);
                        e.width = st.width;
                        e.fill = col(st.fill);
                    }
                    Kind::Line | Kind::Free(_) => {
                        e.stroke = col(st.stroke);
                        e.width = st.width;
                    }
                    Kind::Arrow { double } => {
                        e.stroke = col(st.stroke);
                        e.width = st.width;
                        *double = st.arrow_double;
                    }
                    Kind::Highlight => e.stroke = col(st.highlight),
                    Kind::Blur(v) => *v = st.blur,
                    Kind::Mosaic(v) => *v = st.mosaic,
                    Kind::Text(t) => {
                        e.stroke = col(st.text_color);
                        t.size = st.text_size;
                        t.box_fill = st.text_box_fill.then(|| col(st.box_fill));
                        t.box_border = st.text_box_border.then(|| col(st.box_border));
                    }
                    Kind::Weld(w) => {
                        w.show_method = st.show_method;
                        w.style = st.marker_style;
                        w.scale = st.marker_scale;
                    }
                    _ => {}
                }
            }
        }

        let mut start_ocr = false;
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icons::sized_button(ui, Icon::Ocr, "Läs av text i bilden (OCR)", false, 30.0).clicked() {
                start_ocr = true;
            }
            divider(ui);
            if icons::sized_button(ui, Icon::Flip, "Spegelvänd", false, 30.0).clicked() {
                d.flip_horizontal();
            }
            if icons::sized_button(ui, Icon::RotateRight, "Rotera medurs", false, 30.0).clicked() {
                d.rotate90(true);
            }
            if icons::sized_button(ui, Icon::RotateLeft, "Rotera moturs", false, 30.0).clicked() {
                d.rotate90(false);
            }
            if !d.sel.is_empty() {
                divider(ui);
                if icons::sized_button(ui, Icon::Trash, "Ta bort markerat (Delete)", false, 30.0).clicked() {
                    d.checkpoint();
                    let sel = std::mem::take(&mut d.sel);
                    d.els.retain(|e| !sel.contains(&e.id));
                }
            }
        });
        if start_ocr {
            let ctx = ui.ctx().clone();
            self.start_ocr(&ctx);
        }
    }

    fn status_bar(&mut self, ui: &mut Ui) {
        let tool = self.canvas.tool;
        let Some(Tab::Doc(d)) = self.tabs.get_mut(self.active) else { return };
        dim(ui, &format!("{} × {} px", d.base.width(), d.base.height()));
        divider(ui);
        dim(ui, &format!("{} objekt", d.els.len()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if icons::sized_button(ui, Icon::Fit, "Anpassa till fönstret (Ctrl+0)", false, 22.0).clicked() {
                d.fit = true;
            }
            if ui.add(egui::Button::new(RichText::new("+").size(13.0)).frame(false)).clicked() {
                d.zoom = (d.zoom * 1.25).min(40.0);
            }
            if ui.add(egui::Button::new(RichText::new(format!("{:.0} %", d.zoom * 100.0)).size(12.0).color(theme::TEXT_DIM)).frame(false)).on_hover_text("Klicka för 100 %").clicked() {
                d.zoom = 1.0;
            }
            if ui.add(egui::Button::new(RichText::new("−").size(13.0)).frame(false)).clicked() {
                d.zoom = (d.zoom * 0.8).max(0.03);
            }
            divider(ui);
            let hint = match tool {
                Tool::Select => "Rullhjul zoomar · mellanslag + dra panorerar",
                Tool::Weld => "Klicka på skarven · dra för att placera brickan · dubbelklick ändrar",
                Tool::Film => "Klicka där filmen ska ligga · dubbelklick ändrar nummer och format",
                Tool::Text => "Klicka och skriv direkt i rutan",
                Tool::Crop => "Dra in kanterna · Enter beskär",
                _ => "Skift låser till kvadrat eller 45°",
            };
            dim(ui, hint);
        });
    }

    fn home(&mut self, ui: &mut Ui, settings: &Settings) {
        let logo = self.logo(ui.ctx());
        let full = ui.available_rect_before_wrap();
        let mut content = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_center_size(full.center(), vec2(640.0f32.min(full.width() - 40.0), 460.0))).layout(Layout::top_down(Align::Center)));
        let ui = &mut content;
        ui.add_space(8.0);
        ui.add(egui::Image::new((logo, Vec2::splat(92.0))));
        ui.add_space(14.0);
        ui.label(RichText::new("TRK ForceShot").font(FontId::new(28.0, theme::bold())));
        ui.label(RichText::new("Skärmklipp och bildredigering för provningsdokumentation").color(theme::TEXT_DIM).size(14.0));
        ui.add_space(26.0);

        let card = |ui: &mut Ui, icon: Icon, title: &str, sub: &str| -> bool {
            let (rect, resp) = ui.allocate_exact_size(vec2(196.0, 84.0), Sense::click());
            let bg = if resp.hovered() { theme::ELEVATED } else { theme::SURFACE };
            ui.painter().rect(rect, 12.0, bg, Stroke::new(1.0, if resp.hovered() { theme::ACCENT_DEEP } else { theme::BORDER }), StrokeKind::Inside);
            icons::paint(ui.painter(), Rect::from_min_size(rect.min + vec2(16.0, 16.0), Vec2::splat(22.0)), icon, theme::ACCENT);
            ui.painter().text(rect.min + vec2(50.0, 16.0), egui::Align2::LEFT_TOP, title, FontId::new(14.5, theme::bold()), theme::TEXT);
            ui.painter().text(rect.min + vec2(16.0, 52.0), egui::Align2::LEFT_TOP, sub, FontId::proportional(12.0), theme::TEXT_DIM);
            resp.clicked()
        };
        let region_key = settings.hotkeys.region.first().cloned().unwrap_or_default();
        ui.allocate_ui_with_layout(vec2(612.0, 84.0), Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            if card(ui, Icon::Camera, "Klipp ett område", &region_key) {
                self.requests.push(Request::Capture(Action::Region));
            }
            if card(ui, Icon::Folder, "Öppna bild eller PDF", "Ctrl+O, eller släpp en fil här") {
                let ctx = ui.ctx().clone();
                self.open_dialog(&ctx);
            }
            if card(ui, Icon::Paste, "Från urklipp", "Öppna bilden du kopierat") {
                self.open_clipboard();
            }
        });
        ui.add_space(12.0);
        let sketches = sketch::catalog();
        ui.allocate_ui_with_layout(vec2(612.0, 84.0), Layout::left_to_right(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            for s in &sketches {
                let sub: String = s.description.chars().take(30).collect::<String>() + "…";
                if card(ui, Icon::Sketch, &s.name, &sub) {
                    self.open_sketch(&s.id);
                }
            }
            if card(ui, Icon::Gear, "Inställningar", "Genvägar, sparmapp, metodfärger") {
                self.show_settings = true;
            }
        });
        ui.add_space(22.0);
        ui.label(RichText::new("Appen ligger kvar i aktivitetsfältet när fönstret stängs.").color(theme::TEXT_FAINT).size(12.0));
    }

    fn draw_toast(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        let Some((text, since)) = &mut self.toast else { return };
        if *since < 0.0 {
            *since = now;
        }
        if now - *since > 2.6 {
            self.toast = None;
            return;
        }
        let text = text.clone();
        let screen = ctx.content_rect();
        egui::Area::new(egui::Id::unique("avisering")).order(egui::Order::Tooltip).anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, -46.0)).interactable(false).show(ctx, |ui| {
            ui.set_max_width(screen.width() - 80.0);
            egui::Frame::new().fill(Color32::from_rgb(0x0C, 0x0E, 0x11)).stroke(Stroke::new(1.0, theme::ACCENT_DEEP)).corner_radius(9).inner_margin(egui::Margin::symmetric(14, 8)).show(ui, |ui| {
                ui.label(RichText::new(text).size(13.0));
            });
        });
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    fn dialogs(&mut self, ctx: &egui::Context, settings: &mut Settings) {
        // ----- Osparade ändringar -----
        if let Some(what) = &self.confirm {
            let close_window = matches!(what, Confirm::CloseWindow);
            let index = if let Confirm::CloseTab(i) = what { *i } else { 0 };
            let mut choice = None;
            egui::Modal::new(egui::Id::unique("osparat")).show(ctx, |ui| {
                ui.set_width(340.0);
                ui.label(RichText::new("Osparade ändringar").font(FontId::new(16.0, theme::bold())));
                ui.add_space(4.0);
                ui.label(RichText::new(if close_window { "Det finns bilder med ändringar som inte är sparade." } else { "Bilden har ändringar som inte är sparade." }).color(theme::TEXT_DIM));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("Stäng utan att spara").clicked() {
                        choice = Some(true);
                    }
                    if ui.button("Avbryt").clicked() || ui.input(|i| i.key_pressed(Key::Escape)) {
                        choice = Some(false);
                    }
                });
            });
            match choice {
                Some(true) => {
                    self.confirm = None;
                    if close_window { self.close_all() } else { self.close_tab(index, true) }
                }
                Some(false) => self.confirm = None,
                None => {}
            }
        }

        // ----- Ändra svetsmarkör eller film -----
        if let Some(id) = self.canvas.edit_dialog {
            let mut open = true;
            let mut done = false;
            let methods = settings.methods.clone();
            if let Some(Tab::Doc(d)) = self.tabs.get_mut(self.active) {
                if self.dialog_saved != Some(id) {
                    d.checkpoint();
                    self.dialog_saved = Some(id);
                }
                match d.get_mut(id) {
                    Some(el) => {
                        egui::Window::new("Ändra markering").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
                            ui.set_width(300.0);
                            match &mut el.kind {
                                Kind::Weld(w) => {
                                    ui.horizontal(|ui| {
                                        dim(ui, "Skarvnummer");
                                        ui.add(egui::TextEdit::singleline(&mut w.number).desired_width(90.0));
                                    });
                                    ui.add_space(4.0);
                                    let mut free = w.free_text.is_some();
                                    if ui.checkbox(&mut free, "Egen text i stället för metod").changed() {
                                        w.free_text = free.then(String::new);
                                    }
                                    if let Some(t) = &mut w.free_text {
                                        ui.add(egui::TextEdit::singleline(t).hint_text("t.ex. Reparerad").desired_width(260.0));
                                        if w.colors.is_empty() {
                                            w.colors = vec![Color32::from_rgb(0x79, 0x55, 0x48)];
                                        }
                                    } else {
                                        ui.horizontal_wrapped(|ui| {
                                            for m in &methods {
                                                let on = w.methods.contains(&m.code);
                                                if chip(ui, &m.code, on, Some(Color32::from_rgb(m.color[0], m.color[1], m.color[2]))).clicked() {
                                                    if on && w.methods.len() > 1 { w.methods.retain(|x| x != &m.code) } else if !on { w.methods.push(m.code.clone()) }
                                                }
                                            }
                                        });
                                        w.colors = w.methods.iter().map(|c| methods.iter().find(|m| &m.code == c).map(|m| Color32::from_rgb(m.color[0], m.color[1], m.color[2])).unwrap_or(Color32::GRAY)).collect();
                                    }
                                    ui.checkbox(&mut w.show_method, "Visa metodbeteckningen");
                                }
                                Kind::Film(f) => {
                                    ui.horizontal(|ui| {
                                        dim(ui, "Filmnummer");
                                        ui.add(egui::DragValue::new(&mut f.number).range(1..=9999));
                                    });
                                    ui.horizontal_wrapped(|ui| {
                                        for (w, h) in FILM_FORMATS {
                                            if chip(ui, &format!("{}×{}", w as u32, h as u32), f.w_cm == w && f.h_cm == h, None).clicked() {
                                                f.w_cm = w;
                                                f.h_cm = h;
                                            }
                                        }
                                    });
                                }
                                _ => {}
                            }
                            ui.add_space(8.0);
                            if icons::labeled(ui, Icon::Select, "Klar", true).clicked() || ui.input(|i| i.key_pressed(Key::Enter)) {
                                done = true;
                            }
                        });
                    }
                    None => done = true,
                }
                // Ett nytt filmformat ska få rätt storlek i skissens skala.
                if d.px_per_cm > 0.0 {
                    let s = d.px_per_cm;
                    d.set_film_scale(s);
                } else if let Some(el) = d.get_mut(id) {
                    if let Kind::Film(f) = &el.kind {
                        let k = el.rect().width().max(1.0) / f.w_cm.max(0.1);
                        let cur = el.rect();
                        let want = vec2(f.w_cm, f.h_cm) * k;
                        if (cur.height() - want.y).abs() > 0.5 {
                            let r = Rect::from_center_size(cur.center(), vec2(cur.width(), cur.width() / f.w_cm * f.h_cm));
                            el.a = r.min;
                            el.b = r.max;
                        }
                    }
                }
            } else {
                done = true;
            }
            if !open || done {
                self.canvas.edit_dialog = None;
                self.dialog_saved = None;
            }
        }

        // ----- Ny version -----
        if let Some(up) = &mut self.update {
            let mut close = false;
            let mut skip = false;
            let mut start = false;
            let mut run: Option<PathBuf> = None;
            let mut failed: Option<String> = None;
            if let UpState::Downloading { rx, .. } = &up.state {
                match rx.try_recv() {
                    Ok(Ok(path)) => run = Some(path),
                    Ok(Err(e)) => failed = Some(e),
                    Err(_) => ctx.request_repaint_after(std::time::Duration::from_millis(150)),
                }
            }
            if let Some(e) = failed {
                up.state = UpState::Failed(e);
            }
            let r = up.release.clone();
            egui::Window::new("Ny version").collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
                ui.set_width(440.0);
                ui.label(RichText::new(format!("TRK ForceShot {} finns att hämta", r.version)).font(FontId::new(16.0, theme::bold())));
                dim(ui, &format!("Du har version {}. Hämtningen är {:.1} MB.", crate::install::VERSION, r.size as f64 / 1_048_576.0));
                if !r.notes.is_empty() {
                    ui.add_space(6.0);
                    egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                        ui.label(RichText::new(&r.notes).size(13.0));
                    });
                }
                ui.add_space(10.0);
                match &up.state {
                    UpState::Offer => {
                        ui.horizontal(|ui| {
                            if icons::labeled(ui, Icon::Save, "Ladda ner och uppdatera", true).clicked() {
                                start = true;
                            }
                            if ui.button("Senare").clicked() {
                                close = true;
                            }
                            if ui.button("Hoppa över den här").clicked() {
                                skip = true;
                            }
                        });
                        dim(ui, "Programmet stängs och startar om. Spara det du håller på med först.");
                    }
                    UpState::Downloading { progress, .. } => {
                        let got = progress.load(std::sync::atomic::Ordering::Relaxed) as f32;
                        let frac = if r.size > 0 { (got / r.size as f32).min(1.0) } else { 0.0 };
                        ui.add(egui::ProgressBar::new(frac).text(format!("Hämtar … {:.0} %", frac * 100.0)));
                    }
                    UpState::Failed(e) => {
                        ui.label(RichText::new(e.as_str()).color(theme::DANGER));
                        if ui.button("Stäng").clicked() {
                            close = true;
                        }
                    }
                }
            });
            if start {
                let progress = Arc::new(std::sync::atomic::AtomicU64::new(0));
                let (tx, rx) = std::sync::mpsc::channel();
                let (p, rel, ctx2) = (progress.clone(), r.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(crate::update::download(&rel, &p));
                    ctx2.request_repaint();
                });
                up.state = UpState::Downloading { progress, rx };
            }
            if skip {
                settings.skipped_update = r.version.clone();
                settings.save();
                close = true;
            }
            if let Some(path) = run {
                self.requests.push(Request::RunInstaller(path));
                close = true;
            }
            if close {
                self.update = None;
            }
        }

        // ----- Textavläsning -----
        if let Some(job) = &mut self.ocr {
            if job.result.is_none() {
                if let Ok(r) = job.rx.try_recv() {
                    job.result = Some(r.map(|lines| lines.join("\n")));
                }
            }
            let mut open = true;
            let mut copy: Option<String> = None;
            egui::Window::new("Text i bilden").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
                ui.set_width(520.0);
                match &mut job.result {
                    None => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            dim(ui, "Läser av texten …");
                        });
                    }
                    Some(Err(e)) => {
                        ui.label(RichText::new(e.as_str()).color(theme::DANGER));
                    }
                    Some(Ok(text)) if text.trim().is_empty() => dim(ui, "Hittade ingen text i bilden."),
                    Some(Ok(text)) => {
                        egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                            ui.add(egui::TextEdit::multiline(text).desired_width(f32::INFINITY).desired_rows(8));
                        });
                        ui.add_space(8.0);
                        if icons::labeled(ui, Icon::Copy, "Kopiera all text", true).clicked() {
                            copy = Some(text.clone());
                        }
                    }
                }
            });
            if let Some(t) = copy {
                let ok = self.clipboard().is_some_and(|c| c.set_text(t).is_ok());
                self.say(if ok { "Texten är kopierad" } else { "Kunde inte kopiera texten" });
            }
            if !open {
                self.ocr = None;
            }
        }

        // ----- Inställningar -----
        if self.show_settings {
            let mut open = true;
            let mut rebind = false;
            let mut autostart = *self.autostart.get_or_insert_with(crate::install::autostart_enabled);
            let (mut autostart_changed, mut check_now) = (false, false);
            let checking = self.update_check.is_some();
            let mut found = None;
            if let Some(rx) = &self.update_check {
                if let Ok(result) = rx.try_recv() {
                    self.update_status = match result {
                        Ok(Some(r)) => {
                            let text = format!("Version {} finns att hämta.", r.version);
                            found = Some(r);
                            text
                        }
                        Ok(None) => format!("Du har den senaste versionen ({}).", crate::install::VERSION),
                        Err(e) => e,
                    };
                    self.update_check = None;
                }
            }
            let status = if checking && self.update_check.is_some() { "Söker …".to_string() } else { self.update_status.clone() };
            egui::Window::new("Inställningar").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).show(ctx, |ui| {
                ui.set_width(520.0);
                egui::ScrollArea::vertical().max_height(520.0).show(ui, |ui| {
                    let head = |ui: &mut Ui, t: &str| {
                        ui.add_space(8.0);
                        ui.label(RichText::new(t).font(FontId::new(14.5, theme::bold())).color(theme::ACCENT));
                        ui.add_space(2.0);
                    };
                    head(ui, "Allmänt");
                    if cfg!(windows) {
                        if ui.checkbox(&mut autostart, "Starta ForceShot när jag loggar in").changed() {
                            autostart_changed = true;
                        }
                    }
                    ui.checkbox(&mut settings.check_updates, "Sök efter uppdateringar när programmet startar");
                    ui.horizontal(|ui| {
                        dim(ui, "Utgåvor hämtas från");
                        ui.add(egui::TextEdit::singleline(&mut settings.update_repo).hint_text("ägare/repo på GitHub").desired_width(220.0));
                        if ui.add_enabled(!checking, egui::Button::new("Sök nu")).clicked() {
                            check_now = true;
                        }
                    });
                    if !status.is_empty() {
                        dim(ui, &status);
                    }

                    head(ui, "Skärmklipp");
                    ui.horizontal(|ui| {
                        dim(ui, "Efter markering");
                        for (v, t) in [(AfterCapture::Meny, "Visa meny"), (AfterCapture::Redigera, "Öppna redigeraren"), (AfterCapture::Kopiera, "Kopiera"), (AfterCapture::Spara, "Spara")] {
                            ui.selectable_value(&mut settings.after_capture, v, t);
                        }
                    });
                    ui.checkbox(&mut settings.show_magnifier, "Visa förstoringsglas vid markering");
                    ui.checkbox(&mut settings.include_cursor, "Ta med muspekaren i bilden");

                    head(ui, "Spara");
                    ui.horizontal(|ui| {
                        dim(ui, "Mapp");
                        ui.label(settings.save_folder.display().to_string());
                        if ui.button("Byt …").clicked() {
                            if let Some(p) = rfd::FileDialog::new().set_directory(&settings.save_folder).pick_folder() {
                                settings.save_folder = p;
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        dim(ui, "Filnamn");
                        ui.add(egui::TextEdit::singleline(&mut settings.filename_pattern).desired_width(330.0));
                    });
                    ui.horizontal(|ui| {
                        dim(ui, "Format");
                        ui.selectable_value(&mut settings.jpeg, false, "PNG");
                        ui.selectable_value(&mut settings.jpeg, true, "JPEG");
                        if settings.jpeg {
                            ui.add(egui::Slider::new(&mut settings.jpeg_quality, 40..=100).text("kvalitet"));
                        }
                    });
                    ui.checkbox(&mut settings.ask_filename, "Fråga efter filnamn när ett klipp sparas");

                    head(ui, "Svetsskarvar");
                    ui.horizontal(|ui| {
                        dim(ui, "Löpnummer");
                        ui.selectable_value(&mut settings.numbering, Numbering::PerMetod, "En serie per metod");
                        ui.selectable_value(&mut settings.numbering, Numbering::Gemensam, "Gemensam serie");
                    });
                    egui::Grid::new("metoder").num_columns(3).spacing(vec2(14.0, 5.0)).show(ui, |ui| {
                        for m in &mut settings.methods {
                            ui.color_edit_button_srgb(&mut m.color);
                            ui.label(RichText::new(&m.code).font(FontId::new(13.0, theme::bold())));
                            ui.label(RichText::new(&m.name).color(theme::TEXT_DIM));
                            ui.end_row();
                        }
                    });

                    head(ui, "Snabbtangenter");
                    dim(ui, "Skriv t.ex. Ctrl+Alt+Shift+P. Flera genvägar skiljs med kommatecken.");
                    egui::Grid::new("genvägar").num_columns(2).spacing(vec2(14.0, 5.0)).show(ui, |ui| {
                        let h = &mut settings.hotkeys;
                        for (label, list) in [
                            ("Klipp ett område", &mut h.region),
                            ("Hela skärmen", &mut h.full_screen),
                            ("Aktivt fönster", &mut h.window),
                            ("Samma område igen", &mut h.last_region),
                            ("Öppna urklippet", &mut h.open_clipboard),
                        ] {
                            ui.label(label);
                            let mut text = list.join(", ");
                            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(300.0)).changed() {
                                *list = text.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
                                rebind = true;
                            }
                            ui.end_row();
                        }
                    });
                    ui.add_space(8.0);
                    dim(ui, &format!("TRK ForceShot {} · inställningarna sparas i {}", env!("CARGO_PKG_VERSION"), crate::settings::config_dir().display()));
                });
            });
            if rebind {
                self.requests.push(Request::RebindHotkeys);
            }
            if autostart_changed {
                let exe = std::env::current_exe().unwrap_or_default();
                match crate::install::set_autostart(autostart, &exe) {
                    Ok(()) => self.autostart = Some(autostart),
                    Err(e) => self.say(format!("Kunde inte ändra autostarten: {e}")),
                }
            }
            if check_now {
                let (tx, rx) = std::sync::mpsc::channel();
                let (repo, ctx2) = (settings.update_repo.clone(), ctx.clone());
                std::thread::spawn(move || {
                    let _ = tx.send(crate::update::check(&repo));
                    ctx2.request_repaint();
                });
                self.update_check = Some(rx);
            }
            if let Some(r) = found {
                self.offer_update(r);
                open = false;
            }
            if !open {
                self.show_settings = false;
                settings.save();
            }
        }
    }
}
