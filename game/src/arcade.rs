//! The arcade look of the games' entrance screens — the same 8-bit "chrome"
//! as the README's pictures (`cargo run --bin genarcade`): a navy night with a
//! slowly twinkling starfield, 80s stripe bands, notched bevelled panels with
//! a hard ink outline, pixel icons, and the Tiny5 pixel font.
//!
//! Everything is drawn on a coarse grid of `p` real pixels per 8-bit "pixel"
//! ([`pixel_unit`]), with plain rectangles, so it stays crisp at any window
//! size. Text uses **Tiny5** (OFL, `assets/fonts/Tiny5-Regular.ttf`): its
//! glyphs sit on a 9-row grid, so it is loaded with one texel per font pixel
//! and drawn at whole multiples of that with nearest-neighbour filtering. It
//! covers every language the app ships (Latin with Polish/German/French/…
//! accents, and Cyrillic), so children's names render whatever they are.
//!
//! **Calm by design**: bright colours sit on a deep navy, and the only motion
//! is a slow twinkle and a small cursor bob — nothing scrolls or flashes.

use raylib::prelude::*;

// ---- palette (genarcade's, plus vivid tile colours) -------------------------

pub const NIGHT: Color = Color::new(0x1D, 0x1A, 0x3A, 255);
pub const NIGHT_HI: Color = Color::new(0x2E, 0x2A, 0x5C, 255);
pub const NIGHT_LO: Color = Color::new(0x12, 0x10, 0x27, 255);
pub const INK: Color = Color::new(0x0A, 0x09, 0x17, 255);
pub const ORANGE: Color = Color::new(0xFF, 0x6A, 0x1A, 255);
pub const BUTTER: Color = Color::new(0xFF, 0xD8, 0x4A, 255);
pub const PINK: Color = Color::new(0xFF, 0x4F, 0xA3, 255);
pub const CYAN: Color = Color::new(0x3F, 0xE0, 0xFF, 255);
pub const CREAM: Color = Color::new(0xFF, 0xF3, 0xE8, 255);
pub const GREEN: Color = Color::new(0x3F, 0xD6, 0x7A, 255);

/// A child's card colour, vivid versions of the app's pastel tile colours in
/// the same order (sky, lilac, butter, mint, rose), so a child keeps their
/// colour family everywhere. A touch deeper than full neon, so a row of them
/// stays pleasant to look at.
pub const TILE_COLORS: [Color; 5] = [
    Color::new(0x3A, 0xA8, 0xF0, 255), // sky → bright blue
    Color::new(0x9A, 0x6B, 0xF2, 255), // lilac → violet
    Color::new(0xF7, 0xC8, 0x3A, 255), // butter → sunflower
    Color::new(0x35, 0xC7, 0x7E, 255), // mint → green
    Color::new(0xF2, 0x5C, 0x9A, 255), // rose → pink
];

/// Lighten (`amount` > 0) or darken (< 0) a colour toward white/black.
pub fn shade(c: Color, amount: f32) -> Color {
    let t = amount.abs().clamp(0.0, 1.0);
    let target = if amount >= 0.0 { 255.0 } else { 0.0 };
    let mix = |v: u8| (v as f32 + (target - v as f32) * t).round() as u8;
    Color::new(mix(c.r), mix(c.g), mix(c.b), c.a)
}

/// Same colour at alpha `a` (0..=255).
pub fn with_alpha(c: Color, a: u8) -> Color {
    Color::new(c.r, c.g, c.b, a)
}

/// Real pixels per 8-bit "pixel" for a window at scale `s` (1.0 = 1280×720).
pub fn pixel_unit(s: f32) -> f32 {
    (4.0 * s).round().max(2.0)
}

// ---- shapes -------------------------------------------------------------------

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rectangle {
    Rectangle {
        x: x.round(),
        y: y.round(),
        width: w.round(),
        height: h.round(),
    }
}

