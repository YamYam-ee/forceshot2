//! Programkärnan: händelseloopen, fönstren, aktivitetsfältsikonen och flödet från
//! snabbtangent till färdig bild. Programmet ligger i aktivitetsfältet utan fönster
//! tills något behövs, och drar då inga grafikresurser.

use crate::capture::{Outcome, Overlay, Then};
use crate::editor::{Editor, Env, Request};
use crate::platform::{self, Action, Hotkeys, Shot};
use crate::settings::{AfterCapture, Settings};
use crate::{imaging, install, sketch, theme};
use egui::ViewportId;
use egui_wgpu::winit::Painter;
use image::RgbaImage;
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId, WindowLevel};

#[derive(Debug)]
pub enum UserEvent {
    Repaint(ViewportId, Instant),
    Hotkey(Action),
    Menu(String),
    TrayClick,
    Sketch(u64, String),
    /// Filer och kommandon från en andra start av programmet.
    Inbox(Vec<String>),
    /// En nyare utgåva hittades vid starten.
    Update(crate::update::Release),
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Overlay(usize),
    Editor,
    Setup,
}

struct Win {
    window: Arc<Window>,
    state: egui_winit::State,
    vid: ViewportId,
    kind: Kind,
}

pub struct Startup {
    pub files: Vec<PathBuf>,
    pub sketch: Option<String>,
    pub tray_only: bool,
    /// Provbild med alla objekttyper; med en sökväg exporteras den dit.
    pub demo: Option<Option<PathBuf>>,
    /// Öppnar markeringsöverlägget direkt vid start.
    pub capture: bool,
    /// Läser av texten i den öppnade bilden direkt vid start.
    pub ocr: bool,
    /// Installationsläge. Sant = installera utan att fråga (uppdatering).
    pub setup: Option<bool>,
    /// Utvecklingslägen för fiberblad och arbetsyta: (läge, valfri utfil eller utmapp).
    pub dev: Option<(String, Option<PathBuf>)>,
}

pub struct App {
    proxy: EventLoopProxy<UserEvent>,
    ctx: egui::Context,
    painter: Painter,
    wins: HashMap<WindowId, Win>,
    repaint_at: HashMap<WindowId, Instant>,
    pending: egui::TexturesDelta,
    start: Instant,
    settings: Settings,
    overlay: Option<Overlay>,
    editor: Editor,
    tray: Option<TrayIcon>,
    hotkeys: Option<Hotkeys>,
    startup: Option<Startup>,
    next_vid: u64,
    setup: Option<SetupUi>,
}
/// Installationsfönstrets tillstånd.
struct SetupUi {
    installed: Option<String>,
    autostart: bool,
    v1_autostart: bool,
    disable_v1: bool,
    error: Option<String>,
    done: bool,
    logo: Option<egui::TextureHandle>,
}

impl SetupUi {
    fn new() -> Self {
        let installed = install::installed_version();
        Self {
            // En uppdatering behåller valet som redan gjorts; en ny installation startar med Windows.
            autostart: installed.is_none() || install::autostart_enabled(),
            installed,
            v1_autostart: install::v1_autostart(),
            disable_v1: false,
            error: None,
            done: false,
            logo: None,
        }
    }

    fn options(&self) -> install::Options {
        install::Options { autostart: self.autostart, disable_v1_autostart: self.disable_v1 }
    }

