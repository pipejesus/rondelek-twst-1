//! "Who's playing?" for the games: a reusable pre-start screen that lets a
//! child (or a grown-up) pick whose voice calibration to use.
//!
//! The sampler app always passes `--profile` when it launches a game, so this
//! screen only appears when a game is started on its own (double-clicked, or
//! run without arguments). It reads the same profile library as the app and
//! mirrors the app's Home screen: a pastel tile per child (same colour as in
//! the app, via `rondelek_core::util::stable_pick`), their photo or character,
//! their name, and a small badge showing whether their voice check is done.
//!
//! Using it from a game is one call:
//!
//! ```ignore
//! match profile_picker::choose_child(&mut rl, &thread, &PickerOptions::default()) {
//!     Pick::Child(profile) => { /* use profile.load_calibration() */ }
//!     Pick::NoChildren => { /* library empty: play with the keyboard */ }
//!     Pick::Closed => return Ok(()),
//! }
//! ```
//!
//! [`crate::run`] already does this for every registered game.
//!
//! Controls: click/tap a tile, or arrow keys + Enter/Space. The mouse wheel
//! scrolls when there are more children than fit. Text-free apart from the
//! children's names, which are drawn with the bundled Space Grotesk font
//! (loaded with exactly the letters the names need, so Polish or Ukrainian
//! names render correctly).

use raylib::prelude::*;

use rondelek_core::profile::{self, Profile};

use crate::{BUTTER, CHARCOAL, CREAM, LILAC, MINT, PEACH, ROSE, SKY, snap};

/// Tile colours in the same order as the app's `ui::shell::tile_color`, so a
/// child's colour matches everywhere.
const TILE_COLORS: [Color; 5] = [SKY, LILAC, BUTTER, MINT, ROSE];
const OK_GREEN: Color = Color::new(0x3C, 0xB0, 0x4B, 255);
const ORANGE: Color = Color::new(0xFF, 0x6A, 0x1A, 255);
const PAPER: Color = Color::new(255, 252, 247, 255);

const FONT_TTF: &[u8] = include_bytes!("../../assets/fonts/SpaceGrotesk.ttf");

/// What the picker returns.
pub enum Pick {
    /// A child was chosen.
    Child(Profile),
    /// The library is empty (no child added in the app yet): carry on without
    /// a profile. The keyboard still works; voice detection stays idle.
    NoChildren,
    /// The window was closed (Esc or the close button).
    Closed,
}

/// Knobs for the smoke-test harness; games normally use the default.
#[derive(Default)]
pub struct PickerOptions<'a> {
    /// Render this many frames, then return [`Pick::Closed`] (headless runs).
    pub harness_frames: Option<u64>,
    /// With `harness_frames`: save a screenshot here on the last frame.
    pub shot: Option<&'a str>,
}

struct Entry {
    profile: Profile,
    picture: Option<Texture2D>,
    color: Color,
    calibrated: bool,
}

