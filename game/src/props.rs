//! Vowel Runner's code-built props: the meadow the hero runs on and the four
//! obstacles, each a [`Grid`] of coloured bricks (see `bricks.rs`), lit by
//! Lam::pula like everything else in the scene.
//!
//! Everything here is deterministic — a hash of the brick's place, never a
//! random generator — so the meadow and the obstacles look the same every
//! game, and a test can hold them to their sizes.
//!
//! The obstacles' bricks are half the meadow's, so they carry a little
//! detail (a toy brick's studs, a wall's mortar) at the size of the hitboxes
//! in `runner.rs`; the colours keep the families the signs use: pink for the
//! jump vowel's block and pillar, purple for the duck vowel's bridge, brick
//! red for the wall the shoot vowel knocks down.

use super::bricks::Grid;

/// Deterministic hash of a brick's place (plus a salt) → 0..1.
fn hash(x: usize, y: usize, z: usize, salt: u64) -> f32 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ salt.wrapping_mul(0x27D4_EB2F_1656_67C5);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 29;
    (h & 0xFFFF) as f32 / 65535.0
}

// ---- the meadow -------------------------------------------------------------

/// One meadow brick, world units: the water's brick, so the two meet as one.
pub const GROUND_CELL: f32 = 0.25;
/// Bricks along one tile. The meadow is laid tile after tile, so it repeats
/// every `GROUND_TILE × GROUND_CELL` units (32: some seconds of running).
pub const GROUND_TILE: usize = 128;
/// Bricks front to back: the meadow is 3 units deep, z −1.5 → 1.5, the hero's
/// lane down its middle.
pub const GROUND_DEPTH: usize = 12;
/// Layers from the bottom of the bank up to the grass, which is the last of
/// them; the grass top is the ground (y = 0 in the world).
pub const GROUND_LAYERS: usize = 6;

// The meadow's colours (grid colour n = GROUND_PALETTE[n - 1]): a sunny day,
// the grass and earth of the old blocks, with some shades round them.
const GRASS: u8 = 1;
const GRASS_LIGHT: u8 = 2;
const GRASS_DEEP: u8 = 3;
const TUFT: u8 = 4;
const BUTTERCUP: u8 = 5;
const DAISY: u8 = 6;
const EARTH: u8 = 7;
const EARTH_LIGHT: u8 = 8;
const EARTH_DARK: u8 = 9;
const EARTH_DEEP: u8 = 10;
const STONE: u8 = 11;
const STONE_DARK: u8 = 12;
pub const GROUND_PALETTE: [[u8; 3]; 12] = [
    [112, 202, 92],  // grass
    [132, 214, 100], // grass, light
    [94, 186, 80],   // grass, deep
    [70, 160, 66],   // a tuft
    [255, 214, 72],  // a buttercup
    [252, 250, 240], // a daisy
    [184, 122, 78],  // earth
    [198, 138, 90],  // earth, light
    [164, 106, 66],  // earth, dark
    [138, 88, 58],   // earth, deep down
    [160, 154, 148], // a stone
    [128, 122, 118], // a stone, dark
];

/// The hero's lane, in meadow bricks front to back (world z −0.5 → 0.5):
/// nothing grows there, so nothing pokes through the hero or an obstacle.
const LANE: std::ops::Range<usize> = 4..8;

