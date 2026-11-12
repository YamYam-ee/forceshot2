//! TRK ForceShot 2 — skärmklipp och bildredigering för provningsdokumentation.
//! Samma kod för Windows och macOS; det som skiljer ligger i `platform`.

// Inget konsolfönster i den färdiga Windows-utgåvan.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod capture;
mod editor;
mod icons;
mod imaging;
mod install;
mod map;
mod pdf;
mod platform;
mod settings;
mod sketch;
mod theme;
mod update;

use std::path::PathBuf;

fn uninstall() {
    use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};
    let answer = MessageDialog::new()
        .set_title("TRK ForceShot 2")
        .set_description("Vill du avinstallera TRK ForceShot 2?\n\nInställningarna ligger kvar om du installerar igen.")
        .set_buttons(MessageButtons::YesNo)
        .show();
    if answer != MessageDialogResult::Yes {
        return;
    }
    let (level, text) = match install::uninstall() {
        Ok(()) => (MessageLevel::Info, "TRK ForceShot 2 är avinstallerat.".to_string()),
        Err(e) => (MessageLevel::Error, format!("Avinstallationen misslyckades: {e}")),
    };
    MessageDialog::new().set_title("TRK ForceShot 2").set_level(level).set_description(text).show();
}

fn main() {
    let mut startup = app::Startup { files: Vec::new(), sketch: None, tray_only: false, demo: None, capture: false, ocr: false, setup: None, dev: None };
    let (mut force_setup, mut auto, mut portable, mut dev) = (false, false, false, false);
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            // Startar tyst i aktivitetsfältet, t.ex. vid inloggning.
            "--tray" => startup.tray_only = true,
            "--skiss" => startup.sketch = args.next(),
            // Installation: --setup visar installationsfönstret oavsett filnamn, --auto
            // installerar utan att fråga (används av uppdateringen), --portable kör
            // programmet som det är även om filen heter Setup.
            "--setup" => force_setup = true,
            "--auto" => auto = true,
            "--portable" => portable = true,
            "--uninstall" => return uninstall(),
            // Utan frågor och rutor, för skript.
            "--uninstall-tyst" => {
                if let Err(e) = install::uninstall() {
                    eprintln!("{e}");
                }
                return;
            }
            // Utvecklingsläge: fråga GitHub efter senaste utgåvan och skriv ut svaret.
            "--uppdateringskoll" => {
                let repo = settings::Settings::load().update_repo;
                println!("{repo}: {:?}", update::check(&repo));
                return;
            }
            // Utvecklingsläge: hämta senaste utgåvans Setup-fil oavsett version och
            // kontrollera den mot kontrollsumman, utan att köra den.
            // Utvecklingsläge: hämta en karta över en adress och spara den som PNG.
            "--karttest" => {
                let (query, out) = (args.next().unwrap_or_default(), args.next().unwrap_or_default());
                match map::fetch(&query, 16) {
                    Ok(m) => println!("{} · {}x{} · sparad: {:?}", m.place, m.image.width(), m.image.height(), m.image.save(&out).is_ok()),
                    Err(e) => println!("FEL: {e}"),
                }
                return;
            }
            "--hamtningstest" => {
                let repo = settings::Settings::load().update_repo;
                match update::check_against(&repo, "0.0.0") {
                    Ok(Some(r)) => {
                        let progress = std::sync::atomic::AtomicU64::new(0);
                        let result = update::download(&r, &progress);
                        println!("version {} · sha256 i utgåvan: {} · hämtat {} byte · {:?}", r.version, r.sha256.is_some(), progress.into_inner(), result);
                    }
                    other => println!("{other:?}"),
                }
                return;
            }
            // Utvecklingslägen: öppna överlägget direkt, eller visa en provbild.
            "--klipp" => (startup.capture, dev) = (true, true),
            "--ocr" => (startup.ocr, dev) = (true, true),
            "--demo" => (startup.demo, dev) = (Some(None), true),
            "--fiberdemo" => (startup.dev, dev) = (Some(("fiber".into(), args.next().map(PathBuf::from))), true),
            "--ytademo" => (startup.dev, dev) = (Some(("yta".into(), args.next().map(PathBuf::from))), true),
            "--demo-ut" => (startup.demo, dev) = (Some(args.next().map(PathBuf::from)), true),
            other if !other.starts_with("--") => {
                // Fullständig sökväg, så att en redan körande instans hittar filen.
                let p = PathBuf::from(other);
                startup.files.push(std::path::absolute(&p).unwrap_or(p));
            }
            _ => {}
        }
    }
    if !portable && install::wants_setup(force_setup) {
        startup.setup = Some(auto);
        return app::run(startup);
    }
    // Bara en instans åt gången: kör programmet redan får den instansen ta hand om filerna.
    let _guard = if dev {
        None
    } else {
        let mut forward: Vec<String> = startup.files.iter().map(|p| p.to_string_lossy().to_string()).collect();
        if let Some(s) = &startup.sketch {
            forward.push(format!("--skiss={s}"));
        }
        if startup.tray_only {
            // En tyst start ska inte väcka fönstret hos den som redan kör.
            forward.push("--tyst".into());
        }
        match install::single_instance(&forward) {
            Some(g) => Some(g),
            None => return,
        }
    };
    app::run(startup);
}
