// Bakar in ikonen och versionsuppgifterna i EXE-filen på Windows.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=assets/sketches");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "TRK ForceShot");
        res.set("FileDescription", "TRK ForceShot");
        res.set("CompanyName", "TRK Kontrolladministration AB");
        if let Err(e) = res.compile() {
            println!("cargo:warning=Kunde inte baka in ikonen: {e}");
        }
    }
}