/// A rectangle with its four corner pixels cut off — the 8-bit rounded corner.
pub fn notched(d: &mut impl RaylibDraw, r: Rectangle, p: f32, c: Color) {
    d.draw_rectangle_rec(rect(r.x + p, r.y, r.width - 2.0 * p, r.height), c);
    d.draw_rectangle_rec(rect(r.x, r.y + p, r.width, r.height - 2.0 * p), c);
}

/// An arcade panel: ink outline, then `face` with a light top-left bevel and a
/// dark bottom-right one, like genarcade's cabinet frame.
pub fn panel(d: &mut impl RaylibDraw, r: Rectangle, p: f32, face: Color) {
    notched(d, r, p, INK);
    let inner = Rectangle {
        x: r.x + p,
        y: r.y + p,
        width: r.width - 2.0 * p,
        height: r.height - 2.0 * p,
    };
    notched(d, inner, p, shade(face, -0.35));
    notched(
        d,
        Rectangle {
            width: inner.width - p,
            height: inner.height - p,
            ..inner
        },
        p,
        shade(face, 0.35),
    );
    notched(
        d,
        Rectangle {
            x: inner.x + p,
            y: inner.y + p,
            width: inner.width - 2.0 * p,
            height: inner.height - 2.0 * p,
        },
        p,
        face,
    );
}

/// A two-line selection frame around `r`: a coloured line, an ink gap — the
/// cabinet's screen well, used as the "this one" highlight.
pub fn highlight(d: &mut impl RaylibDraw, r: Rectangle, p: f32, c: Color) {
    let out = Rectangle {
        x: r.x - 3.0 * p,
        y: r.y - 3.0 * p,
        width: r.width + 6.0 * p,
        height: r.height + 6.0 * p,
    };
    notched(d, out, p, c);
    let gap = Rectangle {
        x: r.x - p,
        y: r.y - p,
        width: r.width + 2.0 * p,
        height: r.height + 2.0 * p,
    };
    // Punch the middle back out down to the ink gap: redraw only the ring by
    // covering the inside with ink, which the caller then draws over.
    notched(d, gap, p, INK);
}

// ---- backdrop -----------------------------------------------------------------

/// Deterministic 0..1 hash of a star index.
fn hash01(i: u32, salt: u32) -> f32 {
    let mut h = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// The whole-screen backdrop: [`sky`] plus [`stripes`].
pub fn backdrop(d: &mut impl RaylibDraw, w: f32, h: f32, p: f32, t: f32) {
    sky(d, w, h, p, t);
    stripes(d, w, h, p);
}

/// Height of the stripe bands along each edge.
pub fn stripes_h(p: f32) -> f32 {
    8.0 * p
}

/// Navy night with a starfield that twinkles slowly (`t` = seconds).
pub fn sky(d: &mut impl RaylibDraw, w: f32, h: f32, p: f32, t: f32) {
    d.clear_background(NIGHT);

    // Stars on the pixel grid; a few are 2×2. Each breathes on its own slow
    // cycle — a twinkle, not a flicker.
    let cols = (w / p) as u32;
    let rows = (h / p) as u32;
    let n = (cols * rows / 380).clamp(40, 260);
    for i in 0..n {
        let x = (hash01(i, 1) * cols as f32).floor() * p;
        let y = (hash01(i, 2) * rows as f32).floor() * p;
        let col = match (hash01(i, 3) * 6.0) as u32 {
            0 => CYAN,
            1 => PINK,
            2 => BUTTER,
            _ => NIGHT_HI,
        };
        let phase = hash01(i, 4) * std::f32::consts::TAU;
        let speed = 0.5 + hash01(i, 5) * 0.7;
        let glow = 0.45 + 0.55 * (0.5 + 0.5 * (t * speed + phase).sin());
        let size = if hash01(i, 6) > 0.9 { 2.0 * p } else { p };
        d.draw_rectangle_rec(
            rect(x, y, size, size),
            with_alpha(col, (255.0 * glow) as u8),
        );
    }
}

/// The 80s stripe bands along the top and bottom edges. Drawn last on a
/// scrolling screen, they frame whatever slides under them.
pub fn stripes(d: &mut impl RaylibDraw, w: f32, h: f32, p: f32) {
    // Orange, butter, pink, cyan — from each edge inward.
    let bands = [ORANGE, BUTTER, PINK, CYAN];
    for (i, c) in bands.iter().enumerate() {
        let off = i as f32 * 2.0 * p;
        d.draw_rectangle_rec(rect(0.0, off, w, 2.0 * p), *c);
        d.draw_rectangle_rec(rect(0.0, h - off - 2.0 * p, w, 2.0 * p), *c);
    }
}

// ---- pixel icons -----------------------------------------------------------------

/// A pixel bitmap, one string per row, `#` = filled.
pub type Bitmap = &'static [&'static str];

