//! Generates the built-in character avatars into `assets/avatars/`.
//!
//! Run from the repo root: `cargo run --bin genavatars`
//!
//! These are **placeholders by design**: simple, friendly faces in the app's
//! pastel palette, drawn procedurally so there's something good-looking until
//! real artwork exists. To replace one, overwrite `assets/avatars/<name>.png`
//! with any square PNG (256×256 or larger) and rebuild. The name list lives in
//! `core/src/characters.rs` and profiles store the *name*, so new art shows up
//! on every profile that uses it. Once an image has been hand-drawn, don't run
//! this generator again, or it will overwrite it.

use std::path::Path;

#[path = "../pixelart.rs"]
mod pixelart;
use pixelart::{Canvas, hex, shade};

const S: u32 = 256;
type Rgb = [f32; 3];
type Draw = fn(&mut Canvas);

/// Fill every pixel whose 4×4 sub-samples fall inside `inside`, with
/// anti-aliased coverage. `bbox` = (x0, y0, x1, y1) limits the scan.
fn fill(
    c: &mut Canvas,
    bbox: (f32, f32, f32, f32),
    rgb: Rgb,
    alpha: f32,
    inside: impl Fn(f32, f32) -> bool,
) {
    let (x0, y0, x1, y1) = bbox;
    for y in (y0.floor().max(0.0) as u32)..(y1.ceil().min(S as f32) as u32) {
        for x in (x0.floor().max(0.0) as u32)..(x1.ceil().min(S as f32) as u32) {
            let mut n = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    let fx = x as f32 + (sx as f32 + 0.5) / 4.0;
                    let fy = y as f32 + (sy as f32 + 0.5) / 4.0;
                    if inside(fx, fy) {
                        n += 1;
                    }
                }
            }
            if n > 0 {
                c.blend(x, y, rgb, alpha * n as f32 / 16.0);
            }
        }
    }
}

fn ellipse(c: &mut Canvas, cx: f32, cy: f32, rx: f32, ry: f32, rgb: Rgb) {
    fill(c, (cx - rx, cy - ry, cx + rx, cy + ry), rgb, 1.0, |x, y| {
        let (dx, dy) = ((x - cx) / rx, (y - cy) / ry);
        dx * dx + dy * dy <= 1.0
    });
}

fn circle(c: &mut Canvas, cx: f32, cy: f32, r: f32, rgb: Rgb) {
    ellipse(c, cx, cy, r, r, rgb);
}

fn soft_ellipse(c: &mut Canvas, cx: f32, cy: f32, rx: f32, ry: f32, rgb: Rgb, alpha: f32) {
    fill(
        c,
        (cx - rx, cy - ry, cx + rx, cy + ry),
        rgb,
        alpha,
        |x, y| {
            let (dx, dy) = ((x - cx) / rx, (y - cy) / ry);
            dx * dx + dy * dy <= 1.0
        },
    );
}

fn triangle(c: &mut Canvas, p: [(f32, f32); 3], rgb: Rgb) {
    let xs = [p[0].0, p[1].0, p[2].0];
    let ys = [p[0].1, p[1].1, p[2].1];
    let bbox = (
        xs.iter().cloned().fold(f32::MAX, f32::min),
        ys.iter().cloned().fold(f32::MAX, f32::min),
        xs.iter().cloned().fold(f32::MIN, f32::max),
        ys.iter().cloned().fold(f32::MIN, f32::max),
    );
    let edge = |a: (f32, f32), b: (f32, f32), x: f32, y: f32| {
        (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)
    };
    fill(c, bbox, rgb, 1.0, |x, y| {
        let (e0, e1, e2) = (
            edge(p[0], p[1], x, y),
            edge(p[1], p[2], x, y),
            edge(p[2], p[0], x, y),
        );
        (e0 >= 0.0 && e1 >= 0.0 && e2 >= 0.0) || (e0 <= 0.0 && e1 <= 0.0 && e2 <= 0.0)
    });
}

fn rrect(c: &mut Canvas, cx: f32, cy: f32, hw: f32, hh: f32, r: f32, rgb: Rgb) {
    fill(c, (cx - hw, cy - hh, cx + hw, cy + hh), rgb, 1.0, |x, y| {
        pixelart::sd_rrect(x, y, cx, cy, hw, hh, r) <= 0.0
    });
}

/// A lower-half arc (smile) centred at (cx, cy).
fn smile(c: &mut Canvas, cx: f32, cy: f32, r: f32, t: f32, rgb: Rgb) {
    fill(
        c,
        (cx - r - t, cy - t, cx + r + t, cy + r + t),
        rgb,
        1.0,
        |x, y| {
            let d = ((x - cx).powi(2) + (y - cy).powi(2)).sqrt();
            y > cy + r * 0.25 && (d - r).abs() <= t * 0.5
        },
    );
}

