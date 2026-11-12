//! Installation på Windows. Det finns ingen separat installerare: programfilen
//! installerar sig själv när den heter något med "Setup" (eller startas med `--setup`).
//!
//! Allt sker per användare, utan administratörsrättigheter:
//!   programmet    %LOCALAPPDATA%\TRK ForceShot 2\TRK ForceShot.exe
//!   Start-menyn   TRK ForceShot 2.lnk
//!   avinstallera  HKCU\...\Uninstall\TRKForceShot2
//!   autostart     HKCU\...\Run, värdet "TRK ForceShot 2"
//!
//! Version 1 ligger i en egen mapp med egna poster och rörs inte, utom att användaren
//! kan välja att stänga av dess autostart (båda versionerna vill ha PrintScreen).
//!
//! Här finns också det som ser till att bara en instans kör åt gången: en andra start
//! lämnar över sina filer till den som redan kör och avslutar sig.

use std::path::{Path, PathBuf};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const FOLDER: &str = "TRK ForceShot 2";
const EXE_NAME: &str = "TRK ForceShot.exe";

pub fn install_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_default().join(FOLDER)
}

pub fn installed_exe() -> PathBuf {
    install_dir().join(EXE_NAME)
}

fn same_file(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").trim_start_matches("\\\\?\\").to_lowercase();
    norm(a) == norm(b)
}

/// Sant när den här processen är den installerade kopian.
pub fn running_installed() -> bool {
    std::env::current_exe().is_ok_and(|me| same_file(&me, &installed_exe()))
}

/// Ska installationsfönstret visas i stället för programmet?
pub fn wants_setup(forced: bool) -> bool {
    if !cfg!(windows) {
        return false;
    }
    if forced {
        return true;
    }
    let Ok(me) = std::env::current_exe() else { return false };
    let named_setup = me.file_stem().is_some_and(|n| n.to_string_lossy().to_lowercase().contains("setup"));
    named_setup && !running_installed()
}

