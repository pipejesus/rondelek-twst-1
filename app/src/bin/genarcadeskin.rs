//! Generates the built-in **Arcade** skin into `skins/arcade/`: the sampler as
//! the app's arcade machine, in the same 8-bit look as the shell and the games.
//!
//! Run from the repo root: `cargo run --bin genarcadeskin`
//!
//! Same spritesheet contract as the Classic skin (`genskin`, and
//! `rondelek_core::config::atlas`): one `skin.png` with every faceplate element
//! at a fixed rectangle, plus `skin.json` colours. What makes it arcade:
//!
//! - **Case**: a navy cabinet body with a hard ink outline, a bevel, notched
//!   pixel corners and the 80s stripe trim along the top. The nine-slice
//!   corners are drawn 1:1 on screen, so they stay crisp; the edges only vary
//!   *across* their length, so stretching doesn't smear them.
//! - **Bezel**: the cabinet's screen well — ink, a cyan line, ink — around a
//!   dark CRT glass.
//! - **Keys**: chunky pixel keycaps (32×32 art pixels, 8 real pixels each) with
//!   an ink outline, a bevelled face on a dark lip and an embossed 5×7 pixel
//!   label. The twelve pads run in a diagonal rainbow; REC is red with a pixel
//!   record dot; BACK and CYCLE are navy with cream / cyan pixel icons.
//! - **Colours**: navy panels, a near-black screen with neon cyan → pink →
//!   butter bars, green sample lights.
//!
//! Every value comes from the arcade palette (`rondelek_core::arcade`), so the
//! skin, the shell and the games stay one family.

use rondelek_core::arcade as pal;
use rondelek_core::config::atlas::{self, Sprite};
use std::path::Path;

#[path = "../pixelart.rs"]
mod pixelart;
use pixelart::{Canvas, char_bits};

type Rgb = [f32; 3];

fn c(v: pal::Rgb) -> Rgb {
    [
        v[0] as f32 / 255.0,
        v[1] as f32 / 255.0,
        v[2] as f32 / 255.0,
    ]
}

fn shade(v: pal::Rgb, amount: f32) -> Rgb {
    c(pal::shade(v, amount))
}

/// Opaque rectangle.
fn fill(cv: &mut Canvas, x: u32, y: u32, w: u32, h: u32, rgb: Rgb) {
    fill_a(cv, x, y, w, h, rgb, 1.0);
}

fn fill_a(cv: &mut Canvas, x: u32, y: u32, w: u32, h: u32, rgb: Rgb, a: f32) {
    for yy in y..y + h {
        for xx in x..x + w {
            cv.blend(xx, yy, rgb, a);
        }
    }
}

/// A rectangle with its corner `p`×`p` cells cut off (the 8-bit corner).
fn notched(cv: &mut Canvas, x: u32, y: u32, w: u32, h: u32, p: u32, rgb: Rgb) {
    fill(cv, x + p, y, w - 2 * p, h, rgb);
    fill(cv, x, y + p, w, h - 2 * p, rgb);
}

// ---- background ------------------------------------------------------------------

/// Deep navy, lighter toward the bottom, with a soft violet horizon glow and a
/// scatter of soft star dots. It is scaled up to fill the window, so it holds
/// no hard pixel detail (that would only blur).
fn gen_background(cv: &mut Canvas, s: Sprite) {
    let (top, bottom) = (c(pal::NIGHT_LO), c(pal::NIGHT));
    for y in 0..s.h {
        let t = y as f32 / s.h as f32;
        let row = [
            top[0] + (bottom[0] - top[0]) * t,
            top[1] + (bottom[1] - top[1]) * t,
            top[2] + (bottom[2] - top[2]) * t,
        ];
        for x in 0..s.w {
            cv.blend(s.x + x, s.y + y, row, 1.0);
        }
    }
    let glow = c(pal::TILE_COLORS[1]);
    for y in 0..s.h {
        for x in 0..s.w {
            let dx = (x as f32 - s.w as f32 / 2.0) / (s.w as f32 * 0.55);
            let dy = (y as f32 - s.h as f32 * 1.1) / (s.h as f32 * 0.7);
            let d = (dx * dx + dy * dy).sqrt();
            let a = (1.0 - d).clamp(0.0, 1.0).powi(2) * 0.28;
            if a > 0.0 {
                cv.blend(s.x + x, s.y + y, glow, a);
            }
        }
    }
    let mut seed: u32 = 0x2545_F491;
    let mut rnd = |n: u32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed % n
    };
    for _ in 0..70 {
        let (x, y) = (rnd(s.w - 2), rnd(s.h - 2));
        let col = match rnd(3) {
            0 => c(pal::CYAN),
            1 => c(pal::PINK),
            _ => c(pal::BUTTER),
        };
        fill_a(cv, s.x + x, s.y + y, 2, 2, col, 0.55);
    }
}

// ---- case, bezel, avatar frame ----------------------------------------------------