/// Two shiny eyes, rosy cheeks and a smile — shared by every character.
fn face(c: &mut Canvas, cx: f32, cy: f32, spread: f32, eye_r: f32) {
    let ink = hex("#4A443C");
    let white = hex("#FFFFFF");
    for side in [-1.0, 1.0] {
        let ex = cx + side * spread;
        circle(c, ex, cy, eye_r, ink);
        circle(c, ex - eye_r * 0.3, cy - eye_r * 0.35, eye_r * 0.32, white);
        soft_ellipse(
            c,
            cx + side * spread * 1.35,
            cy + eye_r * 2.4,
            eye_r * 1.3,
            eye_r * 0.8,
            hex("#F5A9BC"),
            0.55,
        );
    }
    smile(c, cx, cy + eye_r * 0.9, spread * 0.45, eye_r * 0.45, ink);
}

fn background(c: &mut Canvas, top: Rgb) {
    let bottom = shade(top, -0.10);
    for y in 0..S {
        let t = y as f32 / S as f32;
        let col = [
            top[0] + (bottom[0] - top[0]) * t,
            top[1] + (bottom[1] - top[1]) * t,
            top[2] + (bottom[2] - top[2]) * t,
        ];
        for x in 0..S {
            c.blend(x, y, col, 1.0);
        }
    }
}

fn fox(c: &mut Canvas) {
    background(c, hex("#FBF2E4"));
    let fur = hex("#F08A4B");
    let light = hex("#FFF4E6");
    triangle(c, [(58.0, 40.0), (112.0, 92.0), (60.0, 120.0)], fur);
    triangle(c, [(198.0, 40.0), (144.0, 92.0), (196.0, 120.0)], fur);
    triangle(
        c,
        [(66.0, 60.0), (100.0, 94.0), (68.0, 110.0)],
        hex("#4A443C"),
    );
    triangle(
        c,
        [(190.0, 60.0), (156.0, 94.0), (188.0, 110.0)],
        hex("#4A443C"),
    );
    ellipse(c, 128.0, 140.0, 84.0, 74.0, fur);
    ellipse(c, 128.0, 172.0, 52.0, 40.0, light);
    face(c, 128.0, 136.0, 32.0, 9.0);
    ellipse(c, 128.0, 152.0, 9.0, 6.5, hex("#4A443C"));
}

fn cat(c: &mut Canvas) {
    background(c, hex("#EFE8FA"));
    let fur = hex("#B9A3E3");
    triangle(c, [(56.0, 52.0), (106.0, 86.0), (62.0, 126.0)], fur);
    triangle(c, [(200.0, 52.0), (150.0, 86.0), (194.0, 126.0)], fur);
    triangle(
        c,
        [(66.0, 70.0), (96.0, 90.0), (70.0, 112.0)],
        hex("#F5A9BC"),
    );
    triangle(
        c,
        [(190.0, 70.0), (160.0, 90.0), (186.0, 112.0)],
        hex("#F5A9BC"),
    );
    ellipse(c, 128.0, 146.0, 86.0, 76.0, fur);
    face(c, 128.0, 140.0, 34.0, 9.5);
    triangle(
        c,
        [(120.0, 152.0), (136.0, 152.0), (128.0, 161.0)],
        hex("#F5A9BC"),
    );
    let ink = hex("#4A443C");
    for (y, dy) in [(160.0, -4.0), (168.0, 4.0)] {
        fill(c, (40.0, y - 6.0, 96.0, y + 6.0), ink, 1.0, move |x, yy| {
            let t = (x - 40.0) / 56.0;
            (yy - (y + dy * (1.0 - t))).abs() <= 1.4
        });
        fill(
            c,
            (160.0, y - 6.0, 216.0, y + 6.0),
            ink,
            1.0,
            move |x, yy| {
                let t = (216.0 - x) / 56.0;
                (yy - (y + dy * (1.0 - t))).abs() <= 1.4
            },
        );
    }
}

fn bear(c: &mut Canvas) {
    background(c, hex("#FBEFE2"));
    let fur = hex("#B78B6A");
    circle(c, 66.0, 70.0, 30.0, fur);
    circle(c, 190.0, 70.0, 30.0, fur);
    circle(c, 66.0, 70.0, 15.0, hex("#E7C9AE"));
    circle(c, 190.0, 70.0, 15.0, hex("#E7C9AE"));
    ellipse(c, 128.0, 142.0, 88.0, 80.0, fur);
    ellipse(c, 128.0, 170.0, 42.0, 32.0, hex("#E7C9AE"));
    face(c, 128.0, 132.0, 34.0, 9.0);
    ellipse(c, 128.0, 158.0, 11.0, 8.0, hex("#4A443C"));
}

