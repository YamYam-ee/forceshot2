//! Windows: GDI-skärmdump och en tangentbordskrok på låg nivå, så att PrintScreen
//! fångas innan Windows eget klippverktyg får den.

use super::{Action, Combo, Shot};
use image::RgbaImage;
use std::ptr::null_mut;
use std::sync::{Arc, Mutex, OnceLock};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::{DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute};
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Fönster placeras i fysiska bildpunkter.
pub const LOGICAL_POSITIONS: bool = false;

unsafe extern "system" fn monitor_proc(_m: HMONITOR, _dc: HDC, rect: *mut RECT, data: LPARAM) -> i32 {
    let list = unsafe { &mut *(data as *mut Vec<RECT>) };
    list.push(unsafe { *rect });
    1
}

fn monitor_rects() -> Vec<RECT> {
    let mut list: Vec<RECT> = Vec::new();
    unsafe {
        EnumDisplayMonitors(null_mut(), std::ptr::null(), Some(monitor_proc), &mut list as *mut _ as LPARAM);
    }
    list
}

fn grab(x: i32, y: i32, w: i32, h: i32, include_cursor: bool) -> Option<RgbaImage> {
    if w <= 0 || h <= 0 {
        return None;
    }
    unsafe {
        let screen = GetDC(null_mut());
        if screen.is_null() {
            return None;
        }
        let mem = CreateCompatibleDC(screen);
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        info.bmiHeader.biWidth = w;
        info.bmiHeader.biHeight = -h; // negativt = uppifrån och ned
        info.bmiHeader.biPlanes = 1;
        info.bmiHeader.biBitCount = 32;
        let mut bits: *mut core::ffi::c_void = null_mut();
        let dib = CreateDIBSection(mem, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        let mut out = None;
        if !dib.is_null() && !bits.is_null() {
            let old = SelectObject(mem, dib as _);
            // CAPTUREBLT tar med lagerfönster som verktygstips.
            if BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY | CAPTUREBLT) == 0 {
                BitBlt(mem, 0, 0, w, h, screen, x, y, SRCCOPY);
            }
            if include_cursor {
                draw_cursor(mem, x, y);
            }
            let len = (w as usize) * (h as usize) * 4;
            let mut buf = std::slice::from_raw_parts(bits as *const u8, len).to_vec();
            crate::imaging::bgra_to_rgba(&mut buf);
            out = RgbaImage::from_raw(w as u32, h as u32, buf);
            SelectObject(mem, old);
        }
        if !dib.is_null() {
            DeleteObject(dib as _);
        }
        DeleteDC(mem);
        ReleaseDC(null_mut(), screen);
        out
    }
}

unsafe fn draw_cursor(dc: HDC, ox: i32, oy: i32) {
    unsafe {
        let mut ci: CURSORINFO = std::mem::zeroed();
        ci.cbSize = std::mem::size_of::<CURSORINFO>() as u32;
        if GetCursorInfo(&mut ci) == 0 || ci.flags & CURSOR_SHOWING == 0 || ci.hCursor.is_null() {
            return;
        }
        let mut ii: ICONINFO = std::mem::zeroed();
        let (mut hx, mut hy) = (0, 0);
        if GetIconInfo(ci.hCursor, &mut ii) != 0 {
            hx = ii.xHotspot as i32;
            hy = ii.yHotspot as i32;
            if !ii.hbmColor.is_null() {
                DeleteObject(ii.hbmColor as _);
            }
            if !ii.hbmMask.is_null() {
                DeleteObject(ii.hbmMask as _);
            }
        }
        DrawIconEx(dc, ci.ptScreenPos.x - ox - hx, ci.ptScreenPos.y - oy - hy, ci.hCursor, 0, 0, 0, null_mut(), DI_NORMAL);
    }
}

/// En frusen bild per skärm.
pub fn capture_monitors(include_cursor: bool) -> Vec<Shot> {
    monitor_rects()
        .into_iter()
        .filter_map(|r| {
            let (w, h) = (r.right - r.left, r.bottom - r.top);
            grab(r.left, r.top, w, h, include_cursor).map(|image| Shot {
                x: r.left,
                y: r.top,
                w: w as u32,
                h: h as u32,
                image: Arc::new(image),
            })
        })
        .collect()
}

/// Pekarens läge i skrivbordets enheter.
pub fn cursor_pos() -> (i32, i32) {
    let mut p = POINT { x: 0, y: 0 };
    unsafe {
        if GetPhysicalCursorPos(&mut p) == 0 {
            GetCursorPos(&mut p);
        }
    }
    (p.x, p.y)
}

