//! Generates the built-in "base" skin into `skins/base/`.
//!
//! Run from the repo root: `cargo run --bin genskin`
//!
//! The whole faceplate is painted into ONE spritesheet, `skin.png`, at the
//! fixed regions defined in `rondelek_core::config::atlas`. A designer edits
//! that single file (stack the elements in any image editor); the app slices
//! the same regions back out. This binary is also the style reference: a flat,
//! matte, light "plastic" look echoing the Teenage Engineering EP-133 — neutral
//! caps with printed key labels, one orange accent for REC.

use rondelek_core::config::atlas::{self, Sprite};
use std::path::Path;

#[path = "../pixelart.rs"]
mod pixelart;
use pixelart::{Canvas, cov, draw_label, hex, sd_rrect, shade, smoothstep};

// ---- faceplate pieces (painted at their atlas offsets) -------------------

/// Flat window background with a barely-there vertical gradient.
fn gen_background(c: &mut Canvas, s: Sprite) {
    let top = hex("#F3F0E8");
    let bottom = hex("#EAE6DC");
    for y in 0..s.h {
        let t = y as f32 / s.h as f32;
        let col = [
            top[0] + (bottom[0] - top[0]) * t,
            top[1] + (bottom[1] - top[1]) * t,
            top[2] + (bottom[2] - top[2]) * t,
        ];
        for x in 0..s.w {
            c.blend(s.x + x, s.y + y, col, 1.0);
        }
    }
}

/// The sampler case: nine-sliced by the app, so all shading is edge-local and
/// the stretched centre stays a flat matte panel.
fn gen_case(c: &mut Canvas, s: Sprite) {
    let base = hex("#E5E1D7");
    let (cx, cy, hw) = (256.0f32, 256.0, 248.0);
    let radius = 40.0;
    for y in 0..s.h {
        for x in 0..s.w {
            let sd = sd_rrect(x as f32 + 0.5, y as f32 + 0.5, cx, cy, hw, hw, radius);
            let a = cov(sd);
            if a <= 0.0 {
                continue;
            }
            // Gentle top light + a soft edge shade for a moulded-but-flat look.
            let toplight = smoothstep(80.0, 0.0, y as f32) * 0.05;
            let vign = smoothstep(-52.0, -4.0, sd) * 0.06;
            c.blend(s.x + x, s.y + y, shade(base, toplight - vign), a);
            // Thin crisp rim right at the silhouette.
            let line = smoothstep(-3.0, -0.5, sd) * cov(sd + 0.5);
            c.blend(s.x + x, s.y + y, shade(base, -0.22), line * 0.5);
        }
    }
}

/// Dark recessed screen frame. Nine-sliced with a thin inset, so the visualizer
/// fills the flat centre and only the frame ring shows.
fn gen_bezel(c: &mut Canvas, s: Sprite) {
    let base = hex("#26231E");
    let (cc, hw, radius) = (192.0f32, 188.0, 22.0);
    for y in 0..s.h {
        for x in 0..s.w {
            let sd = sd_rrect(x as f32 + 0.5, y as f32 + 0.5, cc, cc, hw, hw, radius);
            let a = cov(sd);
            if a <= 0.0 {
                continue;
            }
            // Inner shading reads as a recess; outer edge catches a little light.
            let inner = smoothstep(-30.0, -2.0, sd) * 0.12;
            let toplight = smoothstep(40.0, 0.0, y as f32) * 0.05;
            c.blend(s.x + x, s.y + y, shade(base, toplight - inner), a);
        }
    }
}

/// Light ring around the child's photo (centre kept transparent).
fn gen_avatar_frame(c: &mut Canvas, s: Sprite) {
    let base = hex("#DCD7CA");
    for y in 0..s.h {
        for x in 0..s.w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let sd_out = sd_rrect(fx, fy, 128.0, 128.0, 122.0, 122.0, 30.0);
            let sd_in = sd_rrect(fx, fy, 128.0, 128.0, 102.0, 102.0, 20.0);
            let a = cov(sd_out) * (1.0 - cov(sd_in));
            if a <= 0.0 {
                continue;
            }
            let vign = smoothstep(-8.0, -1.0, sd_out) * 0.10;
            c.blend(s.x + x, s.y + y, shade(base, -vign), a);
        }
    }
}

/// A flat matte keycap: a raised light face with a thin darker wall below, so
/// it reads as slightly 3-D without gloss. `icon` paints on top of the face.
fn gen_cap(c: &mut Canvas, s: Sprite, face: [f32; 3], icon: impl Fn(&mut Canvas, u32, u32)) {
    let m = 14.0f32; // transparent margin
    let wall = shade(face, -0.16);
    let (cc, hw) = (128.0f32, 128.0 - m);
    let rim_r = 26.0;

    // Face sits a touch high, leaving an ~8 px wall at the bottom.
    let (cap_top, cap_bot) = (m, 256.0 - m - 8.0);
    let cap_cx = 128.0;
    let cap_cy = (cap_top + cap_bot) * 0.5;
    let cap_hw = hw - 8.0;
    let cap_hh = (cap_bot - cap_top) * 0.5;
    let cap_r = 22.0;

    for y in 0..s.h {
        for x in 0..s.w {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let a = cov(sd_rrect(fx, fy, cc, cc, hw, hw, rim_r));
            if a <= 0.0 {
                continue;
            }
            c.blend(s.x + x, s.y + y, wall, a);

            let ca = cov(sd_rrect(fx, fy, cap_cx, cap_cy, cap_hw, cap_hh, cap_r));
            if ca > 0.0 {
                // Matte vertical shading only: lighter at top, no gloss band.
                let t = (fy - cap_top) / (cap_bot - cap_top);
                c.blend(s.x + x, s.y + y, shade(face, 0.05 - 0.10 * t), ca);
            }
        }
    }
    icon(c, s.x, s.y);
}

