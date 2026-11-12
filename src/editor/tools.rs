//! Redigerarens större verktyg med egna dialogrutor: fiberskiss, bladexport, förstora,
//! komprimera, arbetsyta (ForceSpace), karta och dela via mejl.

use super::doc::{Doc, Element, Kind, TextData};
use super::{Editor, Job, JobKind, Tab, chip, dim};
use crate::icons::{self, Icon};
use crate::settings::Settings;
use crate::{imaging, theme};
use egui::{Color32, FontId, Rect, RichText, Vec2, pos2, vec2};
use image::RgbaImage;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

/// Rapportens bildyta; arbetsytans guide och fiberbladen har samma proportion.
pub const SPACE_SIZE: (u32, u32) = (565, 744);
/// Större sida än så här klarar inte alla grafikkort som textur.
const MAX_SIDE: u32 = 8000;

#[derive(Default)]
pub struct Extras {
    sheet: Option<SheetDialog>,
    export: Option<()>,
    enlarge: Option<EnlargeDialog>,
    compact: Option<CompactDialog>,
    map: Option<MapDialog>,
    pdf: Option<PdfDialog>,
}

impl Extras {
    pub fn any_open(&self) -> bool {
        self.sheet.is_some() || self.export.is_some() || self.enlarge.is_some() || self.compact.is_some() || self.map.is_some() || self.pdf.is_some()
    }
}

struct SheetDialog {
    number: String,
    files: Vec<PathBuf>,
}

struct EnlargeDialog {
    factor: u32,
    mode: imaging::Enhance,
}

struct CompactDialog {
    max_side: u32,
    quality: u8,
}

/// Vilka flikar som ska bli sidor: flikens nummer, namn och om den är med.
struct PdfDialog {
    picks: Vec<(u64, String, bool)>,
}

type MapMsg = (crate::map::View, RgbaImage, Option<String>);

struct MapDialog {
    query: String,
    level: usize,
    tiles: Arc<std::sync::Mutex<crate::map::Tiles>>,
    rx: Option<Receiver<Result<MapMsg, String>>>,
    error: Option<String>,
    /// Utsnittet användaren har dragit fram.
    view: Option<crate::map::View>,
    /// Utsnittet som den hämtade bilden visar.
    shown: Option<(crate::map::View, RgbaImage, egui::TextureHandle)>,
    place: String,
}

