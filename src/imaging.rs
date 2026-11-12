//! Bildoperationer på processorn: utsnitt, oskärpa, mosaik, överstrykning, vridning.

use image::{Rgba, RgbaImage};

/// Klipper ut ett område, klämt till bildens kanter. Ger minst 1×1.
pub fn crop(img: &RgbaImage, x: i32, y: i32, w: i32, h: i32) -> RgbaImage {
    let x0 = x.clamp(0, img.width() as i32 - 1);
    let y0 = y.clamp(0, img.height() as i32 - 1);
    let x1 = (x + w).clamp(x0 + 1, img.width() as i32);
    let y1 = (y + h).clamp(y0 + 1, img.height() as i32);
    image::imageops::crop_imm(img, x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32).to_image()
}

fn box_blur_pass(src: &[[f32; 3]], dst: &mut [[f32; 3]], w: usize, h: usize, r: usize, horizontal: bool) {
    let (outer, inner) = if horizontal { (h, w) } else { (w, h) };
    let idx = |o: usize, i: usize| if horizontal { o * w + i } else { i * w + o };
    let norm = 1.0 / (2 * r + 1) as f32;
    for o in 0..outer {
        let mut acc = [0.0f32; 3];
        let at = |i: isize| src[idx(o, i.clamp(0, inner as isize - 1) as usize)];
        for k in -(r as isize)..=(r as isize) {
            let p = at(k);
            for c in 0..3 {
                acc[c] += p[c];
            }
        }
        for i in 0..inner {
            dst[idx(o, i)] = [acc[0] * norm, acc[1] * norm, acc[2] * norm];
            let add = at(i as isize + r as isize + 1);
            let sub = at(i as isize - r as isize);
            for c in 0..3 {
                acc[c] += add[c] - sub[c];
            }
        }
    }
}

/// Oskärpa: tre varv lådfilter ligger mycket nära en gaussisk kärna.
pub fn blur(img: &RgbaImage, radius: f32) -> RgbaImage {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let r = (radius.max(1.0)) as usize;
    let mut a: Vec<[f32; 3]> = img.pixels().map(|p| [p[0] as f32, p[1] as f32, p[2] as f32]).collect();
    let mut b = a.clone();
    for _ in 0..3 {
        box_blur_pass(&a, &mut b, w, h, r, true);
        box_blur_pass(&b, &mut a, w, h, r, false);
    }
    let mut out = RgbaImage::new(w as u32, h as u32);
    for (p, v) in out.pixels_mut().zip(a.iter()) {
        *p = Rgba([v[0].round() as u8, v[1].round() as u8, v[2].round() as u8, 255]);
    }
    out
}