// ---- main ----------------------------------------------------------------

fn main() {
    let dir = Path::new("skins/base");
    std::fs::create_dir_all(dir).expect("create skin dir");

    let mut c = Canvas::new(atlas::ATLAS_W, atlas::ATLAS_H);

    gen_background(&mut c, atlas::BG);
    gen_case(&mut c, atlas::CASE);
    gen_bezel(&mut c, atlas::BEZEL);
    gen_avatar_frame(&mut c, atlas::AVATAR);

    // Twelve uniform light sample caps, printed with the keyboard key labels.
    let face = hex("#F1ECE1");
    let ink = hex("#2A2622");
    let labels = ['1', '2', '3', '4', 'Q', 'W', 'E', 'R', 'A', 'S', 'D', 'F'];
    for (i, ch) in labels.iter().enumerate() {
        let ch = *ch;
        gen_cap(&mut c, atlas::cap(i), face, move |c, ox, oy| {
            draw_label(c, ch, ox as f32 + 128.0, oy as f32 + 122.0, 96.0, ink);
        });
    }

    // REC: the one orange accent, with a record dot.
    let cream = hex("#FFF3E8");
    gen_cap(&mut c, atlas::cap(atlas::REC_CAP), hex("#FF6A1A"), |c, ox, oy| {
        let (cx, cy) = (ox as f32 + 128.0, oy as f32 + 118.0);
        for y in 0..256u32 {
            for x in 0..256u32 {
                let d = ((ox as f32 + x as f32 + 0.5 - cx).powi(2)
                    + (oy as f32 + y as f32 + 0.5 - cy).powi(2))
                .sqrt();
                c.blend(ox + x, oy + y, cream, cov(d - 34.0));
            }
        }
    });

    // BACK: dark cap with a left-chevron (back to profiles).
    let dark = hex("#2E2A24");
    gen_cap(&mut c, atlas::cap(atlas::BACK_CAP), dark, |c, ox, oy| {
        draw_chevron(c, ox as f32 + 128.0, oy as f32 + 118.0, 46.0, cream);
    });

    // CYCLE: dark cap with three ascending bars (cycle the visualizer).
    gen_cap(&mut c, atlas::cap(atlas::CYCLE_CAP), dark, |c, ox, oy| {
        draw_bars(c, ox, oy, cream);
    });

    c.save(&dir.join("skin.png"));
    write_skin_json(dir);
    println!("done.");
}

/// A left-pointing chevron, centred at (cx, cy), half-height `s`.
fn draw_chevron(c: &mut Canvas, cx: f32, cy: f32, s: f32, rgb: [f32; 3]) {
    let thick = s * 0.34;
    for y in ((cy - s) as u32)..((cy + s) as u32) {
        for x in ((cx - s * 0.7) as u32)..((cx + s * 0.7) as u32) {
            // Point the chevron left (back): tip at the −x side.
            let (dx, dy) = (-(x as f32 + 0.5 - cx), y as f32 + 0.5 - cy);
            let d = (dx + dy.abs()).abs() / std::f32::consts::SQRT_2 - thick * 0.5;
            let inside = dy.abs() <= s && dx + dy.abs() <= s * 0.7;
            if inside {
                c.blend(x, y, rgb, cov(d));
            }
        }
    }
}

/// Three ascending bars filling a cap face at (ox, oy).
fn draw_bars(c: &mut Canvas, ox: u32, oy: u32, rgb: [f32; 3]) {
    let bars = [(94.0f32, 150.0f32), (128.0, 118.0), (162.0, 90.0)];
    for (bx, top) in bars {
        for y in (top as u32)..168u32 {
            for x in ((bx - 15.0) as u32)..((bx + 15.0) as u32) {
                c.blend(ox + x, oy + y, rgb, 1.0);
            }
        }
    }
}

/// Base palette overrides, mirroring `theme_light` so the file documents every
/// tweakable colour for skin authors.
fn write_skin_json(dir: &Path) {
    let json = r##"{
  "name": "Base",
  "author": "Rondelek",
  "colors": {
    "panel_bg": "#E6E1D5",
    "panel_fg": "#D5D0C2",
    "pad_play_bg": "#F3EEE4",
    "pad_play_fg": "#26231E",
    "pad_record_bg": "#FF6A1A",
    "pad_record_fg": "#FFFFFF",
    "pad_function_bg": "#2E2A24",
    "pad_function_fg": "#FF6A1A",
    "led_empty": "#BEB8AA",
    "led_full": "#FF6A1A",
    "case_shadow": "#00000030",
    "case_border": "#CFC9BB",
    "text_primary": "#26231E",
    "text_secondary": "#8A8475",
    "visualizer_bg": "#1A1814",
    "visualizer_dot_off": "#2C2922",
    "visualizer_bar_low": "#B85A12",
    "visualizer_bar_mid": "#FF6A1A",
    "visualizer_bar_high": "#FFC06A"
  }
}
"##;
    let path = dir.join("skin.json");
    std::fs::write(&path, json).expect("write skin.json");
    println!("wrote {}", path.display());
}
