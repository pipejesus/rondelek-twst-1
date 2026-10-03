//! Vowel Runner's code-built props: the meadow the hero runs on, the four
//! obstacles and the stone tablets their vowels are carved in, each a
//! [`Grid`] of coloured bricks (see `bricks.rs`), lit by Lam::pula like
//! everything else in the scene.
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

// ---- the backdrops: 90s-style parallax planes --------------------------------
//
// Two far planes, each one tile of bricks laid end to end and scrolled at its
// own rate (runner.rs): a mountain range at the back, a jungle in front of
// it. Both are pixel-art silhouettes extruded a couple of bricks deep — the
// way flat-draw turns a drawing into a prop — and both start below the
// meadow's sightline, so no floor ever shows under them.

/// Signed distance from `a` to `b` along a loop `n` long (the shorter way
/// round), so a tile's last column meets its first without a seam.
fn loop_dx(a: f32, b: f32, n: f32) -> f32 {
    let d = (a - b).rem_euclid(n);
    if d > n / 2.0 { d - n } else { d }
}

/// One mountain brick, world units: big, because it is far — on screen about
/// a cloud brick.
pub const MOUNTAIN_CELL: f32 = 0.4;
/// Bricks along one tile (64 units).
pub const MOUNTAIN_TILE: usize = 160;
/// Bricks from the bottom of the range to the sky.
pub const MOUNTAIN_LAYERS: usize = 26;
/// World y of the range's bottom.
pub const MOUNTAIN_BASE: f32 = -2.0;

// The range's colours: a paler back range with snowy peaks, a darker front
// ridge, each slope in shadow on the side away from the sun (the left).
const PEAK_ROCK: u8 = 1;
const PEAK_SHADE: u8 = 2;
const PEAK_SNOW: u8 = 3;
const RIDGE_ROCK: u8 = 4;
const RIDGE_SHADE: u8 = 5;
const RIDGE_SNOW: u8 = 6;
const SNOW_SHADE: u8 = 7;
pub const MOUNTAIN_PALETTE: [[u8; 3]; 7] = [
    [118, 132, 214], // back range rock: a 90s blue-violet
    [92, 104, 186],  // back range, shadow side
    [250, 252, 255], // back range snow
    [84, 112, 190],  // front ridge rock
    [66, 90, 166],   // front ridge, shadow side
    [236, 242, 252], // front ridge snow
    [192, 206, 240], // snow in shadow
];

/// A peak: where, how high (bricks) and how steeply its slopes fall (bricks
/// down per brick along).
struct Peak {
    x: f32,
    top: f32,
    slope: f32,
}

/// `n` peaks spread round a loop `w` bricks long.
fn peaks(n: usize, w: usize, tops: (f32, f32), slopes: (f32, f32), salt: u64) -> Vec<Peak> {
    (0..n)
        .map(|i| Peak {
            x: (i as f32 + 0.2 + 0.6 * hash(i, 0, 0, salt)) * w as f32 / n as f32,
            top: tops.0 + (tops.1 - tops.0) * hash(i, 1, 0, salt),
            slope: slopes.0 + (slopes.1 - slopes.0) * hash(i, 2, 0, salt),
        })
        .collect()
}

/// A ridge's height over column `x` (bricks), and which way from its
/// highest peak the column lies (negative: the sunny left slope).
fn ridge(peaks: &[Peak], x: usize, w: usize) -> (f32, f32) {
    peaks
        .iter()
        .map(|p| {
            let dx = loop_dx(x as f32 + 0.5, p.x, w as f32);
            (p.top - p.slope * dx.abs(), dx)
        })
        .fold((0.0, 0.0), |best, r| if r.0 > best.0 { r } else { best })
}

