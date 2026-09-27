//! "Who's playing?" for the games: a reusable pre-start screen that lets a
//! child (or a grown-up) pick whose voice calibration to use.
//!
//! The sampler app always passes `--profile` when it launches a game, so this
//! screen only appears when a game is started on its own (double-clicked, or
//! run without arguments). It reads the same profile library as the app: a
//! card per child in their colour family (as in the app, via
//! `rondelek_core::util::stable_pick`, but vivid), their photo or character,
//! their name, and a small badge showing whether their voice check is done.
//! Drawn in the arcade style of [`crate::arcade`]: a night sky with twinkling
//! stars, chunky notched cards, a pixel "?" for a title.
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
//! children's names, which are drawn in the Tiny5 pixel font (loaded with
//! exactly the letters the names need; it covers Polish, Ukrainian and the
//! other shipped languages).

use raylib::prelude::*;

use rondelek_core::profile::{self, Profile};

use crate::arcade::{
    self, BUTTER, CREAM, CYAN, GREEN, ICON_CHECK, ICON_CURSOR, ICON_MIC, ICON_QUESTION, ICON_STAR,
    INK, NIGHT_LO, ORANGE, PixelFont, TILE_COLORS, notched, shade,
};
use crate::snap;

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
    let font = PixelFont::load(rl, thread, &names);
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
            let t = rl.get_time() as f32;
            let p = arcade::pixel_unit(grid.s);
            let mut d = rl.begin_drawing(thread);
            arcade::sky(&mut d, w, h, p, t);
            draw_header(&mut d, w, grid.s, p);
            // Cards scroll inside the band between the title and the bottom
            // stripes, so a long list never slides over the "?"; the stripes,
            // drawn last, frame the bottom edge.
            let clip_top = (HEADER_BOTTOM * grid.s).round();
            {
                let mut cards = d.begin_scissor_mode(
                    0,
                    clip_top as i32,
                    w as i32,
                    (h - clip_top - arcade::stripes_h(p)) as i32,
                );
                for (i, e) in entries.iter().enumerate() {
                    let r = grid.rect(i, scroll);
                    if r.y + r.height < 0.0 || r.y > h {
                        continue;
                    }
                    let lit = hovered == Some(i) || focus == i;
                    let look = TileLook {
                        lit,
                        pressed: lit && pressed && hovered == Some(i),
                        focused: focus == i,
                    };
                    draw_tile(&mut cards, &font, e, r, grid.s, p, look, t);
                }
            }
            arcade::stripes(&mut d, w, h, p);
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
        let header = GRID_TOP * s;
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
        if r.y - s < self.top.min(GRID_TOP * self.s) {
            s = r.y - self.top.min(GRID_TOP * self.s);
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

/// Where the title area ends (1280×720 design px). Cards are clipped below it,
/// which still leaves room above the first row for the focus cursor.
const HEADER_BOTTOM: f32 = 122.0;
/// Where the first row of cards may start (design px): far enough below the
/// title that the focus cursor over a top-row card can't be read as part of
/// the "?".
const GRID_TOP: f32 = 196.0;

/// A big pixel "?" between two little stars: "who's playing?" without words.
/// Butter over an orange shadow, like the README marquee's PRESS START.
fn draw_header(d: &mut RaylibDrawHandle, w: f32, s: f32, p: f32) {
    let cy = 78.0 * s;
    arcade::icon_shadowed(d, ICON_QUESTION, w / 2.0, cy, 2.0 * p, BUTTER, ORANGE);
    let gap = 80.0 * s;
    for side in [-1.0, 1.0] {
        arcade::icon_shadowed(d, ICON_STAR, w / 2.0 + side * gap, cy, p, CYAN, INK);
    }
}

/// How a card is showing this frame.
#[derive(Clone, Copy)]
struct TileLook {
    /// Hovered or keyboard-focused: slightly brighter, lifted a pixel.
    lit: bool,
    /// Being clicked: sinks a pixel.
    pressed: bool,
    /// The keyboard focus: framed, with the cursor above.
    focused: bool,
}

/// One child's card: a chunky notched arcade card in their colour, their
/// picture in an ink-framed well, their name on a dark plate, and the voice
/// check badge. `t` (seconds) gently bobs the focus cursor.
#[allow(clippy::too_many_arguments)]
fn draw_tile(
    d: &mut RaylibDrawHandle,
    font: &PixelFont,
    e: &Entry,
    r: Rectangle,
    s: f32,
    p: f32,
    look: TileLook,
    t: f32,
) {
    let dy = if look.pressed {
        p
    } else if look.lit {
        -p
    } else {
        0.0
    };
    let card = Rectangle { y: r.y + dy, ..r };

    // Hard drop shadow, then the focus frame, then the card itself.
    notched(
        d,
        Rectangle {
            x: r.x + p,
            y: r.y + 2.0 * p,
            ..r
        },
        p,
        arcade::with_alpha(INK, 170),
    );
    if look.focused {
        arcade::highlight(d, card, p, CYAN);
        let bob = ((t * 2.4).sin() * p).round();
        arcade::icon_shadowed(
            d,
            ICON_CURSOR,
            card.x + card.width / 2.0,
            card.y - 8.0 * p + bob,
            (1.5 * p).round(),
            BUTTER,
            INK,
        );
    }
    let face = if look.lit {
        shade(e.color, 0.12)
    } else {
        e.color
    };
    arcade::panel(d, card, p, face);

    // Picture well: an ink frame, a cream ground, the picture.
    let pad = 4.0 * p;
    let side = card.width - 2.0 * pad;
    let pic = Rectangle {
        x: card.x + pad,
        y: card.y + pad,
        width: side,
        height: side,
    };
    notched(
        d,
        Rectangle {
            x: pic.x - p,
            y: pic.y - p,
            width: pic.width + 2.0 * p,
            height: pic.height + 2.0 * p,
        },
        p,
        INK,
    );
    d.draw_rectangle_rec(pic, CREAM);
    match &e.picture {
        Some(tex) => {
            let src = Rectangle::new(0.0, 0.0, tex.width as f32, tex.height as f32);
            d.draw_texture_pro(tex, src, pic, Vector2::zero(), 0.0, Color::WHITE);
        }
        None => draw_default_face(d, pic, e.color),
    }

    // Name plate: dark, with the name in cream pixels.
    let plate = Rectangle {
        x: card.x + pad,
        y: pic.y + pic.height + 2.0 * p,
        width: side,
        height: card.y + card.height - 3.0 * p - (pic.y + pic.height + 2.0 * p),
    };
    notched(d, plate, p, NIGHT_LO);
    let (name, k) = fit_to_width(font, e.profile.name(), plate.width - 2.0 * p, s);
    font.draw_centred(
        d,
        &name,
        plate.x + plate.width / 2.0,
        plate.y + plate.height / 2.0,
        k,
        CREAM,
        Some(INK),
    );

    // Voice-check badge (for the grown-ups), over the picture's corner.
    let b = 10.0 * p;
    let badge = Rectangle {
        x: pic.x + pic.width - b + 2.0 * p,
        y: pic.y - 2.0 * p,
        width: b,
        height: b,
    };
    let (fill, glyph, ink) = if e.calibrated {
        (GREEN, ICON_CHECK, CREAM)
    } else {
        (BUTTER, ICON_MIC, INK)
    };
    arcade::panel(d, badge, p, fill);
    let px = (p * 0.75).round().max(1.0);
    arcade::icon(
        d,
        glyph,
        badge.x + badge.width / 2.0,
        badge.y + badge.height / 2.0,
        px,
        ink,
    );
}

/// The name at the biggest pixel size (3×, then 2× the font) that fits
/// `max_w`, shortened with `…` if even the smaller one doesn't.
fn fit_to_width(font: &PixelFont, name: &str, max_w: f32, s: f32) -> (String, f32) {
    let big = (3.0 * s).round().max(2.0);
    let small = (2.0 * s).round().max(1.0);
    for k in [big, small] {
        if font.measure(name, k).x <= max_w {
            return (name.to_string(), k);
        }
    }
    let mut n = name.chars().count();
    while n > 2 {
        n -= 1;
        let short = fit_name(name, n);
        if font.measure(&short, small).x <= max_w {
            return (short, small);
        }
    }
    (fit_name(name, 2), small)
}

/// A friendly drawn face for children without a picture (matches the app's).
fn draw_default_face(d: &mut RaylibDrawHandle, r: Rectangle, bg: Color) {
    d.draw_rectangle_rec(r, shade(bg, 0.55));
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
    fn long_names_are_shortened() {
        assert_eq!(fit_name("Maya", 14), "Maya");
        assert_eq!(fit_name("Aleksandra Maria", 14).chars().count(), 14);
        assert!(fit_name("Aleksandra Maria", 14).ends_with('…'));
    }
}