/// Show the picker until a child is chosen or the window closes. Needs an open
/// raylib window. Loads every child's picture once, up front.
pub fn choose_child(rl: &mut RaylibHandle, thread: &RaylibThread, opts: &PickerOptions) -> Pick {
    let profiles = profile::list_profiles();
    if profiles.is_empty() && opts.harness_frames.is_none() {
        return Pick::NoChildren;
    }

    let names: String = profiles.iter().map(|p| p.name()).collect();
    let font = load_font(rl, thread, &names);
    let mut entries: Vec<Entry> = profiles
        .into_iter()
        .map(|p| Entry {
            picture: load_picture(rl, thread, &p),
            color: TILE_COLORS
                [rondelek_core::util::stable_pick(&p.manifest.uid, TILE_COLORS.len())],
            calibrated: p.load_calibration().is_some(),
            profile: p,
        })
        .collect();

    let mut focus = 0usize;
    let mut scroll = 0.0f32;
    let mut last_mouse = rl.get_mouse_position();
    let mut frame = 0u64;

    while !rl.window_should_close() {
        let (w, h) = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
        let grid = Grid::new(entries.len(), w, h);
        scroll =
            (scroll - rl.get_mouse_wheel_move() * grid.tile_h * 0.5).clamp(0.0, grid.max_scroll());

        // --- input ---
        let mouse = rl.get_mouse_position();
        let moved = mouse.x != last_mouse.x || mouse.y != last_mouse.y;
        last_mouse = mouse;
        let hovered =
            (0..entries.len()).find(|&i| grid.rect(i, scroll).check_collision_point_rec(mouse));
        if moved && let Some(i) = hovered {
            focus = i;
        }
        let mut chosen = None;
        if rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT) && hovered.is_some() {
            chosen = hovered;
        }
        if let Some(step) = arrow_step(rl, grid.cols) {
            focus = move_focus(focus, step, entries.len());
            scroll = grid.scroll_to_show(focus, scroll, h);
        }
        if (rl.is_key_pressed(KeyboardKey::KEY_ENTER) || rl.is_key_pressed(KeyboardKey::KEY_SPACE))
            && !entries.is_empty()
        {
            chosen = Some(focus);
        }
        if let Some(i) = chosen {
            return Pick::Child(entries.swap_remove(i).profile);
        }

        // --- draw ---
        {
            let pressed = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
            let mut d = rl.begin_drawing(thread);
            d.clear_background(CREAM);
            d.draw_rectangle_gradient_v(0, 0, w as i32, h as i32, CREAM, PEACH);
            draw_header(&mut d, font.as_ref(), w, grid.s);
            for (i, e) in entries.iter().enumerate() {
                let r = grid.rect(i, scroll);
                if r.y + r.height < 0.0 || r.y > h {
                    continue;
                }
                let lit = hovered == Some(i) || focus == i;
                draw_tile(
                    &mut d,
                    font.as_ref(),
                    e,
                    r,
                    grid.s,
                    lit,
                    lit && pressed && hovered == Some(i),
                    focus == i,
                );
            }
        }

        frame += 1;
        if let Some(max) = opts.harness_frames {
            if let Some(path) = opts.shot
                && frame == max.saturating_sub(1)
            {
                snap(rl, thread, path);
            }
            if frame >= max {
                return Pick::Closed;
            }
        }
    }
    Pick::Closed
}

// ---- layout (pure, unit-tested) ---------------------------------------------

/// Where the tiles go for `n` children in a `w`×`h` window.
#[derive(Debug, Clone, Copy)]
struct Grid {
    /// Scale relative to the 1280×720 design size.
    s: f32,
    cols: usize,
    rows: usize,
    tile_w: f32,
    tile_h: f32,
    gap: f32,
    left: f32,
    top: f32,
    /// Height available for tiles (below the header).
    view_h: f32,
}

impl Grid {
    fn new(n: usize, w: f32, h: f32) -> Self {
        let s = (w / 1280.0).min(h / 720.0).clamp(0.4, 3.0);
        let tile_w = 200.0 * s;
        let tile_h = tile_w + 54.0 * s;
        let gap = 28.0 * s;
        let margin = 48.0 * s;
        let fit = ((w - 2.0 * margin + gap) / (tile_w + gap)).floor().max(1.0) as usize;
        let cols = fit.min(n.max(1));
        let rows = n.div_ceil(cols).max(1);
        let grid_w = cols as f32 * tile_w + (cols - 1) as f32 * gap;
        let header = 170.0 * s;
        let view_h = (h - header - margin).max(tile_h);
        let content_h = rows as f32 * tile_h + (rows - 1) as f32 * gap;
        // Centre vertically when everything fits; otherwise start under the header.
        let top = header + ((view_h - content_h) / 2.0).max(0.0);
        Self {
            s,
            cols,
            rows,
            tile_w,
            tile_h,
            gap,
            left: (w - grid_w) / 2.0,
            top,
            view_h,
        }
    }

    fn rect(&self, i: usize, scroll: f32) -> Rectangle {
        let (row, col) = (i / self.cols, i % self.cols);
        Rectangle {
            x: self.left + col as f32 * (self.tile_w + self.gap),
            y: self.top + row as f32 * (self.tile_h + self.gap) - scroll,
            width: self.tile_w,
            height: self.tile_h,
        }
    }

    fn content_h(&self) -> f32 {
        self.rows as f32 * self.tile_h + (self.rows - 1) as f32 * self.gap
    }

    fn max_scroll(&self) -> f32 {
        (self.content_h() - self.view_h).max(0.0)
    }

