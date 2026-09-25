//! Offscreen pictures of the whole Anvil window, without a display.
//!
//! Only built with the `devtools` feature, so the shipped application
//! carries none of this code:
//!
//! ```text
//! cargo run -p anvil-ui --features devtools --example shot -- --help
//! ```
//!
//! How it works: egui turns a frame into triangles with texture
//! coordinates (`Context::tessellate`). This module draws those triangles
//! into a plain pixel buffer, the same idea as `raster.rs` but in 2D and
//! with alpha blending. The 3D view arrives as one textured rectangle,
//! because the software viewport already renders it into an egui texture,
//! so a picture taken here shows the real model, the real ribbon and the
//! real panels.
//!
//! Every pixel starts as [`UNPAINTED`]. Any pixel still that colour at the
//! end is a hole in the layout: an area no panel drew. Tests use this to
//! catch layout faults such as a gap beside a side panel.

use crate::settings::UiSettings;
use crate::AnvilApp;
use egui::{Color32, ColorImage, Pos2, Rect, TextureId};
use std::collections::HashMap;

/// Start colour of the canvas. A vivid magenta that no theme uses, so a
/// leftover pixel means "nothing drew here".
pub const UNPAINTED: Color32 = Color32::from_rgb(255, 0, 255);

/// One picture request: window size in points, and the settings to use.
pub struct ShotOptions {
    pub width: u32,
    pub height: u32,
    pub settings: UiSettings,
    /// Frames to run before the picture is taken. egui needs a few to
    /// settle panel sizes and a zoom factor change.
    pub frames: usize,
    /// Feature to select in the Part Navigator before the picture.
    pub select: Option<usize>,
    /// Open the Settings window before the picture.
    pub open_settings: bool,
    /// Ribbon tab to show, by name. `None` keeps the Solid tab.
    pub tab: Option<String>,
    /// Keep only this part of the picture: (x, y, width, height) in
    /// pixels. Useful for a document that shows one panel or one ribbon
    /// group rather than the whole window.
    pub crop: Option<(u32, u32, u32, u32)>,
}

impl Default for ShotOptions {
    fn default() -> Self {
        ShotOptions {
            width: 1500,
            height: 950,
            settings: UiSettings::default(),
            frames: 4,
            select: None,
            open_settings: false,
            tab: None,
            crop: None,
        }
    }
}

/// Runs `app` headless and returns a picture of the whole window.
pub fn window_image(app: &mut AnvilApp, opts: &ShotOptions) -> ColorImage {
    let ctx = egui::Context::default();
    ctx.set_theme(opts.settings.theme);
    let mut canvas = Canvas::new(opts.width, opts.height);
    // Time moves on between frames, otherwise egui's fade in never
    // finishes and a window that just opened stays half transparent.
    let input = |frame: usize| egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(opts.width as f32, opts.height as f32))),
        time: Some(frame as f64 * 0.25),
        predicted_dt: 0.25,
        ..Default::default()
    };
    let mut last: Option<egui::FullOutput> = None;
    for i in 0..opts.frames.max(1) {
        // The settings are applied from the second frame on: egui needs a
        // known previous frame size before a zoom factor change.
        let out = ctx.run(input(i), |ctx| {
            app.settings = opts.settings;
            // The text size and the theme apply from the first frame, so
            // a panel gets its starting width for that text size. The
            // zoom factor waits for the second frame: egui needs a
            // previous frame's size before the zoom may change.
            if i == 0 {
                UiSettings { scale: 1.0, ..opts.settings }.apply(ctx);
            } else {
                opts.settings.apply(ctx);
            }
            app.frame_ui(ctx);
        });
        canvas.apply_textures(&out.textures_delta);
        last = Some(out);
    }
    let out = last.expect("at least one frame runs");
    let ppp = ctx.pixels_per_point();
    for prim in ctx.tessellate(out.shapes, ppp) {
        canvas.draw(&prim, ppp);
    }
    let img = canvas.into_image();
    match opts.crop {
        Some(c) => crop(&img, c),
        None => img,
    }
}

/// Keep only (x, y, width, height) of a picture, clamped to what is
/// there.
pub fn crop(img: &ColorImage, (x, y, w, h): (u32, u32, u32, u32)) -> ColorImage {
    let [iw, ih] = img.size;
    let (x, y) = ((x as usize).min(iw), (y as usize).min(ih));
    let (w, h) = ((w as usize).min(iw - x), (h as usize).min(ih - y));
    let mut px = Vec::with_capacity(w * h);
    for row in 0..h {
        px.extend_from_slice(&img.pixels[(y + row) * iw + x..(y + row) * iw + x + w]);
    }
    ColorImage { size: [w, h], pixels: px, source_size: egui::Vec2::ZERO }
}

/// A picture of the window with a sample loaded, by the sample's name.
/// Returns `None` when the name is not one of [`SAMPLES`].
pub fn sample_image(name: &str, opts: &ShotOptions) -> Option<ColorImage> {
    let action = sample_action(name)?;
    let mut app = AnvilApp::new_headless();
    if let Some(a) = action {
        app.run_action(a);
        app.fit_view();
    }
    app.panels.selected = opts.select;
    if opts.open_settings {
        app.run_action(crate::ribbon::RibbonAction::ToggleSettings);
    }
    if let Some(t) = &opts.tab {
        app.show_tab(t);
    }
    Some(window_image(&mut app, opts))
}

