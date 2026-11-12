//! Skissverktygen (bottenskiss, tankskiss) är små webbappar som följer med inbyggda i
//! programmet. De visas i en webbvy som ligger som en flik i redigerarfönstret:
//! WebView2 på Windows, WKWebView på Mac.

use include_dir::{Dir, include_dir};
use std::borrow::Cow;
use std::cell::Cell;
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::http::{Request, Response};

static SKETCHES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/sketches");

pub struct SketchInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    start: String,
}

/// Alla skisser som följer med i programmet, lästa ur varje mapps `skiss.json`.
pub fn catalog() -> Vec<SketchInfo> {
    let mut out = Vec::new();
    for dir in SKETCHES.dirs() {
        let Some(file) = dir.get_file(dir.path().join("skiss.json")) else { continue };
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(file.contents()) else { continue };
        let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let id = s("id");
        if id.is_empty() {
            continue;
        }
        let start = if s("start").is_empty() { "index.html".to_string() } else { s("start") };
        out.push(SketchInfo { id, name: s("namn"), description: s("beskrivning"), start });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn mime(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn serve(req: Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let path = req.uri().path().trim_start_matches('/');
    match SKETCHES.get_file(path) {
        Some(f) => Response::builder()
            .header("Content-Type", mime(path))
            .header("Cache-Control", "no-cache")
            .body(Cow::Borrowed(f.contents()))
            .unwrap(),
        None => Response::builder().status(404).body(Cow::Borrowed(&b"saknas"[..])).unwrap(),
    }
}

/// Bryggan mellan skissen och programmet. Nedladdningar (PNG, projektfil, SVG, CSV)
/// fångas upp och skickas hit i stället för till nedladdningsmappen, och skissen kan
/// tala om sin skala via `window.forceshot.post`.
const BRIDGE: &str = r#"
(function () {
  const post = o => { try { window.ipc.postMessage(JSON.stringify(o)); } catch (e) {} };
  window.forceshot = { post: m => post({ t: 'msg', m: m }) };
  const send = (href, name) =>
    fetch(href).then(r => r.blob()).then(b => new Promise(done => {
      const fr = new FileReader();
      fr.onload = () => done(fr.result);
      fr.readAsDataURL(b);
    })).then(d => post({ t: 'file', name: name, data: String(d) }));
  const local = a => /^(blob:|data:)/.test(a.href || '');
  const original = HTMLAnchorElement.prototype.click;
  HTMLAnchorElement.prototype.click = function () {
    if (this.hasAttribute('download') && local(this)) { send(this.href, this.download || 'fil'); return; }
    return original.call(this);
  };
  document.addEventListener('click', e => {
    const a = e.target && e.target.closest && e.target.closest('a[download]');
    if (a && local(a)) { e.preventDefault(); send(a.href, a.download || 'fil'); }
  }, true);
})();
"#;

thread_local! {
    static WEB_CONTEXT: Cell<*mut wry::WebContext> = const { Cell::new(std::ptr::null_mut()) };
}

/// En gemensam webbprofil för alla skisser, så att `localStorage` finns kvar mellan körningar.
fn web_context() -> &'static mut wry::WebContext {
    WEB_CONTEXT.with(|c| {
        if c.get().is_null() {
            let dir = crate::settings::config_dir().join("webbvy");
            let _ = std::fs::create_dir_all(&dir);
            c.set(Box::into_raw(Box::new(wry::WebContext::new(Some(dir)))));
        }
        // Profilen skapas en gång på huvudtråden och lever tills programmet avslutas.
        unsafe { &mut *c.get() }
    })
}

pub struct SketchTab {
    pub uid: u64,
    pub title: String,
    pub sketch_id: String,
    pub view: Option<wry::WebView>,
    /// Skalan som skissen senast rapporterade, i bildpunkter per centimeter.
    pub px_per_cm: f32,
    pub error: Option<String>,
    shown: Option<bool>,
    bounds: [f32; 4],
    created: Option<std::time::Instant>,
    /// Fönstret webbvyn sitter i.
    host: Option<winit::window::WindowId>,
}

impl SketchTab {
    pub fn new(uid: u64, info: &SketchInfo) -> Self {
        Self { uid, title: info.name.clone(), sketch_id: info.id.clone(), view: None, px_per_cm: 0.0, error: None, shown: None, bounds: [0.0; 4], created: None, host: None }
    }

    /// Skapar webbvyn första gången fliken visas.
    pub fn ensure(&mut self, window: &winit::window::Window, on_message: impl Fn(u64, String) + 'static) {
        self.rehost(window);
        if self.view.is_some() || self.error.is_some() {
            return;
        }
        self.host = Some(window.id());
        let start = catalog().into_iter().find(|s| s.id == self.sketch_id).map(|s| s.start).unwrap_or_else(|| "index.html".into());
        let url = if cfg!(windows) {
            format!("http://skiss.localhost/{}/{}", self.sketch_id, start)
        } else {
            format!("skiss://localhost/{}/{}", self.sketch_id, start)
        };
        let uid = self.uid;
        let built = wry::WebViewBuilder::new_with_web_context(web_context())
            .with_custom_protocol("skiss".into(), |_id, req| serve(req))
            .with_initialization_script(BRIDGE)
            .with_ipc_handler(move |req: Request<String>| on_message(uid, req.body().clone()))
            .with_url(url)
            .with_visible(false)
            .with_devtools(cfg!(debug_assertions))
            .build_as_child(window);
        match built {
            Ok(v) => {
                self.view = Some(v);
                self.created = Some(std::time::Instant::now());
            }
            Err(e) => self.error = Some(format!("Kunde inte starta webbvyn: {e}")),
        }
    }

    /// Flyttar webbvyn till ett annat fönster när fliken har bytt fönster.
    fn rehost(&mut self, window: &winit::window::Window) {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let Some(view) = &self.view else { return };
        if self.host == Some(window.id()) {
            return;
        }
        self.host = Some(window.id());
        let Ok(handle) = window.window_handle() else { return };
        let moved: Result<(), String> = match handle.as_raw() {
            #[cfg(windows)]
            RawWindowHandle::Win32(h) => {
                use wry::WebViewExtWindows;
                view.reparent(h.hwnd.get()).map_err(|e| e.to_string())
            }
            #[cfg(target_os = "macos")]
            RawWindowHandle::AppKit(h) => {
                use objc2::msg_send;
                use objc2::runtime::AnyObject;
                use wry::WebViewExtMacOS;
                let ns_view = h.ns_view.as_ptr() as *mut AnyObject;
                let ns_window: *mut AnyObject = unsafe { msg_send![ns_view, window] };
                view.reparent(ns_window as *mut _).map_err(|e| e.to_string())
            }
            _ => Err("fönstertypen stöds inte".into()),
        };
        if let Err(e) = moved {
            self.error = Some(format!("Kunde inte flytta skissen till det nya fönstret: {e}"));
        }
        // Läge och synlighet sätts om i det nya fönstret.
        self.bounds = [0.0; 4];
        self.shown = None;
    }

    /// Utvecklingsläge: kör skriptet i miljövariabeln FORCESHOT_SKISSTEST en gång, tre
    /// sekunder efter att skissen laddats. Används för att prova exporten utan att klicka.
    /// Ger sant medan testet väntar, så att fönstret fortsätter rita om.
    pub fn run_test_script(&mut self) -> bool {
        let Some(t) = self.created else { return false };
        let Ok(js) = std::env::var("FORCESHOT_SKISSTEST") else { return false };
        if t.elapsed() < std::time::Duration::from_secs(3) {
            return true;
        }
        self.created = None;
        if let Some(v) = &self.view {
            let _ = v.evaluate_script(&js);
        }
        false
    }

    /// Lägger webbvyn över ritytan (i fönstrets punkter) eller gömmer den.
    pub fn place(&mut self, rect: Option<egui::Rect>) {
        let Some(view) = &self.view else { return };
        let show = rect.is_some();
        if let Some(r) = rect {
            let b = [r.min.x.round(), r.min.y.round(), r.width().round(), r.height().round()];
            if b != self.bounds {
                self.bounds = b;
                let _ = view.set_bounds(wry::Rect {
                    position: LogicalPosition::new(b[0] as f64, b[1] as f64).into(),
                    size: LogicalSize::new(b[2].max(1.0) as f64, b[3].max(1.0) as f64).into(),
                });
            }
        }
        if self.shown != Some(show) {
            self.shown = Some(show);
            let _ = view.set_visible(show);
        }
    }
}

/// Ett meddelande från skissens brygga.
pub enum SketchMessage {
    Scale(f32),
    File { name: String, bytes: Vec<u8> },
}

pub fn parse_message(json: &str) -> Option<SketchMessage> {
    use base64::Engine;
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    match v.get("t")?.as_str()? {
        "msg" => {
            let m = v.get("m")?;
            (m.get("forceshot")?.as_str()? == "skala").then_some(())?;
            Some(SketchMessage::Scale(m.get("bildpunkterPerCm")?.as_f64()? as f32))
        }
        "file" => {
            let name = v.get("name")?.as_str()?.to_string();
            let data = v.get("data")?.as_str()?;
            let b64 = data.split_once("base64,")?.1;
            let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
            Some(SketchMessage::File { name, bytes })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_sketches_are_found() {
        let ids: Vec<String> = catalog().into_iter().map(|s| s.id).collect();
        assert!(ids.contains(&"bottenskiss".to_string()), "{ids:?}");
        assert!(ids.contains(&"tankskiss".to_string()), "{ids:?}");
    }

    #[test]
    fn serves_bundled_files() {
        let req = Request::builder().uri("http://skiss.localhost/tankskiss/index.html").body(Vec::new()).unwrap();
        let resp = serve(req);
        assert_eq!(resp.status(), 200);
        assert!(resp.body().len() > 100);
        let missing = Request::builder().uri("http://skiss.localhost/finns/inte.js").body(Vec::new()).unwrap();
        assert_eq!(serve(missing).status(), 404);
    }

    #[test]
    fn parses_bridge_messages() {
        let scale = r#"{"t":"msg","m":{"forceshot":"skala","bildpunkterPerCm":3.5}}"#;
        assert!(matches!(parse_message(scale), Some(SketchMessage::Scale(v)) if (v - 3.5).abs() < 1e-6));
        let file = r#"{"t":"file","name":"a.png","data":"data:image/png;base64,AQID"}"#;
        match parse_message(file) {
            Some(SketchMessage::File { name, bytes }) => {
                assert_eq!(name, "a.png");
                assert_eq!(bytes, vec![1, 2, 3]);
            }
            _ => panic!("fel meddelandetyp"),
        }
    }
}