    /// Adjust `scroll` so tile `i` is fully visible in a window `h` tall.
    fn scroll_to_show(&self, i: usize, scroll: f32, h: f32) -> f32 {
        let r = self.rect(i, 0.0);
        let bottom_limit = h - 24.0 * self.s;
        let mut s = scroll;
        if r.y - s < self.top.min(170.0 * self.s) {
            s = r.y - self.top.min(170.0 * self.s);
        } else if r.y + r.height - s > bottom_limit {
            s = r.y + r.height - bottom_limit;
        }
        s.clamp(0.0, self.max_scroll())
    }
}

/// Arrow keys as a signed index step (left/right = ±1, up/down = ±cols).
fn arrow_step(rl: &RaylibHandle, cols: usize) -> Option<isize> {
    let c = cols as isize;
    [
        (KeyboardKey::KEY_RIGHT, 1),
        (KeyboardKey::KEY_LEFT, -1),
        (KeyboardKey::KEY_DOWN, c),
        (KeyboardKey::KEY_UP, -c),
    ]
    .into_iter()
    .find(|(k, _)| rl.is_key_pressed(*k))
    .map(|(_, step)| step)
}

/// Move the keyboard focus, staying inside `0..n` (no wrap-around).
fn move_focus(focus: usize, step: isize, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let next = focus as isize + step;
    if (0..n as isize).contains(&next) {
        next as usize
    } else {
        focus
    }
}

/// ASCII plus every letter used in the children's names, for the font atlas.
fn font_codepoints(names: &str) -> String {
    let mut chars: Vec<char> = (' '..='~').chain(names.chars()).chain(['…']).collect();
    chars.sort_unstable();
    chars.dedup();
    chars.into_iter().collect()
}

/// Shorten a long name to fit a tile.
fn fit_name(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        let mut s: String = name.chars().take(max - 1).collect();
        s.push('…');
        s
    }
}

// ---- resources ------------------------------------------------------------------

/// The bundled font with exactly the letters needed. `None` only if loading
/// fails, in which case raylib's built-in (ASCII) font is used instead.
fn load_font(rl: &mut RaylibHandle, thread: &RaylibThread, names: &str) -> Option<Font> {
    let chars = font_codepoints(names);
    let font = rl
        .load_font_from_memory(thread, ".ttf", FONT_TTF, 72, Some(&chars))
        .ok()?;
    font.texture()
        .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
    Some(font)
}

/// Measure `text` at `size` px with the picker font (or the fallback).
fn measure(d: &RaylibDrawHandle, font: Option<&Font>, text: &str, size: f32) -> Vector2 {
    match font {
        Some(f) => f.measure_text(text, size, 0.0),
        None => Vector2::new(d.measure_text(text, size as i32) as f32, size),
    }
}

/// Draw `text` with its top-left at `pos`.
fn text(
    d: &mut RaylibDrawHandle,
    font: Option<&Font>,
    text: &str,
    pos: Vector2,
    size: f32,
    color: Color,
) {
    match font {
        Some(f) => d.draw_text_ex(f, text, pos, size, 0.0, color),
        None => d.draw_text(text, pos.x as i32, pos.y as i32, size as i32, color),
    }
}

/// The child's photo, else their character, else `None` (a drawn face).
fn load_picture(rl: &mut RaylibHandle, thread: &RaylibThread, p: &Profile) -> Option<Texture2D> {
    let image = match p.avatar_path() {
        Some(path) => Image::load_image(&path.to_string_lossy()).ok(),
        None => p
            .character()
            .and_then(rondelek_core::characters::png)
            .and_then(|bytes| Image::load_image_from_mem(".png", bytes).ok()),
    }?;
    let mut tex = rl.load_texture_from_image(thread, &image).ok()?;
    tex.gen_texture_mipmaps();
    tex.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_TRILINEAR);
    Some(tex)
}

// ---- drawing ----------------------------------------------------------------------

fn shade(c: Color, amount: f32) -> Color {
    let t = amount.abs().clamp(0.0, 1.0);
    let target = if amount >= 0.0 { 255.0 } else { 0.0 };
    let mix = |v: u8| (v as f32 + (target - v as f32) * t).round() as u8;
    Color::new(mix(c.r), mix(c.g), mix(c.b), c.a)
}