/// Names accepted by [`sample_image`], with what each one loads.
pub const SAMPLES: &[(&str, &str)] = &[
    ("empty", "a new, empty document"),
    ("demo", "the demo part (plate and ring)"),
    ("wb1", "workbook 1: plate"),
    ("wb2", "workbook 2: bracket"),
    ("wb3", "workbook 3: shaft"),
    ("wb4", "workbook 4: nut"),
    ("wb5", "workbook 5: elbow"),
    ("wb6", "workbook 6: adapter"),
    ("card", "the business card"),
    ("kettle", "the kettle casting sample"),
];

fn sample_action(name: &str) -> Option<Option<crate::ribbon::RibbonAction>> {
    use crate::ribbon::RibbonAction as A;
    Some(match name {
        "empty" => None,
        "demo" => Some(A::DemoPart),
        "wb1" => Some(A::Workbook(0)),
        "wb2" => Some(A::Workbook(1)),
        "wb3" => Some(A::Workbook(2)),
        "wb4" => Some(A::Workbook(3)),
        "wb5" => Some(A::Workbook(4)),
        "wb6" => Some(A::Workbook(5)),
        "card" => Some(A::SampleCard),
        "kettle" => Some(A::Kettle),
        _ => return None,
    })
}

// ---------------------------------------------------------------------
// The 2D rasterizer
// ---------------------------------------------------------------------

struct Tex {
    size: [usize; 2],
    px: Vec<Color32>,
}

struct Canvas {
    width: usize,
    height: usize,
    px: Vec<Color32>,
    textures: HashMap<TextureId, Tex>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1) as usize, height.max(1) as usize);
        Canvas { width, height, px: vec![UNPAINTED; width * height], textures: HashMap::new() }
    }

    fn apply_textures(&mut self, delta: &egui::TexturesDelta) {
        for (id, d) in &delta.set {
            let egui::epaint::ImageData::Color(img) = &d.image;
            match d.pos {
                None => {
                    self.textures.insert(*id, Tex { size: img.size, px: img.pixels.clone() });
                }
                Some([x0, y0]) => {
                    if let Some(t) = self.textures.get_mut(id) {
                        for y in 0..img.size[1] {
                            for x in 0..img.size[0] {
                                let (dx, dy) = (x0 + x, y0 + y);
                                if dx < t.size[0] && dy < t.size[1] {
                                    t.px[dy * t.size[0] + dx] = img.pixels[y * img.size[0] + x];
                                }
                            }
                        }
                    }
                }
            }
        }
        for id in &delta.free {
            self.textures.remove(id);
        }
    }

    fn draw(&mut self, prim: &egui::ClippedPrimitive, ppp: f32) {
        let egui::epaint::Primitive::Mesh(mesh) = &prim.primitive else {
            // A paint callback is the GPU viewport. It has no software
            // form, so a picture taken with the GPU path on shows a hole.
            return;
        };
        let clip = Rect::from_min_max(
            Pos2::new(prim.clip_rect.min.x * ppp, prim.clip_rect.min.y * ppp),
            Pos2::new(prim.clip_rect.max.x * ppp, prim.clip_rect.max.y * ppp),
        );
        let tex = self.textures.get(&mesh.texture_id);
        let Some(tex) = tex else { return };
        // Copy the small amount of texture data the triangles need. The
        // borrow checker will not let us read `self.textures` while
        // writing `self.px`, and a picture is not a hot path.
        let tex = Tex { size: tex.size, px: tex.px.clone() };
        for idx in mesh.indices.as_chunks::<3>().0 {
            let v: Vec<&egui::epaint::Vertex> = idx.iter().map(|&i| &mesh.vertices[i as usize]).collect();
            self.triangle(v[0], v[1], v[2], &tex, clip, ppp);
        }
    }

    fn triangle(
        &mut self,
        a: &egui::epaint::Vertex,
        b: &egui::epaint::Vertex,
        c: &egui::epaint::Vertex,
        tex: &Tex,
        clip: Rect,
        ppp: f32,
    ) {
        let p = |v: &egui::epaint::Vertex| Pos2::new(v.pos.x * ppp, v.pos.y * ppp);
        let (pa, pb, pc) = (p(a), p(b), p(c));
        let area = (pb.x - pa.x) * (pc.y - pa.y) - (pb.y - pa.y) * (pc.x - pa.x);
        if area.abs() < 1e-9 {
            return;
        }
        let min_x = pa.x.min(pb.x).min(pc.x).max(clip.min.x).max(0.0).floor() as i64;
        let max_x = (pa.x.max(pb.x).max(pc.x).min(clip.max.x).min(self.width as f32)).ceil() as i64;
        let min_y = pa.y.min(pb.y).min(pc.y).max(clip.min.y).max(0.0).floor() as i64;
        let max_y = (pa.y.max(pb.y).max(pc.y).min(clip.max.y).min(self.height as f32)).ceil() as i64;
        for y in min_y..max_y {
            for x in min_x..max_x {
                let px = Pos2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((pb.x - pa.x) * (px.y - pa.y) - (pb.y - pa.y) * (px.x - pa.x)) / area;
                let w1 = ((px.x - pa.x) * (pc.y - pa.y) - (px.y - pa.y) * (pc.x - pa.x)) / area;
                let (u, v, w) = (1.0 - w0 - w1, w1, w0);
                if u < 0.0 || v < 0.0 || w < 0.0 {
                    continue;
                }
                let uv = egui::vec2(a.uv.x * u + b.uv.x * v + c.uv.x * w, a.uv.y * u + b.uv.y * v + c.uv.y * w);
                let t = sample(tex, uv.x, uv.y);
                let vc = mix3(a.color, b.color, c.color, u, v, w);
                let src = modulate(vc, t);
                let i = y as usize * self.width + x as usize;
                self.px[i] = over(src, self.px[i]);
            }
        }
    }

    fn into_image(self) -> ColorImage {
        ColorImage { size: [self.width, self.height], pixels: self.px, source_size: egui::Vec2::ZERO }
    }
}