/// One tile of meadow: a flat grass top, flecked with tufts and flowers off
/// the lane, over layered earth with the odd stone, and grass hanging over the
/// bank's front edge. Flat on purpose: bricks standing up out of the grass
/// read as toys left lying about, and pulled the eye off the obstacles. Its
/// last column meets its first, so tiles join without a seam.
pub fn ground() -> Grid {
    let (w, d) = (GROUND_TILE, GROUND_DEPTH);
    let top = GROUND_LAYERS - 1;
    let mut g = Grid::new(w, GROUND_LAYERS, d);
    for z in 0..d {
        for x in 0..w {
            let r = |salt| hash(x, 0, z, salt);
            let mut grass = match r(1) {
                v if v < 0.6 => GRASS,
                v if v < 0.82 => GRASS_LIGHT,
                _ => GRASS_DEEP,
            };
            // Tufts and flowers: a few toward the back, fewer in front, and
            // no flower on the front edge, where its side would show in the
            // bank as a stray coloured brick.
            if !LANE.contains(&z) {
                let density = if z < LANE.start { 0.08 } else { 0.04 };
                if r(5) < density {
                    grass = match r(6) {
                        _ if z == d - 1 => TUFT,
                        v if v < 0.14 => BUTTERCUP,
                        v if v < 0.22 => DAISY,
                        _ => TUFT,
                    };
                }
            }
            g.set(x, top, z, grass);
            for y in 0..top {
                let below = top - y; // 1 = just under the grass
                let v = hash(x, y, z, 2);
                let earth = if v < 0.04 {
                    STONE
                } else if v < 0.06 {
                    STONE_DARK
                } else if below <= 2 {
                    if v < 0.35 { EARTH_LIGHT } else { EARTH }
                } else if below <= 4 {
                    if v < 0.5 { EARTH } else { EARTH_DARK }
                } else {
                    EARTH_DEEP
                };
                g.set(x, y, z, earth);
            }
            // Grass hanging over the bank, along its front edge.
            if z == d - 1 && r(3) < 0.4 {
                g.set(x, top - 1, z, if r(4) < 0.5 { GRASS } else { GRASS_DEEP });
            }
        }
    }
    g
}

// ---- the ledges -------------------------------------------------------------

/// Ledge lengths, in meadow bricks (25 logical px each).
pub const LEDGE_SHORT: usize = 12;
pub const LEDGE_LONG: usize = 16;
/// A ledge's thickness, in meadow bricks: the grass and one row of earth.
pub const LEDGE_LAYERS: usize = 2;

/// A ledge to jump onto: a floating strip of meadow `len` bricks long and 3
/// deep (the hero's lane), its grass top over a row of earth that stops a
/// brick short of each end, with grass hanging over the front edge. Same
/// bricks and colours as the meadow, so it reads as a piece of it, lifted.
pub fn ledge(len: usize) -> Grid {
    let d = 3;
    let mut g = Grid::new(len, LEDGE_LAYERS, d);
    for z in 0..d {
        for x in 0..len {
            let r = |salt| hash(x, len, z, salt);
            let grass = match r(21) {
                v if v < 0.6 => GRASS,
                v if v < 0.82 => GRASS_LIGHT,
                _ => GRASS_DEEP,
            };
            g.set(x, 1, z, grass);
            if x == 0 || x == len - 1 {
                continue;
            }
            let earth = match r(22) {
                _ if z == d - 1 && r(23) < 0.4 => GRASS_DEEP, // hanging over
                v if v < 0.06 => STONE,
                v if v < 0.45 => EARTH_LIGHT,
                _ => EARTH,
            };
            g.set(x, 0, z, earth);
        }
    }
    g
}

// ---- the obstacles ----------------------------------------------------------

/// One obstacle brick, world units: half a meadow brick.
pub const OBSTACLE_CELL: f32 = 0.125;

const PINK: u8 = 1;
const PINK_LIGHT: u8 = 2;
const PINK_DARK: u8 = 3;
const PURPLE: u8 = 4;
const PURPLE_LIGHT: u8 = 5;
const PURPLE_DARK: u8 = 6;
const BRICK: u8 = 7;
const BRICK_LIGHT: u8 = 8;
const BRICK_DARK: u8 = 9;
const MORTAR: u8 = 10;
const CREAM: u8 = 11;
const GOLD: u8 = 12;
pub const OBSTACLE_PALETTE: [[u8; 3]; 12] = [
    [236, 84, 150],  // pink: the jump vowel's family
    [250, 128, 182], // pink, light
    [204, 60, 124],  // pink, dark
    [140, 100, 228], // purple: the duck vowel's
    [174, 142, 246], // purple, light
    [106, 74, 190],  // purple, dark
    [200, 86, 64],   // brick red: the shoot vowel's wall
    [218, 108, 80],  // brick, light
    [174, 70, 52],   // brick, dark
    [232, 220, 204], // mortar
    [255, 236, 216], // cream
    [255, 198, 64],  // gold
];