fn window<'a>(title: &'a str, open: &'a mut bool) -> egui::Window<'a> {
    egui::Window::new(title).open(open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
}

/// Bygger bladen: fyra stående foton per blad, med filnamnet som text i varje fack.
/// Ger dokumenten i ordning. Liggande bilder vänds till stående.
pub fn build_sheets(number: &str, photos: Vec<(String, RgbaImage)>, first_uid: u64) -> Vec<Doc> {
    let photos: Vec<(String, RgbaImage)> = photos
        .into_iter()
        .map(|(name, img)| {
            let img = if img.width() > img.height() { image::imageops::rotate90(&img) } else { img };
            // Jättebilder från en kamera krymps; ett blad ska gå att hantera som en bild.
            let img = if img.width() > 1200 {
                let h = (img.height() as f32 * 1200.0 / img.width() as f32).round() as u32;
                image::imageops::resize(&img, 1200, h.max(1), image::imageops::FilterType::Lanczos3)
            } else {
                img
            };
            (name, img)
        })
        .collect();
    let Some((_, first)) = photos.first() else { return Vec::new() };
    // Ett fack rymmer en bild i dess egen storlek, så inget skalas upp.
    let (cw, ch) = (first.width() as f32, first.height() as f32);
    let m = (cw * 0.038).round().max(12.0);
    let (sw, sh) = (2.0 * cw + 3.0 * m, 2.0 * ch + 3.0 * m);
    let cells = [
        Rect::from_min_size(pos2(m, m), vec2(cw, ch)),
        Rect::from_min_size(pos2(2.0 * m + cw, m), vec2(cw, ch)),
        Rect::from_min_size(pos2(m, 2.0 * m + ch), vec2(cw, ch)),
        Rect::from_min_size(pos2(2.0 * m + cw, 2.0 * m + ch), vec2(cw, ch)),
    ];
    // Den vita ramen ligger överst och gömmer det som sticker ut ur facken.
    let bars = vec![
        Rect::from_min_max(pos2(0.0, 0.0), pos2(sw, m)),
        Rect::from_min_max(pos2(0.0, m + ch), pos2(sw, 2.0 * m + ch)),
        Rect::from_min_max(pos2(0.0, sh - m), pos2(sw, sh)),
        Rect::from_min_max(pos2(0.0, 0.0), pos2(m, sh)),
        Rect::from_min_max(pos2(m + cw, 0.0), pos2(2.0 * m + cw, sh)),
        Rect::from_min_max(pos2(sw - m, 0.0), pos2(sw, sh)),
    ];
    let blank = |kind: Kind, r: Rect| Element { id: 0, kind, a: r.min, b: r.max, rot: 0.0, stroke: Color32::BLACK, fill: Color32::TRANSPARENT, width: 1.5 };
    let mut docs = Vec::new();
    for (n, chunk) in photos.chunks(4).enumerate() {
        let name = format!("{}-skiss-{}", if number.trim().is_empty() { "fiber" } else { number.trim() }, n + 1);
        let base = RgbaImage::from_pixel(sw as u32, sh as u32, image::Rgba([255, 255, 255, 255]));
        let mut d = Doc::new(first_uid + n as u64, name.clone(), base, None);
        d.sheet = Some(name);
        for (i, (file, img)) in chunk.iter().enumerate() {
            let cell = cells[i];
            let (w, h) = (img.width() as f32, img.height() as f32);
            let k = (cw / w).min(ch / h).min(1.0);
            d.add(blank(Kind::Image(Arc::new(img.clone())), Rect::from_center_size(cell.center(), vec2(w, h) * k)));
            let size = (cw * 0.034).clamp(11.0, 30.0);
            let width = (file.chars().count() as f32 * size * 0.62 + 16.0).min(cw - 12.0);
            let label = Rect::from_min_size(cell.min + vec2(6.0, 6.0), vec2(width, size * 1.6));
            let text = TextData { text: file.clone(), size, box_fill: Some(Color32::from_rgba_unmultiplied(255, 255, 255, 230)), box_border: None, align: 0 };
            d.add(blank(Kind::Text(text), label));
        }
        d.add(blank(Kind::SheetFrame(bars.clone()), Rect::from_min_size(pos2(0.0, 0.0), vec2(sw, sh))));
        d.dirty = true;
        docs.push(d);
    }
    docs
}

impl Editor {
    pub fn open_sheet_dialog(&mut self) {
        self.x.sheet = Some(SheetDialog { number: String::new(), files: Vec::new() });
    }

    pub fn open_enlarge(&mut self) {
        self.x.enlarge = Some(EnlargeDialog { factor: 2, mode: imaging::Enhance::Skarmklipp });
    }

    pub fn open_compact(&mut self) {
        self.x.compact = Some(CompactDialog { max_side: 1920, quality: 82 });
    }

    pub fn open_map(&mut self) {
        self.x.map = Some(MapDialog { query: String::new(), level: 1, tiles: Default::default(), rx: None, error: None, view: None, shown: None, place: String::new() });
    }

    pub fn open_pdf_dialog(&mut self) {
        let picks = self.tabs.iter().filter_map(|t| if let Tab::Doc(d) = t { Some((d.uid, d.title.clone(), true)) } else { None }).collect();
        self.x.pdf = Some(PdfDialog { picks });
    }

    /// Köar en flersidig PDF av flikarna, i den ordning de anges.
    pub fn queue_pdf(&mut self, path: PathBuf, uids: &[u64]) {
        self.pdf_out = Some((path, Vec::new()));
        for (i, uid) in uids.iter().enumerate() {
            self.jobs.push(Job { uid: *uid, kind: JobKind::PdfPage { last: i + 1 == uids.len() }, warmed: false });
        }
    }

    pub fn open_sheet_export(&mut self) {
        self.x.export = Some(());
    }

    pub fn sheet_count(&self) -> usize {
        self.tabs.iter().filter(|t| matches!(t, Tab::Doc(d) if d.sheet.is_some())).count()
    }

    /// En fri arbetsyta: vit yta där guiden (den streckade ramen) är det som blir bild.
    pub fn new_space(&mut self) {
        let base = RgbaImage::from_pixel(SPACE_SIZE.0, SPACE_SIZE.1, image::Rgba([255, 255, 255, 255]));
        self.add_image(base, Some("Arbetsyta".into()), None, 0.0);
        if let Some(d) = self.doc() {
            d.space = true;
        }
    }

    /// Bifogar den färdigritade bilden till ett nytt mejl i datorns e-postprogram.
    pub fn share_by_mail(&mut self) {
        if let Some(d) = self.doc() {
            let uid = d.uid;
            self.jobs.push(Job { uid, kind: JobKind::Mail, warmed: false });
        }
    }

    /// Utvecklingsläge: sex påhittade kamerabilder blir två fiberblad. Med en mapp
    /// exporteras bladen dit, på samma sätt som exporten till skrivbordet.
    pub fn fiber_demo(&mut self, out: Option<PathBuf>, settings: &Settings) {
        let photos = (0..6u32)
            .map(|i| {
                let mut img = RgbaImage::new(768, 576);
                for (x, y, p) in img.enumerate_pixels_mut() {
                    let stripe = ((x + y + i * 40) / 48) % 2 == 0;
                    let base = [60 + i as u8 * 30, 110, 200 - i as u8 * 25];
                    let k = if stripe { 1.0 } else { 0.78 };
                    *p = image::Rgba([(base[0] as f32 * k) as u8, (base[1] as f32 * k) as u8, (base[2] as f32 * k) as u8, 255]);
                }
                (format!("IMG_{:04}", 1201 + i), img)
            })
            .collect();
        let docs = build_sheets("P-2026-0412", photos, super::reserve_uids(64));
        for d in docs {
            if let Some(dir) = &out {
                let path = dir.join(format!("{}.jpg", d.sheet.clone().unwrap_or_default()));
                self.jobs.push(Job { uid: d.uid, kind: JobKind::Export { path, max_side: settings.sheet_max_width, width_only: true, quality: settings.sheet_quality }, warmed: false });
            }
            self.tabs.push(Tab::Doc(Box::new(d)));
        }
        self.active = 0;
    }

    /// Utvecklingsläge: två fiberblad sparas som en tvåsidig PDF.
    pub fn pdf_demo(&mut self, out: Option<PathBuf>, settings: &Settings) {
        self.fiber_demo(None, settings);
        let uids: Vec<u64> = self.tabs.iter().map(|t| t.uid()).collect();
        if let Some(path) = out {
            self.queue_pdf(path, &uids);
        }
    }

    /// Utvecklingsläge: två fiberblad, där det andra dras ut till ett eget fönster.
    pub fn detach_demo(&mut self, settings: &Settings) {
        self.fiber_demo(None, settings);
        self.send_tab(1, super::Dest::New, None);
    }

    /// Utvecklingsläge: en arbetsyta med innehåll både i och utanför guiden.
    pub fn space_demo(&mut self, out: Option<PathBuf>) {
        self.new_space();
        let Some(d) = self.doc() else { return };
        let el = |kind: Kind, a: egui::Pos2, b: egui::Pos2, stroke: Color32| Element { id: 0, kind, a, b, rot: 0.0, stroke, fill: Color32::TRANSPARENT, width: 4.0 };
        d.add(el(Kind::Rect, pos2(60.0, 80.0), pos2(380.0, 300.0), Color32::from_rgb(0xE5, 0x39, 0x35)));
        d.add(el(Kind::Arrow { double: false }, pos2(-220.0, 420.0), pos2(300.0, 520.0), Color32::from_rgb(0x1E, 0x88, 0xE5)));
        d.add(el(Kind::Ellipse, pos2(430.0, 560.0), pos2(820.0, 900.0), Color32::from_rgb(0x43, 0xA0, 0x47)));
        let text = TextData { text: "Centrerad text\ni arbetsytan".into(), size: 26.0, box_fill: Some(Color32::WHITE), box_border: Some(Color32::from_rgb(0x00, 0xAC, 0xC1)), align: 1 };
        d.add(el(Kind::Text(text), pos2(90.0, 600.0), pos2(400.0, 660.0), Color32::BLACK));
        if let Some(path) = out {
            let uid = d.uid;
            self.jobs.push(Job { uid, kind: JobKind::Export { path, max_side: 4000, width_only: false, quality: 90 }, warmed: false });
        }
    }

    pub(super) fn tools_dialogs(&mut self, ctx: &egui::Context, settings: &mut Settings) {
        self.sheet_dialog(ctx);
        self.export_dialog(ctx, settings);
        self.enlarge_dialog(ctx);
        self.compact_dialog(ctx);
        self.map_dialog(ctx);
        self.pdf_dialog(ctx);
    }

    fn pdf_dialog(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.x.pdf else { return };
        let mut open = true;
        let mut go = false;
        window("Spara flikar som PDF", &mut open).show(ctx, |ui| {
            ui.set_width(380.0);
            dim(ui, "Varje ikryssad flik blir en sida, i flikarnas ordning.");
            ui.add_space(4.0);
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                for (n, (_, title, on)) in dlg.picks.iter_mut().enumerate() {
                    ui.checkbox(on, format!("{}.  {}", n + 1, title.chars().take(44).collect::<String>()));
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button("Alla").clicked() {
                    dlg.picks.iter_mut().for_each(|p| p.2 = true);
                }
                if ui.button("Ingen").clicked() {
                    dlg.picks.iter_mut().for_each(|p| p.2 = false);
                }
            });
            ui.add_space(8.0);
            let count = dlg.picks.iter().filter(|p| p.2).count();
            ui.add_enabled_ui(count > 0, |ui| {
                if icons::labeled(ui, Icon::Save, &format!("Spara {count} sidor som …"), true).clicked() {
                    go = true;
                }
            });
        });
        if go {
            let dlg = self.x.pdf.take().unwrap();
            let uids: Vec<u64> = dlg.picks.iter().filter(|p| p.2).map(|p| p.0).collect();
            let first = dlg.picks.iter().find(|p| p.2).map(|p| p.1.clone()).unwrap_or_default();
            // "rapport.pdf – s. 1" ska föreslå "rapport.pdf".
            let stem = first.split(" – s. ").next().unwrap_or("").trim_end_matches(".pdf").to_string();
            let name = format!("{}.pdf", if stem.is_empty() { "sidor" } else { stem.as_str() });
            if let Some(mut path) = rfd::FileDialog::new().set_file_name(name).add_filter("PDF", &["pdf"]).save_file() {
                path.set_extension("pdf");
                self.queue_pdf(path, &uids);
                ctx.request_repaint();
            }
        } else if !open {
            self.x.pdf = None;
        }
    }

    fn sheet_dialog(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.x.sheet else { return };
        let mut open = true;
        let mut create = false;
        window("Fiberskiss – fyra foton per blad", &mut open).show(ctx, |ui| {
            ui.set_width(460.0);
            ui.horizontal(|ui| {
                dim(ui, "Provningsnummer");
                ui.add(egui::TextEdit::singleline(&mut dlg.number).hint_text("t.ex. P-2026-0412").desired_width(200.0));
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("Välj bilder …").clicked() {
                    if let Some(files) = rfd::FileDialog::new().add_filter("Bilder", &["png", "jpg", "jpeg", "bmp", "tif", "tiff", "webp"]).pick_files() {
                        dlg.files.extend(files);
                    }
                }
                if !dlg.files.is_empty() && ui.button("Töm listan").clicked() {
                    dlg.files.clear();
                }
                let sheets = dlg.files.len().div_ceil(4);
                dim(ui, &format!("{} bilder → {} blad", dlg.files.len(), sheets));
            });
            if !dlg.files.is_empty() {
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    let mut remove = None;
                    for (i, f) in dlg.files.iter().enumerate() {
                        ui.horizontal(|ui| {
                            dim(ui, &format!("{}.", i + 1));
                            ui.label(f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
                            if ui.small_button("×").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        dlg.files.remove(i);
                    }
                });
            }
            ui.add_space(4.0);
            dim(ui, "Liggande bilder vänds till stående. Varje bild får sitt filnamn som text och går att flytta, förstora och vrida i sitt fack.");
            ui.add_space(8.0);
            ui.add_enabled_ui(!dlg.files.is_empty(), |ui| {
                if icons::labeled(ui, Icon::Plus, "Skapa blad", true).clicked() {
                    create = true;
                }
            });
        });
        if create {
            let dlg = self.x.sheet.take().unwrap();
            let mut photos = Vec::new();
            let mut failed = 0;
            for f in &dlg.files {
                match image::open(f) {
                    Ok(img) => photos.push((f.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), img.to_rgba8())),
                    Err(_) => failed += 1,
                }
            }
            let first_uid = super::reserve_uids(4096);
            let docs = build_sheets(&dlg.number, photos, first_uid);
            let n = docs.len();
            let first_tab = self.tabs.len();
            for d in docs {
                self.tabs.push(Tab::Doc(Box::new(d)));
            }
            if n > 0 {
                if let Some(Tab::Doc(old)) = self.tabs.get_mut(self.active) {
                    self.canvas.finish_edit(old);
                }
                self.active = first_tab;
            }
            self.say(if failed > 0 { format!("{n} blad skapade, {failed} bilder gick inte att läsa") } else { format!("{n} blad skapade") });
        } else if !open {
            self.x.sheet = None;
        }
    }

    fn export_dialog(&mut self, ctx: &egui::Context, settings: &mut Settings) {
        if self.x.export.is_none() {
            return;
        }
        let count = self.sheet_count();
        let mut open = true;
        let mut go = false;
        window("Exportera blad till skrivbordet", &mut open).show(ctx, |ui| {
            ui.set_width(400.0);
            dim(ui, &format!("{count} blad skrivs som egna filer. Valet gäller hela serien och sparas till nästa gång."));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                dim(ui, "Format");
                ui.selectable_value(&mut settings.sheet_jpeg, true, "JPEG");
                ui.selectable_value(&mut settings.sheet_jpeg, false, "PNG");
            });
            if settings.sheet_jpeg {
                ui.add(egui::Slider::new(&mut settings.sheet_quality, 40..=100).text("kvalitet"));
            }
            ui.add(egui::Slider::new(&mut settings.sheet_max_width, 600..=3000).text("största bredd (px)"));
            ui.add_space(8.0);
            ui.add_enabled_ui(count > 0, |ui| {
                if icons::labeled(ui, Icon::Save, "Exportera", true).clicked() {
                    go = true;
                }
            });
        });
        if go {
            let folder = dirs::desktop_dir().or_else(dirs::home_dir).unwrap_or_default();
            let ext = if settings.sheet_jpeg { "jpg" } else { "png" };
            for t in &self.tabs {
                if let Tab::Doc(d) = t {
                    if let Some(name) = &d.sheet {
                        let path = folder.join(format!("{name}.{ext}"));
                        self.jobs.push(Job { uid: d.uid, kind: JobKind::Export { path, max_side: settings.sheet_max_width, width_only: true, quality: settings.sheet_quality }, warmed: false });
                    }
                }
            }
            settings.save();
            self.x.export = None;
            ctx.request_repaint();
        } else if !open {
            self.x.export = None;
        }
    }

    fn enlarge_dialog(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.x.enlarge else { return };
        let size = match self.tabs.get(self.active) {
            Some(Tab::Doc(d)) => (d.base.width(), d.base.height()),
            _ => {
                self.x.enlarge = None;
                return;
            }
        };
        let mut open = true;
        let mut go = false;
        let (nw, nh) = (size.0 * dlg.factor, size.1 * dlg.factor);
        let too_big = nw > MAX_SIDE || nh > MAX_SIDE;
        window("Förstora", &mut open).show(ctx, |ui| {
            ui.set_width(430.0);
            ui.horizontal(|ui| {
                dim(ui, "Förstoring");
                for f in [2, 3, 4] {
                    if chip(ui, &format!("{f}×"), dlg.factor == f, None).clicked() {
                        dlg.factor = f;
                    }
                }
                dim(ui, &format!("{} × {} → {nw} × {nh} px", size.0, size.1));
            });
            ui.add_space(6.0);
            dim(ui, "Bildtyp");
            for (mode, name, text) in [
                (imaging::Enhance::Skarmklipp, "Skärmklipp", "Skarp omsampling utan att röra tonerna. För skärmbilder, ritningar och diagram."),
                (imaging::Enhance::Dokument, "Inskannat dokument", "Sträcker ut svart- och vitpunkten så att grått papper blir vitt, och skärper bokstavskanterna."),
                (imaging::Enhance::Foto, "Foto", "Dämpar bildbrus och lyfter kontrasten försiktigt. För foton av svetsar och provbitar."),
            ] {
                ui.radio_value(&mut dlg.mode, mode, RichText::new(name).font(FontId::new(13.5, theme::bold())));
                ui.indent(name, |ui| dim(ui, text));
            }
            ui.add_space(6.0);
            if too_big {
                ui.label(RichText::new("Resultatet blir för stort. Välj en mindre förstoring.").color(theme::DANGER));
            } else {
                dim(ui, "Annoteringarna följer med. Förstoringen skapar inga detaljer som inte finns i bilden, och går att ångra.");
            }
            ui.add_space(8.0);
            ui.add_enabled_ui(!too_big, |ui| {
                if icons::labeled(ui, Icon::Fit, "Förstora", true).clicked() {
                    go = true;
                }
            });
        });
        if go {
            let dlg = self.x.enlarge.take().unwrap();
            if let Some(d) = self.doc() {
                let big = imaging::enlarge(&d.base, dlg.factor, dlg.mode);
                d.checkpoint();
                d.base = Arc::new(big);
                d.scale_elements(dlg.factor as f32);
                d.fit = true;
            }
            self.say(format!("Förstorad {}×", dlg.factor));
        } else if !open {
            self.x.enlarge = None;
        }
    }

    fn compact_dialog(&mut self, ctx: &egui::Context) {
        let Some(dlg) = &mut self.x.compact else { return };
        let (size, uid, name) = match self.tabs.get(self.active) {
            Some(Tab::Doc(d)) => ((d.base.width(), d.base.height()), d.uid, d.title.clone()),
            _ => {
                self.x.compact = None;
                return;
            }
        };
        let mut open = true;
        let mut go = false;
        let k = (dlg.max_side as f32 / size.0.max(size.1) as f32).min(1.0);
        window("Komprimera och spara", &mut open).show(ctx, |ui| {
            ui.set_width(400.0);
            ui.add(egui::Slider::new(&mut dlg.max_side, 400..=4000).text("största sida (px)"));
            ui.add(egui::Slider::new(&mut dlg.quality, 40..=100).text("JPEG-kvalitet"));
            dim(ui, &format!("{} × {} → {} × {} px", size.0, size.1, (size.0 as f32 * k) as u32, (size.1 as f32 * k) as u32));
            dim(ui, "Sparas som JPEG. Filstorleken visas när filen är skriven.");
            ui.add_space(8.0);
            if icons::labeled(ui, Icon::Save, "Spara som …", true).clicked() {
                go = true;
            }
        });
        if go {
            let dlg = self.x.compact.take().unwrap();
            let stem = std::path::Path::new(&name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "bild".into());
            if let Some(path) = rfd::FileDialog::new().set_file_name(format!("{stem}-liten.jpg")).add_filter("JPEG", &["jpg", "jpeg"]).save_file() {
                self.jobs.push(Job { uid, kind: JobKind::Export { path, max_side: dlg.max_side, width_only: false, quality: dlg.quality }, warmed: false });
                ctx.request_repaint();
            }
        } else if !open {
            self.x.compact = None;
        }
    }

    fn map_dialog(&mut self, ctx: &egui::Context) {
        use crate::map::{self, View};
        let Some(dlg) = &mut self.x.map else { return };
        if let Some(rx) = &dlg.rx {
            if let Ok(r) = rx.try_recv() {
                dlg.rx = None;
                match r {
                    Ok((view, image, place)) => {
                        let ci = egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw());
                        let tex = ctx.load_texture("karta", ci, egui::TextureOptions::LINEAR);
                        if let Some(p) = place {
                            // En ny sökning: utsnittet börjar på platsen.
                            dlg.place = p;
                            dlg.view = Some(view);
                        }
                        dlg.shown = Some((view, image, tex));
                    }
                    Err(e) => {
                        // Gå tillbaka till det som faktiskt visas, så att hämtningen inte görs om i en slinga.
                        dlg.view = dlg.shown.as_ref().map(|s| s.0);
                        dlg.error = Some(e);
                    }
                }
            }
        }
        let mut open = true;
        let (mut search, mut insert) = (false, false);
        let busy = dlg.rx.is_some();
        window("Karta", &mut open).show(ctx, |ui| {
            ui.set_width(500.0);
            ui.horizontal(|ui| {
                let field = ui.add(egui::TextEdit::singleline(&mut dlg.query).hint_text("Adress, t.ex. Staffanstorp Meteorgatan 9").desired_width(410.0));
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add_enabled_ui(!busy && !dlg.query.trim().is_empty(), |ui| {
                    if ui.button("Sök").clicked() || (enter && !busy) {
                        search = true;
                    }
                });
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                dim(ui, "Utsnitt");
                for (i, (name, zoom)) in map::LEVELS.iter().enumerate() {
                    let selected = dlg.view.map_or(dlg.level == i, |v| v.zoom == *zoom);
                    if chip(ui, name, selected, None).clicked() {
                        dlg.level = i;
                        dlg.view = dlg.view.map(|v| v.with_zoom(*zoom));
                    }
                }
                if let Some(v) = &mut dlg.view {
                    if icons::sized_button(ui, Icon::Plus, "Zooma in", false, 26.0).clicked() {
                        *v = v.with_zoom(v.zoom + 1);
                    }
                    if ui.add(egui::Button::new("−").min_size(Vec2::splat(26.0))).on_hover_text("Zooma ut").clicked() {
                        *v = v.with_zoom(v.zoom.saturating_sub(1));
                    }
                }
                if busy {
                    ui.spinner();
                }
            });
            ui.add_space(6.0);

            // Förhandsvisningen: dra för att flytta utsnittet, rulla för att zooma.
            let size = vec2(500.0, 500.0 * map::HEIGHT as f32 / map::WIDTH as f32);
            let k = size.x / map::WIDTH as f32;
            let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::drag());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 6.0, Color32::from_rgb(0xE8, 0xE8, 0xE8));
            if let (Some(view), Some((sv, _, tex))) = (dlg.view.as_mut(), &dlg.shown) {
                if resp.dragged() {
                    let d = resp.drag_delta();
                    *view = view.moved(-(d.x / k) as f64, -(d.y / k) as f64);
                }
                if resp.hovered() {
                    let steps: i32 = ui.input(|i| {
                        i.events
                            .iter()
                            .map(|e| match e {
                                egui::Event::MouseWheel { delta, .. } if delta.y > 0.0 => 1,
                                egui::Event::MouseWheel { delta, .. } if delta.y < 0.0 => -1,
                                _ => 0,
                            })
                            .sum()
                    });
                    if steps != 0 {
                        *view = view.with_zoom((view.zoom as i32 + steps.clamp(-1, 1)).max(0) as u32);
                    }
                    ui.ctx().set_cursor_icon(if resp.dragged() { egui::CursorIcon::Grabbing } else { egui::CursorIcon::Grab });
                }
                // Bilden ritas förskjuten och skalad tills den nya har hämtats.
                let f = 2f64.powi(view.zoom as i32 - sv.zoom as i32);
                let off = vec2(((sv.x * f - view.x) as f32) * k, ((sv.y * f - view.y) as f32) * k);
                let r = Rect::from_center_size(rect.center() + off, size * f as f32);
                painter.image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                let g = painter.layout_no_wrap(map::ATTRIBUTION.into(), FontId::proportional(10.5), Color32::BLACK);
                let tag = Rect::from_min_size(rect.max - g.size() - vec2(10.0, 6.0), g.size() + vec2(8.0, 4.0));
                painter.rect_filled(tag, 0.0, Color32::from_rgba_unmultiplied(255, 255, 255, 215));
                painter.galley(tag.min + vec2(4.0, 2.0), g, Color32::BLACK);
            } else {
                let text = if busy { "Hämtar kartan …" } else { "Sök en adress så visas kartan här" };
                painter.text(rect.center(), egui::Align2::CENTER_CENTER, text, FontId::proportional(13.0), Color32::from_gray(0x60));
            }
            ui.add_space(4.0);
            if dlg.place.is_empty() {
                dim(ui, "Kartan hämtas från OpenStreetMap. Adressen skickas till OpenStreetMaps söktjänst.");
            } else {
                dim(ui, &dlg.place.chars().take(90).collect::<String>());
                dim(ui, "Dra i kartan för att flytta utsnittet och rulla för att zooma. Det du ser är det som läggs in.");
            }
            if let Some(e) = &dlg.error {
                ui.label(RichText::new(e.as_str()).color(theme::DANGER));
            }
            ui.add_space(8.0);
            let ready = !busy && dlg.view.is_some() && dlg.shown.as_ref().map(|s| s.0) == dlg.view;
            ui.add_enabled_ui(ready, |ui| {
                if icons::labeled(ui, Icon::Plus, "Lägg in", true).clicked() {
                    insert = true;
                }
            });
        });

        let spawn = |dlg: &mut MapDialog, query: Option<String>, view: Option<View>, zoom: u32| {
            let (tx, rx) = std::sync::mpsc::channel();
            let (tiles, ctx2) = (dlg.tiles.clone(), ctx.clone());
            std::thread::spawn(move || {
                let work = || -> Result<MapMsg, String> {
                    let mut tiles = tiles.lock().map_err(|_| "Kartan gick inte att hämta.".to_string())?;
                    match (query, view) {
                        (Some(q), _) => {
                            let (lat, lon, place) = tiles.locate(&q)?;
                            let v = View::at(lat, lon, zoom);
                            Ok((v, tiles.render(v)?, Some(place)))
                        }
                        (None, Some(v)) => Ok((v, tiles.render(v)?, None)),
                        (None, None) => Err("Inget utsnitt valt.".into()),
                    }
                };
                let _ = tx.send(work());
                ctx2.request_repaint();
            });
            dlg.rx = Some(rx);
            dlg.error = None;
        };
        if search {
            let zoom = dlg.view.map_or(map::LEVELS[dlg.level].1, |v| v.zoom);
            let query = dlg.query.clone();
            spawn(dlg, Some(query), None, zoom);
        } else if !busy && !ctx.input(|i| i.pointer.any_down()) {
            // Utsnittet har flyttats eller zoomats: hämta bilden som hör till det.
            let shown = dlg.shown.as_ref().map(|s| s.0);
            if let Some(v) = dlg.view.filter(|v| shown != Some(*v)) {
                spawn(dlg, None, Some(v), v.zoom);
            }
        }

        if insert {
            let Some(dlg) = self.x.map.take() else { return };
            let Some((_, image, _)) = dlg.shown else { return };
            self.paste_image(image);
            // OpenStreetMaps villkor kräver att upphovet står i bilden.
            if let Some(d) = self.doc() {
                if let Some(map_el) = d.sel.first().and_then(|id| d.get(*id)).cloned() {
                    let r = map_el.rect();
                    let size = (r.width() * 0.022).clamp(9.0, 16.0);
                    let label = Rect::from_min_size(pos2(r.max.x - size * 9.6, r.max.y - size * 1.9), vec2(size * 9.4, size * 1.7));
                    let text = TextData { text: map::ATTRIBUTION.into(), size, box_fill: Some(Color32::from_rgba_unmultiplied(255, 255, 255, 215)), box_border: None, align: 0 };
                    d.add(Element { id: 0, kind: Kind::Text(text), a: label.min, b: label.max, rot: 0.0, stroke: Color32::BLACK, fill: Color32::TRANSPARENT, width: 1.0 });
                }
            }
            self.say(format!("Karta inlagd: {}", dlg.place.chars().take(60).collect::<String>()));
        } else if !open {
            self.x.map = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, image::Rgba([90, 120, 150, 255]))
    }

    #[test]
    fn four_photos_per_sheet_in_report_proportion() {
        let photos = (0..6).map(|i| (format!("bild{i}"), photo(768, 576))).collect();
        let docs = build_sheets("P-17", photos, 10);
        assert_eq!(docs.len(), 2);
        // Kamerabilder på 768 × 576 vänds till 576 × 768 och ger blad på 1218 × 1602.
        assert_eq!(docs[0].base.dimensions(), (1218, 1602));
        assert_eq!(docs[0].sheet.as_deref(), Some("P-17-skiss-1"));
        assert_eq!(docs[1].sheet.as_deref(), Some("P-17-skiss-2"));
        // Fyra bilder + fyra texter + ramen, och ramen ligger överst.
        assert_eq!(docs[0].els.len(), 9);
        assert!(matches!(docs[0].els.last().unwrap().kind, Kind::SheetFrame(_)));
        assert_eq!(docs[1].els.len(), 5);
        // Bladet har nästan samma proportion som rapportens bildyta 565:744.
        let ratio = 1218.0 / 1602.0;
        assert!((ratio - 565.0 / 744.0f32).abs() < 0.01);
    }

    #[test]
    fn frame_stays_on_top_and_cannot_be_hit() {
        let mut docs = build_sheets("x", vec![("a".into(), photo(300, 400))], 1);
        let d = &mut docs[0];
        d.add(Element { id: 0, kind: Kind::Rect, a: pos2(0.0, 0.0), b: pos2(50.0, 50.0), rot: 0.0, stroke: Color32::RED, fill: Color32::TRANSPARENT, width: 2.0 });
        assert!(matches!(d.els.last().unwrap().kind, Kind::SheetFrame(_)));
        // En punkt i marginalen träffar varken ramen eller något annat.
        assert_eq!(d.hit(pos2(2.0, 300.0), 1.0), None);
    }
}