fn frog(c: &mut Canvas) {
    background(c, hex("#E8F5EC"));
    let skin = hex("#8FCB8A");
    circle(c, 82.0, 78.0, 34.0, skin);
    circle(c, 174.0, 78.0, 34.0, skin);
    ellipse(c, 128.0, 150.0, 96.0, 70.0, skin);
    let ink = hex("#4A443C");
    for x in [82.0, 174.0] {
        circle(c, x, 76.0, 20.0, hex("#FFFFFF"));
        circle(c, x, 78.0, 11.0, ink);
        circle(c, x - 4.0, 73.0, 4.0, hex("#FFFFFF"));
    }
    soft_ellipse(c, 70.0, 160.0, 16.0, 10.0, hex("#F5A9BC"), 0.6);
    soft_ellipse(c, 186.0, 160.0, 16.0, 10.0, hex("#F5A9BC"), 0.6);
    smile(c, 128.0, 138.0, 44.0, 5.0, ink);
}

fn owl(c: &mut Canvas) {
    background(c, hex("#E6F2FB"));
    let feathers = hex("#8FB9DE");
    triangle(c, [(60.0, 44.0), (98.0, 78.0), (62.0, 96.0)], feathers);
    triangle(c, [(196.0, 44.0), (158.0, 78.0), (194.0, 96.0)], feathers);
    ellipse(c, 128.0, 146.0, 86.0, 86.0, feathers);
    ellipse(c, 128.0, 186.0, 50.0, 34.0, hex("#DCEBF7"));
    let ink = hex("#4A443C");
    for x in [92.0, 164.0] {
        circle(c, x, 122.0, 30.0, hex("#FFFFFF"));
        circle(c, x, 124.0, 14.0, ink);
        circle(c, x - 5.0, 118.0, 5.0, hex("#FFFFFF"));
    }
    triangle(
        c,
        [(118.0, 146.0), (138.0, 146.0), (128.0, 164.0)],
        hex("#F5C35A"),
    );
}

fn bunny(c: &mut Canvas) {
    background(c, hex("#FCEDF1"));
    let fur = hex("#FFFFFF");
    let pink = hex("#F5A9BC");
    rrect(c, 96.0, 58.0, 18.0, 52.0, 18.0, fur);
    rrect(c, 160.0, 58.0, 18.0, 52.0, 18.0, fur);
    rrect(c, 96.0, 62.0, 8.0, 38.0, 8.0, pink);
    rrect(c, 160.0, 62.0, 8.0, 38.0, 8.0, pink);
    ellipse(c, 128.0, 158.0, 80.0, 70.0, fur);
    face(c, 128.0, 150.0, 30.0, 9.0);
    ellipse(c, 128.0, 164.0, 7.0, 5.0, pink);
}

fn robot(c: &mut Canvas) {
    background(c, hex("#FBF6E1"));
    let metal = hex("#C9C2B4");
    let ink = hex("#4A443C");
    rrect(c, 128.0, 44.0, 4.0, 22.0, 2.0, ink);
    circle(c, 128.0, 26.0, 11.0, hex("#FF6A1A"));
    rrect(c, 128.0, 146.0, 88.0, 80.0, 28.0, metal);
    rrect(c, 40.0, 146.0, 10.0, 26.0, 6.0, shade(metal, -0.2));
    rrect(c, 216.0, 146.0, 10.0, 26.0, 6.0, shade(metal, -0.2));
    rrect(c, 128.0, 134.0, 64.0, 36.0, 16.0, hex("#2C2922"));
    for x in [100.0, 156.0] {
        circle(c, x, 134.0, 12.0, hex("#F5E29E"));
    }
    rrect(c, 128.0, 196.0, 30.0, 8.0, 4.0, ink);
}

fn dino(c: &mut Canvas) {
    background(c, hex("#EAF6EF"));
    let skin = hex("#9ED6B5");
    for (x, y) in [(88.0, 70.0), (128.0, 58.0), (168.0, 70.0)] {
        triangle(
            c,
            [(x - 16.0, y + 20.0), (x, y - 14.0), (x + 16.0, y + 20.0)],
            hex("#F5E29E"),
        );
    }
    ellipse(c, 128.0, 148.0, 90.0, 80.0, skin);
    ellipse(c, 128.0, 180.0, 56.0, 30.0, hex("#CDEBD9"));
    face(c, 128.0, 138.0, 34.0, 9.5);
    for x in [110.0, 146.0] {
        circle(c, x, 176.0, 3.5, shade(skin, -0.35));
    }
}

fn main() {
    let dir = Path::new("assets/avatars");
    std::fs::create_dir_all(dir).expect("create assets/avatars");
    let chars: [(&str, Draw); 8] = [
        ("fox", fox),
        ("cat", cat),
        ("bear", bear),
        ("frog", frog),
        ("owl", owl),
        ("bunny", bunny),
        ("robot", robot),
        ("dino", dino),
    ];
    for (name, draw) in chars {
        let mut c = Canvas::new(S, S);
        draw(&mut c);
        c.save(&dir.join(format!("{name}.png")));
        println!("  {name}.png");
    }
    println!("done.");
}