pub const ICON_UP: Bitmap = &["...#...", "..###..", ".#####.", "#######"];
pub const ICON_DOWN: Bitmap = &["#######", ".#####.", "..###..", "...#..."];
pub const ICON_STAR: Bitmap = &[
    "...#...", "..###..", "#######", ".#####.", "..###..", ".##.##.", ".#...#.",
];
pub const ICON_PLAY: Bitmap = &[
    "#....", "##...", "###..", "####.", "###..", "##...", "#....",
];
pub const ICON_HEART: Bitmap = &[
    ".##.##.", "#######", "#######", ".#####.", "..###..", "...#...",
];
pub const ICON_CHECK: Bitmap = &[
    "......#", ".....##", "#...##.", "##.##..", ".###...", "..#....",
];
pub const ICON_MIC: Bitmap = &[
    "..###..", ".#####.", ".#####.", ".#####.", "#.###.#", "#.....#", ".#####.", "...#...",
    "..###..",
];
/// A bold "?" (two-pixel strokes), for the "who's playing?" title: Tiny5's own
/// "?" is one pixel thin and falls apart under a drop shadow at this size.
pub const ICON_QUESTION: Bitmap = &[
    ".#####.", "##...##", ".....##", "....##.", "...##..", "...##..", ".......", "...##..",
    "...##..",
];
pub const ICON_CURSOR: Bitmap = &["#####", ".###.", "..#.."];
pub const ICON_TURTLE: Bitmap = &[
    "....####......",
    "..########....",
    ".##########.##",
    "##############",
    ".#.#....#.#...",
];
pub const ICON_RABBIT: Bitmap = &[
    "..#.#....",
    "..#.#....",
    "..#.#....",
    ".####....",
    ".#####...",
    ".########",
    "#########",
    ".########",
    "..#...#..",
];

/// The six vowels as bold lowercase pixel letters (two-pixel strokes). Tiny5's
/// own lowercase is a five-pixel design that turns ambiguous when blown up
/// ("a" reads as "d"), and these are the letters a child is learning, so they
/// get unmistakable shapes of their own. Each comes with its baseline row, so
/// "i" (dot above) and "y" (tail below) line up with the rest.
pub fn vowel_glyph(label: &str) -> Option<(Bitmap, usize)> {
    const A: Bitmap = &[
        ".####..", ".....##", ".######", "##...##", "##...##", ".######",
    ];
    const E: Bitmap = &[
        ".#####.", "##...##", "#######", "##.....", "##.....", ".######",
    ];
    const I: Bitmap = &[
        ".##.", "....", "###.", ".##.", ".##.", ".##.", ".##.", "####",
    ];
    const O: Bitmap = &[
        ".#####.", "##...##", "##...##", "##...##", "##...##", ".#####.",
    ];
    const U: Bitmap = &[
        "##...##", "##...##", "##...##", "##...##", "##..###", ".###.##",
    ];
    const Y: Bitmap = &[
        "##...##", "##...##", "##...##", "##...##", "##...##", ".######", ".....##", "##...##",
        ".#####.",
    ];
    Some(match label {
        "a" => (A, 6),
        "e" => (E, 6),
        "i" => (I, 8),
        "o" => (O, 6),
        "u" => (U, 6),
        "y" => (Y, 6),
        _ => return None,
    })
}

/// Height of the vowels' x-height in bitmap rows (what "letter size" means).
pub const VOWEL_ROWS: f32 = 6.0;

