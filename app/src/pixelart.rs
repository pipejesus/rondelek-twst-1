//! Shared procedural-art primitives for the build-time image generators
//! (`genskin`, `genbanner`). Not part of the app binary — it's pulled in by the
//! `src/bin/*` tools via `#[path = "../pixelart.rs"]`, so the whole visual
//! language (matte shading, the 5×7 pixel font) lives in one place.
//!
//! Each tool uses a different subset, so unused items here are expected.
#![allow(dead_code)]

use std::path::Path;

/// A tiny straight-alpha RGBA (f32 0..1) canvas with source-over blend.
pub struct Canvas {
    w: u32,
    h: u32,
    px: Vec<[f32; 4]>,
}

impl Canvas {
    pub fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![[0.0; 4]; (w * h) as usize],
        }
    }

    /// Source-over blend of `rgb` at opacity `a` onto pixel (x, y).
    pub fn blend(&mut self, x: u32, y: u32, rgb: [f32; 3], a: f32) {
        if a <= 0.0 || x >= self.w || y >= self.h {
            return;
        }
        let a = a.min(1.0);
        let p = &mut self.px[(y * self.w + x) as usize];
        let out_a = a + p[3] * (1.0 - a);
        if out_a <= 0.0 {
            return;
        }
        for i in 0..3 {
            p[i] = (rgb[i] * a + p[i] * p[3] * (1.0 - a)) / out_a;
        }
        p[3] = out_a;
    }

    pub fn save(&self, path: &Path) {
        let mut img = image::RgbaImage::new(self.w, self.h);
        for (i, p) in self.px.iter().enumerate() {
            let x = i as u32 % self.w;
            let y = i as u32 / self.w;
            img.put_pixel(
                x,
                y,
                image::Rgba([
                    (p[0].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (p[1].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (p[2].clamp(0.0, 1.0) * 255.0).round() as u8,
                    (p[3].clamp(0.0, 1.0) * 255.0).round() as u8,
                ]),
            );
        }
        img.save(path).expect("write png");
        println!("wrote {}", path.display());
    }
}

// ---- colour helpers ------------------------------------------------------

pub fn hex(s: &str) -> [f32; 3] {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap();
    [
        ((v >> 16) & 0xFF) as f32 / 255.0,
        ((v >> 8) & 0xFF) as f32 / 255.0,
        (v & 0xFF) as f32 / 255.0,
    ]
}

/// Mix toward white (amount > 0) or black (amount < 0).
pub fn shade(c: [f32; 3], amount: f32) -> [f32; 3] {
    let target = if amount >= 0.0 { 1.0 } else { 0.0 };
    let t = amount.abs();
    [
        c[0] + (target - c[0]) * t,
        c[1] + (target - c[1]) * t,
        c[2] + (target - c[2]) * t,
    ]
}

// ---- shape helpers -------------------------------------------------------

/// Signed distance to a rounded rectangle centred at (cx, cy) with half
/// extents (hw, hh) and corner radius r. Negative inside.
pub fn sd_rrect(x: f32, y: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (x - cx).abs() - (hw - r);
    let qy = (y - cy).abs() - (hh - r);
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// Antialiased coverage from a signed distance (1 px falloff).
pub fn cov(sd: f32) -> f32 {
    (0.5 - sd).clamp(0.0, 1.0)
}

pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

// ---- 5x7 pixel font ------------------------------------------------------

/// Row bitmaps (low 5 bits, MSB = leftmost). Covers the key labels plus the
/// letters/digits used in the banner wordmark; unknown chars render blank.
pub fn char_bits(ch: char) -> [u8; 7] {
    match ch {
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1E, 0x01, 0x01, 0x0E, 0x01, 0x01, 0x1E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x1E],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        _ => [0; 7],
    }
}

/// Draw `ch` centred at (cx, cy), `h` pixels tall, 3×3 supersampled.
pub fn draw_label(c: &mut Canvas, ch: char, cx: f32, cy: f32, h: f32, rgb: [f32; 3]) {
    let bits = char_bits(ch);
    let cell = h / 7.0;
    let w = cell * 5.0;
    let (x0, y0) = (cx - w * 0.5, cy - h * 0.5);
    let hit = |fx: f32, fy: f32| -> bool {
        let (ix, iy) = (fx.floor() as i32, fy.floor() as i32);
        (0..5).contains(&ix) && (0..7).contains(&iy) && (bits[iy as usize] >> (4 - ix)) & 1 == 1
    };
    let (px0, px1) = (x0.floor().max(0.0) as u32, (x0 + w).ceil() as u32);
    let (py0, py1) = (y0.floor().max(0.0) as u32, (y0 + h).ceil() as u32);
    for y in py0..py1 {
        for x in px0..px1 {
            let mut n = 0;
            for sy in 0..3 {
                for sx in 0..3 {
                    let fx = (x as f32 + (sx as f32 + 0.5) / 3.0 - x0) / cell;
                    let fy = (y as f32 + (sy as f32 + 0.5) / 3.0 - y0) / cell;
                    if hit(fx, fy) {
                        n += 1;
                    }
                }
            }
            if n > 0 {
                c.blend(x, y, rgb, 0.95 * n as f32 / 9.0);
            }
        }
    }
}

/// Lay a string left to right from `x_left`, each glyph `h` tall and centred on
/// `cy`, with `gap` pixels between cells. Returns the x just past the string.
pub fn draw_text(
    c: &mut Canvas,
    text: &str,
    x_left: f32,
    cy: f32,
    h: f32,
    gap: f32,
    rgb: [f32; 3],
) -> f32 {
    let cell = h / 7.0;
    let advance = cell * 5.0 + gap;
    let mut x = x_left;
    for ch in text.chars() {
        if ch != ' ' {
            draw_label(c, ch, x + cell * 2.5, cy, h, rgb);
        }
        x += advance;
    }
    x - gap
}
