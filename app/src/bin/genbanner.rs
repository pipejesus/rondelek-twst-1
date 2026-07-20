//! Generates the GitHub banner into `docs/images/banner.png`.
//!
//! Run from the repo root: `cargo run --bin genbanner`
//!
//! Same procedural pixel-art engine as the skin (`pixelart`), so the banner
//! speaks the app's own visual language: flat matte plastic, a 5×7 pixel
//! wordmark, one orange accent, and a little dot-matrix screen with keycaps.

use std::path::Path;

#[path = "../pixelart.rs"]
mod pixelart;
use pixelart::{Canvas, cov, draw_text, hex, sd_rrect, shade, smoothstep};

const W: u32 = 1280;
const H: u32 = 400;

fn main() {
    let mut c = Canvas::new(W, H);

    // Matte background: the app's window gradient.
    let top = hex("#F3F0E8");
    let bottom = hex("#E7E3D8");
    for y in 0..H {
        let t = y as f32 / H as f32;
        let col = [
            top[0] + (bottom[0] - top[0]) * t,
            top[1] + (bottom[1] - top[1]) * t,
            top[2] + (bottom[2] - top[2]) * t,
        ];
        for x in 0..W {
            c.blend(x, y, col, 1.0);
        }
    }

    let ink = hex("#2A2622");
    let orange = hex("#FF6A1A");

    // Wordmark: RONDELEK in ink, TWST-1 in the accent orange beneath it.
    draw_text(&mut c, "RONDELEK", 96.0, 158.0, 90.0, 20.0, ink);
    draw_text(&mut c, "TWST-1", 100.0, 250.0, 46.0, 16.0, orange);

    // Right-hand motif: a dot-matrix screen over a little row of keycaps.
    draw_screen(&mut c, 792.0, 70.0, 396.0, 120.0);
    let face = hex("#F1ECE1");
    draw_cap(&mut c, 792.0, 224.0, 108.0, face, Some('Q'), false);
    draw_cap(&mut c, 924.0, 224.0, 108.0, face, Some('W'), false);
    draw_cap(&mut c, 1056.0, 224.0, 108.0, orange, None, true);

    let dir = Path::new("docs/images");
    std::fs::create_dir_all(dir).expect("create images dir");
    c.save(&dir.join("banner.png"));
    println!("done.");
}

/// A dark recessed screen at (x, y) with amber bars, echoing the visualizer.
fn draw_screen(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32) {
    let base = hex("#26231E");
    let (cx, cy) = (x + w * 0.5, y + h * 0.5);
    for py in (y as u32)..((y + h) as u32) {
        for px in (x as u32)..((x + w) as u32) {
            let sd = sd_rrect(px as f32 + 0.5, py as f32 + 0.5, cx, cy, w * 0.5, h * 0.5, 16.0);
            let a = cov(sd);
            if a > 0.0 {
                let inner = smoothstep(-24.0, -2.0, sd) * 0.10;
                c.blend(px, py, shade(base, -inner), a);
            }
        }
    }
    // Five amber bars, left-aligned, like a running spectrum.
    let widths = [0.90f32, 0.55, 0.30, 0.72, 0.44];
    let cols = ["#FFC06A", "#FF6A1A", "#B85A12", "#FF6A1A", "#FF8A3A"];
    let pad = 22.0;
    let bar_h = 10.0;
    let gap = (h - 2.0 * pad - widths.len() as f32 * bar_h) / (widths.len() as f32 - 1.0);
    for (i, (wf, col)) in widths.iter().zip(cols).enumerate() {
        let by = y + pad + i as f32 * (bar_h + gap);
        let bw = (w - 2.0 * pad) * wf;
        let rgb = hex(col);
        for py in (by as u32)..((by + bar_h) as u32) {
            for px in ((x + pad) as u32)..((x + pad + bw) as u32) {
                c.blend(px, py, rgb, 1.0);
            }
        }
    }
}

/// A flat matte keycap centred in a `size` box at (x, y), like the pads.
fn draw_cap(c: &mut Canvas, x: f32, y: f32, size: f32, face: [f32; 3], label: Option<char>, dot: bool) {
    let wall = shade(face, -0.16);
    let (cx, cy) = (x + size * 0.5, y + size * 0.5);
    let hw = size * 0.5;
    let rim_r = size * 0.20;
    // Face sits high, ~7% wall at the bottom.
    let face_hh = hw - size * 0.10;
    let face_cy = cy - size * 0.035;
    for py in (y as u32)..((y + size) as u32) {
        for px in (x as u32)..((x + size) as u32) {
            let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
            let a = cov(sd_rrect(fx, fy, cx, cy, hw, hw, rim_r));
            if a <= 0.0 {
                continue;
            }
            c.blend(px, py, wall, a);
            let fa = cov(sd_rrect(fx, fy, cx, face_cy, hw - size * 0.09, face_hh, rim_r * 0.85));
            if fa > 0.0 {
                let t = (fy - (face_cy - face_hh)) / (2.0 * face_hh);
                c.blend(px, py, shade(face, 0.05 - 0.10 * t), fa);
            }
        }
    }
    if let Some(ch) = label {
        pixelart::draw_label(c, ch, cx, face_cy, size * 0.42, hex("#2A2622"));
    }
    if dot {
        let cream = hex("#FFF3E8");
        for py in (y as u32)..((y + size) as u32) {
            for px in (x as u32)..((x + size) as u32) {
                let d = ((px as f32 + 0.5 - cx).powi(2) + (py as f32 + 0.5 - face_cy).powi(2)).sqrt();
                c.blend(px, py, cream, cov(d - size * 0.14));
            }
        }
    }
}