/// Mosaik: varje ruta får medelfärgen av sina bildpunkter.
pub fn mosaic(img: &RgbaImage, cell: u32) -> RgbaImage {
    let cell = cell.max(2);
    let mut out = img.clone();
    let (w, h) = img.dimensions();
    let mut y = 0;
    while y < h {
        let mut x = 0;
        while x < w {
            let (x1, y1) = ((x + cell).min(w), (y + cell).min(h));
            let mut sum = [0u32; 3];
            for yy in y..y1 {
                for xx in x..x1 {
                    let p = img.get_pixel(xx, yy);
                    sum[0] += p[0] as u32;
                    sum[1] += p[1] as u32;
                    sum[2] += p[2] as u32;
                }
            }
            let n = ((x1 - x) * (y1 - y)).max(1);
            let c = Rgba([(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8, 255]);
            for yy in y..y1 {
                for xx in x..x1 {
                    out.put_pixel(xx, yy, c);
                }
            }
            x += cell;
        }
        y += cell;
    }
    out
}

/// Överstrykning som multiplikation: ljusa ytor tar färgen, svart text förblir svart.
pub fn multiply(img: &RgbaImage, color: [u8; 3]) -> RgbaImage {
    let mut out = img.clone();
    for p in out.pixels_mut() {
        for c in 0..3 {
            p[c] = ((p[c] as u16 * color[c] as u16) / 255) as u8;
        }
        p[3] = 255;
    }
    out
}

/// Fyller en ruta med omgivningens innehåll ("radera element").
///
/// Kanten runt rutan är randvillkor. Först läggs en mjuk gissning ut genom att väga
/// ihop de fyra kanterna, sedan löses Laplace-ekvationen med Gauss-Seidel så att ytan
/// följer toningar i underlaget i stället för att bli en platt färg.
pub fn inpaint(img: &RgbaImage, x: i32, y: i32, w: i32, h: i32) -> RgbaImage {
    let (w, h) = (w.max(1) as usize, h.max(1) as usize);
    let (gw, gh) = (w + 2, h + 2);
    let (iw, ih) = (img.width() as i32, img.height() as i32);
    // Rutnät med en bildpunkts ram. Ligger rutan mot bildkanten lånas närmaste punkt inuti bilden.
    let sample = |gx: usize, gy: usize| -> [f32; 3] {
        let px = (x - 1 + gx as i32).clamp(0, iw - 1) as u32;
        let py = (y - 1 + gy as i32).clamp(0, ih - 1) as u32;
        let p = img.get_pixel(px, py);
        [p[0] as f32, p[1] as f32, p[2] as f32]
    };
    let mut g = vec![[0.0f32; 3]; gw * gh];
    for gx in 0..gw {
        g[gx] = sample(gx, 0);
        g[(gh - 1) * gw + gx] = sample(gx, gh - 1);
    }
    for gy in 0..gh {
        g[gy * gw] = sample(0, gy);
        g[gy * gw + gw - 1] = sample(gw - 1, gy);
    }
    // En ram som delvis ligger utanför bilden bär ingen egen information: kopiera motstående sida.
    if y <= 0 {
        for gx in 0..gw { g[gx] = g[(gh - 1) * gw + gx] }
    } else if y + h as i32 >= ih {
        for gx in 0..gw { g[(gh - 1) * gw + gx] = g[gx] }
    }
    if x <= 0 {
        for gy in 0..gh { g[gy * gw] = g[gy * gw + gw - 1] }
    } else if x + w as i32 >= iw {
        for gy in 0..gh { g[gy * gw + gw - 1] = g[gy * gw] }
    }
    // Startgissning: Coons-yta mellan de fyra kanterna.
    let corner = [g[0], g[gw - 1], g[(gh - 1) * gw], g[(gh - 1) * gw + gw - 1]];
    for gy in 1..gh - 1 {
        let v = gy as f32 / (gh - 1) as f32;
        for gx in 1..gw - 1 {
            let u = gx as f32 / (gw - 1) as f32;
            let (l, r, t, b) = (g[gy * gw], g[gy * gw + gw - 1], g[gx], g[(gh - 1) * gw + gx]);
            let mut c = [0.0f32; 3];
            for k in 0..3 {
                let bil = (1.0 - u) * (1.0 - v) * corner[0][k] + u * (1.0 - v) * corner[1][k] + (1.0 - u) * v * corner[2][k] + u * v * corner[3][k];
                c[k] = (1.0 - u) * l[k] + u * r[k] + (1.0 - v) * t[k] + v * b[k] - bil;
            }
            g[gy * gw + gx] = c;
        }
    }
    // Gauss-Seidel med överrelaxation. Antalet varv hålls nere för stora ytor.
    let iters = (30_000_000 / (w * h).max(1)).clamp(24, 400);
    let omega = 1.85f32;
    for _ in 0..iters {
        for gy in 1..gh - 1 {
            for gx in 1..gw - 1 {
                let i = gy * gw + gx;
                for k in 0..3 {
                    let avg = (g[i - 1][k] + g[i + 1][k] + g[i - gw][k] + g[i + gw][k]) * 0.25;
                    g[i][k] += omega * (avg - g[i][k]);
                }
            }
        }
    }
    let mut out = RgbaImage::new(w as u32, h as u32);
    for (px, py, p) in out.enumerate_pixels_mut() {
        let c = g[(py as usize + 1) * gw + px as usize + 1];
        *p = Rgba([c[0].round().clamp(0.0, 255.0) as u8, c[1].round().clamp(0.0, 255.0) as u8, c[2].round().clamp(0.0, 255.0) as u8, 255]);
    }
    out
}

/// Bildtyp vid förstoring: styr vad som görs utöver själva omsamplingen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Enhance {
    Skarmklipp,
    Dokument,
    Foto,
}