/// The cabinet body. Nine-sliced with a 44-px corner, so everything within 44
/// px of an edge is the frame, and the centre (a flat panel) stretches.
fn gen_case(cv: &mut Canvas, s: Sprite) {
    let p = 4;
    let body = pal::NIGHT_HI;
    let (x, y, w, h) = (s.x, s.y, s.w, s.h);
    notched(cv, x, y, w, h, p, c(pal::INK));
    // Bevel: light top-left, dark bottom-right, then the flat face.
    notched(cv, x + p, y + p, w - 2 * p, h - 2 * p, p, shade(body, -0.4));
    notched(cv, x + p, y + p, w - 3 * p, h - 3 * p, p, shade(body, 0.28));
    notched(cv, x + 2 * p, y + 2 * p, w - 4 * p, h - 4 * p, p, c(body));
    // The 80s stripe trim along the top (constant along x: stretches cleanly).
    for (i, col) in [pal::ORANGE, pal::BUTTER, pal::PINK, pal::CYAN]
        .iter()
        .enumerate()
    {
        fill(cv, x + 16, y + 14 + i as u32 * 4, w - 32, 4, c(*col));
    }
    // A thin ink line under the trim, and one along the bottom edge.
    fill(cv, x + 16, y + 30, w - 32, 2, c(pal::INK));
    fill(cv, x + 16, y + h - 20, w - 32, 4, shade(body, -0.25));
}

/// The screen well: ink, a cyan line, ink, then dark glass up to the 26-px
/// inset (where the visualizer's opening starts).
fn gen_bezel(cv: &mut Canvas, s: Sprite) {
    let (x, y, w, h) = (s.x, s.y, s.w, s.h);
    notched(cv, x, y, w, h, 4, c(pal::INK));
    notched(cv, x + 4, y + 4, w - 8, h - 8, 4, c(pal::CYAN));
    notched(cv, x + 8, y + 8, w - 16, h - 16, 4, c(pal::INK));
    notched(cv, x + 12, y + 12, w - 24, h - 24, 4, [0.043, 0.039, 0.11]);
    // A faint glass highlight along the inner top edge.
    fill_a(cv, x + 18, y + 16, w - 36, 2, c(pal::NIGHT_HI), 0.8);
}

/// The frame over the child's picture: ink, butter with a bevel, ink; the
/// centre stays transparent (the picture shows through).
fn gen_avatar_frame(cv: &mut Canvas, s: Sprite) {
    let (x, y, w) = (s.x, s.y, s.w);
    let ring = |cv: &mut Canvas, inset: u32, t: u32, rgb: Rgb| {
        let (a, b) = (inset, w - inset);
        fill(cv, x + a + 8, y + a, b - a - 16, t, rgb); // top
        fill(cv, x + a + 8, y + b - t, b - a - 16, t, rgb); // bottom
        fill(cv, x + a, y + a + 8, t, b - a - 16, rgb); // left
        fill(cv, x + b - t, y + a + 8, t, b - a - 16, rgb); // right
        // Stepped corners.
        for (cx, cy) in [(a, a), (b - 16, a), (a, b - 16), (b - 16, b - 16)] {
            fill(cv, x + cx + 4, y + cy + 4, 12, 12, rgb);
        }
    };
    ring(cv, 0, 8, c(pal::INK));
    ring(cv, 8, 12, c(pal::BUTTER));
    ring(cv, 8, 4, shade(pal::BUTTER, 0.4));
    ring(cv, 20, 6, c(pal::INK));
}

// ---- key caps ------------------------------------------------------------------------

/// One art pixel of a cap, in real pixels (caps are 32×32 art pixels).
const A: u32 = 8;

/// A chunky pixel keycap in `face`, then `icon` painted in art-pixel
/// coordinates (0..32). The top-right of the face stays clear for the
/// engine's sample LED.
fn gen_cap(
    cv: &mut Canvas,
    s: Sprite,
    face: pal::Rgb,
    icon: impl Fn(&mut dyn FnMut(u32, u32, Rgb)),
) {
    let (ox, oy) = (s.x, s.y);
    let mut px = |x: u32, y: u32, w: u32, h: u32, rgb: Rgb| {
        fill(cv, ox + x * A, oy + y * A, w * A, h * A, rgb);
    };
    // Ink outline (2..30), notched.
    px(3, 2, 26, 28, c(pal::INK));
    px(2, 3, 28, 26, c(pal::INK));
    // Dark lip under the face.
    px(4, 3, 24, 26, shade(face, -0.42));
    px(3, 4, 26, 24, shade(face, -0.42));
    // The face (rows 3..25) with a light bevel along its top and left.
    px(4, 3, 24, 22, c(face));
    px(3, 4, 26, 20, c(face));
    px(4, 3, 24, 1, shade(face, 0.38));
    px(3, 4, 1, 20, shade(face, 0.38));
    // The icon, in art pixels.
    let mut set = |x: u32, y: u32, rgb: Rgb| {
        fill(cv, ox + x * A, oy + y * A, A, A, rgb);
    };
    icon(&mut set);
}

