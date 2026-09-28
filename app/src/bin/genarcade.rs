//! Generates the README's arcade-style picture section into
//! `docs/images/arcade/`:
//!
//! - `banner.png`: the README's title, a "RONDELEK" marquee.
//! - `marquee.png`: an 8-bit "PRESS START" marquee over a starfield.
//! - `home.png`, `hub.png`, `calibration.png`, `sampler.png`: the screenshots in
//!   `docs/images/` framed like arcade screens, with a pixel caption plate.
//! - `runner-frame.png`: the same frame around a transparent 640×360 hole,
//!   which `docs/images/arcade/record-runner.sh` lays over recorded gameplay
//!   to make `runner.gif`.
//!
//! Run from the repo root: `cargo run --bin genarcade`
//!
//! Everything is drawn on a coarse grid (`P` px per "pixel") with the 5×7 font
//! from `pixelart`, so it reads as chunky 8-bit art at README size.

use std::path::Path;

#[path = "../pixelart.rs"]
mod pixelart;
use pixelart::{Canvas, draw_text, hex};

/// One 8-bit "pixel" of the frames and marquee, in real pixels.
const P: u32 = 4;

/// The gameplay hole in `runner-frame.png` (16:9).
const RUNNER_W: u32 = 640;
const RUNNER_H: u32 = 360;

struct Palette {
    night: [f32; 3],
    night_hi: [f32; 3],
    night_lo: [f32; 3],
    ink: [f32; 3],
    orange: [f32; 3],
    butter: [f32; 3],
    pink: [f32; 3],
    cyan: [f32; 3],
    cream: [f32; 3],
}

fn palette() -> Palette {
    Palette {
        night: hex("#1D1A3A"),
        night_hi: hex("#2E2A5C"),
        night_lo: hex("#121027"),
        ink: hex("#0A0917"),
        orange: hex("#FF6A1A"),
        butter: hex("#FFD84A"),
        pink: hex("#FF4FA3"),
        cyan: hex("#3FE0FF"),
        cream: hex("#FFF3E8"),
    }
}

fn main() {
    let src = Path::new("docs/images");
    let out = src.join("arcade");
    std::fs::create_dir_all(&out).expect("create arcade dir");
    let pal = palette();

    marquee(
        &pal,
        ["1UP 000000", "HI-SCORE 999999", "CREDIT 01"],
        "PRESS START",
        12,
        "* SAY A VOWEL TO PLAY *",
    )
    .save(&out.join("marquee.png"));
    marquee(
        &pal,
        ["1UP", "TWST-1", "CREDIT 01"],
        "RONDELEK",
        14,
        "* A SOUND BOARD AND VOICE GAMES FOR KIDS *",
    )
    .save(&out.join("banner.png"));

    let shots = [
        ("home.png", 760, "> PLAYER SELECT", "1UP"),
        ("hub.png", 460, "> PICK A MODE", "*"),
        ("calibration.png", 460, "> VOICE CALIBRATION", "*"),
        ("sampler.png", 300, "> SOUND BOARD", "REC"),
    ];
    for (name, width, caption, badge) in shots {
        let img = image::open(src.join(name))
            .unwrap_or_else(|e| panic!("read docs/images/{name}: {e}"))
            .into_rgba8();
        let h = (img.height() as f32 * width as f32 / img.width() as f32).round() as u32;
        let img = image::imageops::resize(&img, width, h, image::imageops::FilterType::Lanczos3);
        let (mut c, sx, sy) = frame(&pal, width, h, caption, badge);
        for (x, y, px) in img.enumerate_pixels() {
            let [r, g, b, a] = px.0;
            let rgb = [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0];
            c.blend(sx + x, sy + y, rgb, a as f32 / 255.0);
        }
        c.save(&out.join(name));
    }

    // Left transparent: the recording script composites gameplay under it.
    let (c, sx, sy) = frame(&pal, RUNNER_W, RUNNER_H, "> STAGE 1: VOWEL RUNNER", "♥♥♥");
    c.save(&out.join("runner-frame.png"));
    println!("runner hole: {RUNNER_W}x{RUNNER_H} at {sx},{sy}");
    println!("done.");
}

// ---- drawing helpers -----------------------------------------------------

fn fill(c: &mut Canvas, x: u32, y: u32, w: u32, h: u32, rgb: [f32; 3]) {
    for py in y..y + h {
        for px in x..x + w {
            c.blend(px, py, rgb, 1.0);
        }
    }
}

/// A rectangle with its corners stepped in by one `P`, the classic 8-bit
/// window shape.
fn notched(c: &mut Canvas, x: u32, y: u32, w: u32, h: u32, rgb: [f32; 3]) {
    fill(c, x + P, y, w - 2 * P, h, rgb);
    fill(c, x, y + P, P, h - 2 * P, rgb);
    fill(c, x + w - P, y + P, P, h - 2 * P, rgb);
}

/// Width of `text` drawn by `pixelart::draw_text` with glyph cell `cell`
/// (glyphs 7 cells tall, one cell apart).
fn text_w(text: &str, cell: u32) -> u32 {
    let n = text.chars().count() as u32;
    n * cell * 6 - cell
}