/// Draw a vowel from [`vowel_glyph`] centred on `cx`, sitting on `baseline`.
/// Unknown labels draw nothing.
pub fn vowel(d: &mut impl RaylibDraw, label: &str, cx: f32, baseline: f32, px: f32, c: Color) {
    let Some((b, base)) = vowel_glyph(label) else {
        return;
    };
    let (_, bh) = bitmap_size(b);
    let top = baseline - base as f32 * px;
    icon(d, b, cx, top + bh * px / 2.0, px, c);
}

/// Size of a bitmap in bitmap pixels.
pub fn bitmap_size(b: Bitmap) -> (f32, f32) {
    (
        b.iter().map(|r| r.len()).max().unwrap_or(0) as f32,
        b.len() as f32,
    )
}

/// Draw a bitmap centred at `(cx, cy)`, each bitmap pixel `px` real pixels.
pub fn icon(d: &mut impl RaylibDraw, b: Bitmap, cx: f32, cy: f32, px: f32, c: Color) {
    let (bw, bh) = bitmap_size(b);
    let (x0, y0) = ((cx - bw * px / 2.0).round(), (cy - bh * px / 2.0).round());
    for (row, line) in b.iter().enumerate() {
        for (col, ch) in line.chars().enumerate() {
            if ch == '#' {
                d.draw_rectangle_rec(rect(x0 + col as f32 * px, y0 + row as f32 * px, px, px), c);
            }
        }
    }
}

/// [`icon`] with a hard drop shadow one bitmap pixel down-right.
pub fn icon_shadowed(
    d: &mut impl RaylibDraw,
    b: Bitmap,
    cx: f32,
    cy: f32,
    px: f32,
    c: Color,
    shadow: Color,
) {
    icon(d, b, cx + px, cy + px, px, shadow);
    icon(d, b, cx, cy, px, c);
}

// ---- pixel text --------------------------------------------------------------------

const TINY5_TTF: &[u8] = include_bytes!("../../assets/fonts/Tiny5-Regular.ttf");

/// Tiny5's line height in font pixels: raylib sizes a font by ascent−descent,
/// which for Tiny5 is 9 of its 128-unit pixels (7 above the baseline, 2 below).
pub const TINY5_ROWS: i32 = 9;

/// The pixel font, loaded with the letters a screen needs. Falls back to
/// raylib's built-in font (ASCII only) if the TTF won't load.
pub struct PixelFont {
    font: Option<Font>,
}

impl PixelFont {
    /// Load Tiny5 with printable ASCII, `…`, and every character in `extra`
    /// (e.g. the children's names).
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, extra: &str) -> Self {
        let chars = codepoints(extra);
        let font = rl
            .load_font_from_memory(thread, ".ttf", TINY5_TTF, TINY5_ROWS, Some(&chars))
            .ok();
        if let Some(f) = &font {
            // Nearest-neighbour: a font pixel scaled up stays a hard square.
            f.texture()
                .set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_POINT);
        }
        Self { font }
    }

    /// Size of `text` drawn at `k` real pixels per font pixel.
    pub fn measure(&self, text: &str, k: f32) -> Vector2 {
        let size = TINY5_ROWS as f32 * k;
        match &self.font {
            Some(f) => f.measure_text(text, size, 0.0),
            None => Vector2::new(text.chars().count() as f32 * 6.0 * k, size),
        }
    }

    /// Draw `text` with its top-left at `pos`, `k` real pixels per font pixel.
    pub fn draw(&self, d: &mut RaylibDrawHandle, text: &str, pos: Vector2, k: f32, c: Color) {
        let size = TINY5_ROWS as f32 * k;
        let pos = Vector2::new(pos.x.round(), pos.y.round());
        match &self.font {
            Some(f) => d.draw_text_ex(f, text, pos, size, 0.0, c),
            None => d.draw_text(text, pos.x as i32, pos.y as i32, size as i32, c),
        }
    }

    /// [`PixelFont::draw`] with a hard drop shadow one font pixel down-right.
    pub fn draw_shadowed(
        &self,
        d: &mut RaylibDrawHandle,
        text: &str,
        pos: Vector2,
        k: f32,
        c: Color,
        shadow: Color,
    ) {
        self.draw(d, text, Vector2::new(pos.x + k, pos.y + k), k, shadow);
        self.draw(d, text, pos, k, c);
    }

    /// Draw `text` centred on `(cx, cy)`, with a drop shadow if given. (Big
    /// letters read cleaner without one: Tiny5's strokes are a single font
    /// pixel, and a one-pixel-off copy muddles the shape.)
    #[allow(clippy::too_many_arguments)]
    pub fn draw_centred(
        &self,
        d: &mut RaylibDrawHandle,
        text: &str,
        cx: f32,
        cy: f32,
        k: f32,
        c: Color,
        shadow: Option<Color>,
    ) {
        let m = self.measure(text, k);
        let pos = Vector2::new(cx - m.x / 2.0, cy - m.y / 2.0);
        match shadow {
            Some(sh) => self.draw_shadowed(d, text, pos, k, c, sh),
            None => self.draw(d, text, pos, k, c),
        }
    }
}