/// Flyttar svart- och vitpunkten till bildens verkliga percentiler, så att grått papper
/// blir vitt och blek text svart.
pub fn auto_levels(img: &mut RgbaImage, low_pct: f32, high_pct: f32) {
    let mut hist = [0u32; 256];
    for p in img.pixels() {
        let l = (p[0] as u32 * 3 + p[1] as u32 * 6 + p[2] as u32) / 10;
        hist[l as usize] += 1;
    }
    let total: u32 = hist.iter().sum();
    let find = |pct: f32| -> u8 {
        let target = (total as f32 * pct) as u32;
        let mut acc = 0;
        for (i, n) in hist.iter().enumerate() {
            acc += n;
            if acc > target {
                return i as u8;
            }
        }
        255
    };
    let (lo, hi) = (find(low_pct) as f32, find(high_pct) as f32);
    if hi - lo < 8.0 {
        return;
    }
    let lut: Vec<u8> = (0..256).map(|v| (((v as f32 - lo) / (hi - lo)) * 255.0).round().clamp(0.0, 255.0) as u8).collect();
    for p in img.pixels_mut() {
        for c in 0..3 {
            p[c] = lut[p[c] as usize];
        }
    }
}

/// Förstorar bilden `factor` gånger med Lanczos-filter och förbättrar den efter bildtyp.
pub fn enlarge(img: &RgbaImage, factor: u32, mode: Enhance) -> RgbaImage {
    use image::imageops::{FilterType, resize, unsharpen};
    let (w, h) = (img.width() * factor, img.height() * factor);
    match mode {
        Enhance::Skarmklipp => resize(img, w, h, FilterType::Lanczos3),
        Enhance::Dokument => {
            // Tonerna rättas före skalningen; efteråt har kornet blivit lika stort som detaljerna.
            let mut src = img.clone();
            auto_levels(&mut src, 0.02, 0.70);
            unsharpen(&resize(&src, w, h, FilterType::Lanczos3), 1.2 * factor as f32, 3)
        }
        Enhance::Foto => {
            let soft = image::imageops::blur(img, 0.6);
            let mut big = resize(&soft, w, h, FilterType::Lanczos3);
            auto_levels(&mut big, 0.005, 0.995);
            unsharpen(&big, 0.8 * factor as f32, 4)
        }
    }
}

/// En ensidig PDF med bilden som JPEG.
pub fn pdf_page(jpeg: &[u8], width: u32, height: u32) -> Vec<u8> {
    pdf_pages(&[(jpeg.to_vec(), width, height)])
}