/// The low block (jump over it): a pink toy brick, studs on top. 0.75 wide,
/// 0.875 tall with its studs (the hitbox is 0.8 × 0.88).
pub fn block() -> Grid {
    let (w, h, d) = (6, 7, 5);
    let mut g = Grid::new(w, h, d);
    g.fill([0, 0, 0], [w, h - 1, d], PINK);
    g.fill([0, 0, 0], [w, 1, d], PINK_DARK);
    for x in [1, 4] {
        for z in [1, 3] {
            g.set(x, h - 1, z, PINK_LIGHT);
        }
    }
    g
}

/// The bar (duck under it): a purple bridge, its deck 0.75 → 1.25 above the
/// ground (the hitbox: 0.78 → 1.24, 1.4 wide) on a post at each end, with a
/// little brace in each corner. A ducking hero (0.58 tall) passes under.
pub fn bridge() -> Grid {
    let (w, h, d) = (11, 10, 4);
    let mut g = Grid::new(w, h, d);
    g.fill([0, 6, 0], [w, h, d], PURPLE);
    g.fill([0, h - 1, 0], [w, h, d], PURPLE_LIGHT);
    for x in [0, w - 1] {
        g.fill([x, 0, 1], [x + 1, 6, 3], PURPLE_DARK);
    }
    for x in [1, w - 2] {
        g.fill([x, 5, 1], [x + 1, 6, 3], PURPLE_DARK);
    }
    g
}

/// The wall (shoot it): red bricks in staggered courses, 0.875 wide and 3
/// tall (the hitbox: 0.92 × 3). The mortar is sunk one brick in, so every
/// brick stands out and catches the light at its corners.
pub fn wall() -> Grid {
    let (w, h, d) = (7, 24, 5);
    let mut g = Grid::new(w, h, d);
    for y in 0..h {
        let course = y / 3;
        let offset = if course % 2 == 0 { 0 } else { 2 };
        for x in 0..w {
            // Two rows of bricks, then a row of mortar (the top row is bricks).
            let bed = y % 3 == 2 && y + 1 < h;
            let joint = !bed && (x + offset) % 4 == 3;
            for z in 0..d {
                let surface = x == 0 || x == w - 1 || z == 0 || z == d - 1;
                let c = if bed || joint {
                    if surface {
                        continue; // sunk in
                    }
                    MORTAR
                } else {
                    match hash((x + offset) / 4, course, 0, 7) {
                        v if v < 0.45 => BRICK,
                        v if v < 0.75 => BRICK_LIGHT,
                        _ => BRICK_DARK,
                    }
                };
                g.set(x, y, z, c);
            }
        }
    }
    g
}