/// One tile of mountains: a paler back range — four big snowy peaks with
/// smaller shoulders round them — behind a darker, lower ridge of rolling
/// foothills. The slopes step two bricks along for one up, never steeper:
/// sheer columns of bricks read as towers, and this is no city. The big
/// peaks rise into the clouds' band, so now and then one stands in front of
/// a cloud.
pub fn mountains() -> Grid {
    let (w, h) = (MOUNTAIN_TILE, MOUNTAIN_LAYERS);
    let mut g = Grid::new(w, h, 3);
    let mut back = peaks(4, w, (19.0, 25.0), (0.42, 0.58), 31);
    back.extend(peaks(7, w, (11.0, 17.0), (0.45, 0.6), 33));
    let front = peaks(6, w, (7.0, 12.0), (0.32, 0.5), 37);
    // (peaks, z rows, rock, shade, snow, snowline in bricks, salt)
    let ranges = [
        (&back, 0..1, PEAK_ROCK, PEAK_SHADE, PEAK_SNOW, 18.0, 41),
        (&front, 1..3, RIDGE_ROCK, RIDGE_SHADE, RIDGE_SNOW, 13.0, 43),
    ];
    for x in 0..w {
        for (peaks, zs, rock, shade, snow, snowline, salt) in ranges.iter().cloned() {
            let (top, dx) = ridge(peaks, x, w);
            let top = top.clamp(0.0, h as f32) as usize;
            let snow_depth = 2 + (hash(x, 1, 0, salt) * 2.0) as usize;
            let sunny = dx < 0.0;
            for y in 0..top {
                let c = if top as f32 >= snowline && y + snow_depth >= top {
                    if sunny { snow } else { SNOW_SHADE }
                } else if sunny {
                    rock
                } else {
                    shade
                };
                for z in zs.clone() {
                    g.set(x, y, z, c);
                }
            }
        }
    }
    g
}

/// One jungle brick, world units.
pub const JUNGLE_CELL: f32 = 0.25;
/// Bricks along one tile (48 units).
pub const JUNGLE_TILE: usize = 192;
/// Bricks from the bottom of the jungle to its tallest treetop.
pub const JUNGLE_LAYERS: usize = 15;
/// World y of the jungle's bottom.
pub const JUNGLE_BASE: f32 = -1.0;

const LEAF_SUN: u8 = 1;
const LEAF: u8 = 2;
const LEAF_DEEP: u8 = 3;
const UNDERGROWTH: u8 = 4;
pub const JUNGLE_PALETTE: [[u8; 3]; 4] = [
    [132, 204, 96], // leaves in the sun
    [76, 162, 78],  // leaves
    [46, 124, 72],  // leaves in shade
    [30, 92, 64],   // undergrowth
];

/// One tile of jungle: a bumpy canopy of round treetops (lit on the left,
/// shaded below) over dark undergrowth. The palms stand in a plane of their
/// own behind it (`palm_grove`).
pub fn jungle() -> Grid {
    let (w, h) = (JUNGLE_TILE, JUNGLE_LAYERS);
    let mut g = Grid::new(w, h, 2);
    let n = w / 7;
    let crowns: Vec<(f32, f32, f32)> = (0..n)
        .map(|i| {
            let x = (i as f32 + hash(i, 0, 0, 51)) * w as f32 / n as f32;
            let r = 3.0 + 3.0 * hash(i, 1, 0, 51);
            let c = 6.0 + 3.0 * hash(i, 2, 0, 51);
            (x, r, c)
        })
        .collect();
    for x in 0..w {
        // The tallest crown over this column, and which side of it we're on.
        let (top, dx) = crowns
            .iter()
            .filter_map(|&(cx, r, c)| {
                let dx = loop_dx(x as f32 + 0.5, cx, w as f32);
                (dx.abs() <= r).then(|| (c + (r * r - dx * dx).sqrt(), dx))
            })
            .fold((6.0, 0.0), |best, t| if t.0 > best.0 { t } else { best });
        let top = (top as usize).min(h);
        let sunny = dx < 0.0;
        for y in 0..top {
            let below = top - y;
            let c = if y < 3 {
                UNDERGROWTH
            } else if below <= 2 && sunny {
                LEAF_SUN
            } else if below <= 3 {
                LEAF
            } else {
                LEAF_DEEP
            };
            g.set(x, y, 0, c);
            g.set(x, y, 1, c);
        }
    }
    g
}