/// En PDF med en JPEG-bild per sida. Varje sida får måtten bilden har vid 150 punkter
/// per tum, så att den blir lika stor igen när filen öppnas här.
pub fn pdf_pages(pages: &[(Vec<u8>, u32, u32)]) -> Vec<u8> {
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    let mut object = |out: &mut Vec<u8>, head: String, stream: Option<&[u8]>| {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{head}\n", offsets.len()).as_bytes());
        if let Some(s) = stream {
            out.extend_from_slice(b"stream\n");
            out.extend_from_slice(s);
            out.extend_from_slice(b"\nendstream\n");
        }
        out.extend_from_slice(b"endobj\n");
    };
    // Objekt 1 och 2 är katalogen och sidlistan; sedan tre objekt per sida.
    let kids: Vec<String> = (0..pages.len()).map(|i| format!("{} 0 R", 3 + 3 * i)).collect();
    object(&mut out, "<< /Type /Catalog /Pages 2 0 R >>".into(), None);
    object(&mut out, format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), pages.len()), None);
    for (i, (jpeg, width, height)) in pages.iter().enumerate() {
        let n = 3 + 3 * i;
        // Ett uns extra, så att sidan inte tappar en bildpunkt när måttet räknas tillbaka och kapas.
        let (pw, ph) = (*width as f64 * 0.48 + 0.0005, *height as f64 * 0.48 + 0.0005);
        let content = format!("q {pw:.4} 0 0 {ph:.4} 0 0 cm /Im0 Do Q");
        object(&mut out, format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {pw:.4} {ph:.4}] /Resources << /XObject << /Im0 {} 0 R >> >> /Contents {} 0 R >>", n + 2, n + 1), None);
        object(&mut out, format!("<< /Length {} >>", content.len()), Some(content.as_bytes()));
        object(
            &mut out,
            format!("<< /Type /XObject /Subtype /Image /Width {width} /Height {height} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode /Length {} >>", jpeg.len()),
            Some(jpeg),
        );
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1).as_bytes());
    for o in &offsets {
        out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", offsets.len() + 1).as_bytes());
    out
}