/// Läser text ur bilden med Windows inbyggda textigenkänning. Svenska väljs om språket
/// finns installerat, annars användarens egna språk. Ger en sträng per rad.
/// Ska anropas från en bakgrundstråd: avläsningen väntar tills den är klar.
pub fn ocr(img: &RgbaImage) -> Result<Vec<String>, String> {
    use windows::Globalization::Language;
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    use windows::core::HSTRING;
    let run = || -> windows::core::Result<Vec<String>> {
        // Motorn tar inte hur stora bilder som helst; större bilder krymps först.
        let max = OcrEngine::MaxImageDimension()?.max(1000);
        let scaled;
        let src = if img.width() > max || img.height() > max {
            let k = max as f32 / img.width().max(img.height()) as f32;
            scaled = image::imageops::resize(img, (img.width() as f32 * k) as u32, (img.height() as f32 * k) as u32, image::imageops::FilterType::Triangle);
            &scaled
        } else {
            img
        };
        let mut bgra = src.as_raw().clone();
        for px in bgra.chunks_exact_mut(4) {
            px.swap(0, 2);
        }
        let writer = DataWriter::new()?;
        writer.WriteBytes(&bgra)?;
        let buffer = writer.DetachBuffer()?;
        let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(&buffer, BitmapPixelFormat::Bgra8, src.width() as i32, src.height() as i32, BitmapAlphaMode::Premultiplied)?;
        let swedish = Language::CreateLanguage(&HSTRING::from("sv-SE")).and_then(|l| OcrEngine::TryCreateFromLanguage(&l));
        let engine = match swedish {
            Ok(e) => e,
            Err(_) => OcrEngine::TryCreateFromUserProfileLanguages()?,
        };
        let result = engine.RecognizeAsync(&bitmap)?.join()?;
        let mut lines = Vec::new();
        for line in result.Lines()? {
            lines.push(line.Text()?.to_string());
        }
        Ok(lines)
    };
    run().map_err(|e| format!("Textavläsningen misslyckades: {}", e.message()))
}

/// Öppnar ett nytt mejl i datorns e-postprogram med filen bifogad (Simple MAPI).
/// Ingenting skickas härifrån. Anropet väntar tills mejlfönstret stängts, så det ska
/// köras i en bakgrundstråd.
pub fn send_mail(path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::FreeLibrary;
    use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows_sys::Win32::System::Mapi::{MAPI_DIALOG, MAPI_LOGON_UI, MapiFileDescW, MapiMessageW};
    let wide = |s: &std::ffi::OsStr| -> Vec<u16> { s.encode_wide().chain(std::iter::once(0)).collect() };
    unsafe {
        let lib = LoadLibraryW(wide(std::ffi::OsStr::new("mapi32.dll")).as_ptr());
        if lib.is_null() {
            return Err("Hittade inget e-postprogram.".into());
        }
        let Some(entry) = GetProcAddress(lib, c"MAPISendMailW".as_ptr() as *const u8) else {
            FreeLibrary(lib);
            return Err("E-postprogrammet stöder inte bilagor på det här sättet.".into());
        };
        let send: unsafe extern "system" fn(usize, usize, *const MapiMessageW, u32, u32) -> u32 = std::mem::transmute(entry);
        let mut full = wide(path.as_os_str());
        let mut name = wide(path.file_name().unwrap_or_default());
        let mut file: MapiFileDescW = std::mem::zeroed();
        file.nPosition = u32::MAX;
        file.lpszPathName = full.as_mut_ptr();
        file.lpszFileName = name.as_mut_ptr();
        let mut msg: MapiMessageW = std::mem::zeroed();
        msg.nFileCount = 1;
        msg.lpFiles = &mut file;
        let code = send(0, 0, &msg, MAPI_DIALOG | MAPI_LOGON_UI, 0);
        FreeLibrary(lib);
        // 0 = klart, 1 = användaren stängde fönstret utan att skicka.
        if code <= 1 { Ok(()) } else { Err(format!("E-postprogrammet svarade med felkod {code}.")) }
    }
}

/// Det aktiva fönstrets synliga ram (utan den osynliga skuggkanten), i skrivbordets enheter.
pub fn active_window_rect() -> Option<(i32, i32, i32, i32)> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }
        let mut r: RECT = std::mem::zeroed();
        let ok = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            &mut r as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        ) == 0
            || GetWindowRect(hwnd, &mut r) != 0;
        (ok && r.right > r.left && r.bottom > r.top).then(|| (r.left, r.top, r.right - r.left, r.bottom - r.top))
    }
}