/// A 5×7 glyph at 2 art pixels per glyph pixel, embossed: a darker copy one
/// art pixel down-right, then the ink on top.
fn glyph(set: &mut dyn FnMut(u32, u32, Rgb), ch: char, x0: u32, y0: u32, ink: Rgb, emboss: Rgb) {
    let bits = char_bits(ch);
    for pass in 0..2 {
        let (dx, col) = if pass == 0 { (1, emboss) } else { (0, ink) };
        for (row, b) in bits.iter().enumerate() {
            for colbit in 0..5 {
                if (b >> (4 - colbit)) & 1 == 1 {
                    for sy in 0..2 {
                        for sx in 0..2 {
                            set(
                                x0 + colbit * 2 + sx + dx,
                                y0 + row as u32 * 2 + sy + dx,
                                col,
                            );
                        }
                    }
                }
            }
        }
    }
}

fn main() {
    let dir = Path::new("skins/arcade");
    std::fs::create_dir_all(dir).expect("create skin dir");
    let mut cv = Canvas::new(atlas::ATLAS_W, atlas::ATLAS_H);

    gen_background(&mut cv, atlas::BG);
    gen_case(&mut cv, atlas::CASE);
    gen_bezel(&mut cv, atlas::BEZEL);
    gen_avatar_frame(&mut cv, atlas::AVATAR);

    // The twelve pads: a diagonal rainbow, key letter embossed in ink.
    let rainbow = [
        pal::TILE_COLORS[4], // pink
        [0xFF, 0x8A, 0x3A],  // orange
        pal::TILE_COLORS[2], // sunflower
        pal::TILE_COLORS[3], // green
        pal::TILE_COLORS[0], // blue
        pal::TILE_COLORS[1], // violet
    ];
    let labels = ['1', '2', '3', '4', 'Q', 'W', 'E', 'R', 'A', 'S', 'D', 'F'];
    for (i, ch) in labels.iter().copied().enumerate() {
        let face = rainbow[(i / 4 + i % 4) % rainbow.len()];
        gen_cap(&mut cv, atlas::cap(i), face, |set| {
            glyph(set, ch, 11, 7, c(pal::INK), shade(face, -0.35));
        });
    }

    // REC: red, with a round pixel record dot.
    let red = pal::RED;
    gen_cap(&mut cv, atlas::cap(atlas::REC_CAP), red, |set| {
        let dot = [
            "..####..", ".######.", "########", "########", "########", "########", ".######.",
            "..####..",
        ];
        for (y, row) in dot.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                if ch == '#' {
                    set(12 + x as u32, 10 + y as u32, c(pal::CREAM));
                }
            }
        }
    });

    // BACK: navy, a cream pixel chevron.
    let navy = pal::shade(pal::NIGHT_HI, 0.12);
    gen_cap(&mut cv, atlas::cap(atlas::BACK_CAP), navy, |set| {
        glyph(set, '<', 11, 7, c(pal::CREAM), c(pal::INK));
    });

    // CYCLE: navy, three ascending cyan bars.
    gen_cap(&mut cv, atlas::cap(atlas::CYCLE_CAP), navy, |set| {
        for (k, h) in [4u32, 7, 10].iter().enumerate() {
            let x = 10 + k as u32 * 4;
            for yy in 0..*h {
                for xx in 0..3 {
                    set(x + xx, 20 - yy, c(pal::CYAN));
                }
            }
        }
    });

    cv.save(&dir.join("skin.png"));
    write_skin_json(dir);
    println!("done.");
}

fn write_skin_json(dir: &Path) {
    let json = r##"{
  "name": "Arcade",
  "author": "Rondelek",
  "colors": {
    "panel_bg": "#2E2A5C",
    "panel_fg": "#1D1A3A",
    "pad_play_bg": "#3AA8F0",
    "pad_play_fg": "#0A0917",
    "pad_record_bg": "#F03E3E",
    "pad_record_fg": "#FFF3E8",
    "pad_function_bg": "#2E2A5C",
    "pad_function_fg": "#3FE0FF",
    "led_empty": "#3A3670",
    "led_full": "#3FD67A",
    "case_shadow": "#00000070",
    "case_border": "#0A0917",
    "text_primary": "#FFF3E8",
    "text_secondary": "#A9A4D6",
    "visualizer_bg": "#0B0A1C",
    "visualizer_dot_off": "#1F1C40",
    "visualizer_bar_low": "#3FE0FF",
    "visualizer_bar_mid": "#FF4FA3",
    "visualizer_bar_high": "#FFD84A"
  }
}
"##;
    let path = dir.join("skin.json");
    std::fs::write(&path, json).expect("write skin.json");
    println!("wrote {}", path.display());
}