/// Pixel text with a hard drop shadow one cell down-right.
fn shadow_text(
    c: &mut Canvas,
    text: &str,
    x: u32,
    cy: u32,
    cell: u32,
    rgb: [f32; 3],
    shadow: [f32; 3],
) {
    let (h, gap) = ((cell * 7) as f32, cell as f32);
    draw_text(
        c,
        text,
        (x + cell) as f32,
        (cy + cell) as f32,
        h,
        gap,
        shadow,
    );
    draw_text(c, text, x as f32, cy as f32, h, gap, rgb);
}

/// An arcade-cabinet frame around a `sw`×`sh` screen, with a caption plate
/// underneath: `caption` on the left, `badge` on the right. Returns the canvas
/// and the screen's top-left corner; the screen itself is left transparent.
fn frame(pal: &Palette, sw: u32, sh: u32, caption: &str, badge: &str) -> (Canvas, u32, u32) {
    let m = 5 * P; // frame thickness around the screen
    // Caption glyph cell: as big as fits, capped for the wide frames.
    let cell = {
        let room = sw.saturating_sub(4 * P);
        let n = (caption.chars().count() + badge.chars().count() + 2) as u32;
        (room / (n * 6)).clamp(2, P)
    };
    let cap = cell * 7 + 10 * P; // caption plate height
    let (w, h) = (sw + 2 * m, sh + m + cap);
    let mut c = Canvas::new(w, h);

    // Body: ink outline, then the panel with a light top-left bevel and a
    // dark bottom-right one.
    notched(&mut c, 0, 0, w, h, pal.ink);
    notched(&mut c, P, P, w - 2 * P, h - 2 * P, pal.night_lo);
    notched(&mut c, P, P, w - 3 * P, h - 3 * P, pal.night_hi);
    notched(&mut c, 2 * P, 2 * P, w - 4 * P, h - 4 * P, pal.night);

    // Screen well: a cyan line, an ink gap, then the (transparent) screen.
    let (sx, sy) = (m, m);
    fill(
        &mut c,
        sx - 2 * P,
        sy - 2 * P,
        sw + 4 * P,
        sh + 4 * P,
        pal.cyan,
    );
    fill(&mut c, sx - P, sy - P, sw + 2 * P, sh + 2 * P, pal.ink);
    c.clear(sx, sy, sw, sh);

    // Caption plate: centred below the screen well (the shadow hangs one
    // cell lower, so nudge up by half of it).
    let cy = sh + m + cap / 2 - cell / 2;
    shadow_text(&mut c, caption, sx, cy, cell, pal.butter, pal.ink);
    let bw = text_w(badge, cell);
    shadow_text(
        &mut c,
        badge,
        sx + sw - bw - cell,
        cy,
        cell,
        pal.pink,
        pal.ink,
    );
    (c, sx, sy)
}

/// A marquee: rainbow stripes, a starfield, a score row (`row`: left, centre,
/// right), a big `title` (glyph cell `big`) and a line of `sub` under it.
fn marquee(pal: &Palette, row: [&str; 3], title: &str, big: u32, sub: &str) -> Canvas {
    let (w, h) = (1280, 300);
    let mut c = Canvas::new(w, h);
    notched(&mut c, 0, 0, w, h, pal.ink);
    notched(&mut c, P, P, w - 2 * P, h - 2 * P, pal.night);

    // Starfield: a fixed pseudo-random scatter of P-sized dots.
    let mut seed: u32 = 0x9E37_79B9;
    let mut rnd = |n: u32| {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed % n
    };
    for _ in 0..140 {
        let x = 2 * P + rnd((w - 4 * P) / P) * P;
        let y = 10 * P + rnd((h - 20 * P) / P) * P;
        let col = match rnd(6) {
            0 => pal.cyan,
            1 => pal.pink,
            2 => pal.butter,
            _ => pal.night_hi,
        };
        fill(&mut c, x, y, P, P, col);
    }

    // 80s stripes top and bottom.
    let bands = [pal.orange, pal.butter, pal.pink, pal.cyan];
    for (i, col) in bands.iter().enumerate() {
        let i = i as u32;
        fill(&mut c, 2 * P, 2 * P + i * 2 * P, w - 4 * P, 2 * P, *col);
        fill(&mut c, 2 * P, h - 4 * P - i * 2 * P, w - 4 * P, 2 * P, *col);
    }

    // Score row.
    let cell = 3;
    let row_y = 72;
    let [left, hi, credit] = row;
    shadow_text(&mut c, left, 12 * P, row_y, cell, pal.pink, pal.ink);
    shadow_text(
        &mut c,
        hi,
        (w - text_w(hi, cell)) / 2,
        row_y,
        cell,
        pal.cream,
        pal.ink,
    );
    shadow_text(
        &mut c,
        credit,
        w - 12 * P - text_w(credit, cell),
        row_y,
        cell,
        pal.cyan,
        pal.ink,
    );

    // The title, orange shadow under butter (`ty` is its vertical centre).
    let tx = (w - text_w(title, big)) / 2;
    let ty = 146;
    draw_text(
        &mut c,
        title,
        (tx + big) as f32,
        (ty + big) as f32,
        (big * 7) as f32,
        big as f32,
        pal.orange,
    );
    draw_text(
        &mut c,
        title,
        tx as f32,
        ty as f32,
        (big * 7) as f32,
        big as f32,
        pal.butter,
    );

    let sc = 4;
    shadow_text(
        &mut c,
        sub,
        (w - text_w(sub, sc)) / 2,
        228,
        sc,
        pal.cyan,
        pal.ink,
    );
    c
}
