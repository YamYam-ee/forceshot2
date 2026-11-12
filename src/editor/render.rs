//! Export: ritar underlaget och alla annoteringar till en bild i full upplösning.
//! Samma formkod som på skärmen, men mot en textur utanför fönstret.

use super::canvas::Canvas;
use super::doc::Doc;
use super::draw::{self, Res, Xf};
use egui::epaint::{ClippedShape, RectShape};
use egui::{Color32, Rect, Shape, pos2, vec2};
use egui_wgpu::{RenderState, ScreenDescriptor};
use image::RgbaImage;

/// Bygger formerna. Anropas också en bildruta i förväg så att bokstäverna hinner
/// hamna i teckenatlasen innan bilden ritas.
pub fn shapes(ctx: &egui::Context, canvas: &mut Canvas, doc: &Doc, ppp: f32) -> Vec<ClippedShape> {
    canvas.sync_textures(ctx, doc);
    let xf = Xf { s: 1.0 / ppp, o: egui::Vec2::ZERO };
    let tex = canvas.textures(doc.uid);
    let res = Res { ctx, tex: &tex, editing: None, image_rect: doc.image_rect() };
    let full = xf.r(doc.image_rect());
    let mut out = Vec::new();
    if let Some(base) = canvas.base_texture(doc.uid) {
        out.push(Shape::Rect(RectShape::filled(full, 0.0, Color32::WHITE).with_texture(base, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)))));
    }
    for el in &doc.els {
        draw::element(el, &xf, &res, &mut out);
    }
    out.into_iter().map(|shape| ClippedShape::new(full, shape)).collect()
}

pub fn render(ctx: &egui::Context, rs: &RenderState, canvas: &mut Canvas, doc: &Doc) -> Result<RgbaImage, String> {
    let (w, h) = (doc.base.width(), doc.base.height());
    let limit = rs.device.limits().max_texture_dimension_2d;
    if w > limit || h > limit {
        return Err(format!("Bilden är större än grafikkortet klarar ({limit} px)."));
    }
    let ppp = ctx.pixels_per_point();
    let prims = ctx.tessellate(shapes(ctx, canvas, doc, ppp), ppp);
    let _ = vec2(0.0, 0.0);

    let texture = rs.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("export"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: rs.target_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let sd = ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: ppp };
    let mut encoder = rs.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("export") });
    let mut renderer = rs.renderer.write();
    let extra = renderer.update_buffers(&rs.device, &rs.queue, &mut encoder, &prims, &sd);
    {
        let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("export"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::WHITE), store: wgpu::StoreOp::Store },
            })],
            ..Default::default()
        });
        renderer.render(&mut pass.forget_lifetime(), &prims, &sd);
    }
    drop(renderer);

    // Raderna i en läsbuffert måste vara jämnt delbara med 256 byte.
    let row = (w * 4).div_ceil(256) * 256;
    let buffer = rs.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("export"),
        size: row as u64 * h as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buffer, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    rs.queue.submit(extra.into_iter().chain(std::iter::once(encoder.finish())));

    let slice = buffer.slice(..);
    let (tx, rx) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |r| {
        let _ = tx.send(r);
    });
    rs.device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    let data = slice.get_mapped_range().map_err(|e| e.to_string())?;
    let bgra = matches!(rs.target_format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
    let mut out = RgbaImage::new(w, h);
    for y in 0..h as usize {
        let src = &data[y * row as usize..y * row as usize + w as usize * 4];
        let dst = &mut out.as_mut()[y * w as usize * 4..(y + 1) * w as usize * 4];
        dst.copy_from_slice(src);
        for px in dst.chunks_exact_mut(4) {
            if bgra {
                px.swap(0, 2);
            }
            px[3] = 255;
        }
    }
    Ok(out)
}