fn hwnd_of(window: &winit::window::Window) -> Option<HWND> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get() as HWND),
        _ => None,
    }
}

/// Ger fönstret tangentbordsfokus även när appen inte äger förgrunden. Windows vägrar
/// annars, och då hamnar Esc och Enter i fönstret under överlägget.
pub fn force_foreground(window: &winit::window::Window) {
    let Some(hwnd) = hwnd_of(window) else { return };
    unsafe {
        let fg = GetForegroundWindow();
        let ours = GetCurrentThreadId();
        let theirs = if fg.is_null() { 0 } else { GetWindowThreadProcessId(fg, null_mut()) };
        let attach = theirs != 0 && theirs != ours;
        if attach {
            AttachThreadInput(ours, theirs, 1);
        }
        BringWindowToTop(hwnd);
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
        if attach {
            AttachThreadInput(ours, theirs, 0);
        }
    }
}

/// Inget extra behövs på Windows; fönstret skapas redan överst och utan ram.
pub fn prepare_overlay_window(_window: &winit::window::Window) {}

fn vk_of(key: &str) -> Option<u32> {
    let k = key.to_ascii_uppercase();
    if k.len() == 1 {
        let c = k.as_bytes()[0];
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return Some(c as u32);
        }
    }
    if let Some(n) = k.strip_prefix('F').and_then(|n| n.parse::<u32>().ok()) {
        if (1..=24).contains(&n) {
            return Some(VK_F1 as u32 + n - 1);
        }
    }
    Some(match k.as_str() {
        "PRINTSCREEN" | "PRTSC" | "SNAPSHOT" => VK_SNAPSHOT as u32,
        "SPACE" => VK_SPACE as u32,
        "INSERT" => VK_INSERT as u32,
        "PAUSE" => VK_PAUSE as u32,
        "SCROLLLOCK" => VK_SCROLL as u32,
        _ => return None,
    })
}

struct HookState {
    bindings: Vec<(Combo, u32, Action)>,
    send: Box<dyn Fn(Action) + Send>,
}

static HOOK: OnceLock<Mutex<HookState>> = OnceLock::new();

fn down(vk: VIRTUAL_KEY) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && (wparam as u32 == WM_KEYDOWN || wparam as u32 == WM_SYSKEYDOWN) {
        let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
        if let Some(lock) = HOOK.get() {
            if let Ok(state) = lock.lock() {
                let (ctrl, alt, shift) = (down(VK_CONTROL), down(VK_MENU), down(VK_SHIFT));
                let meta = down(VK_LWIN) || down(VK_RWIN);
                let hit = state
                    .bindings
                    .iter()
                    .find(|(c, vk, _)| *vk == info.vkCode && c.ctrl == ctrl && c.alt == alt && c.shift == shift && c.meta == meta);
                if let Some((_, _, action)) = hit {
                    (state.send)(*action);
                    return 1; // tangenten är förbrukad
                }
            }
        }
    }
    unsafe { CallNextHookEx(null_mut(), code, wparam, lparam) }
}

/// Globala snabbtangenter. Kroken lever i en egen tråd med meddelandeloop.
pub struct Hotkeys;

impl Hotkeys {
    pub fn start(bindings: Vec<(Combo, Action)>, send: impl Fn(Action) + Send + 'static) -> Self {
        let resolved = |b: Vec<(Combo, Action)>| -> Vec<(Combo, u32, Action)> {
            b.into_iter().filter_map(|(c, a)| vk_of(&c.key).map(|vk| (c, vk, a))).collect()
        };
        let first = HOOK.get().is_none();
        let state = HOOK.get_or_init(|| Mutex::new(HookState { bindings: Vec::new(), send: Box::new(|_| {}) }));
        if let Ok(mut s) = state.lock() {
            s.bindings = resolved(bindings);
            s.send = Box::new(send);
        }
        if first {
            std::thread::spawn(|| unsafe {
                let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), GetModuleHandleW(std::ptr::null()), 0);
                if hook.is_null() {
                    log::error!("Kunde inte installera tangentbordskroken");
                    return;
                }
                let mut msg: MSG = std::mem::zeroed();
                while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                UnhookWindowsHookEx(hook);
            });
        }
        Hotkeys
    }
}
