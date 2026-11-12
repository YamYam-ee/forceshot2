//! Öppnar PDF-filer genom att rita varje sida till en bild. Motorn (hayro) är ren Rust
//! och fungerar likadant på Windows och Mac. Sidorna ritas i en egen tråd och skickas
//! en i taget, så att fönstret inte fryser medan ett långt dokument läses in.

use image::RgbaImage;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

/// Upplösningen sidorna ritas i. 150 punkter per tum ger läsbar text utan jättebilder.
const DPI: f32 = 150.0;
/// Fler sidor än så här öppnas inte på en gång; varje sida blir en egen flik.
pub const MAX_PAGES: usize = 40;

pub enum PdfMsg {
    Page { index: usize, total: usize, image: RgbaImage },
    Error(String),
}

/// Ritar en sida (räknat från noll) ur ett PDF-dokument i minnet.
pub fn render_page(pdf: &hayro::hayro_syntax::Pdf, index: usize) -> Option<RgbaImage> {
    let scale = DPI / 72.0;
    let pages = hayro::render_pdf(pdf, scale, Default::default(), &hayro::RenderSettings::default(), Some(index..=index))?;
    let pixmap = pages.into_iter().next()?;
    let (w, h) = (pixmap.width() as u32, pixmap.height() as u32);
    let mut img = RgbaImage::from_raw(w, h, pixmap.data_as_u8_slice().to_vec())?;
    // Bakgrunden är vit och täckande; alfakanalen fylls för säkerhets skull.
    for p in img.pixels_mut() {
        p[3] = 255;
    }
    Some(img)
}

pub fn page_count(pdf: &hayro::hayro_syntax::Pdf) -> usize {
    pdf.pages().len()
}

pub fn parse(data: Vec<u8>) -> Result<hayro::hayro_syntax::Pdf, String> {
    hayro::hayro_syntax::Pdf::new(data).map_err(|e| match e {
        hayro::hayro_syntax::LoadPdfError::Decryption(_) => "PDF-filen är lösenordsskyddad och kan inte öppnas.".to_string(),
        _ => "Filen gick inte att läsa som PDF.".to_string(),
    })
}

/// Läser filen i en bakgrundstråd. `wake` anropas varje gång en sida är klar.
pub fn load(path: PathBuf, wake: impl Fn() + Send + 'static) -> Receiver<PdfMsg> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let result = (|| -> Result<(), String> {
            let data = std::fs::read(&path).map_err(|e| format!("Kunde inte läsa filen: {e}"))?;
            let pdf = parse(data)?;
            let total = page_count(&pdf);
            if total == 0 {
                return Err("PDF-filen har inga sidor.".into());
            }
            for index in 0..total.min(MAX_PAGES) {
                match render_page(&pdf, index) {
                    Some(image) => {
                        if tx.send(PdfMsg::Page { index, total, image }).is_err() {
                            return Ok(());
                        }
                        wake();
                    }
                    None => return Err(format!("Sida {} gick inte att rita.", index + 1)),
                }
            }
            Ok(())
        })();
        if let Err(e) = result {
            let _ = tx.send(PdfMsg::Error(e));
            wake();
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minsta möjliga PDF: en sida på 200 × 100 punkter med en svart ruta.
    fn tiny_pdf() -> Vec<u8> {
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] /Contents 4 0 R >>".to_string(),
            {
                let stream = "0 0 0 rg 20 20 60 40 re f";
                format!("<< /Length {} >>\nstream\n{}\nendstream", stream.len(), stream)
            },
        ];
        let mut out = String::from("%PDF-1.4\n");
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.push_str(&format!("{} 0 obj\n{}\nendobj\n", i + 1, body));
        }
        let xref = out.len();
        out.push_str(&format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1));
        for o in offsets {
            out.push_str(&format!("{:010} 00000 n \n", o));
        }
        out.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objects.len() + 1, xref));
        out.into_bytes()
    }

    #[test]
    fn renders_a_page_at_150_dpi() {
        let pdf = parse(tiny_pdf()).expect("ska gå att läsa");
        assert_eq!(page_count(&pdf), 1);
        let img = render_page(&pdf, 0).expect("ska gå att rita");
        // 200 × 100 punkter vid 150 dpi.
        assert_eq!(img.dimensions(), (416, 208));
        // Rutan ligger 20–80 punkter från vänster och 20–60 från underkanten.
        assert_eq!(img.get_pixel(100, 120).0, [0, 0, 0, 255]);
        assert_eq!(img.get_pixel(300, 50).0, [255, 255, 255, 255]);
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse(b"inte en pdf".to_vec()).is_err());
    }
}
