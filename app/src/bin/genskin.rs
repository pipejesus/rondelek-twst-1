//! Generates the built-in "base-pastel" skin into `skins/base-pastel/`.
//!
//! Run from the repo root: `cargo run --bin genskin`
//!
//! Everything is drawn per-pixel with signed-distance functions plus hash
//! noise, so the whole skin is reproducible and tweakable from this one file.
//! It doubles as the reference for skin authors: which files a skin contains
//! and at what sizes (see docs/SKINS.md).

use std::path::Path;

// ---- tiny canvas ---------------------------------------------------------

struct Canvas {
    w: u32,
    h: u32,
    /// Straight-alpha RGBA, f32 0..1.
    px: Vec<[f32; 4]>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![[0.0; 4]; (w * h) as usize],
        }
    }

    /// Source-over blend of `rgb` at opacity `a` onto pixel (x, y).
    fn blend(&mut self, x: u32, y: u32, rgb: [f32; 3], a: f32) {
        if a <= 0.0 {
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

    fn save(&self, path: &Path) {
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

// ---- helpers -------------------------------------------------------------

fn hex(s: &str) -> [f32; 3] {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap();
    [
        ((v >> 16) & 0xFF) as f32 / 255.0,
        ((v >> 8) & 0xFF) as f32 / 255.0,
        (v & 0xFF) as f32 / 255.0,
    ]
}

/// Mix toward white (amount > 0) or black (amount < 0).
fn shade(c: [f32; 3], amount: f32) -> [f32; 3] {
    let target = if amount >= 0.0 { 1.0 } else { 0.0 };
    let t = amount.abs();
    [
        c[0] + (target - c[0]) * t,
        c[1] + (target - c[1]) * t,
        c[2] + (target - c[2]) * t,
    ]
}

fn mul(c: [f32; 3], f: f32) -> [f32; 3] {
    [
        (c[0] * f).clamp(0.0, 1.0),
        (c[1] * f).clamp(0.0, 1.0),
        (c[2] * f).clamp(0.0, 1.0),
    ]
}

/// Signed distance to a rounded rectangle centred at (cx, cy) with half
/// extents (hw, hh) and corner radius r. Negative inside.
fn sd_rrect(x: f32, y: f32, cx: f32, cy: f32, hw: f32, hh: f32, r: f32) -> f32 {
    let qx = (x - cx).abs() - (hw - r);
    let qy = (y - cy).abs() - (hh - r);
    let ox = qx.max(0.0);
    let oy = qy.max(0.0);
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - r
}

/// Antialiased coverage from a signed distance (1 px falloff).
fn cov(sd: f32) -> f32 {
    (0.5 - sd).clamp(0.0, 1.0)
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Deterministic hash noise in 0..1.
fn noise(x: u32, y: u32, seed: u32) -> f32 {
    let mut h = x
        .wrapping_mul(0x85EB_CA6B)
        .wrapping_add(y.wrapping_mul(0xC2B2_AE35))
        .wrapping_add(seed.wrapping_mul(0x27D4_EB2F));
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

// ---- glyphs --------------------------------------------------------------

/// Baked pad icons, drawn as boolean membership tests in [-1, 1] coords
/// (y grows downward), supersampled 3x3 for antialiasing.
#[derive(Clone, Copy)]
enum Glyph {
    Star,
    Heart,
    Flower,
    Sun,
    Cloud,
    Moon,
    Drop,
    Leaf,
    Fish,
    Apple,
    Boat,
    Note,
    RecDot,
    House,
    Bars,
}

fn in_circle(x: f32, y: f32, cx: f32, cy: f32, r: f32) -> bool {
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

fn glyph_hit(g: Glyph, x: f32, y: f32) -> bool {
    match g {
        Glyph::Star => {
            // 5-point star as a 10-gon, point-in-polygon.
            let (r_out, r_in) = (0.95f32, 0.42f32);
            let mut pts = [(0.0f32, 0.0f32); 10];
            for (i, p) in pts.iter_mut().enumerate() {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let r = if i % 2 == 0 { r_out } else { r_in };
                *p = (r * a.cos(), r * a.sin());
            }
            let mut inside = false;
            let mut j = 9;
            for i in 0..10 {
                let (xi, yi) = pts[i];
                let (xj, yj) = pts[j];
                if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
                    inside = !inside;
                }
                j = i;
            }
            inside
        }
        Glyph::Heart => {
            // Classic implicit heart, y up.
            let xs = x / 0.75;
            let ys = -(y + 0.12) / 0.75;
            let f = xs * xs + ys * ys - 1.0;
            f * f * f - xs * xs * ys * ys * ys <= 0.0
        }
        Glyph::Flower => {
            let r = (x * x + y * y).sqrt();
            let a = y.atan2(x);
            r <= 0.34 + 0.52 * (3.0 * a).cos().abs() * 0.9_f32.min(1.0) || r <= 0.24
        }
        Glyph::Sun => {
            let r = (x * x + y * y).sqrt();
            if r <= 0.48 {
                return true;
            }
            let a = y.atan2(x) / std::f32::consts::TAU;
            let seg = (a * 8.0).fract().abs();
            let seg = seg.min(1.0 - seg);
            (0.60..=0.92).contains(&r) && seg < 0.11
        }
        Glyph::Cloud => {
            in_circle(x, y, -0.42, 0.12, 0.34)
                || in_circle(x, y, 0.02, -0.14, 0.44)
                || in_circle(x, y, 0.44, 0.12, 0.32)
                || (x.abs() <= 0.44 && (0.12..=0.44).contains(&y))
        }
        Glyph::Moon => in_circle(x, y, -0.08, 0.0, 0.75) && !in_circle(x, y, 0.34, -0.18, 0.62),
        Glyph::Drop => {
            in_circle(x, y, 0.0, 0.28, 0.46)
                || ((-0.72..=0.28).contains(&y) && x.abs() <= 0.46 * (y + 0.72) / 1.0)
        }
        Glyph::Leaf => in_circle(x, y, -0.30, 0.30, 0.92) && in_circle(x, y, 0.30, -0.30, 0.92),
        Glyph::Fish => {
            // Body vesica pointing right, triangular tail on the left, eye cut out.
            let body = in_circle(x, y, 0.1, -0.38, 0.8) && in_circle(x, y, 0.1, 0.38, 0.8);
            let tail = (-0.85..=-0.45).contains(&x) && y.abs() <= -(x + 0.45) * 0.9 + 0.03;
            let eye = in_circle(x, y, 0.42, -0.06, 0.10);
            (body || tail) && !eye
        }
        Glyph::Apple => {
            let body = in_circle(x, y, -0.22, 0.18, 0.5) || in_circle(x, y, 0.22, 0.18, 0.5);
            let stem = (-0.62..=-0.18).contains(&y) && (x - 0.06 - (y + 0.62) * 0.2).abs() <= 0.07;
            let leaf = in_circle(x, y, -0.28, -0.42, 0.30) && in_circle(x, y, -0.62, -0.60, 0.34);
            body || stem || leaf
        }
        Glyph::Boat => {
            // Hull trapezoid + mast + triangular sail.
            let hull = (0.28..=0.62).contains(&y) && x.abs() <= 0.72 - (y - 0.28) * 0.8;
            let mast = (-0.72..=0.28).contains(&y) && (x - 0.02).abs() <= 0.05;
            let sail = (-0.68..=0.12).contains(&y)
                && (0.12..=0.66).contains(&x)
                && x - 0.12 <= (y + 0.68) * 0.68;
            hull || mast || sail
        }
        Glyph::Note => {
            // Eighth note: head, stem, flag.
            let head = in_circle(x * 1.15, (y - 0.45) * 1.45, -0.28, 0.0, 0.42);
            let stem = (-0.72..=0.45).contains(&y) && (x - 0.06).abs() <= 0.07;
            let flag = (-0.72..=-0.25).contains(&y)
                && (x - 0.06 >= 0.0)
                && x - 0.06 <= 0.5 * (1.0 - ((y + 0.72) / 0.47 - 0.5).abs() * 2.0 * 0.4)
                && x - 0.06 <= -(y + 0.25) * 1.1;
            head || stem || flag
        }
        Glyph::RecDot => {
            let r = (x * x + y * y).sqrt();
            r <= 0.34 || (0.55..=0.72).contains(&r)
        }
        Glyph::House => {
            // Roof triangle over a body with a door notch.
            let roof = (-0.75..=-0.05).contains(&y) && x.abs() <= (y + 0.75) * 1.05;
            let body = (-0.05..=0.68).contains(&y) && x.abs() <= 0.55;
            let door = (0.18..=0.68).contains(&y) && x.abs() <= 0.16;
            (roof || body) && !door
        }
        Glyph::Bars => {
            let bar = |cx: f32, top: f32| (top..=0.7).contains(&y) && (x - cx).abs() <= 0.14;
            bar(-0.5, 0.1) || bar(0.0, -0.2) || bar(0.5, -0.55)
        }
    }
}

/// Draw a glyph centred at (cx, cy) with half-size `s`, 3x3 supersampled.
fn draw_glyph(c: &mut Canvas, g: Glyph, cx: f32, cy: f32, s: f32, rgb: [f32; 3], alpha: f32) {
    let (x0, x1) = (((cx - s) as u32).max(0), ((cx + s) as u32 + 1).min(c.w));
    let (y0, y1) = (((cy - s) as u32).max(0), ((cy + s) as u32 + 1).min(c.h));
    for y in y0..y1 {
        for x in x0..x1 {
            let mut hits = 0;
            for sy in 0..3 {
                for sx in 0..3 {
                    let fx = (x as f32 + (sx as f32 + 0.5) / 3.0 - cx) / s;
                    let fy = (y as f32 + (sy as f32 + 0.5) / 3.0 - cy) / s;
                    if glyph_hit(g, fx, fy) {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                c.blend(x, y, rgb, alpha * hits as f32 / 9.0);
            }
        }
    }
}

// ---- skin pieces ---------------------------------------------------------

fn gen_background(dir: &Path) {
    let (w, h) = (1280u32, 800u32);
    let mut c = Canvas::new(w, h);
    let top = hex("#FBF2E4");
    let bottom = hex("#F6DCC6");
    // Soft warm blobs, barely-there, for a hand-made feel.
    let blobs = [
        (0.18f32, 0.25f32, 0.30f32),
        (0.80, 0.18, 0.26),
        (0.55, 0.78, 0.34),
    ];
    for y in 0..h {
        let t = y as f32 / h as f32;
        for x in 0..w {
            let mut col = [
                top[0] + (bottom[0] - top[0]) * t,
                top[1] + (bottom[1] - top[1]) * t,
                top[2] + (bottom[2] - top[2]) * t,
            ];
            for (bx, by, br) in blobs {
                let dx = x as f32 / w as f32 - bx;
                let dy = y as f32 / h as f32 - by;
                let d = (dx * dx + dy * dy).sqrt() / br;
                if d < 1.0 {
                    let a = 0.06 * (1.0 - d) * (1.0 - d);
                    let peach = hex("#F0C9A8");
                    for i in 0..3 {
                        col[i] += (peach[i] - col[i]) * a;
                    }
                }
            }
            let n = (noise(x, y, 1) - 0.5) * 0.02;
            c.blend(x, y, [col[0] + n, col[1] + n, col[2] + n], 1.0);
        }
    }
    c.save(&dir.join("background.png"));
}

/// The sampler case: nine-sliced by the engine with a 96 px inset (see
/// skin.json), so all shading must be edge-local; the centre stays flat and
/// gets its matte grain from the tiled grain.png overlay instead.
fn gen_case(dir: &Path) {
    let size = 512u32;
    let mut c = Canvas::new(size, size);
    let base = hex("#C6E5D3");
    let (cx, cy, hw) = (256.0f32, 256.0, 250.0);
    let radius = 72.0;
    for y in 0..size {
        for x in 0..size {
            let sd = sd_rrect(x as f32 + 0.5, y as f32 + 0.5, cx, cy, hw, hw, radius);
            let a = cov(sd);
            if a <= 0.0 {
                continue;
            }
            // Edge vignette: darken toward the silhouette for a moulded look.
            let vign = smoothstep(-64.0, -4.0, sd) * 0.10;
            // Gentle top light inside the top edge band.
            let toplight = smoothstep(96.0, 0.0, y as f32) * 0.05;
            let n = (noise(x, y, 2) - 0.5) * 0.035;
            let col = mul(shade(base, toplight - vign), 1.0 + n);
            c.blend(x, y, col, a);
            // Crisp darker rim line right at the silhouette.
            let line = smoothstep(-4.0, -1.0, sd) * cov(sd + 0.5);
            c.blend(x, y, shade(base, -0.30), line * 0.55);
        }
    }
    c.save(&dir.join("case.png"));
}

/// Tileable matte grain, overlaid by the engine on the flat case centre
/// (nine-slice stretching would smear baked noise there).
fn gen_grain(dir: &Path) {
    let size = 128u32;
    let mut c = Canvas::new(size, size);
    for y in 0..size {
        for x in 0..size {
            let v = noise(x, y, 3) - 0.5;
            let (col, a) = if v > 0.0 {
                ([1.0, 1.0, 1.0], v * 0.09)
            } else {
                ([0.0, 0.0, 0.0], -v * 0.09)
            };
            c.blend(x, y, col, a);
        }
    }
    c.save(&dir.join("grain.png"));
}

fn gen_bezel(dir: &Path) {
    let size = 384u32;
    let mut c = Canvas::new(size, size);
    let base = hex("#4E4A45");
    let (cc, hw, radius) = (192.0f32, 188.0, 48.0);
    for y in 0..size {
        for x in 0..size {
            let sd = sd_rrect(x as f32 + 0.5, y as f32 + 0.5, cc, cc, hw, hw, radius);
            let a = cov(sd);
            if a <= 0.0 {
                continue;
            }
            let vign = smoothstep(-40.0, -2.0, sd) * 0.16;
            let toplight = smoothstep(56.0, 0.0, y as f32) * 0.06;
            let n = (noise(x, y, 4) - 0.5) * 0.03;
            let col = mul(shade(base, toplight - vign), 1.0 + n);
            c.blend(x, y, col, a);
        }
    }
    c.save(&dir.join("bezel.png"));
}

/// A chunky toy keycap: darker rim walls, raised matte cap with a baked
/// gloss band, and a friendly glyph. `pressed` sinks the cap and dims it.
fn gen_keycap(dir: &Path, name: &str, base: [f32; 3], glyph: Glyph, glyph_rgb: [f32; 3]) {
    for pressed in [false, true] {
        let size = 256u32;
        let mut c = Canvas::new(size, size);
        let m = 10.0f32; // transparent margin
        let rim = shade(base, -0.28);
        let (cc, hw) = (128.0f32, 128.0 - m);
        let rim_r = 46.0;

        // Cap geometry: sits high when idle, sinks flush when pressed.
        let (cap_top, cap_bot, cap_col, gloss_a) = if pressed {
            (m + 20.0, 246.0 - 12.0, mul(base, 0.93), 0.08)
        } else {
            (m + 6.0, 246.0 - 24.0, base, 0.16)
        };
        let cap_cx = 128.0;
        let cap_cy = (cap_top + cap_bot) * 0.5;
        let cap_hw = hw - 12.0;
        let cap_hh = (cap_bot - cap_top) * 0.5;
        let cap_r = 40.0;

        for y in 0..size {
            for x in 0..size {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let sd_rim = sd_rrect(fx, fy, cc, cc, hw, hw, rim_r);
                let a = cov(sd_rim);
                if a <= 0.0 {
                    continue;
                }
                c.blend(x, y, rim, a);

                let sd_cap = sd_rrect(fx, fy, cap_cx, cap_cy, cap_hw, cap_hh, cap_r);
                let ca = cov(sd_cap);
                if ca > 0.0 {
                    // Vertical shading + matte noise on the cap face.
                    let t = (fy - cap_top) / (cap_bot - cap_top);
                    let n = (noise(x, y, 5) - 0.5) * 0.03;
                    let col = mul(cap_col, 1.05 - 0.09 * t + n);
                    c.blend(x, y, col, ca);
                    // Gloss band across the top of the cap.
                    let band = smoothstep(0.46, 0.34, t);
                    c.blend(x, y, [1.0, 1.0, 1.0], ca * band * gloss_a);
                }
            }
        }

        let gy = cap_cy + if pressed { 6.0 } else { 0.0 };
        draw_glyph(&mut c, glyph, 128.0, gy, 62.0, glyph_rgb, 0.95);

        let file = if pressed {
            format!("{name}_pressed.png")
        } else {
            format!("{name}.png")
        };
        c.save(&dir.join(file));
    }
}

fn gen_avatar_frame(dir: &Path) {
    let size = 256u32;
    let mut c = Canvas::new(size, size);
    let base = hex("#F3EBDC");
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let sd_out = sd_rrect(fx, fy, 128.0, 128.0, 122.0, 122.0, 44.0);
            let sd_in = sd_rrect(fx, fy, 128.0, 128.0, 100.0, 100.0, 30.0);
            let a = cov(sd_out) * (1.0 - cov(sd_in));
            if a <= 0.0 {
                continue;
            }
            let t = fy / size as f32;
            let n = (noise(x, y, 6) - 0.5) * 0.03;
            let vign = smoothstep(-8.0, -1.0, sd_out) * 0.12;
            c.blend(x, y, mul(shade(base, -vign), 1.03 - 0.06 * t + n), a);
        }
    }
    c.save(&dir.join("avatar_frame.png"));
}

// ---- main ----------------------------------------------------------------

fn main() {
    let dir = Path::new("skins/base-pastel");
    std::fs::create_dir_all(dir).expect("create skin dir");

    gen_background(dir);
    gen_case(dir);
    gen_grain(dir);
    gen_bezel(dir);
    gen_avatar_frame(dir);

    // Twelve candy-pastel sample pads (4x3 grid), one friendly icon each.
    let pads: [(&str, Glyph); 12] = [
        ("#F5A9BC", Glyph::Star),
        ("#F8C9A0", Glyph::Heart),
        ("#F5E29E", Glyph::Flower),
        ("#AADDC2", Glyph::Sun),
        ("#A9D4EF", Glyph::Cloud),
        ("#C9B8E8", Glyph::Moon),
        ("#F5A08F", Glyph::Drop),
        ("#A2DEDA", Glyph::Leaf),
        ("#F8BFD8", Glyph::Fish),
        ("#BCE3A8", Glyph::Apple),
        ("#FFD9A8", Glyph::Boat),
        ("#B8CFF2", Glyph::Note),
    ];
    for (i, (col, glyph)) in pads.iter().enumerate() {
        let base = hex(col);
        gen_keycap(
            dir,
            &format!("pad_{}", i + 1),
            base,
            *glyph,
            shade(base, -0.55),
        );
    }

    // Function keys.
    let charcoal = hex("#4E4A45");
    let cream = hex("#F5EFE2");
    gen_keycap(dir, "rec", hex("#E9655A"), Glyph::RecDot, hex("#FFF6EE"));
    gen_keycap(dir, "back", charcoal, Glyph::House, cream);
    gen_keycap(dir, "cycle", charcoal, Glyph::Bars, cream);

    println!("done.");
}