/// Printable ASCII, `…`, and the characters of `extra`, deduplicated.
pub fn codepoints(extra: &str) -> String {
    let mut chars: Vec<char> = (' '..='~').chain(extra.chars()).chain(['…']).collect();
    chars.sort_unstable();
    chars.dedup();
    chars.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmaps_are_rectangular() {
        for b in [
            ICON_UP,
            ICON_DOWN,
            ICON_STAR,
            ICON_PLAY,
            ICON_HEART,
            ICON_CHECK,
            ICON_MIC,
            ICON_QUESTION,
            ICON_CURSOR,
            ICON_TURTLE,
            ICON_RABBIT,
        ] {
            let w = b[0].len();
            assert!(b.iter().all(|r| r.len() == w), "ragged bitmap {b:?}");
            assert!(b.iter().all(|r| r.chars().all(|c| c == '#' || c == '.')));
        }
    }

    #[test]
    fn every_vowel_has_a_glyph_on_a_shared_baseline() {
        for v in ["a", "e", "i", "o", "u", "y"] {
            let (b, base) = vowel_glyph(v).unwrap_or_else(|| panic!("no glyph for {v}"));
            let w = b[0].len();
            assert!(b.iter().all(|r| r.len() == w), "ragged {v}");
            // The x-height part (the VOWEL_ROWS rows above the baseline) is
            // inked on its top and bottom row, so all six line up.
            let top = base - VOWEL_ROWS as usize;
            assert!(b[top].contains('#') && b[base - 1].contains('#'), "{v}");
        }
        assert!(vowel_glyph("x").is_none());
    }

    #[test]
    fn pixel_unit_is_whole_and_never_below_two() {
        assert_eq!(pixel_unit(1.0), 4.0);
        assert_eq!(pixel_unit(0.2), 2.0);
        assert_eq!(pixel_unit(1.5), 6.0);
    }

    #[test]
    fn codepoints_cover_names_in_every_script() {
        let set = codepoints("Łucja Żółć Ярина Ïgor");
        for ch in "ŁŻółćЯринаÏ…?A~".chars() {
            assert!(set.contains(ch), "missing {ch}");
        }
    }

    #[test]
    fn tile_colours_keep_the_apps_order() {
        // Same families, same order as the app's ui::shell::TILE_COLORS
        // [SKY, LILAC, BUTTER, MINT, ROSE]: blue, violet, yellow, green, pink.
        let [blue, violet, yellow, green, pink] = TILE_COLORS;
        assert!(blue.b > blue.r && blue.b > blue.g);
        assert!(violet.b > violet.g && violet.r > violet.g);
        assert!(yellow.r > yellow.b && yellow.g > yellow.b);
        assert!(green.g > green.r && green.g > green.b);
        assert!(pink.r > pink.g && pink.r > pink.b);
    }

    #[test]
    fn shade_moves_toward_white_and_black() {
        let c = Color::new(100, 100, 100, 255);
        assert_eq!(shade(c, 1.0).r, 255);
        assert_eq!(shade(c, -1.0).r, 0);
        assert_eq!(shade(c, 0.0), c);
    }
}