/// A palm grove: one tile of palms, laid end to end in a plane of its own.
/// There are two: [`NEAR_GROVE`] behind the jungle, and [`FAR_GROVE`] farther
/// back, plainer and a little paler, so the farther a plane lies, the more it
/// looks like the sky. (Most of the distance comes from the mist it stands
/// in; recoloured all the way to the mountains' blue, it stopped reading as
/// palms.)
pub struct Grove {
    /// One brick, world units.
    pub cell: f32,
    /// Bricks along one tile.
    pub tile: usize,
    /// Bricks from the grove's bottom to the tip of the tallest frond.
    pub layers: usize,
    /// World y of the grove's bottom: below the planes in front, which hide
    /// the trunks' feet.
    pub base: f32,
    pub palette: [[u8; 3]; 6],
    /// Trunk lengths, bricks (inclusive), and fronds per crown.
    heights: (usize, usize),
    fronds: (usize, usize),
    /// Leaflets, coconuts, trunk rings and thick feet: the near grove's.
    detail: bool,
    salt: u64,
}

const TRUNK: u8 = 1;
const TRUNK_RING: u8 = 2;
const FROND: u8 = 3;
const FROND_SUN: u8 = 4;
const FROND_DEEP: u8 = 5;
const COCONUT: u8 = 6;

/// The palms behind the jungle: every detail, in the jungle's bricks.
pub const NEAR_GROVE: Grove = Grove {
    cell: 0.25,
    tile: 160, // 40 units
    layers: 30,
    base: -0.5,
    palette: [
        [150, 112, 76],  // trunk
        [116, 84, 58],   // a ring round the trunk
        [70, 160, 82],   // fronds
        [126, 204, 100], // fronds in the sun
        [42, 120, 70],   // leaflets underneath, in shade
        [104, 70, 44],   // coconuts
    ],
    heights: (10, 20),
    fronds: (6, 9),
    detail: true,
    salt: 61,
};

/// The palms beyond them: chunkier bricks, plainer crowns, and greens a
/// step lighter and cooler, toward the sky.
pub const FAR_GROVE: Grove = Grove {
    cell: 0.32,
    tile: 128, // 41 units
    layers: 24,
    base: -0.5,
    palette: [
        [146, 126, 108], // trunk
        [146, 126, 108], // (no rings this far off)
        [94, 168, 112],  // fronds
        [140, 206, 136], // fronds in the sun
        [94, 168, 112],  // (no leaflets)
        [146, 126, 108], // (no coconuts)
    ],
    heights: (8, 14),
    fronds: (4, 5),
    detail: false,
    salt: 71,
};

/// One palm of a grove, in bricks.
#[derive(Debug)]
pub struct Palm {
    /// The trunk's foot, a column of the tile.
    pub x: usize,
    /// Trunk length.
    pub height: usize,
    /// How far the crown sits sideways from the foot (negative: left). The
    /// trunk leans from the foot and bends upright toward the crown, the way
    /// a coconut palm grows toward the light.
    pub lean: f32,
    /// Each frond: (angle in degrees from pointing right, through up, to
    /// pointing left; length; how far its tip droops).
    pub fronds: Vec<(f32, f32, f32)>,
    pub coconuts: bool,
}

impl Palm {
    /// The trunk's sideways shift `y` bricks up.
    fn shift(&self, y: usize) -> isize {
        let u = y as f32 / self.height as f32;
        (self.lean * (1.0 - (1.0 - u) * (1.0 - u))).round() as isize
    }

    /// Where the crown sits: (column, row).
    fn crown(&self) -> (isize, isize) {
        (
            self.x as isize + self.shift(self.height),
            self.height as isize,
        )
    }
}

/// A grove's palms: spread along the tile in loose clumps, every one its own
/// — short or tall, straight or leaning either way, a sparse crown or a full
/// one, with coconuts or without (near ones only).
pub fn palms(grove: &Grove) -> Vec<Palm> {
    let salt = grove.salt;
    let (h0, h1) = grove.heights;
    let (f0, f1) = grove.fronds;
    let mut out = Vec::new();
    let mut x = (hash(0, 0, 0, salt) * 8.0) as usize;
    let mut i = 0;
    let mut leaners = 0;
    while x < grove.tile {
        let r = |k: usize| hash(i, k, 0, salt);
        let height = h0 + (r(1) * (h1 - h0 + 1) as f32) as usize;
        // About a third stand straight; the rest lean, by turns one way and
        // the other, the way palms in a clump lean apart.
        let lean = if r(2) < 0.3 {
            0.0
        } else {
            leaners += 1;
            let amount = 2.0 + 4.0 * r(3);
            if leaners % 2 == 0 { amount } else { -amount }
        };
        let n = f0 + (r(4) * (f1 - f0 + 1) as f32) as usize;
        let reach = 5.5 + height as f32 / 5.0; // taller palms, longer fronds
        let fronds = (0..n)
            .map(|k| {
                let rk = |m: usize| hash(i, 10 + k, m, salt + 2);
                // Half to each side, fanned from nearly level to steeply up
                // — never straight up — so every frond arches out on its own.
                let side = k % 2;
                let fan = (k / 2) as f32 / n.div_ceil(2) as f32;
                let rise = -5.0 + 75.0 * (fan + 0.25 * rk(0)).min(1.0);
                let angle = if side == 0 { rise } else { 180.0 - rise };
                let len = reach * (0.8 + 0.3 * rk(1));
                // Long and heavy: the tip falls back below where it rose.
                let droop = len * (0.45 + 0.4 * rk(2));
                (angle, len, droop)
            })
            .collect();
        out.push(Palm {
            x,
            height,
            lean,
            fronds,
            coconuts: grove.detail && r(5) < 0.65,
        });
        // Loose clumps: a close neighbour now and then, else a wider gap.
        x += if r(6) < 0.3 { 5 } else { 11 } + (r(7) * 9.0) as usize;
        i += 1;
    }
    out
}