#[derive(Clone, Copy)]
pub struct Options {
    pub autostart: bool,
    /// Ta bort version 1:s autostart så att de två inte slåss om PrintScreen.
    pub disable_v1_autostart: bool,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::process::CommandExt;
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS};
    use windows_sys::Win32::System::Threading::*;
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_SET_VALUE};

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const RUN_VALUE: &str = "TRK ForceShot 2";
    const V1_RUN_VALUES: [&str; 2] = ["TRK ForceShot", "ForceShot"];
    const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\TRKForceShot2";
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
    }

    fn shortcut() -> PathBuf {
        dirs::data_dir().unwrap_or_default().join(r"Microsoft\Windows\Start Menu\Programs").join(format!("{FOLDER}.lnk"))
    }

    pub fn installed_version() -> Option<String> {
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(UNINSTALL_KEY, KEY_READ).ok()?;
        key.get_value::<String, _>("DisplayVersion").ok()
    }

    pub fn autostart_enabled() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN_KEY, KEY_READ)
            .and_then(|k| k.get_value::<String, _>(RUN_VALUE))
            .is_ok()
    }

    /// Slår av eller på start vid inloggning, för den angivna programfilen.
    pub fn set_autostart(on: bool, exe: &Path) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
        if on {
            key.set_value(RUN_VALUE, &format!("\"{}\" --tray", exe.display())).map_err(|e| e.to_string())
        } else {
            match key.delete_value(RUN_VALUE) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
                _ => Ok(()),
            }
        }
    }

    /// Startar version 1 automatiskt vid inloggning?
    pub fn v1_autostart() -> bool {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_READ) else { return false };
        V1_RUN_VALUES.iter().any(|v| key.get_value::<String, _>(v).is_ok())
    }

    fn disable_v1_autostart() {
        if let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN_KEY, KEY_SET_VALUE) {
            for v in V1_RUN_VALUES {
                let _ = key.delete_value(v);
            }
        }
    }

    /// Avslutar andra processer som kör just den här programfilen, och väntar tills de är borta.
    /// Windows låter inte en programfil skrivas över medan den kör.
    fn stop_running(target: &Path) {
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE {
                return;
            }
            let me = GetCurrentProcessId();
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut more = Process32FirstW(snap, &mut entry) != 0;
            while more {
                if entry.th32ProcessID != me {
                    let h: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE | PROCESS_SYNCHRONIZE, 0, entry.th32ProcessID);
                    if !h.is_null() {
                        let mut buf = [0u16; 1024];
                        let mut len = buf.len() as u32;
                        if QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut len) != 0 {
                            let path = PathBuf::from(String::from_utf16_lossy(&buf[..len as usize]));
                            if same_file(&path, target) {
                                TerminateProcess(h, 0);
                                WaitForSingleObject(h, 4000);
                            }
                        }
                        CloseHandle(h);
                    }
                }
                more = Process32NextW(snap, &mut entry) != 0;
            }
            CloseHandle(snap);
        }
    }

    fn powershell(script: &str) -> Result<(), String> {
        let status = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() { Ok(()) } else { Err(format!("PowerShell avslutades med {status}")) }
    }

    fn write_shortcut(target: &Path) -> Result<(), String> {
        let link = shortcut();
        if let Some(dir) = link.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Enkla citattecken dubblas i PowerShell-strängar.
        let q = |p: &Path| p.to_string_lossy().replace('\'', "''");
        let script = format!(
            "$s=(New-Object -ComObject WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.WorkingDirectory='{}';$s.Description='TRK ForceShot';$s.Save()",
            q(&link),
            q(target),
            q(target.parent().unwrap_or(Path::new("")))
        );
        powershell(&script)
    }

    fn write_uninstall_entry(exe: &Path) -> Result<(), String> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(UNINSTALL_KEY).map_err(|e| e.to_string())?;
        let size_kb = std::fs::metadata(exe).map(|m| (m.len() / 1024) as u32).unwrap_or(0);
        let set = |name: &str, value: &str| key.set_value(name, &value.to_string()).map_err(|e| e.to_string());
        set("DisplayName", FOLDER)?;
        set("DisplayVersion", VERSION)?;
        set("Publisher", "TRK Kontrolladministration AB")?;
        set("DisplayIcon", &exe.to_string_lossy())?;
        set("InstallLocation", &install_dir().to_string_lossy())?;
        set("UninstallString", &format!("\"{}\" --uninstall", exe.display()))?;
        key.set_value("NoModify", &1u32).map_err(|e| e.to_string())?;
        key.set_value("NoRepair", &1u32).map_err(|e| e.to_string())?;
        key.set_value("EstimatedSize", &size_kb).map_err(|e| e.to_string())
    }

    const MENU_TYPES: [&str; 9] = [".png", ".jpg", ".jpeg", ".bmp", ".tif", ".tiff", ".gif", ".webp", ".pdf"];
    const MENU_KEY: &str = "TRKForceShot2";

    /// Lägger "Redigera med TRK ForceShot" i Utforskarens högerklicksmeny för bilder och
    /// PDF:er. I Windows 11 hamnar kommandot under "Visa fler alternativ".
    fn write_context_menu(exe: &Path) {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        for ext in MENU_TYPES {
            let base = format!(r"Software\Classes\SystemFileAssociations\{ext}\shell\{MENU_KEY}");
            if let Ok((key, _)) = root.create_subkey(&base) {
                let _ = key.set_value("", &"Redigera med TRK ForceShot".to_string());
                let _ = key.set_value("Icon", &format!("{},0", exe.display()));
            }
            if let Ok((cmd, _)) = root.create_subkey(format!(r"{base}\command")) {
                let _ = cmd.set_value("", &format!("\"{}\" \"%1\"", exe.display()));
            }
        }
    }

    fn remove_context_menu() {
        let root = RegKey::predef(HKEY_CURRENT_USER);
        for ext in MENU_TYPES {
            let _ = root.delete_subkey_all(format!(r"Software\Classes\SystemFileAssociations\{ext}\shell\{MENU_KEY}"));
        }
    }

    /// Installerar eller uppdaterar: kopierar den här filen till installationsmappen och
    /// skriver genväg, avinstallationspost och autostart.
    pub fn install(opts: Options) -> Result<(), String> {
        let me = std::env::current_exe().map_err(|e| e.to_string())?;
        let target = installed_exe();
        std::fs::create_dir_all(install_dir()).map_err(|e| format!("Kunde inte skapa mappen: {e}"))?;
        if !same_file(&me, &target) {
            stop_running(&target);
            // Filen kan vara låst ett ögonblick efter att processen avslutats.
            let mut last = String::new();
            let mut copied = false;
            for _ in 0..20 {
                match std::fs::copy(&me, &target) {
                    Ok(_) => {
                        copied = true;
                        break;
                    }
                    Err(e) => {
                        last = e.to_string();
                        std::thread::sleep(std::time::Duration::from_millis(250));
                    }
                }
            }
            if !copied {
                return Err(format!("Kunde inte kopiera programmet: {last}"));
            }
        }
        write_uninstall_entry(&target)?;
        write_context_menu(&target);
        set_autostart(opts.autostart, &target)?;
        if opts.disable_v1_autostart {
            disable_v1_autostart();
        }
        // Genvägen är inte värd att stoppa installationen för.
        if let Err(e) = write_shortcut(&target) {
            log::warn!("Kunde inte skapa genvägen i Start-menyn: {e}");
        }
        Ok(())
    }

    pub fn launch_installed() {
        let _ = std::process::Command::new(installed_exe()).current_dir(install_dir()).spawn();
    }

    /// Tar bort programmet. Inställningarna i %APPDATA%\ForceShot2 lämnas kvar.
    pub fn uninstall() -> Result<(), String> {
        let target = installed_exe();
        stop_running(&target);
        let _ = set_autostart(false, &target);
        let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(UNINSTALL_KEY);
        let _ = std::fs::remove_file(shortcut());
        remove_context_menu();
        let dir = install_dir();
        if dir.exists() {
            // En körande programfil kan inte ta bort sig själv; cmd gör det strax efter att vi avslutat.
            std::process::Command::new("cmd")
                // raw_arg: cmd tolkar citattecken på sitt eget sätt och tål inte Rusts vanliga citering.
                .raw_arg(format!("/c ping -n 3 127.0.0.1 >nul & rmdir /s /q \"{}\"", dir.display()))
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn inbox() -> PathBuf {
        crate::settings::config_dir().join("inkorg.txt")
    }

    /// Håller instanslåset så länge programmet kör.
    pub struct InstanceGuard(#[allow(dead_code)] isize);

    /// Tar instanslåset. Kör programmet redan lämnas argumenten över till den instansen
    /// och `None` returneras – då ska den här processen avsluta sig.
    pub fn single_instance(args: &[String]) -> Option<InstanceGuard> {
        unsafe {
            let mutex = CreateMutexW(null(), 0, wide("Local\\TRKForceShot2-instans").as_ptr());
            if !mutex.is_null() && GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = std::fs::create_dir_all(crate::settings::config_dir());
                let mut text = String::new();
                if args.is_empty() {
                    text.push_str("--visa\n");
                }
                for a in args {
                    text.push_str(a);
                    text.push('\n');
                }
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(inbox()) {
                    let _ = f.write_all(text.as_bytes());
                }
                let event = CreateEventW(null(), 0, 0, wide("Local\\TRKForceShot2-inkorg").as_ptr());
                if !event.is_null() {
                    SetEvent(event);
                    CloseHandle(event);
                }
                CloseHandle(mutex);
                return None;
            }
            Some(InstanceGuard(mutex as isize))
        }
    }

    /// Lyssnar efter filer och kommandon som en andra start lämnar över.
    pub fn watch_inbox(send: impl Fn(Vec<String>) + Send + 'static) {
        let _ = std::fs::remove_file(inbox());
        std::thread::spawn(move || unsafe {
            let event = CreateEventW(null(), 0, 0, wide("Local\\TRKForceShot2-inkorg").as_ptr());
            if event.is_null() {
                return;
            }
            loop {
                if WaitForSingleObject(event, INFINITE) != 0 {
                    return;
                }
                // Den andra processen hinner oftast skriva klart först, men vänta en aning för säkerhets skull.
                std::thread::sleep(std::time::Duration::from_millis(60));
                if let Ok(text) = std::fs::read_to_string(inbox()) {
                    let _ = std::fs::remove_file(inbox());
                    let lines: Vec<String> = text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
                    if !lines.is_empty() {
                        send(lines);
                    }
                }
            }
        });
        let _ = null_mut::<u8>();
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn installed_version() -> Option<String> {
        None
    }
    pub fn autostart_enabled() -> bool {
        false
    }
    pub fn set_autostart(_on: bool, _exe: &Path) -> Result<(), String> {
        Err("Autostart ställs in i Systeminställningar på Mac.".into())
    }
    pub fn v1_autostart() -> bool {
        false
    }
    pub fn install(_opts: Options) -> Result<(), String> {
        Err("Installationen finns bara på Windows.".into())
    }
    pub fn launch_installed() {}
    pub fn uninstall() -> Result<(), String> {
        Err("Avinstallationen finns bara på Windows.".into())
    }
    pub struct InstanceGuard;
    pub fn single_instance(_args: &[String]) -> Option<InstanceGuard> {
        Some(InstanceGuard)
    }
    pub fn watch_inbox(_send: impl Fn(Vec<String>) + Send + 'static) {}
}

pub use imp::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_paths_loosely() {
        assert!(same_file(Path::new(r"C:\A\b.exe"), Path::new(r"c:/a/B.EXE")));
        assert!(same_file(Path::new(r"\\?\C:\A\b.exe"), Path::new(r"C:\A\b.exe")));
        assert!(!same_file(Path::new(r"C:\A\b.exe"), Path::new(r"C:\A\c.exe")));
    }

    #[test]
    fn install_dir_is_separate_from_version_1() {
        assert!(install_dir().ends_with("TRK ForceShot 2"));
    }
}