    fn run(&mut self) {
        match install::install(self.options()) {
            Ok(()) => {
                install::launch_installed();
                self.done = true;
            }
            Err(e) => self.error = Some(e),
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) {
        use egui::{FontId, RichText};
        let ctx = ui.ctx().clone();
        let logo = self
            .logo
            .get_or_insert_with(|| {
                let (rgba, w, h) = icon_rgba(include_bytes!("../assets/icon-256.png")).unwrap_or((vec![0; 4], 1, 1));
                ctx.load_texture("logga", egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba), egui::TextureOptions::LINEAR)
            })
            .id();
        let frame = egui::Frame::new().fill(theme::SURFACE).inner_margin(28.0);
        egui::CentralPanel::default().frame(frame).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Image::new((logo, egui::Vec2::splat(56.0))));
                ui.add_space(8.0);
                ui.vertical(|ui| {
                    ui.add_space(4.0);
                    ui.label(RichText::new("TRK ForceShot 2").font(FontId::new(24.0, theme::bold())));
                    ui.label(RichText::new(format!("Version {}", install::VERSION)).color(theme::TEXT_DIM));
                });
            });
            ui.add_space(18.0);
            let (action, note) = match &self.installed {
                None => ("Installera", "Programmet är inte installerat på den här datorn.".to_string()),
                Some(v) if crate::update::newer(install::VERSION, v) => ("Uppdatera", format!("Version {v} är installerad och byts ut mot {}.", install::VERSION)),
                Some(v) if crate::update::newer(v, install::VERSION) => ("Installera ändå", format!("Obs: version {v} är installerad, och den är nyare än den här filen.")),
                Some(v) => ("Installera om", format!("Version {v} är redan installerad.")),
            };
            ui.label(note);
            ui.add_space(4.0);
            ui.label(RichText::new("Läggs i din egen användarmapp – inga administratörsrättigheter behövs. Inställningar och metodfärger rörs inte av en uppdatering.").color(theme::TEXT_DIM).size(12.5));
            ui.add_space(14.0);
            ui.checkbox(&mut self.autostart, "Starta ForceShot när jag loggar in");
            if self.v1_autostart {
                ui.add_space(6.0);
                ui.label(RichText::new("Version 1 startar också vid inloggning. Båda vill ha PrintScreen, så bara en bör starta av sig själv.").color(theme::TEXT_DIM).size(12.5));
                ui.checkbox(&mut self.disable_v1, "Stäng av autostarten för version 1");
            }
            if let Some(e) = &self.error {
                ui.add_space(10.0);
                ui.label(RichText::new(e.as_str()).color(theme::DANGER));
            }
            ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.horizontal(|ui| {
                    if crate::icons::labeled(ui, crate::icons::Icon::Save, action, true).clicked() {
                        self.run();
                    }
                    if ui.button("Avbryt").clicked() {
                        self.done = true;
                    }
                });
            });
        });
    }
}


fn icon_rgba(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((img.into_raw(), w, h))
}

pub fn run(startup: Startup) {
    let event_loop = match EventLoop::<UserEvent>::with_user_event().build() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("Kunde inte starta händelseloopen: {e}");
            return;
        }
    };
    let proxy = event_loop.create_proxy();
    let ctx = egui::Context::default();
    theme::apply(&ctx);
    {
        let proxy = proxy.clone();
        ctx.set_request_repaint_callback(move |info| {
            let _ = proxy.send_event(UserEvent::Repaint(info.viewport_id, Instant::now() + info.delay));
        });
    }
    let painter = pollster::block_on(Painter::new(ctx.clone(), egui_wgpu::WgpuConfiguration::default(), false, egui_wgpu::RendererOptions::default()));
    let mut app = App {
        proxy,
        ctx,
        painter,
        wins: HashMap::new(),
        repaint_at: HashMap::new(),
        pending: Default::default(),
        start: Instant::now(),
        settings: Settings::load(),
        overlay: None,
        editor: Editor::new(),
        tray: None,
        hotkeys: None,
        startup: Some(startup),
        next_vid: 1,
        setup: None,
    };
    if let Err(e) = event_loop.run_app(&mut app) {
        eprintln!("Händelseloopen avslutades med fel: {e}");
    }
}

impl App {
    fn bind_hotkeys(&mut self) {
        let proxy = self.proxy.clone();
        self.hotkeys = Some(Hotkeys::start(platform::bindings(&self.settings.hotkeys), move |a| {
            let _ = proxy.send_event(UserEvent::Hotkey(a));
        }));
    }