/// One tile of a grove: its [`palms`], one brick deep. Their fronds wrap
/// round the tile's ends, so tiles join without a seam.
pub fn palm_grove(grove: &Grove) -> Grid {
    let (w, h) = (grove.tile, grove.layers);
    let detail = grove.detail;
    let mut g = Grid::new(w, h, 1);
    let mut put = |x: isize, y: isize, c: u8| {
        if y >= 0 {
            g.set(x.rem_euclid(w as isize) as usize, y as usize, 0, c);
        }
    };
    for p in palms(grove) {
        // The trunk: ringed, and two bricks thick at the foot of a tall one,
        // up close; plain far off.
        for y in 0..p.height {
            let x = p.x as isize + p.shift(y);
            let c = if detail && y % 3 == 2 {
                TRUNK_RING
            } else {
                TRUNK
            };
            put(x, y as isize, c);
            if detail && p.height >= 15 && y < p.height / 3 {
                put(x + 1, y as isize, c);
            }
        }
        let (cx, cy) = p.crown();
        // The fronds: arcs out from the crown, rising and then drooping
        // toward their tips, sunlit on the left; up close, leaflets hang
        // along their outer halves.
        for &(angle, len, droop) in &p.fronds {
            let (dx, dy) = (angle.to_radians().cos(), angle.to_radians().sin());
            let colour = if dx < 0.0 { FROND_SUN } else { FROND };
            let steps = (len * 2.0) as usize;
            for s in 2..=steps {
                let t = s as f32 / 2.0;
                let fx = cx + (t * dx).round() as isize;
                let fy = cy + (t * dy - droop * (t / len).powi(2)).round() as isize;
                put(fx, fy, colour);
                if detail && s % 3 == 0 && t > len * 0.45 && t < len - 0.5 {
                    put(fx, fy - 1, FROND_DEEP);
                }
            }
        }
        put(cx, cy, FROND_SUN);
        put(cx, cy + 1, FROND_SUN);
        if p.coconuts {
            put(cx, cy - 1, COCONUT);
            put(cx - 1, cy - 1, COCONUT);
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

// ---- the vowel tablets ------------------------------------------------------

/// One tablet brick, world units: a little finer than an obstacle's, so a
/// letter's two-brick strokes stay crisp.
pub const TABLET_CELL: f32 = 0.1;
/// A tablet's size in bricks: 13 wide and 15 tall (1.3 × 1.5 units, about
/// the old flat sign), and 4 thick, the carving one brick deep.
pub const TABLET_W: usize = 13;
pub const TABLET_H: usize = 15;
pub const TABLET_D: usize = 4;
/// The outline's corner radii, in bricks: an arched top over a squarer
/// bottom, the shape of a tablet of old.
const TABLET_ARCH: f32 = 5.0;
const TABLET_FOOT: f32 = 3.0;

const LIME: u8 = 1;
const LIME_LIGHT: u8 = 2;
const LIME_DARK: u8 = 3;
const LIME_SPECK: u8 = 4;
const PAINT: u8 = 5;

/// The paint in each move's carved letter (jump, duck, shoot): the family
/// colour of the obstacles it gets past, deep, so it reads strong on the
/// brightly lit white stone (see runner.rs's `stone_glass`).
pub const TABLET_PAINT: [[u8; 3]; 3] = [[150, 18, 78], [66, 30, 156], [150, 38, 18]];
/// The same letters lit up, while the child is saying their vowel: the
/// family colour at its most vivid. The stone stays white.
pub const TABLET_LIT_PAINT: [[u8; 3]; 3] = [[255, 20, 120], [110, 40, 255], [255, 60, 10]];

/// A tablet's colours: white limestone, and `paint` in the carved letter
/// (grid colour n = `tablet_palette(..)[n - 1]`). The white is the stone's
/// own, kept for it: a tablet stands apart from the colourful world around
/// it, and its letter reads on it at a glance. Its shades are cool greys,
/// never cream, which would sit with the earth.
pub fn tablet_palette(paint: [u8; 3]) -> [[u8; 3]; 5] {
    [
        [248, 249, 250], // limestone
        [255, 255, 255], // limestone, light
        [230, 233, 237], // limestone, weathered
        [204, 208, 214], // a speck
        paint,
    ]
}

/// Whether brick `(x, y)` of a tablet's front (y up) lies inside its
/// outline, drawn `inset` bricks in from the edge: a rectangle with its
/// corners rounded, the top ones more.
fn on_tablet(x: usize, y: usize, inset: f32) -> bool {
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let (w, h) = (TABLET_W as f32, TABLET_H as f32);
    if px < inset || px > w - inset || py < inset || py > h - inset {
        return false;
    }
    let r = if py > h / 2.0 {
        TABLET_ARCH
    } else {
        TABLET_FOOT
    } - inset;
    // The nearest point of the rectangle shrunk by r: within r of it is in.
    let cx = px.clamp(inset + r, w - inset - r);
    let cy = py.clamp(inset + r, h - inset - r);
    (px - cx).powi(2) + (py - cy).powi(2) <= r * r
}

/// Where the vowel's bitmap sits on the tablet's front, in bricks: its
/// top-left corner, x from the left and y from the *top* (the bitmap's own
/// rows). Centred; when it can't be exact, a touch low, under the arch.
fn tablet_letter_at(bitmap: rondelek_core::arcade::Bitmap) -> (usize, usize) {
    let (bw, bh) = rondelek_core::arcade::bitmap_size(bitmap);
    ((TABLET_W + 1 - bw) / 2, (TABLET_H + 1 - bh) / 2)
}

/// A stone tablet with `label`'s vowel carved in it: a slab of limestone,
/// arched on top, pillowed front and back (the outermost ring of each set
/// back a brick, so its edges read as worn round), with a nick in the rim
/// here and there, and the arcade's bold pixel vowel
/// (`rondelek_core::arcade::vowel_glyph`)
/// cut a brick deep into the face, its floor painted. The face itself stays
/// plain, so nothing but the letter reads as a mark. An unknown label gives a
/// blank tablet.
pub fn tablet(label: &str) -> Grid {
    let (w, h, d) = (TABLET_W, TABLET_H, TABLET_D);
    let mut g = Grid::new(w, h, d);
    let salt = label.bytes().map(u64::from).sum::<u64>();
    let letter: Vec<(usize, usize)> = match rondelek_core::arcade::vowel_glyph(label) {
        Some((b, _)) => {
            let (x0, y0) = tablet_letter_at(b);
            rondelek_core::arcade::bitmap_cells(b)
                .map(|(col, row)| (x0 + col, h - 1 - (y0 + row)))
                .collect()
        }
        None => Vec::new(),
    };
    for y in 0..h {
        for x in 0..w {
            if !on_tablet(x, y, 0.0) {
                continue;
            }
            let face = on_tablet(x, y, 1.0);
            let r = |salt2| hash(x, y, salt as usize, salt2);
            // The rim weathers darker, specked; the face only varies faintly.
            let stone = if face {
                if r(31) < 0.3 { LIME_LIGHT } else { LIME }
            } else {
                match r(32) {
                    v if v < 0.12 => LIME_SPECK,
                    v if v < 0.55 => LIME_DARK,
                    _ => LIME,
                }
            };
            // Hewn, just: now and then a nick out of the rim's front half.
            // The outline itself stays whole (notches right through it made
            // the sides look jagged).
            let nick = !face && r(33) < 0.1;
            for z in 0..d {
                let outer = z == 0 || z == d - 1;
                if outer && !face {
                    continue; // the pillowed edge
                }
                if z == d - 2 && nick {
                    continue;
                }
                g.set(x, y, z, stone);
            }
        }
    }
    for &(x, y) in &letter {
        g.set(x, y, d - 1, 0);
        g.set(x, y, d - 2, PAINT);
    }
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
            ("tablet y", tablet("y")),
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

    /// The highest filled brick's top over each column, in bricks.
    fn skyline(g: &Grid) -> Vec<usize> {
        (0..g.w as isize)
            .map(|x| {
                (0..g.h as isize)
                    .rev()
                    .find(|&y| (0..g.d as isize).any(|z| g.get(x, y, z) != 0))
                    .map_or(0, |y| y as usize + 1)
            })
            .collect()
    }

    #[test]
    fn some_mountains_rise_into_the_clouds() {
        // The clouds' band runs from about 3.3 units up to 9 (runner.rs):
        // the big peaks rise into it but stay well under its top, so the
        // clouds still float above them, and the valleys between dip below
        // it, so the range is no wall.
        let tops: Vec<f32> = skyline(&mountains())
            .iter()
            .map(|&b| MOUNTAIN_BASE + b as f32 * MOUNTAIN_CELL)
            .collect();
        let tallest = tops.iter().cloned().fold(0.0, f32::max);
        assert!((5.0..=8.0).contains(&tallest), "tallest peak {tallest}");
        assert!(tops.iter().any(|&t| t < 3.3), "no valley under the clouds");
    }

    #[test]
    fn the_backdrops_have_no_gaps_and_fit_raylibs_indices() {
        use crate::bricks::{Build, vertex_count};
        let how = Build {
            wrap_x: true,
            open_below_and_behind: true,
        };
        for (name, g) in [("mountains", mountains()), ("jungle", jungle())] {
            assert!(skyline(&g).iter().all(|&t| t > 0), "{name} has a gap");
            let n = vertex_count(&g, how);
            assert!(n <= u16::MAX as usize + 1, "{name}: {n} vertices");
        }
    }

    #[test]
    fn the_jungle_is_greenery_only() {
        // The palms moved to a plane of their own.
        assert_eq!(JUNGLE_PALETTE.len(), 4);
        let g = jungle();
        for z in 0..g.d as isize {
            for y in 0..g.h as isize {
                for x in 0..g.w as isize {
                    assert!((g.get(x, y, z) as usize) <= JUNGLE_PALETTE.len());
                }
            }
        }
    }

    #[test]
    fn every_palm_is_its_own() {
        for (name, grove) in [("near", &NEAR_GROVE), ("far", &FAR_GROVE)] {
            let palms = palms(grove);
            assert!(palms.len() >= 7, "{name}: {} palms", palms.len());
            let heights: Vec<usize> = palms.iter().map(|p| p.height).collect();
            let (lo, hi) = (heights.iter().min().unwrap(), heights.iter().max().unwrap());
            assert!(hi - lo >= 4, "{name} heights {heights:?}");
            assert!(
                palms.iter().any(|p| p.lean < -1.0),
                "{name}: none leans left"
            );
            assert!(
                palms.iter().any(|p| p.lean > 1.0),
                "{name}: none leans right"
            );
            assert!(
                palms.iter().any(|p| p.lean == 0.0),
                "{name}: none stands straight"
            );
            let crowns: Vec<usize> = palms.iter().map(|p| p.fronds.len()).collect();
            let (f0, f1) = grove.fronds;
            assert!(
                crowns.iter().all(|n| (f0..=f1).contains(n)),
                "{name} {crowns:?}"
            );
            assert!(
                crowns.iter().any(|&n| n != crowns[0]),
                "{name}: every crown alike"
            );
        }
    }

    #[test]
    fn the_far_grove_is_plainer_and_paler() {
        // Fewer fronds, nothing hung on them, bigger bricks…
        const { assert!(FAR_GROVE.fronds.1 < NEAR_GROVE.fronds.0) };
        assert!(palms(&FAR_GROVE).iter().all(|p| !p.coconuts));
        assert!(palms(&NEAR_GROVE).iter().any(|p| p.coconuts));
        const { assert!(FAR_GROVE.cell > NEAR_GROVE.cell) };
        // …and every colour a step nearer the sky's horizon (runner.rs's
        // SKY_LOW) than the near grove's.
        let sky = [184u8, 228, 255];
        let dist = |c: [u8; 3]| {
            (0..3)
                .map(|k| (c[k] as f32 - sky[k] as f32).powi(2))
                .sum::<f32>()
        };
        for k in [TRUNK, FROND, FROND_SUN] {
            let k = k as usize - 1;
            assert!(dist(FAR_GROVE.palette[k]) < dist(NEAR_GROVE.palette[k]));
        }
    }

    #[test]
    fn every_palm_fits_its_grove() {
        for grove in [&NEAR_GROVE, &FAR_GROVE] {
            // Nothing is cut off at the top: a crown plus its highest frond.
            for p in palms(grove) {
                let up = p
                    .fronds
                    .iter()
                    .map(|f| f.1 * f.0.to_radians().sin())
                    .fold(0.0, f32::max);
                assert!(
                    (p.height + 2) as f32 + up < grove.layers as f32,
                    "a {}-brick palm reaching {up} up",
                    p.height
                );
            }
            // And the tile fits raylib's 16-bit indices, all faces built
            // (crowns rise above the camera, so their undersides show).
            let how = crate::bricks::Build {
                wrap_x: true,
                open_below_and_behind: false,
            };
            let n = crate::bricks::vertex_count(&palm_grove(grove), how);
            assert!(n <= u16::MAX as usize + 1, "{n} vertices");
        }
    }

    #[test]
    fn every_vowel_is_carved_clean_into_its_tablet() {
        let front = TABLET_D as isize - 1;
        for v in ["a", "e", "i", "o", "u", "y"] {
            let g = tablet(v);
            let (b, _) = rondelek_core::arcade::vowel_glyph(v).unwrap();
            let (x0, y0) = tablet_letter_at(b);
            let ink: Vec<(usize, usize)> = rondelek_core::arcade::bitmap_cells(b)
                .map(|(c, r)| (x0 + c, TABLET_H - 1 - (y0 + r)))
                .collect();
            for y in 0..TABLET_H {
                for x in 0..TABLET_W {
                    let (xi, yi) = (x as isize, y as isize);
                    if ink.contains(&(x, y)) {
                        // Cut a brick deep, painted at the bottom, and the
                        // whole letter on the face, clear of the worn rim.
                        assert!(on_tablet(x, y, 1.0), "{v}: ink on the rim at {x},{y}");
                        assert_eq!(g.get(xi, yi, front), 0, "{v}: not carved at {x},{y}");
                        assert_eq!(g.get(xi, yi, front - 1), PAINT, "{v}: unpainted");
                    } else if on_tablet(x, y, 1.0) {
                        // Nothing else on the face is a mark.
                        let c = g.get(xi, yi, front);
                        assert!([LIME, LIME_LIGHT].contains(&c), "{v}: {c} at {x},{y}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_tablets_outline_is_smooth() {
        // Every brick of the outline is there at the back of the rim: the
        // nicks only ever come out of its front half.
        for v in ["a", "e", "i", "o", "u", "y"] {
            let g = tablet(v);
            for y in 0..TABLET_H {
                for x in 0..TABLET_W {
                    let inside = on_tablet(x, y, 0.0);
                    let back = g.get(x as isize, y as isize, 1) != 0;
                    assert_eq!(inside, back, "{v}: outline at {x},{y}");
                }
            }
        }
    }

    #[test]
    fn a_tablet_is_about_the_old_signs_size() {
        // The flat sign was 88 px square on a 720-px screen: about 1.27
        // world units at the hero's depth. A tablet is as wide, a bit taller.
        let [w, h, d] = size(&tablet("a"), TABLET_CELL);
        assert!(
            (1.2..=1.4).contains(&w) && (1.3..=1.6).contains(&h),
            "{w}×{h}"
        );
        assert_eq!(d, TABLET_D as f32 * TABLET_CELL);
        // Unknown labels give a blank slab rather than a panic.
        let blank = tablet("x");
        assert!(
            (0..TABLET_H as isize).all(|y| (0..TABLET_W as isize).all(|x| !on_tablet(
                x as usize, y as usize, 1.0
            ) || blank.get(
                x,
                y,
                TABLET_D as isize - 1
            ) != 0))
        );
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
