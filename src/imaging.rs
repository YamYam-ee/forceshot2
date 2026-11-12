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
    fn mosaic_averages_cells() {
        let mut img = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 0, 255]));
        img.put_pixel(0, 0, Rgba([200, 200, 200, 255]));
        let m = mosaic(&img, 2);
        assert_eq!(m.get_pixel(1, 1), &Rgba([50, 50, 50, 255]));
    }
}
