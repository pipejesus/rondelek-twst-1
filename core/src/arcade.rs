//! The arcade look's shared data: the palette and the pixel bitmaps (icons and
//! the six vowels) that both the app's shell (egui) and the games' entrance
//! screens (raylib) draw with. Only data lives here — plain RGB triples and
//! `#`-row bitmaps — so this crate stays GUI-free; each side turns them into
//! its own colours and rectangles (`app/src/ui/arcade.rs`, `game/src/arcade.rs`).
//!
//! The palette is the README pictures' (`cargo run --bin genarcade`): a navy
//! night, orange / butter / pink / cyan accents, cream ink. Bright colours sit
//! on the deep navy, which is what keeps a vivid screen calm to look at.

/// An sRGB colour.
pub type Rgb = [u8; 3];

pub const NIGHT: Rgb = [0x1D, 0x1A, 0x3A];
pub const NIGHT_HI: Rgb = [0x2E, 0x2A, 0x5C];
pub const NIGHT_LO: Rgb = [0x12, 0x10, 0x27];
pub const INK: Rgb = [0x0A, 0x09, 0x17];
pub const ORANGE: Rgb = [0xFF, 0x6A, 0x1A];
pub const BUTTER: Rgb = [0xFF, 0xD8, 0x4A];
pub const PINK: Rgb = [0xFF, 0x4F, 0xA3];
pub const CYAN: Rgb = [0x3F, 0xE0, 0xFF];
pub const CREAM: Rgb = [0xFF, 0xF3, 0xE8];
pub const GREEN: Rgb = [0x3F, 0xD6, 0x7A];
/// Secondary text on the night (hints, captions).
pub const MIST: Rgb = [0xA9, 0xA4, 0xD6];
/// Destructive actions (delete).
pub const RED: Rgb = [0xF0, 0x3E, 0x3E];

/// A child's colour: vivid versions of the old pastel tile colours, in the
/// same order (sky, lilac, butter, mint, rose), a touch deeper than full neon
/// so a row of them stays pleasant. Pick with [`tile_color`].
pub const TILE_COLORS: [Rgb; 5] = [
    [0x3A, 0xA8, 0xF0], // blue
    [0x9A, 0x6B, 0xF2], // violet
    [0xF7, 0xC8, 0x3A], // sunflower
    [0x35, 0xC7, 0x7E], // green
    [0xF2, 0x5C, 0x9A], // pink
];

/// A child's stable colour, from their uid — the same in the app and the games.
pub fn tile_color(seed: &str) -> Rgb {
    TILE_COLORS[crate::util::stable_pick(seed, TILE_COLORS.len())]
}

/// Lighten (`amount` > 0) or darken (< 0) toward white/black.
pub fn shade(c: Rgb, amount: f32) -> Rgb {
    let t = amount.abs().clamp(0.0, 1.0);
    let target = if amount >= 0.0 { 255.0 } else { 0.0 };
    c.map(|v| (v as f32 + (target - v as f32) * t).round() as u8)
}

// ---- pixel bitmaps --------------------------------------------------------------

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
/// A bold "?" (two-pixel strokes).
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

/// Every icon, for tests and previews.
pub const ICONS: &[Bitmap] = &[
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
];

/// Size of a bitmap in bitmap pixels (width, height).
pub fn bitmap_size(b: Bitmap) -> (usize, usize) {
    (b.iter().map(|r| r.len()).max().unwrap_or(0), b.len())
}

/// The filled cells of a bitmap as (column, row).
pub fn bitmap_cells(b: Bitmap) -> impl Iterator<Item = (usize, usize)> {
    b.iter().enumerate().flat_map(|(row, line)| {
        line.chars()
            .enumerate()
            .filter(|&(_, ch)| ch == '#')
            .map(move |(col, _)| (col, row))
    })
}

/// The six vowels as bold lowercase pixel letters (two-pixel strokes). The
/// pixel font's own lowercase is a five-pixel design that turns ambiguous when
/// blown up ("a" reads as "d"), and these are the letters a child is learning,
/// so they get unmistakable shapes of their own. Each comes with its baseline
/// row, so "i" (dot above) and "y" (tail below) line up with the rest.
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
pub const VOWEL_ROWS: usize = 6;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmaps_are_rectangular() {
        for b in ICONS {
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
            let top = base - VOWEL_ROWS;
            assert!(b[top].contains('#') && b[base - 1].contains('#'), "{v}");
        }
        assert!(vowel_glyph("x").is_none());
    }

    #[test]
    fn cells_walk_the_filled_pixels() {
        let cells: Vec<_> = bitmap_cells(&["#.", ".#"]).collect();
        assert_eq!(cells, vec![(0, 0), (1, 1)]);
        assert_eq!(bitmap_size(ICON_CURSOR), (5, 3));
    }

    #[test]
    fn tile_colours_keep_the_old_families_in_order() {
        // [blue, violet, yellow, green, pink] — the order the pastel tiles
        // had (sky, lilac, butter, mint, rose), so a child keeps their colour.
        let [blue, violet, yellow, green, pink] = TILE_COLORS;
        assert!(blue[2] > blue[0] && blue[2] > blue[1]);
        assert!(violet[2] > violet[1] && violet[0] > violet[1]);
        assert!(yellow[0] > yellow[2] && yellow[1] > yellow[2]);
        assert!(green[1] > green[0] && green[1] > green[2]);
        assert!(pink[0] > pink[1] && pink[0] > pink[2]);
        assert_eq!(tile_color("abc"), tile_color("abc"));
    }

    #[test]
    fn shade_moves_toward_white_and_black() {
        assert_eq!(shade([100, 100, 100], 1.0), [255, 255, 255]);
        assert_eq!(shade([100, 100, 100], -1.0), [0, 0, 0]);
    }
}