/// BGRA (Windows GDI) till RGBA med fylld alfakanal.
pub fn bgra_to_rgba(buf: &mut [u8]) {
    for px in buf.chunks_exact_mut(4) {
        px.swap(0, 2);
        px[3] = 255;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_clamps_to_image() {
        let img = RgbaImage::from_pixel(10, 10, Rgba([1, 2, 3, 255]));
        let c = crop(&img, -5, 8, 100, 100);
        assert_eq!(c.dimensions(), (10, 2));
    }

    #[test]
    fn blur_keeps_flat_colour() {
        let img = RgbaImage::from_pixel(16, 16, Rgba([40, 80, 120, 255]));
        let b = blur(&img, 4.0);
        assert_eq!(b.get_pixel(8, 8), &Rgba([40, 80, 120, 255]));
        assert_eq!(b.get_pixel(0, 0), &Rgba([40, 80, 120, 255]));
    }

    #[test]
    fn multiply_keeps_black_and_tints_white() {
        let mut img = RgbaImage::from_pixel(2, 1, Rgba([255, 255, 255, 255]));
        img.put_pixel(1, 0, Rgba([0, 0, 0, 255]));
        let m = multiply(&img, [255, 235, 59]);
        assert_eq!(m.get_pixel(0, 0), &Rgba([255, 235, 59, 255]));
        assert_eq!(m.get_pixel(1, 0), &Rgba([0, 0, 0, 255]));
    }

    #[test]
    fn inpaint_removes_mark_on_flat_background() {
        let mut img = RgbaImage::from_pixel(40, 40, Rgba([200, 210, 220, 255]));
        for y in 15..25 {
            for x in 15..25 {
                img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
            }
        }
        let patch = inpaint(&img, 12, 12, 16, 16);
        assert_eq!(patch.dimensions(), (16, 16));
        for p in patch.pixels() {
            assert_eq!(p, &Rgba([200, 210, 220, 255]));
        }
    }

    #[test]
    fn inpaint_follows_a_gradient() {
        let mut img = RgbaImage::new(60, 30);
        for (x, _, p) in img.enumerate_pixels_mut() {
            let v = (x * 4) as u8;
            *p = Rgba([v, v, v, 255]);
        }
        let mut marked = img.clone();
        for y in 8..22 {
            for x in 20..40 {
                marked.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        let patch = inpaint(&marked, 20, 8, 20, 14);
        for (x, y, p) in patch.enumerate_pixels() {
            let want = img.get_pixel(x + 20, y + 8)[0] as i32;
            assert!((p[0] as i32 - want).abs() <= 1, "x={x} y={y}: {} mot {want}", p[0]);
        }
    }

    #[test]
    fn inpaint_at_image_edge_does_not_panic() {
        let img = RgbaImage::from_pixel(10, 10, Rgba([9, 9, 9, 255]));
        let patch = inpaint(&img, 0, 0, 10, 10);
        assert_eq!(patch.get_pixel(5, 5), &Rgba([9, 9, 9, 255]));
    }

    #[test]
    fn auto_levels_stretches_grey_paper_to_white() {
        let mut img = RgbaImage::from_pixel(20, 20, Rgba([180, 180, 180, 255]));
        for x in 0..20 {
            img.put_pixel(x, 5, Rgba([70, 70, 70, 255]));
        }
        auto_levels(&mut img, 0.01, 0.90);
        assert_eq!(img.get_pixel(0, 0)[0], 255);
        assert_eq!(img.get_pixel(0, 5)[0], 0);
    }

    #[test]
    fn enlarge_multiplies_the_size() {
        let img = RgbaImage::from_pixel(30, 20, Rgba([200, 100, 50, 255]));
        for mode in [Enhance::Skarmklipp, Enhance::Dokument, Enhance::Foto] {
            assert_eq!(enlarge(&img, 3, mode).dimensions(), (90, 60));
        }
        // En enfärgad skärmbild ska förbli enfärgad.
        assert_eq!(enlarge(&img, 2, Enhance::Skarmklipp).get_pixel(30, 20), &Rgba([200, 100, 50, 255]));
    }

    #[test]
    fn pdf_page_opens_again_at_the_same_size() {
        let img = image::RgbImage::from_pixel(300, 200, image::Rgb([10, 120, 220]));
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90).encode_image(&img).unwrap();
        let pdf = crate::pdf::parse(pdf_page(&jpeg, 300, 200)).expect("ska gå att läsa");
        assert_eq!(crate::pdf::page_count(&pdf), 1);
        let page = crate::pdf::render_page(&pdf, 0).expect("ska gå att rita");
        assert_eq!(page.dimensions(), (300, 200));
        let p = page.get_pixel(150, 100);
        assert!((p[0] as i32 - 10).abs() < 12 && (p[1] as i32 - 120).abs() < 12 && (p[2] as i32 - 220).abs() < 12, "{p:?}");
    }

    #[test]
    fn several_pages_keep_their_order_and_sizes() {
        let page = |w: u32, h: u32, c: [u8; 3]| {
            let img = image::RgbImage::from_pixel(w, h, image::Rgb(c));
            let mut jpeg = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 90).encode_image(&img).unwrap();
            (jpeg, w, h)
        };
        let bytes = pdf_pages(&[page(300, 200, [220, 30, 30]), page(150, 400, [30, 200, 30]), page(640, 480, [30, 30, 220])]);
        let pdf = crate::pdf::parse(bytes).expect("ska gå att läsa");
        assert_eq!(crate::pdf::page_count(&pdf), 3);
        let sizes: Vec<_> = (0..3).map(|i| crate::pdf::render_page(&pdf, i).expect("ska gå att rita")).collect();
        assert_eq!(sizes[0].dimensions(), (300, 200));
        assert_eq!(sizes[1].dimensions(), (150, 400));
        assert_eq!(sizes[2].dimensions(), (640, 480));
        assert!(sizes[0].get_pixel(10, 10)[0] > 180 && sizes[1].get_pixel(10, 10)[1] > 160 && sizes[2].get_pixel(10, 10)[2] > 180);
    }

    #[test]
    fn mosaic_averages_cells() {
        let mut img = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 0, Rgba([200, 200, 200, 255]));
        let m = mosaic(&img, 2);
        assert_eq!(m.get_pixel(1, 1), &Rgba([50, 50, 50, 255]));
    }
}