/// The tall pillar (double jump it): a candy-striped column with a gold knob,
/// 0.625 wide and 3.625 tall (the hitbox: 0.64 × 3.6). The stripes wind
/// round it, diagonal on every side.
pub fn pillar() -> Grid {
    let (w, h, d) = (5, 29, 5);
    let mut g = Grid::new(w, h, d);
    for y in 0..h - 1 {
        for z in 0..d {
            for x in 0..w {
                let stripe = (y + x + (d - 1 - z)) / 3 % 2 == 0;
                g.set(x, y, z, if stripe { PINK } else { CREAM });
            }
        }
    }
    g.fill([1, h - 1, 1], [w - 1, h, d - 1], GOLD);
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The filled box of a grid, in bricks: (min, max exclusive) per axis.
    fn extent(g: &Grid) -> ([usize; 3], [usize; 3]) {
        let mut lo = [usize::MAX; 3];
        let mut hi = [0; 3];
        for z in 0..g.d {
            for y in 0..g.h {
                for x in 0..g.w {
                    if g.get(x as isize, y as isize, z as isize) != 0 {
                        for (k, v) in [x, y, z].into_iter().enumerate() {
                            lo[k] = lo[k].min(v);
                            hi[k] = hi[k].max(v + 1);
                        }
                    }
                }
            }
        }
        (lo, hi)
    }

    fn size(g: &Grid, cell: f32) -> [f32; 3] {
        let (lo, hi) = extent(g);
        [0, 1, 2].map(|k| (hi[k] - lo[k]) as f32 * cell)
    }

    #[test]
    fn obstacles_are_their_hitboxes_size() {
        // (width, height) in world units against runner.rs's hitboxes.
        let near = |a: f32, b: f32| (a - b).abs() <= OBSTACLE_CELL;
        let [w, h, _] = size(&block(), OBSTACLE_CELL);
        assert!(near(w, 0.8) && near(h, 0.88), "block {w}×{h}");
        let [w, h, _] = size(&bridge(), OBSTACLE_CELL);
        assert!(near(w, 1.4) && near(h, 1.24), "bridge {w}×{h}");
        let [w, h, _] = size(&wall(), OBSTACLE_CELL);
        assert!(near(w, 0.92) && near(h, 3.0), "wall {w}×{h}");
        let [w, h, _] = size(&pillar(), OBSTACLE_CELL);
        assert!(near(w, 0.64) && near(h, 3.6), "pillar {w}×{h}");
    }

    #[test]
    fn a_ducking_hero_fits_under_the_bridge() {
        // Nothing between the posts below the deck's hitbox bottom (0.78),
        // over the width a hero (0.88) takes.
        let g = bridge();
        let cell = OBSTACLE_CELL;
        let below = (0.78 / cell) as isize; // whole bricks under 0.78
        let mid = g.w as f32 / 2.0;
        for x in 0..g.w {
            let cx = (x as f32 + 0.5 - mid) * cell;
            if cx.abs() > 0.44 {
                continue;
            }
            for y in 0..below.min(5) {
                for z in 0..g.d as isize {
                    assert_eq!(g.get(x as isize, y, z), 0, "brick at {x},{y},{z}");
                }
            }
        }
    }

    #[test]
    fn the_meadow_is_flat_and_the_lane_plain_grass() {
        let g = ground();
        assert_eq!(g.h, GROUND_LAYERS, "nothing stands up out of the grass");
        let top = GROUND_LAYERS as isize - 1;
        for x in 0..g.w as isize {
            for z in 0..g.d as isize {
                assert_ne!(g.get(x, top, z), 0, "a hole in the meadow");
            }
            for z in LANE {
                let c = g.get(x, top, z as isize);
                assert!(
                    [GRASS, GRASS_LIGHT, GRASS_DEEP].contains(&c),
                    "a flower in the lane"
                );
            }
        }
        // Everything stays inside the palette.
        for z in 0..g.d as isize {
            for y in 0..g.h as isize {
                for x in 0..g.w as isize {
                    assert!((g.get(x, y, z) as usize) <= GROUND_PALETTE.len());
                }
            }
        }
    }

    #[test]
    fn the_meadow_is_3_units_deep_and_its_top_is_the_ground() {
        assert_eq!(GROUND_DEPTH as f32 * GROUND_CELL, 3.0);
        let top_layer_top = GROUND_LAYERS as f32 * GROUND_CELL;
        assert_eq!(top_layer_top, 1.5, "runner.rs lowers the tile by this");
    }

    #[test]
    fn every_prop_fits_raylibs_16_bit_indices() {
        use crate::bricks::{Build, vertex_count};
        let ground = Build {
            wrap_x: true,
            open_below_and_behind: true,
        };
        let n = vertex_count(&super::ground(), ground);
        assert!(n <= u16::MAX as usize + 1, "ground: {n} vertices");
        for (name, g) in [
            ("block", block()),
            ("bridge", bridge()),
            ("wall", wall()),
            ("pillar", pillar()),
        ] {
            let n = vertex_count(&g, Build::default());
            assert!(n <= u16::MAX as usize + 1, "{name}: {n} vertices");
        }
    }

    #[test]
    fn a_ledge_is_its_length_and_two_bricks_thick() {
        for len in [LEDGE_SHORT, LEDGE_LONG] {
            let [w, h, _] = size(&ledge(len), GROUND_CELL);
            assert_eq!(w, len as f32 * GROUND_CELL);
            assert_eq!(h, LEDGE_LAYERS as f32 * GROUND_CELL);
            // The whole top is grass to stand on.
            let g = ledge(len);
            for x in 0..len as isize {
                assert_ne!(g.get(x, 1, 1), 0);
            }
        }
    }

    #[test]
    fn the_same_every_game() {
        let (a, b) = (ground(), ground());
        for z in 0..a.d as isize {
            for x in 0..a.w as isize {
                assert_eq!(a.get(x, 5, z), b.get(x, 5, z));
            }
        }
    }
}
