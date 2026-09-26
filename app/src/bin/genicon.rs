//! Generates the app icon into `assets/icon/rondelek.png` (512×512).
//!
//! Run from the repo root: `cargo run --bin genicon`
//!
//! Same procedural pixel-art engine as the banner (`pixelart`): one flat matte
//! keycap in the accent orange, a cream 5×7 pixel "R" on its face and a small
//! cream REC dot. The PNG is embedded as the window icon of both binaries and
//! is the icon of the Linux AppImage (`packaging/appimage`).

use std::path::Path;

#[path = "../pixelart.rs"]
mod pixelart;
use pixelart::{Canvas, cov, draw_label, hex, sd_rrect, shade};

const S: u32 = 512;

fn main() {
    let mut c = Canvas::new(S, S);

    let orange = hex("#FF6A1A");
    let wall = shade(orange, -0.22);
    let cream = hex("#FFF3E8");

    // Keycap: a darker body (the wall), with the face sitting high on it.
    let (cx, cy) = (S as f32 * 0.5, S as f32 * 0.5);
    let hw = 236.0;
    let rim_r = 104.0;
    let face_cy = cy - 20.0;
    let face_hw = hw - 26.0;
    let face_hh = hw - 42.0;
    for py in 0..S {
        for px in 0..S {
            let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
            let a = cov(sd_rrect(fx, fy, cx, cy, hw, hw, rim_r));
            if a <= 0.0 {
                continue;
            }
            c.blend(px, py, wall, a);
            let fa = cov(sd_rrect(
                fx,
                fy,
                cx,
                face_cy,
                face_hw,
                face_hh,
                rim_r * 0.85,
            ));
            if fa > 0.0 {
                // Matte: a touch lighter at the top of the face.
                let t = (fy - (face_cy - face_hh)) / (2.0 * face_hh);
                c.blend(px, py, shade(orange, 0.08 - 0.12 * t), fa);
            }
        }
    }

    // The wordmark's first letter, big and pixel-crisp.
    draw_label(&mut c, 'R', cx - 6.0, face_cy + 6.0, 252.0, cream);

    // The REC dot, top right of the face.
    let (dx, dy, r) = (cx + 142.0, face_cy - 136.0, 24.0);
    for py in 0..S {
        for px in 0..S {
            let d = ((px as f32 + 0.5 - dx).powi(2) + (py as f32 + 0.5 - dy).powi(2)).sqrt();
            c.blend(px, py, cream, cov(d - r));
        }
    }

    let dir = Path::new("assets/icon");
    std::fs::create_dir_all(dir).expect("create icon dir");
    c.save(&dir.join("rondelek.png"));
    println!("done.");
}