    fn build_tray(&mut self) {
        let menu = Menu::new();
        let item = |id: &str, text: &str| MenuItem::with_id(id, text, true, None);
        let mut entries: Vec<Box<dyn tray_icon::menu::IsMenuItem>> = vec![
            Box::new(item("region", "Klipp ett område")),
            Box::new(item("full", "Hela skärmen")),
            Box::new(item("window", "Aktivt fönster")),
            Box::new(PredefinedMenuItem::separator()),
            Box::new(item("editor", "Öppna ForceShot")),
            Box::new(item("open", "Öppna bild eller PDF …")),
            Box::new(item("paste", "Öppna bilden i urklippet")),
            Box::new(PredefinedMenuItem::separator()),
        ];
        for s in sketch::catalog() {
            entries.push(Box::new(item(&format!("skiss:{}", s.id), &s.name)));
        }
        entries.push(Box::new(PredefinedMenuItem::separator()));
        entries.push(Box::new(item("settings", "Inställningar")));
        entries.push(Box::new(item("quit", "Avsluta")));
        for e in &entries {
            let _ = menu.append(e.as_ref());
        }
        let proxy = self.proxy.clone();
        MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
            let _ = proxy.send_event(UserEvent::Menu(e.id.0.clone()));
        }));
        let proxy = self.proxy.clone();
        TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                let _ = proxy.send_event(UserEvent::TrayClick);
            }
        }));
        let mut builder = TrayIconBuilder::new().with_menu(Box::new(menu)).with_tooltip("TRK ForceShot").with_menu_on_left_click(false);
        if let Some((rgba, w, h)) = icon_rgba(include_bytes!("../assets/icon-32.png")) {
            if let Ok(icon) = tray_icon::Icon::from_rgba(rgba, w, h) {
                builder = builder.with_icon(icon);
            }
        }
        match builder.build() {
            Ok(t) => self.tray = Some(t),
            Err(e) => log::error!("Kunde inte skapa aktivitetsfältsikonen: {e}"),
        }
    }

    fn add_window(&mut self, window: Window, kind: Kind) -> Option<WindowId> {
        let window = Arc::new(window);
        let vid = ViewportId::from_hash_of(("fönster", self.next_vid));
        self.next_vid += 1;
        if let Err(e) = pollster::block_on(self.painter.set_window(vid, Some(window.clone()))) {
            log::error!("Kunde inte skapa ritytan: {e}");
            return None;
        }
        let state = egui_winit::State::new(self.ctx.clone(), vid, &*window, Some(window.scale_factor() as f32), None, self.painter.max_texture_side());
        let id = window.id();
        window.request_redraw();
        self.wins.insert(id, Win { window, state, vid, kind });
        Some(id)
    }

    fn editor_window(&self) -> Option<&Win> {
        self.wins.values().find(|w| w.kind == Kind::Editor)
    }

    /// Visar redigerarfönstret, och skapar det om det inte finns.
    fn show_editor(&mut self, el: &ActiveEventLoop) {
        if let Some(w) = self.editor_window() {
            w.window.set_minimized(false);
            w.window.set_visible(true);
            platform::force_foreground(&w.window);
            w.window.request_redraw();
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title("TRK ForceShot")
            .with_inner_size(LogicalSize::new(1320.0, 860.0))
            .with_min_inner_size(LogicalSize::new(820.0, 520.0));
        if let Some((rgba, w, h)) = icon_rgba(include_bytes!("../assets/icon-256.png")) {
            attrs = attrs.with_window_icon(winit::window::Icon::from_rgba(rgba, w, h).ok());
        }
        match el.create_window(attrs) {
            Ok(window) => {
                if let Some(id) = self.add_window(window, Kind::Editor) {
                    if let Some(w) = self.wins.get(&id) {
                        platform::force_foreground(&w.window);
                    }
                }
            }
            Err(e) => log::error!("Kunde inte skapa fönstret: {e}"),
        }
    }

    fn close_window(&mut self, id: WindowId) {
        if let Some(w) = self.wins.remove(&id) {
            let _ = pollster::block_on(self.painter.set_window(w.vid, None));
        }
        self.repaint_at.remove(&id);
    }

    fn close_overlay(&mut self) -> Option<Overlay> {
        let ids: Vec<WindowId> = self.wins.iter().filter(|(_, w)| matches!(w.kind, Kind::Overlay(_))).map(|(id, _)| *id).collect();
        for id in ids {
            self.close_window(id);
        }
        self.overlay.take()
    }

    fn start_capture(&mut self, action: Action, el: &ActiveEventLoop) {
        if self.overlay.is_some() {
            return;
        }
        if action == Action::OpenClipboard {
            self.editor.open_clipboard();
            self.show_editor(el);
            return;
        }
        let shots = platform::capture_monitors(self.settings.include_cursor);
        if shots.is_empty() {
            self.show_editor(el);
            return;
        }
        let then = match self.settings.after_capture {
            AfterCapture::Kopiera => Then::Copy,
            AfterCapture::Spara => Then::Save,
            _ => Then::Editor,
        };
        let monitor_at = |shots: &[Shot], x: i32, y: i32| shots.iter().position(|s| x >= s.x && y >= s.y && x < s.x + s.w as i32 && y < s.y + s.h as i32).unwrap_or(0);
        match action {
            Action::FullScreen => {
                let (cx, cy) = platform::cursor_pos();
                let m = monitor_at(&shots, cx, cy);
                self.deliver((*shots[m].image).clone(), then, el);
            }
            Action::Window => {
                let Some((x, y, w, h)) = platform::active_window_rect() else { return };
                let m = monitor_at(&shots, x + w / 2, y + h / 2);
                let s = &shots[m];
                // Skrivbordets enheter räknas om till bildens bildpunkter (skiljer sig på Mac).
                let k = s.image.width() as f32 / s.w.max(1) as f32;
                let img = imaging::crop(&s.image, ((x - s.x) as f32 * k) as i32, ((y - s.y) as f32 * k) as i32, (w as f32 * k) as i32, (h as f32 * k) as i32);
                self.deliver(img, then, el);
            }
            Action::LastRegion => match self.settings.last_region {
                Some((m, r)) if m < shots.len() => {
                    let img = imaging::crop(&shots[m].image, r[0] as i32, r[1] as i32, r[2] as i32, r[3] as i32);
                    self.deliver(img, then, el);
                }
                _ => self.open_overlay(shots, el),
            },
            _ => self.open_overlay(shots, el),
        }
    }

    fn open_overlay(&mut self, shots: Vec<Shot>, el: &ActiveEventLoop) {
        let (cx, cy) = platform::cursor_pos();
        let mut focus = None;
        for (i, s) in shots.iter().enumerate() {
            let mut attrs = Window::default_attributes()
                .with_title("ForceShot – markera")
                .with_decorations(false)
                .with_resizable(false)
                .with_window_level(WindowLevel::AlwaysOnTop);
            attrs = if platform::LOGICAL_POSITIONS {
                attrs.with_position(LogicalPosition::new(s.x, s.y)).with_inner_size(LogicalSize::new(s.w, s.h))
            } else {
                attrs.with_position(PhysicalPosition::new(s.x, s.y)).with_inner_size(PhysicalSize::new(s.w, s.h))
            };
            #[cfg(windows)]
            {
                use winit::platform::windows::WindowAttributesExtWindows;
                attrs = attrs.with_skip_taskbar(true);
            }
            let Ok(window) = el.create_window(attrs) else { continue };
            platform::prepare_overlay_window(&window);
            if let Some(id) = self.add_window(window, Kind::Overlay(i)) {
                let inside = cx >= s.x && cy >= s.y && cx < s.x + s.w as i32 && cy < s.y + s.h as i32;
                if inside || focus.is_none() {
                    focus = Some(id);
                }
            }
        }
        self.overlay = Some(Overlay::new(shots, &self.settings));
        if let Some(w) = focus.and_then(|id| self.wins.get(&id)) {
            platform::force_foreground(&w.window);
        }
    }

    /// Tar hand om en färdig bild: redigera, kopiera eller spara.
    fn deliver(&mut self, img: RgbaImage, then: Then, el: &ActiveEventLoop) {
        match then {
            Then::Editor => {
                self.editor.add_image(img, None, None, 0.0);
                self.show_editor(el);
            }
            Then::Copy => {
                self.editor.copy_image(&img);
            }
            Then::Save => {
                if let Err(e) = self.editor.save_direct(&img, &self.settings) {
                    log::error!("Kunde inte spara klippet: {e}");
                    self.editor.add_image(img, None, None, 0.0);
                    self.show_editor(el);
                }
            }
        }
    }

    fn handle_requests(&mut self, el: &ActiveEventLoop) {
        for r in std::mem::take(&mut self.editor.requests) {
            match r {
                Request::Capture(a) => {
                    // Redigeraren göms så att den inte kommer med i bilden.
                    if let Some(w) = self.editor_window() {
                        w.window.set_visible(false);
                    }
                    std::thread::sleep(Duration::from_millis(220));
                    self.start_capture(a, el);
                    if self.overlay.is_none() {
                        self.show_editor(el);
                    }
                }
                Request::CloseWindow => {
                    let id = self.wins.iter().find(|(_, w)| w.kind == Kind::Editor).map(|(id, _)| *id);
                    if let Some(id) = id {
                        self.close_window(id);
                    }
                    self.settings.save();
                }
                Request::Quit => {
                    self.settings.save();
                    el.exit();
                }
                Request::RebindHotkeys => self.bind_hotkeys(),
                Request::RunInstaller(path) => {
                    // Den hämtade Setup-filen stänger den här instansen, byter ut filen och startar om.
                    self.settings.save();
                    match std::process::Command::new(&path).args(["--setup", "--auto"]).spawn() {
                        Ok(_) => el.exit(),
                        Err(e) => log::error!("Kunde inte starta uppdateringen: {e}"),
                    }
                }
            }
        }
    }

    fn paint(&mut self, id: WindowId, el: &ActiveEventLoop) {
        let Some(win) = self.wins.get_mut(&id) else { return };
        let mut input = win.state.take_egui_input(&win.window);
        input.time = Some(self.start.elapsed().as_secs_f64());
        let kind = win.kind;
        let window = win.window.clone();
        let (overlay, editor, settings, proxy, setup) = (&mut self.overlay, &mut self.editor, &mut self.settings, &self.proxy, &mut self.setup);
        let out = self.ctx.run_ui(input, |ui| match kind {
            Kind::Overlay(i) => {
                if let Some(o) = overlay.as_mut() {
                    o.ui(ui, i);
                }
            }
            Kind::Editor => editor.ui(ui, settings, &Env { window: &window, proxy }),
            Kind::Setup => {
                if let Some(s) = setup.as_mut() {
                    s.ui(ui);
                }
            }
        });
        win.state.handle_platform_output(&window, out.platform_output);
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        self.pending.append(out.textures_delta);
        self.painter.paint_and_update_textures(win.vid, out.pixels_per_point, [0.07, 0.078, 0.094, 1.0], &prims, &mut self.pending, vec![], &window);

        match kind {
            Kind::Setup => {
                if self.setup.as_ref().is_some_and(|s| s.done) {
                    el.exit();
                }
            }
            Kind::Editor => {
                self.editor.after_paint(&self.ctx, self.painter.render_state(), &self.settings);
                self.handle_requests(el);
            }
            Kind::Overlay(_) => {
                if self.overlay.as_ref().is_some_and(|o| o.outcome.is_some()) {
                    let Some(mut o) = self.close_overlay() else { return };
                    match o.outcome.take() {
                        Some(Outcome::Take { monitor, rect, then }) => {
                            self.settings.last_region = Some((monitor, rect));
                            let img = imaging::crop(&o.shots[monitor].image, rect[0] as i32, rect[1] as i32, rect[2] as i32, rect[3] as i32);
                            self.deliver(img, then, el);
                        }
                        _ => {
                            // Avbrutet klipp som startades från redigeraren: visa den igen.
                            if !self.editor.tabs.is_empty() {
                                if let Some(w) = self.editor_window() {
                                    w.window.set_visible(true);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        let Some(startup) = self.startup.take() else { return };
        if let Some(auto) = startup.setup {
            let mut s = SetupUi::new();
            if auto {
                // Uppdatering: installera direkt och visa bara fönstret om något går fel.
                s.run();
                if s.done {
                    el.exit();
                    return;
                }
            }
            self.setup = Some(s);
            let mut attrs = Window::default_attributes().with_title("TRK ForceShot 2 – installation").with_inner_size(LogicalSize::new(500.0, 430.0)).with_resizable(false);
            if let Some((rgba, w, h)) = icon_rgba(include_bytes!("../assets/icon-256.png")) {
                attrs = attrs.with_window_icon(winit::window::Icon::from_rgba(rgba, w, h).ok());
            }
            match el.create_window(attrs) {
                Ok(w) => {
                    self.add_window(w, Kind::Setup);
                }
                Err(_) => el.exit(),
            }
            return;
        }
        self.build_tray();
        self.bind_hotkeys();
        {
            let proxy = self.proxy.clone();
            install::watch_inbox(move |lines| {
                let _ = proxy.send_event(UserEvent::Inbox(lines));
            });
        }
        if self.settings.check_updates && !self.settings.update_repo.trim().is_empty() && startup.demo.is_none() {
            // Tyst koll: hittas ingenting, eller svarar inte nätet, händer ingenting.
            let (proxy, repo, skipped) = (self.proxy.clone(), self.settings.update_repo.clone(), self.settings.skipped_update.clone());
            std::thread::spawn(move || {
                if let Ok(Some(release)) = crate::update::check(&repo) {
                    if release.version != skipped {
                        let _ = proxy.send_event(UserEvent::Update(release));
                    }
                }
            });
        }
        for f in &startup.files {
            self.editor.open_path(f, &self.ctx);
        }
        if startup.ocr {
            self.editor.start_ocr(&self.ctx);
        }
        if let Some(s) = &startup.sketch {
            self.editor.open_sketch(s);
        }
        if let Some(out) = startup.demo.clone() {
            self.editor.demo(&self.settings, out);
        }
        match startup.dev.clone() {
            Some((mode, out)) if mode == "fiber" => self.editor.fiber_demo(out, &self.settings),
            Some((_, out)) => self.editor.space_demo(out),
            None => {}
        }
        if startup.capture {
            self.start_capture(Action::Region, el);
        } else if !startup.tray_only {
            self.show_editor(el);
        }
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Repaint(vid, at) => {
                if let Some((id, _)) = self.wins.iter().find(|(_, w)| w.vid == vid) {
                    let e = self.repaint_at.entry(*id).or_insert(at);
                    *e = (*e).min(at);
                }
            }
            UserEvent::Hotkey(a) => self.start_capture(a, el),
            UserEvent::Inbox(lines) => {
                let mut show = true;
                for line in lines {
                    if line == "--tyst" {
                        show = false;
                    } else if line == "--visa" {
                    } else if let Some(s) = line.strip_prefix("--skiss=") {
                        self.editor.open_sketch(s);
                    } else {
                        self.editor.open_path(std::path::Path::new(&line), &self.ctx);
                    }
                }
                if show {
                    self.show_editor(el);
                }
            }
            UserEvent::Update(release) => {
                self.editor.offer_update(release);
                self.show_editor(el);
            }
            UserEvent::TrayClick => self.show_editor(el),
            UserEvent::Sketch(uid, body) => {
                self.editor.on_sketch_message(uid, &body);
                if let Some(w) = self.editor_window() {
                    w.window.request_redraw();
                }
            }
            UserEvent::Menu(id) => match id.as_str() {
                "region" => self.start_capture(Action::Region, el),
                "full" => self.start_capture(Action::FullScreen, el),
                "window" => self.start_capture(Action::Window, el),
                "editor" => self.show_editor(el),
                "open" => {
                    self.show_editor(el);
                    self.editor.open_dialog(&self.ctx);
                }
                "paste" => {
                    self.editor.open_clipboard();
                    self.show_editor(el);
                }
                "settings" => {
                    self.editor.show_settings();
                    self.show_editor(el);
                }
                "quit" => {
                    self.settings.save();
                    el.exit();
                }
                other => {
                    if let Some(s) = other.strip_prefix("skiss:") {
                        self.editor.open_sketch(s);
                        self.show_editor(el);
                    }
                }
            },
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let Some(win) = self.wins.get_mut(&id) else { return };
        if matches!(event, WindowEvent::RedrawRequested) {
            self.repaint_at.remove(&id);
            self.paint(id, el);
            return;
        }
        let resp = win.state.on_window_event(&win.window, &event);
        if resp.repaint {
            win.window.request_redraw();
        }
        match event {
            WindowEvent::Resized(size) => {
                if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) {
                    self.painter.on_window_resized(win.vid, w, h);
                }
                win.window.request_redraw();
            }
            WindowEvent::CloseRequested => match win.kind {
                Kind::Editor => {
                    self.editor.request_close();
                    win.window.request_redraw();
                    self.handle_requests(el);
                }
                Kind::Overlay(_) => {
                    self.close_overlay();
                }
                Kind::Setup => el.exit(),
            },
            _ => {}
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();
        let mut next: Option<Instant> = None;
        self.repaint_at.retain(|id, at| {
            if *at <= now {
                if let Some(w) = self.wins.get(id) {
                    w.window.request_redraw();
                }
                false
            } else {
                next = Some(next.map_or(*at, |n| n.min(*at)));
                true
            }
        });
        el.set_control_flow(match next {
            Some(t) => ControlFlow::WaitUntil(t),
            None => ControlFlow::Wait,
        });
    }
}