/// Bilinear sample of a premultiplied texture, with the edges clamped.
fn sample(tex: &Tex, u: f32, v: f32) -> Color32 {
    let (w, h) = (tex.size[0], tex.size[1]);
    if w == 0 || h == 0 {
        return Color32::WHITE;
    }
    let x = (u * w as f32 - 0.5).clamp(0.0, (w - 1) as f32);
    let y = (v * h as f32 - 0.5).clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let at = |cx: usize, cy: usize| tex.px[cy * w + cx];
    let top = lerp2(at(x0, y0), at(x1, y0), fx);
    let bot = lerp2(at(x0, y1), at(x1, y1), fx);
    lerp2(top, bot, fy)
}

fn lerp2(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |p: u8, q: u8| (p as f32 + (q as f32 - p as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()), f(a.a(), b.a()))
}

fn mix3(a: Color32, b: Color32, c: Color32, u: f32, v: f32, w: f32) -> Color32 {
    let f = |p: u8, q: u8, r: u8| (p as f32 * u + q as f32 * v + r as f32 * w).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_premultiplied(
        f(a.r(), b.r(), c.r()),
        f(a.g(), b.g(), c.g()),
        f(a.b(), b.b(), c.b()),
        f(a.a(), b.a(), c.a()),
    )
}

/// Vertex colour times texture colour, both premultiplied.
fn modulate(v: Color32, t: Color32) -> Color32 {
    let f = |p: u8, q: u8| ((p as u32 * q as u32) / 255) as u8;
    Color32::from_rgba_premultiplied(f(v.r(), t.r()), f(v.g(), t.g()), f(v.b(), t.b()), f(v.a(), t.a()))
}

/// Source over destination, premultiplied.
fn over(src: Color32, dst: Color32) -> Color32 {
    let k = 255 - src.a() as u32;
    let f = |s: u8, d: u8| (s as u32 + (d as u32 * k) / 255).min(255) as u8;
    Color32::from_rgba_premultiplied(f(src.r(), dst.r()), f(src.g(), dst.g()), f(src.b(), dst.b()), f(src.a(), dst.a()))
}

/// Writes a picture as a binary PPM (P6). `scripts/ppm2png.py` turns it
/// into a PNG for the documents.
pub fn write_ppm(path: &std::path::Path, img: &ColorImage) -> std::io::Result<()> {
    let [w, h] = img.size;
    let mut bytes = format!("P6 {w} {h} 255\n").into_bytes();
    for px in &img.pixels {
        bytes.extend_from_slice(&[px.r(), px.g(), px.b()]);
    }
    std::fs::write(path, bytes)
}

/// Every pixel that no widget painted, as (x, y). Empty is the healthy
/// answer for a window whose panels cover it.
pub fn unpainted_pixels(img: &ColorImage) -> Vec<(usize, usize)> {
    let [w, _] = img.size;
    img.pixels.iter().enumerate().filter(|(_, p)| **p == UNPAINTED).map(|(i, _)| (i % w, i / w)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window must cover every pixel: a hole means a layout fault,
    /// such as the gap that appears beside a side panel whose contents
    /// are wider than the panel.
    #[test]
    fn the_window_has_no_unpainted_holes() {
        for (name, w, h, text) in [
            ("demo", 1500, 950, 14.0),
            // The laptop case from the bug report: a large text size.
            ("demo", 1366, 768, 22.0),
            ("wb2", 1366, 768, 22.0),
        ] {
            let opts = ShotOptions {
                width: w,
                height: h,
                settings: UiSettings { base_text: text, ..Default::default() },
                ..Default::default()
            };
            let img = sample_image(name, &opts).expect("known sample");
            let holes = unpainted_pixels(&img);
            assert!(
                holes.is_empty(),
                "{name} at {w}x{h}, text {text}: {} unpainted pixels, first at {:?}",
                holes.len(),
                holes.first()
            );
        }
    }
}