/// A big "?" in a butter bubble: "who's playing?" without words.
fn draw_header(d: &mut RaylibDrawHandle, font: Option<&Font>, w: f32, s: f32) {
    let c = Vector2::new(w / 2.0, 92.0 * s);
    d.draw_circle_v(
        Vector2::new(c.x, c.y + 4.0 * s),
        50.0 * s,
        shade(BUTTER, -0.18),
    );
    d.draw_circle_v(c, 50.0 * s, BUTTER);
    let size = 70.0 * s;
    let m = measure(d, font, "?", size);
    text(
        d,
        font,
        "?",
        Vector2::new(c.x - m.x / 2.0, c.y - m.y / 2.0),
        size,
        CHARCOAL,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_tile(
    d: &mut RaylibDrawHandle,
    font: Option<&Font>,
    e: &Entry,
    r: Rectangle,
    s: f32,
    lit: bool,
    pressed: bool,
    focused: bool,
) {
    let lip = 9.0 * s;
    let round = 0.16;
    // Shadow, lip, face (the face sinks onto the lip while pressed).
    let shadow = Rectangle {
        y: r.y + 4.0 * s,
        ..r
    };
    d.draw_rectangle_rounded(shadow, round, 8, Color::new(0, 0, 0, 22));
    d.draw_rectangle_rounded(r, round, 8, shade(e.color, -0.18));
    let sink = if pressed { lip * 0.75 } else { 0.0 };
    let face = Rectangle {
        x: r.x,
        y: r.y + sink,
        width: r.width,
        height: r.height - lip,
    };
    d.draw_rectangle_rounded(
        face,
        round,
        8,
        if lit { shade(e.color, 0.12) } else { e.color },
    );
    if focused {
        let ring = Rectangle {
            x: r.x - 5.0 * s,
            y: r.y - 5.0 * s,
            width: r.width + 10.0 * s,
            height: r.height + 10.0 * s,
        };
        d.draw_rectangle_rounded_lines_ex(ring, round, 8, 3.5 * s, ORANGE);
    }

    // Picture.
    let pad = 14.0 * s;
    let pic = Rectangle {
        x: face.x + pad,
        y: face.y + pad,
        width: face.width - 2.0 * pad,
        height: face.width - 2.0 * pad,
    };
    match &e.picture {
        Some(tex) => {
            let src = Rectangle::new(0.0, 0.0, tex.width as f32, tex.height as f32);
            d.draw_rectangle_rounded(pic, 0.12, 8, PAPER);
            d.draw_texture_pro(tex, src, pic, Vector2::zero(), 0.0, Color::WHITE);
        }
        None => draw_default_face(d, pic, e.color),
    }

    // Name.
    let size = 26.0 * s;
    let name = fit_name(e.profile.name(), 14);
    let m = measure(d, font, &name, size);
    let name_y = (pic.y + pic.height + face.y + face.height) / 2.0 - m.y / 2.0;
    let pos = Vector2::new(face.x + (face.width - m.x) / 2.0, name_y);
    text(d, font, &name, pos, size, CHARCOAL);

    // Voice-check badge (for the grown-ups).
    let bc = Vector2::new(pic.x + pic.width - 8.0 * s, pic.y + 8.0 * s);
    let br = 16.0 * s;
    d.draw_circle_v(
        Vector2::new(bc.x, bc.y + 1.5 * s),
        br,
        Color::new(0, 0, 0, 30),
    );
    d.draw_circle_v(bc, br + 2.0 * s, PAPER);
    if e.calibrated {
        d.draw_circle_v(bc, br, OK_GREEN);
        let t = 3.0 * s;
        d.draw_line_ex(
            Vector2::new(bc.x - 7.0 * s, bc.y),
            Vector2::new(bc.x - 2.0 * s, bc.y + 5.0 * s),
            t,
            Color::WHITE,
        );
        d.draw_line_ex(
            Vector2::new(bc.x - 2.0 * s, bc.y + 5.0 * s),
            Vector2::new(bc.x + 7.0 * s, bc.y - 5.0 * s),
            t,
            Color::WHITE,
        );
    } else {
        d.draw_circle_v(bc, br, BUTTER);
        let mic = Rectangle {
            x: bc.x - 3.5 * s,
            y: bc.y - 8.0 * s,
            width: 7.0 * s,
            height: 11.0 * s,
        };
        d.draw_rectangle_rounded(mic, 1.0, 6, CHARCOAL);
        d.draw_ring(bc, 6.0 * s, 7.8 * s, 0.0, 180.0, 12, CHARCOAL);
        d.draw_line_ex(
            Vector2::new(bc.x, bc.y + 7.0 * s),
            Vector2::new(bc.x, bc.y + 10.0 * s),
            1.8 * s,
            CHARCOAL,
        );
    }
}

/// A friendly drawn face for children without a picture (matches the app's).
fn draw_default_face(d: &mut RaylibDrawHandle, r: Rectangle, bg: Color) {
    d.draw_rectangle_rounded(r, 0.12, 8, shade(bg, 0.35));
    let c = Vector2::new(r.x + r.width / 2.0, r.y + r.height / 2.0);
    let rad = r.width * 0.34;
    let ink = Color::new(140, 124, 115, 255);
    d.draw_circle_v(c, rad, CREAM);
    d.draw_ring(c, rad - 2.5, rad, 0.0, 360.0, 48, ink);
    let eye = (rad * 0.12).max(2.0);
    d.draw_circle_v(Vector2::new(c.x - rad * 0.38, c.y - rad * 0.16), eye, ink);
    d.draw_circle_v(Vector2::new(c.x + rad * 0.38, c.y - rad * 0.16), eye, ink);
    d.draw_ring(
        Vector2::new(c.x, c.y + rad * 0.06),
        rad * 0.42,
        rad * 0.42 + 2.5,
        20.0,
        160.0,
        16,
        ink,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_fits_and_centres_a_few_children() {
        let g = Grid::new(3, 1280.0, 720.0);
        assert_eq!((g.cols, g.rows), (3, 1));
        let (a, c) = (g.rect(0, 0.0), g.rect(2, 0.0));
        // Horizontally centred.
        assert!(((a.x) - (1280.0 - (c.x + c.width))).abs() < 0.5);
        assert_eq!(g.max_scroll(), 0.0);
    }

    #[test]
    fn many_children_wrap_and_scroll() {
        let g = Grid::new(20, 1280.0, 720.0);
        assert!(g.cols >= 4 && g.cols < 20);
        assert_eq!(g.rows, 20usize.div_ceil(g.cols));
        assert!(g.max_scroll() > 0.0, "20 tiles don't fit a 720p screen");
        // Tiles never overlap within a row.
        let (a, b) = (g.rect(0, 0.0), g.rect(1, 0.0));
        assert!(b.x >= a.x + a.width);
        // Scrolling to the last tile reveals it.
        let s = g.scroll_to_show(19, 0.0, 720.0);
        let last = g.rect(19, s);
        assert!(last.y + last.height <= 720.0);
    }

    #[test]
    fn grid_scales_with_the_window() {
        let small = Grid::new(4, 640.0, 360.0);
        let big = Grid::new(4, 2560.0, 1440.0);
        assert!(big.tile_w > small.tile_w * 3.0);
    }

    #[test]
    fn focus_moves_within_bounds() {
        assert_eq!(move_focus(0, -1, 5), 0);
        assert_eq!(move_focus(0, 1, 5), 1);
        assert_eq!(move_focus(4, 1, 5), 4);
        assert_eq!(move_focus(1, 3, 5), 4);
        assert_eq!(move_focus(2, 3, 5), 2, "no row below: stay put");
        assert_eq!(move_focus(0, 1, 0), 0);
    }

    #[test]
    fn font_covers_every_name_letter() {
        let set = font_codepoints("Łucja Żółć Ярина");
        for ch in "ŁŻółćЯрина?…".chars() {
            assert!(set.contains(ch), "missing {ch}");
        }
        assert!(set.contains('A') && set.contains('~'));
    }

    #[test]
    fn long_names_are_shortened() {
        assert_eq!(fit_name("Maya", 14), "Maya");
        assert_eq!(fit_name("Aleksandra Maria", 14).chars().count(), 14);
        assert!(fit_name("Aleksandra Maria", 14).ends_with('…'));
    }

    #[test]
    fn tile_colours_match_the_app_order() {
        // The app's ui::shell::TILE_COLORS is [SKY, LILAC, BUTTER, MINT, ROSE];
        // keep this list in the same order so a child's colour matches.
        assert_eq!(TILE_COLORS[0], SKY);
        assert_eq!(TILE_COLORS[4], ROSE);
    }
}
