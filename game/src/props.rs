//! Vowel Runner's code-built props: the meadow the hero runs on, its
//! ledges, the far planes, and the stone tablets the obstacles' vowels are
//! carved in, each a [`Grid`] of coloured bricks (see `bricks.rs`), lit by
//! Lam::pula like everything else in the scene. The obstacles themselves are
//! in `obstacles.rs`.
//!
//! Everything here is deterministic — a hash of the brick's place, never a
//! random generator — so the meadow and the jungle look the same every
//! game, and a test can hold them to their sizes.

use super::bricks::{Build, Grid};

/// Deterministic hash of a brick's place (plus a salt) → 0..1.
pub(crate) fn hash(x: usize, y: usize, z: usize, salt: u64) -> f32 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9)
        ^ salt.wrapping_mul(0x27D4_EB2F_1656_67C5);
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 29;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Perlin's gradient noise at `p` (in lattice steps), about −1..1: smooth
/// rises and dips a lattice step or so across, the same for the same place
/// every time. It repeats every `period` steps along x, so a tile laid end to
/// end meets itself without a seam. For `p` at or above 0.
pub(crate) fn gradient_noise(p: [f32; 3], period: usize, salt: u64) -> f32 {
    // The twelve directions from a cube's centre to the middles of its edges.
    const DIRS: [[f32; 3]; 12] = [
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [1.0, -1.0, 0.0],
        [-1.0, -1.0, 0.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
        [1.0, 0.0, -1.0],
        [-1.0, 0.0, -1.0],
        [0.0, 1.0, 1.0],
        [0.0, -1.0, 1.0],
        [0.0, 1.0, -1.0],
        [0.0, -1.0, -1.0],
    ];
    let low = p.map(|v| v.floor() as usize);
    let f = [0, 1, 2].map(|k| p[k] - low[k] as f32);
    // A lattice corner's slope: rising along its own direction, through 0
    // at the corner itself.
    let corner = |c: [usize; 3]| {
        let g = DIRS[(hash((low[0] + c[0]) % period, low[1] + c[1], low[2] + c[2], salt) * 12.0)
            as usize
            % 12];
        (0..3).map(|k| g[k] * (f[k] - c[k] as f32)).sum::<f32>()
    };
    // Blended with Perlin's quintic, smooth to the second derivative: no
    // crease shows where the lattice steps.
    let u = f.map(|t| t * t * t * (t * (t * 6.0 - 15.0) + 10.0));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let along_x = |y, z| lerp(corner([0, y, z]), corner([1, y, z]), u[0]);
    let along_y = |z| lerp(along_x(0, z), along_x(1, z), u[1]);
    lerp(along_y(0), along_y(1), u[2])
}

/// 0 below `a`, 1 above `b`, and an S-curve between.
pub(crate) fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
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
// Three far planes, each one tile of bricks laid end to end and scrolled at
// its own rate (runner.rs): a mountain range at the back, a palm grove, and a
// jungle in front. The mountains and the palms are pixel-art silhouettes
// extruded a brick or few deep — the way flat-draw turns a drawing into a
// prop — while the jungle is built in the round. All start below the
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
pub const MOUNTAIN_LAYERS: usize = 28;
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
/// peaks rise high into the clouds' band, well over the jungle's canopy in
/// front, so now and then one stands in front of a cloud.
pub fn mountains() -> Grid {
    let (w, h) = (MOUNTAIN_TILE, MOUNTAIN_LAYERS);
    let mut g = Grid::new(w, h, 3);
    let mut back = peaks(4, w, (21.0, 27.0), (0.42, 0.58), 31);
    back.extend(peaks(7, w, (13.0, 19.0), (0.45, 0.6), 33));
    let front = peaks(6, w, (9.0, 14.0), (0.32, 0.5), 37);
    // (peaks, z rows, rock, shade, snow, snowline in bricks, salt)
    let ranges = [
        (&back, 0..1, PEAK_ROCK, PEAK_SHADE, PEAK_SNOW, 20.0, 41),
        (&front, 1..3, RIDGE_ROCK, RIDGE_SHADE, RIDGE_SNOW, 15.0, 43),
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
pub const JUNGLE_LAYERS: usize = 26;
/// Bricks front to back (2.5 units): room for the jungle's layers, trees at
/// the back to ferns at the front.
pub const JUNGLE_DEPTH: usize = 10;
/// World y of the jungle's bottom.
pub const JUNGLE_BASE: f32 = -1.0;
/// How the jungle is walled in: no backs (never seen), but bottoms, because
/// the canopies and fronds above the camera's eye show their undersides.
pub const JUNGLE_BUILD: Build = Build {
    wrap_x: true,
    open_below: false,
    open_behind: true,
};

// Two families of leaves, so neighbouring plants differ: a warm, sunny green
// and a cool emerald, each in four shades from sunlit to deep shade. Then the
// floor, the trees' pale bark, the palms' trunks, the lianas, and the light
// midrib down a big leaf.
const WARM: u8 = 1; // the warm family's sunlit shade; WARM + 1..3 darker
const COOL: u8 = 5; // the same for the cool family
const UNDERGROWTH: u8 = 9;
const BARK: u8 = 10;
const BARK_SHADE: u8 = 11;
const PALM_TRUNK: u8 = 12;
const LIANA: u8 = 13;
const MIDRIB: u8 = 14;
pub const JUNGLE_PALETTE: [[u8; 3]; 14] = [
    [156, 216, 92],  // warm: in the sun
    [104, 184, 74],  // warm
    [64, 146, 66],   // warm: in shade
    [40, 108, 60],   // warm: deep shade
    [118, 210, 118], // cool: in the sun
    [66, 172, 98],   // cool
    [40, 132, 86],   // cool: in shade
    [26, 98, 74],    // cool: deep shade
    [24, 78, 58],    // undergrowth
    [176, 172, 150], // a rainforest tree's pale bark
    [128, 126, 110], // bark, the shaded side
    [124, 96, 66],   // a palm's trunk
    [92, 112, 52],   // a liana
    [186, 230, 126], // the midrib of a big leaf
];

/// Bricks up from the jungle's bottom to the forest floor's top.
const FLOOR: usize = 2;

/// A round mass of leaves: an ellipsoid of bricks, centre and radii in
/// bricks, of one leaf family.
struct Lobe {
    x: f32,
    y: f32,
    z: f32,
    r: [f32; 3],
    family: u8,
    /// Added to its light: below 0, a mass deeper in the shade.
    bias: f32,
}

/// Paints a jungle tile: wraps x round the tile (so plants near an end carry
/// on at the other), ignores anything above or behind the grid.
struct Jungle {
    g: Grid,
    /// Which palm painted each cell last (its number + 1; 0 for any other
    /// plant), so the wind knows what it may move: see [`Sway`].
    palm_of: Grid,
    /// The palm being painted now (its number + 1), or 0: what `put` marks.
    painting: u8,
    /// Every palm, by number.
    palms: Vec<PalmShape>,
}

/// Where a jungle palm stands, as the wind needs it, in bricks.
struct PalmShape {
    /// The trunk's foot (y) and the crown's middle, where its fronds start.
    foot: f32,
    crown: [f32; 3],
    /// Its own beat in the wind, 0..1, so no two palms sway in step.
    phase: f32,
}

impl Jungle {
    fn new(w: usize, h: usize, d: usize) -> Self {
        Self {
            g: Grid::new(w, h, d),
            palm_of: Grid::new(w, h, d),
            painting: 0,
            palms: Vec::new(),
        }
    }

    fn put(&mut self, x: isize, y: isize, z: isize, c: u8) {
        if y >= 0 && z >= 0 {
            let x = x.rem_euclid(self.g.w as isize) as usize;
            self.g.set(x, y as usize, z as usize, c);
            self.palm_of.set(x, y as usize, z as usize, self.painting);
        }
    }

    fn put_at(&mut self, p: [f32; 3], c: u8) {
        self.put(
            p[0].floor() as isize,
            p[1].floor() as isize,
            p[2].floor() as isize,
            c,
        );
    }

    fn empty(&self, x: isize, y: isize, z: isize) -> bool {
        let x = x.rem_euclid(self.g.w as isize);
        self.g.get(x, y, z) == 0
    }

    /// A leaf or a frond: from `base`, heading `az` round the vertical
    /// (0 = right, a quarter turn = toward the camera) and rising `el`
    /// (radians), `len` bricks long, its tip drooping `droop` bricks. Its
    /// blade is `width(t)` bricks either side of the spine `t` bricks along,
    /// laid flat across the way it heads; the spine is `spine`, the blade
    /// `blade` on the sunny side and `under` on the other.
    #[allow(clippy::too_many_arguments)]
    fn leaf(
        &mut self,
        base: [f32; 3],
        (az, el): (f32, f32),
        len: f32,
        droop: f32,
        width: impl Fn(f32) -> f32,
        spine: u8,
        (blade, under): (u8, u8),
    ) {
        let (dx, dz) = (az.cos() * el.cos(), az.sin() * el.cos());
        // Across the leaf, level: the way its blade spreads.
        let (sx, sz) = (-az.sin(), az.cos());
        let mut t = 0.0;
        while t <= len {
            let p = [
                base[0] + dx * t,
                base[1] + el.sin() * t - droop * (t / len).powi(2),
                base[2] + dz * t,
            ];
            let half = width(t).round() as i32;
            for s in -half..=half {
                let c = match s {
                    0 => spine,
                    // The side toward the sun (the left) is the lit one.
                    s if (s as f32 * sx) < 0.0 => blade,
                    _ => under,
                };
                self.put_at([p[0] + sx * s as f32, p[1], p[2] + sz * s as f32], c);
            }
            t += 0.5;
        }
    }

    /// A palm frond, the way the palm groves draw theirs: a spine of single
    /// bricks arching out and drooping (see [`Jungle::leaf`] for the
    /// heading), sunlit on the left, with leaflets hanging under its outer
    /// two thirds.
    fn frond(&mut self, base: [f32; 3], (az, el): (f32, f32), len: f32, droop: f32, family: u8) {
        let (dx, dz) = (az.cos() * el.cos(), az.sin() * el.cos());
        let spine = if dx < 0.0 { family } else { family + 1 };
        let mut t = 1.0;
        let mut step = 0;
        while t <= len {
            let p = [
                base[0] + dx * t,
                base[1] + el.sin() * t - droop * (t / len).powi(2),
                base[2] + dz * t,
            ];
            self.put_at(p, spine);
            if t > len * 0.35 && step % 2 == 0 {
                self.put_at([p[0], p[1] - 1.0, p[2]], family + 2);
            }
            t += 0.5;
            step += 1;
        }
    }
}

/// The jungle's plants, from the back of the tile to the front: tall
/// rainforest trees, jungle palms, bushes, then the big-leaved plants and
/// ferns of the forest floor. Each is placed by a hash of its number, never a
/// random generator, so the jungle is the same every game.
fn jungle_plants() -> Jungle {
    let (w, h, d) = (JUNGLE_TILE, JUNGLE_LAYERS, JUNGLE_DEPTH);
    let mut j = Jungle::new(w, h, d);
    let wf = w as f32;
    let mut lobes = Vec::new();

    // The trees: a straight pale trunk, two bricks square, flaring into
    // buttress roots at the floor, a flat, wide canopy of leaf masses on
    // top, and lianas hanging from it.
    let trees = 7;
    let mut trunks = Vec::new();
    for i in 0..trees {
        let r = |k| hash(i, k, 0, 81);
        let x = ((i as f32 + 0.2 + 0.6 * r(0)) * wf / trees as f32) as isize;
        let top = 10 + (r(1) * 4.0) as isize;
        let family = if r(2) < 0.5 { WARM } else { COOL };
        trunks.push((x, top));
        // A crown of round masses heaped over the trunk's top, spreading
        // wide and tumbling lower at its edges, into the next tree's.
        for k in 0..7 + (r(3) * 3.0) as usize {
            let q = |m| hash(i, 10 + k * 5 + m, 0, 81);
            let out = (q(1) - 0.5) * 2.0; // -1 (left edge) .. 1 (right)
            let rad = 2.8 + 1.8 * q(0);
            lobes.push(Lobe {
                x: x as f32 + 1.0 + out * 10.0,
                y: top as f32 + 1.0 + 1.5 * q(2) - 3.5 * out * out,
                z: 2.0 + q(3) * 3.5,
                r: [rad * 1.25, rad * 0.85, rad * 0.8],
                family,
                bias: 0.0,
            });
        }
    }

    // The jungle's dark heart, at the back: masses of leaves in deep shade
    // from the floor up into the canopy, so between the trunks there is
    // jungle all the way back rather than sky.
    for i in 0..18 {
        let r = |k| hash(i, k, 5, 79);
        let rad = 4.5 + 2.0 * r(2);
        lobes.push(Lobe {
            x: (i as f32 + r(0)) * wf / 18.0,
            y: FLOOR as f32 + 4.0 + 4.0 * r(1),
            z: 0.6,
            r: [rad * 1.6, rad, 1.6],
            family: if r(3) < 0.5 { WARM } else { COOL },
            bias: -0.3,
        });
    }

    // Bushes between them, low down, so the floor never shows through.
    for i in 0..16 {
        let r = |k| hash(i, k, 1, 83);
        let rad = 2.6 + 1.6 * r(2);
        let family = if r(3) < 0.5 { WARM } else { COOL };
        let (x, y, z) = (
            (i as f32 + r(0)) * wf / 16.0,
            FLOOR as f32 + 1.5 + 2.5 * r(1),
            3.5 + 2.5 * r(4),
        );
        lobes.push(Lobe {
            x,
            y,
            z,
            r: [rad * 1.2, rad, rad * 0.8],
            family,
            bias: -0.35,
        });
        for k in 0..2 {
            let q = |m: usize| hash(i, 10 + k * 3 + m, 1, 83);
            let a = std::f32::consts::PI * (0.2 + 0.6 * q(0));
            let sub = rad * (0.45 + 0.15 * q(1));
            lobes.push(Lobe {
                x: x + a.cos() * rad,
                y: y + a.sin() * rad * 0.7,
                z: z + (q(2) - 0.5),
                r: [sub * 1.1, sub, sub * 0.9],
                family,
                bias: -0.35,
            });
        }
    }
    shade_lobes(&mut j, &lobes);

    // The forest floor.
    j.g.fill([0, 0, 0], [w, FLOOR, d], UNDERGROWTH);

    for &(x, top) in &trunks {
        // The trunk, lit on its left side; a fin of buttress root out each
        // way at its foot, narrowing as it climbs.
        for y in FLOOR as isize..top {
            for (dx, c) in [(0, BARK), (1, BARK_SHADE)] {
                for z in 2..4 {
                    if j.empty(x + dx, y, z) {
                        j.put(x + dx, y, z, c);
                    }
                }
            }
        }
        for up in 0..3isize {
            let y = FLOOR as isize + up;
            let reach = 3 - up;
            for s in 1..=reach {
                j.put(x - s, y, 2, BARK);
                j.put(x + 1 + s, y, 3, BARK_SHADE);
                j.put(x, y, 3 + s, BARK);
                j.put(x + 1, y, 2 - s.min(2), BARK_SHADE);
            }
        }
    }
    for (i, &(x, top)) in trunks.iter().enumerate() {
        // Lianas: hanging from under the canopy, in front of the trunk,
        // swaying a brick or so as they fall; some end in a few leaves.
        for k in 0..3 {
            let q = |m: usize| hash(i, 40 + k * 4 + m, 0, 81);
            let vx = x + [-7, -2, 5][k] + ((q(0) - 0.5) * 3.0) as isize;
            let vz = 5 + (q(1) * 2.0) as isize;
            let from = top - 1;
            let to = (from - 5 - (q(2) * 8.0) as isize).max(FLOOR as isize + 2);
            let phase = q(3) * 6.0;
            for y in to..from {
                let sway = ((y as f32 * 0.6 + phase).sin() * 0.8).round() as isize;
                j.put(vx + sway, y, vz, LIANA);
            }
            if q(2) > 0.5 {
                let sway = ((to as f32 * 0.6 + phase).sin() * 0.8).round() as isize;
                for (lx, ly) in [(-1, 0), (1, 0), (0, -1)] {
                    j.put(vx + sway + lx, to + ly, vz, COOL + 1);
                }
            }
        }
    }

    // Jungle palms: a slender trunk, leaning a little, rising through the
    // canopy, and a crown of long fronds fanned all the way round, toward the
    // camera too, their leaflets hanging — the jungle's silhouette.
    let palms = 6;
    for i in 0..palms {
        let r = |k| hash(i, k, 2, 85);
        let x = (i as f32 + 0.1 + 0.8 * r(0)) * wf / palms as f32;
        let z = 6.5 + 1.0 * r(1);
        let height = 15.0 + 4.0 * r(2);
        let lean = (r(3) - 0.5) * 8.0;
        let family = if r(4) < 0.5 { WARM } else { COOL };
        let crown = [x + lean, FLOOR as f32 + height, z];
        j.palms.push(PalmShape {
            foot: FLOOR as f32,
            crown,
            phase: r(6),
        });
        j.painting = j.palms.len() as u8;
        let mut y = FLOOR as f32;
        while y < FLOOR as f32 + height {
            let u = (y - FLOOR as f32) / height;
            j.put_at([x + lean * u * u, y, z], PALM_TRUNK);
            y += 1.0;
        }
        let fronds = 8 + (r(5) * 3.0) as usize;
        for k in 0..fronds {
            let q = |m| hash(i, 20 + k * 4 + m, 2, 85);
            let az = (k as f32 + 0.4 * q(0)) / fronds as f32 * std::f32::consts::TAU;
            let el = (10.0 + 35.0 * q(1)).to_radians();
            let len = 6.0 + 3.0 * q(2);
            j.frond(crown, (az, el), len, len * (0.5 + 0.3 * q(3)), family);
        }
        j.put_at(crown, family);
        j.put_at([crown[0], crown[1] + 1.0, crown[2]], family);
    }
    j.painting = 0;

    // The forest floor's big leaves: plants of a few broad leaves on short
    // stalks, opening toward the light and the camera.
    let big_leaves = 11;
    for i in 0..big_leaves {
        let r = |k| hash(i, k, 3, 87);
        let base = [
            (i as f32 + r(0)) * wf / big_leaves as f32,
            FLOOR as f32,
            8.0 + 1.5 * r(1),
        ];
        let family = if r(2) < 0.6 { WARM } else { COOL };
        let leaves = 3 + (r(3) * 3.0) as usize;
        for k in 0..leaves {
            let q = |m| hash(i, 20 + k * 4 + m, 3, 87);
            // Spread round the front half, from the right through the
            // camera to the left.
            let az = std::f32::consts::PI * (k as f32 + 0.5 * q(0)) / leaves as f32;
            let el = (45.0 + 25.0 * q(1)).to_radians();
            let len = 7.0 + 2.5 * q(2);
            let stalk = 2.0;
            j.leaf(
                base,
                (az, el),
                len,
                len * (0.3 + 0.2 * q(3)),
                |t| {
                    let u = (t - stalk) / (len - stalk);
                    if u <= 0.0 {
                        0.0
                    } else {
                        2.2 * (std::f32::consts::PI * u).sin()
                    }
                },
                MIDRIB,
                (family + 1, family + 2),
            );
        }
    }

    // Ferns: low tufts of arching fronds, at the very front.
    let ferns = 18;
    for i in 0..ferns {
        let r = |k| hash(i, k, 4, 89);
        let base = [
            (i as f32 + r(0)) * wf / ferns as f32,
            FLOOR as f32,
            8.0 + 1.5 * r(1),
        ];
        let family = if r(2) < 0.5 { COOL } else { WARM };
        let fronds = 6 + (r(3) * 4.0) as usize;
        for k in 0..fronds {
            let q = |m| hash(i, 20 + k * 4 + m, 4, 89);
            let az = (k as f32 + 0.5 * q(0)) / fronds as f32 * std::f32::consts::TAU;
            let el = (35.0 + 35.0 * q(1)).to_radians();
            let len = 3.0 + 2.0 * q(2);
            j.leaf(
                base,
                (az, el),
                len,
                len * 0.6,
                |t| if t > len * 0.3 { 1.0 } else { 0.0 },
                family + 1,
                (family, family + 2),
            );
        }
    }
    j
}

/// Fill the leaf masses, each brick shaded by where it sits on its own mass
/// — sunlit up and to the left, shaded below, darker toward the ground and
/// toward the back of the jungle — and darker still in the creases where
/// masses meet, so each reads as a ball of leaves, not a cut-out. The
/// surface is roughened brick by brick and the odd leaf is a shade lighter
/// or darker, for a leafy texture.
fn shade_lobes(j: &mut Jungle, lobes: &[Lobe]) {
    let (w, h, d) = (j.g.w, j.g.h, j.g.d);
    // Where the sun comes from: up, to the left and a little in front.
    let sun = {
        let (x, y, z) = (-0.5f32, 0.8f32, 0.45f32);
        let n = (x * x + y * y + z * z).sqrt();
        [x / n, y / n, z / n]
    };
    // First where the leaves are, with each brick's family and light; then
    // the shades, once every brick's neighbours are known.
    let mut leaves: Vec<(usize, usize, usize, u8, f32)> = Vec::new();
    for x in 0..w {
        let px = x as f32 + 0.5;
        let near: Vec<&Lobe> = lobes
            .iter()
            .filter(|l| loop_dx(px, l.x, w as f32).abs() <= l.r[0] + 1.0)
            .collect();
        for y in FLOOR..h {
            for z in 0..d {
                // The mass this brick is most inside of, if any: q < 1 is
                // inside, roughened by up to a seventh either way.
                let rough = 0.28 * (hash(x, y, z, 71) - 0.5);
                let best = near
                    .iter()
                    .map(|l| {
                        let n = [
                            loop_dx(px, l.x, w as f32) / l.r[0],
                            (y as f32 + 0.5 - l.y) / l.r[1],
                            (z as f32 + 0.5 - l.z) / l.r[2],
                        ];
                        (n[0] * n[0] + n[1] * n[1] + n[2] * n[2], n, l)
                    })
                    .filter(|(q, _, _)| *q <= 1.0 + rough)
                    .min_by(|a, b| a.0.total_cmp(&b.0));
                let Some((_, n, lobe)) = best else {
                    continue;
                };
                let family = lobe.family;
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-4);
                let mut lit = (n[0] * sun[0] + n[1] * sun[1] + n[2] * sun[2]) / len + lobe.bias;
                lit -= 0.5 * ((FLOOR as f32 + 3.0 - y as f32) / 4.0).clamp(0.0, 1.0);
                lit -= 0.55 * (1.0 - z as f32 / d as f32).powi(2);
                j.g.set(x, y, z, family);
                leaves.push((x, y, z, family, lit));
            }
        }
    }
    for (x, y, z, family, mut lit) in leaves {
        // A crease: a brick with more neighbours than one on the round of a
        // single mass (about a dozen of the 26) sits where masses meet.
        let mut around = 0;
        for dz in -1..=1isize {
            for dy in -1..=1isize {
                for dx in -1..=1isize {
                    let nx = (x as isize + dx).rem_euclid(w as isize);
                    if (dx, dy, dz) != (0, 0, 0)
                        && j.g.get(nx, y as isize + dy, z as isize + dz) != 0
                    {
                        around += 1;
                    }
                }
            }
        }
        lit -= 0.06 * (around as f32 - 13.0).max(0.0);
        let mut shade: u8 = match lit {
            v if v > 0.35 => 0,
            v if v > -0.05 => 1,
            v if v > -0.4 => 2,
            _ => 3,
        };
        match hash(x, y, z, 73) {
            v if v < 0.1 => shade = shade.saturating_sub(1),
            v if v > 0.92 => shade = (shade + 1).min(3),
            _ => {}
        }
        j.g.set(x, y, z, family + shade);
    }
}

/// One tile of jungle, in the round, the way a film's rainforest is: tall
/// pale-trunked trees with buttress roots, wide canopies and lianas, jungle
/// palms among them, bushes between, and big-leaved plants and ferns
/// crowding the floor — each at its own depth, the near ones hiding the far
/// ones' feet (see [`jungle_plants`]); its nearer bricks glassy (see
/// [`veil`]), its palms swaying in the wind (see [`Sway`]). The palm grove
/// stands in a plane of its own behind it (`palm_grove`).
pub fn jungle() -> JungleTile {
    let mut j = jungle_plants();
    veil(&mut j.g);
    let sway = Sway::of(&j);
    JungleTile { grid: j.g, sway }
}

/// One tile of jungle: its bricks, and what the wind may do to them.
pub struct JungleTile {
    pub grid: Grid,
    pub sway: Sway,
}

/// The longest a jungle palm's frond reaches from the crown, in bricks: a
/// frond's tip flutters the most.
const FROND_REACH: f32 = 9.0;

/// What the wind may do to each corner of a jungle tile's bricks, for the
/// vertex stage that does it (`sway.rs`): `[bend, leaf, phase, 0]`, each
/// 0..255. `bend` is how far the corner goes along with its palm's trunk,
/// nothing at the foot and growing toward the crown, where the fronds go
/// all the way with it; `leaf`, how much it flutters and gives in a gust on
/// top of that, nothing at the crown, the most at a frond's tip; `phase`,
/// its palm's own beat. Everything else stands still.
///
/// Kept per corner, not per brick: every face meeting at a corner moves it
/// alike, so the bricks bend together and never open a crack. (A plant
/// touching a palm gives a little where it touches.)
pub struct Sway {
    w: usize,
    h: usize,
    corners: Vec<[u8; 4]>,
}

impl Sway {
    /// A corner of the brick lattice, `x` round the tile (the corners on a
    /// tile's two ends are one).
    pub fn at(&self, [x, y, z]: [usize; 3]) -> [u8; 4] {
        self.corners
            .get((z * (self.h + 1) + y) * self.w + x % self.w)
            .copied()
            .unwrap_or_default()
    }

    /// Whether the wind moves any corner of brick (x, y, z).
    pub fn moves(&self, [x, y, z]: [usize; 3]) -> bool {
        (0..8).any(|k| {
            let [bend, leaf, ..] = self.at([x + (k & 1), y + (k >> 1 & 1), z + (k >> 2)]);
            bend > 0 || leaf > 0
        })
    }

    fn of(j: &Jungle) -> Self {
        let (w, h, d) = (j.g.w, j.g.h, j.g.d);
        let mut s = Sway {
            w,
            h,
            corners: vec![[0; 4]; w * (h + 1) * (d + 1)],
        };
        let byte = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let (cx, cy, cz) = (x as isize, y as isize, z as isize);
                    let palm = j.palm_of.get(cx, cy, cz) as usize;
                    if palm == 0 || j.g.get(cx, cy, cz) == 0 {
                        continue;
                    }
                    let p = &j.palms[palm - 1];
                    let trunk = j.g.get(cx, cy, cz) == PALM_TRUNK;
                    for k in 0..8 {
                        let c = [x + (k & 1), y + (k >> 1 & 1), z + (k >> 2)];
                        let [bend, leaf] = if trunk {
                            let up = (c[1] as f32 - p.foot) / (p.crown[1] - p.foot);
                            [up.clamp(0.0, 1.0).powi(2), 0.0]
                        } else {
                            let off = [
                                loop_dx(c[0] as f32, p.crown[0], w as f32),
                                c[1] as f32 - p.crown[1],
                                c[2] as f32 - p.crown[2],
                            ];
                            let reach = off.iter().map(|v| v * v).sum::<f32>().sqrt();
                            [1.0, (reach / FROND_REACH).clamp(0.0, 1.0).powf(1.3)]
                        };
                        let i = (c[2] * (h + 1) + c[1]) * w + c[0] % w;
                        let was = s.corners[i];
                        // Where two palms meet, the one moving it more has it.
                        let new = [byte(bend), byte(leaf), byte(p.phase), 0];
                        if new[0] as u16 + new[1] as u16 >= was[0] as u16 + was[1] as u16 {
                            s.corners[i] = new;
                        }
                    }
                }
            }
        }
        s
    }
}

/// How far back the jungle's veil reaches: 0 is the jungle's back, 1 its
/// front. Halfway: the palms, the big leaves and the ferns, the jungle's
/// front plane; the trees and the dark heart behind stay solid.
const VEIL_FROM: f32 = 0.5;
/// The most a veiled brick lets through: at the very front, in the thick of
/// a drift.
const VEIL_MOST: f32 = 0.75;
/// A drift's size: the noise's lattice step, in bricks. It divides
/// [`JUNGLE_TILE`], so the drifts run on across the seam between two tiles.
const VEIL_GRAIN: usize = 2;

/// Make the jungle's nearer bricks see-through, as the score sun's glass is:
/// the more the nearer they stand, and in drifts (a smooth noise) rather
/// than brick by brick, so a clump of leaves goes glassy together while the
/// next stays solid. Its back stays solid: the glass is seen against it.
fn veil(g: &mut Grid) {
    const SALT: u64 = 91;
    let grain = VEIL_GRAIN as f32;
    let period = g.w / VEIL_GRAIN;
    for z in 0..g.d {
        let depth = smoothstep(VEIL_FROM, 1.0, (z as f32 + 0.5) / g.d as f32);
        if depth <= 0.0 {
            continue;
        }
        for y in 0..g.h {
            for x in 0..g.w {
                if g.get(x as isize, y as isize, z as isize) == 0 {
                    continue;
                }
                // Two octaves: broad drifts, ruffled at half their size.
                let at = |k: f32| [x, y, z].map(|v| (v as f32 + 0.5) / grain * k);
                let n = gradient_noise(at(1.0), period, SALT)
                    + 0.5 * gradient_noise(at(2.0), 2 * period, SALT + 1);
                let drift = smoothstep(-0.2, 0.3, n);
                // In sixteenths: finer is never seen, and the barely veiled
                // round back to solid (glass costs faces; see `Grid::covers`).
                let opaque = ((1.0 - VEIL_MOST * depth * drift) * 16.0).round() / 16.0;
                g.set_alpha(x, y, z, (255.0 * opaque).round() as u8);
            }
        }
    }
}

/// A palm grove: one tile of palms, laid end to end in a plane of its own
/// behind the jungle ([`GROVE`]). Its distance comes from the mist it stands
/// in, not its colours: recoloured toward the mountains' blue, it stopped
/// reading as palms. (A second, farther grove was retired once the jungle
/// grew tall: it crowded the scene.)
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
    salt: u64,
}

const TRUNK: u8 = 1;
const TRUNK_RING: u8 = 2;
const FROND: u8 = 3;
const FROND_SUN: u8 = 4;
const FROND_DEEP: u8 = 5;
const COCONUT: u8 = 6;

/// The palms behind the jungle, in the jungle's bricks.
pub const GROVE: Grove = Grove {
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
    salt: 61,
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
/// one, with coconuts or without.
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
            coconuts: r(5) < 0.65,
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
    let mut g = Grid::new(w, h, 1);
    let mut put = |x: isize, y: isize, c: u8| {
        if y >= 0 {
            g.set(x.rem_euclid(w as isize) as usize, y as usize, 0, c);
        }
    };
    for p in palms(grove) {
        // The trunk: ringed, and two bricks thick at the foot of a tall one.
        for y in 0..p.height {
            let x = p.x as isize + p.shift(y);
            let c = if y % 3 == 2 { TRUNK_RING } else { TRUNK };
            put(x, y as isize, c);
            if p.height >= 15 && y < p.height / 3 {
                put(x + 1, y as isize, c);
            }
        }
        let (cx, cy) = p.crown();
        // The fronds: arcs out from the crown, rising and then drooping
        // toward their tips, sunlit on the left, leaflets hanging along
        // their outer halves.
        for &(angle, len, droop) in &p.fronds {
            let (dx, dy) = (angle.to_radians().cos(), angle.to_radians().sin());
            let colour = if dx < 0.0 { FROND_SUN } else { FROND };
            let steps = (len * 2.0) as usize;
            for s in 2..=steps {
                let t = s as f32 / 2.0;
                let fx = cx + (t * dx).round() as isize;
                let fy = cy + (t * dy - droop * (t / len).powi(2)).round() as isize;
                put(fx, fy, colour);
                if s % 3 == 0 && t > len * 0.45 && t < len - 0.5 {
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

// ---- the vowel tablets ------------------------------------------------------

/// How big a tablet is, plate and letter together, against the size it was
/// designed at (1): every brick of it scales, so the design stays the same.
const TABLET_SCALE: f32 = 0.76;
/// One tablet brick, world units: at full size half an obstacle brick, fine
/// enough for crisp edges, the carving's bevels and the frame's notches.
pub const TABLET_CELL: f32 = 0.05 * TABLET_SCALE;
/// A tablet's size in bricks: 30 wide and 32 tall (1.5 × 1.6 units at full
/// size), and 4 thick (0.2, a slab rather than a block).
pub const TABLET_W: usize = 30;
pub const TABLET_H: usize = 32;
pub const TABLET_D: usize = 4;
/// How many bricks across one pixel of the vowel's bitmap is carved.
const TABLET_PIXEL: usize = 2;
/// The frame, in bricks in from the edge: the ring of notches (square, two
/// bricks across, every five along a side), then the groove round the face.
const TABLET_NOTCHES: std::ops::RangeInclusive<usize> = 2..=3;
const TABLET_GROOVE: usize = 5;
/// How far the outline's corners are rounded, in bricks: just off sharp.
const TABLET_CORNER: f32 = 1.5;

const LIME: u8 = 1;
const LIME_LIGHT: u8 = 2;
const LIME_DARK: u8 = 3;
const LIME_SPECK: u8 = 4;
const PAINT: u8 = 5;
const TABLET_MOSS: u8 = 6;
const TABLET_MOSS_LIGHT: u8 = 7;
const CRACK: u8 = 8;

/// The paint in each move's carved letter (jump, duck, shoot): the family
/// colour of the obstacles it gets past, deep, so it reads strong on the
/// brightly lit white stone (see runner.rs's `stone_glass`).
pub const TABLET_PAINT: [[u8; 3]; 3] = [[150, 18, 78], [66, 30, 156], [150, 38, 18]];
/// The same letters lit up, while the child is saying their vowel: the
/// family colour at its most vivid. The stone stays white.
pub const TABLET_LIT_PAINT: [[u8; 3]; 3] = [[255, 20, 120], [110, 40, 255], [255, 60, 10]];

/// A tablet's colours: white limestone, `paint` in the carved letter, and
/// the weathering (grid colour n = `tablet_palette(..)[n - 1]`). The white
/// is the stone's own, kept for it: a tablet stands apart from the colourful
/// world around it, and its letter reads on it at a glance. Its shades are
/// cool greys, never cream, which would sit with the earth.
pub fn tablet_palette(paint: [u8; 3]) -> [[u8; 3]; 8] {
    [
        [248, 249, 250], // limestone
        [255, 255, 255], // limestone, light
        [204, 209, 217], // the floor of a carving, in its shadow
        [196, 200, 208], // a speck
        paint,
        [118, 176, 70],  // moss
        [152, 202, 92],  // moss, sunlit
        [118, 122, 132], // a crack
    ]
}

/// Whether brick `(x, y)` of a tablet's front (y up) lies inside its
/// outline: a rectangle, its corners just rounded.
fn on_tablet(x: usize, y: usize) -> bool {
    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
    let (w, h, r) = (TABLET_W as f32, TABLET_H as f32, TABLET_CORNER);
    let cx = px.clamp(r, w - r);
    let cy = py.clamp(r, h - r);
    (px - cx).powi(2) + (py - cy).powi(2) <= r * r
}

/// How many bricks in from the outline's straight edges brick `(x, y)` is.
fn tablet_ring(x: usize, y: usize) -> usize {
    x.min(y).min(TABLET_W - 1 - x).min(TABLET_H - 1 - y)
}

/// Where the vowel's bitmap sits on the tablet's front, in bricks: its
/// top-left corner, x from the left and y from the *top* (the bitmap's own
/// rows), each of its pixels [`TABLET_PIXEL`] bricks across. Centred; when
/// it can't be exact, a touch low.
fn tablet_letter_at(bitmap: rondelek_core::arcade::Bitmap) -> (usize, usize) {
    let (bw, bh) = rondelek_core::arcade::bitmap_size(bitmap);
    let (bw, bh) = (bw * TABLET_PIXEL, bh * TABLET_PIXEL);
    ((TABLET_W + 1 - bw) / 2, (TABLET_H + 1 - bh) / 2)
}

/// The bricks of `label`'s vowel on a tablet's front, (x, y) with y up.
fn tablet_letter(label: &str) -> Vec<(usize, usize)> {
    let Some((b, _)) = rondelek_core::arcade::vowel_glyph(label) else {
        return Vec::new();
    };
    let (x0, y0) = tablet_letter_at(b);
    let n = TABLET_PIXEL;
    rondelek_core::arcade::bitmap_cells(b)
        .flat_map(|(col, row)| {
            (0..n).flat_map(move |i| {
                (0..n).map(move |j| (x0 + col * n + i, TABLET_H - 1 - (y0 + row * n + j)))
            })
        })
        .collect()
}

/// A stone tablet with `label`'s vowel carved in it, a relic from a jungle
/// temple: a slab of white limestone with crisp edges (its front bevelled a
/// brick), framed by a ring of little notches, dots and dashes, and a groove
/// round its face. The arcade's bold pixel vowel
/// (`rondelek_core::arcade::vowel_glyph`) is cut two bricks deep, its walls
/// bevelled a brick, the whole cut painted. Weathered, each tablet its own
/// way (by its label): a top corner chipped, a hairline crack running in
/// from a side, moss creeping in at a bottom corner and along the groove.
/// An unknown label gives a blank tablet.
pub fn tablet(label: &str) -> Grid {
    let (w, h, d) = (TABLET_W, TABLET_H, TABLET_D);
    let front = d - 1;
    let mut g = Grid::new(w, h, d);
    let salt = label.bytes().map(u64::from).sum::<u64>();
    let r = |x: usize, y: usize, k: u64| hash(x, y, salt as usize, k);
    let letter = tablet_letter(label);
    let ink = |x: usize, y: usize| letter.contains(&(x, y));
    // The weathering's places: the chip on one top corner, moss at the
    // bottom corner across from it, the crack in from the chip's side.
    let right = r(0, 0, 40) < 0.5; // half the vowels each way
    let chip_x = if right { w - 1 } else { 0 };
    let moss_x = if right { 0 } else { w - 1 };
    let crack: Vec<(usize, usize)> = {
        let mut y = (h as f32 * (0.3 + 0.15 * r(0, 0, 41))) as isize;
        (0..10)
            .map(|step| {
                let x = if right { w - 1 - step } else { step };
                y += match r(step, 0, 42) {
                    v if v < 0.3 => -1,
                    v if v > 0.75 => 1,
                    _ => 0,
                };
                (x, y.clamp(0, h as isize - 1) as usize)
            })
            .collect()
    };
    let mossy = |x: usize, y: usize, reach: f32| {
        let (dx, dy) = (x.abs_diff(moss_x) as f32, y as f32);
        let n = gradient_noise([x as f32 / 4.0, y as f32 / 4.0, salt as f32], 1024, 51);
        dx.hypot(1.4 * dy) < reach + 3.0 * n
    };
    for y in 0..h {
        for x in 0..w {
            if !on_tablet(x, y) {
                continue;
            }
            let ring = tablet_ring(x, y);
            // The stone: white, in faint drifts of lighter, a speck here and
            // there.
            let drift = gradient_noise([x as f32 / 6.0, y as f32 / 6.0, salt as f32], 1024, 52);
            let stone = if r(x, y, 31) < 0.015 {
                LIME_SPECK
            } else if drift > 0.15 {
                LIME_LIGHT
            } else {
                LIME
            };
            // The frame: the groove all round, and the notches along their
            // band, from the middle of each side out; each a brick deep.
            let (along, side) = if x.min(w - 1 - x) == ring {
                (y, h)
            } else {
                (x, w)
            };
            let notch = (along as isize - side as isize / 2 + 1).rem_euclid(5) < 2;
            let carved = ring == TABLET_GROOVE || (TABLET_NOTCHES.contains(&ring) && notch);
            let crack_here = crack.contains(&(x, y)) && !ink(x, y) && ring < TABLET_GROOVE + 3;
            // The chip: a corner knocked off, right through, and a step of
            // its fracture beside it, two bricks deep.
            let from_chip = x.abs_diff(chip_x) + (h - 1 - y);
            if from_chip < 4 {
                continue;
            }
            let chipped = from_chip < 7;
            let moss = mossy(x, y, 6.0);
            for z in 0..d {
                let depth = front - z; // 0 on the face
                let c = if ink(x, y) {
                    // Cut two bricks deep where the stroke is whole, one at
                    // its edges: bevelled walls. All of it painted.
                    let edge = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(i, j)| {
                        let (nx, ny) = (x as isize + i, y as isize + j);
                        nx < 0 || ny < 0 || !ink(nx as usize, ny as usize)
                    });
                    let cut = if edge { 1 } else { 2 };
                    match depth {
                        dd if dd < cut => 0,
                        dd if dd == cut => PAINT,
                        _ => LIME,
                    }
                } else if chipped {
                    match depth {
                        0 | 1 => 0,
                        2 => LIME_DARK,
                        _ => LIME,
                    }
                } else if ring == 0 && depth == 0 {
                    0 // the bevel round the front
                } else if carved || crack_here {
                    match depth {
                        0 => 0,
                        // Moss creeps further along the groove.
                        1 if moss || (ring == TABLET_GROOVE && mossy(x, y, 11.0)) => TABLET_MOSS,
                        1 if crack_here => CRACK,
                        1 => LIME_DARK,
                        _ => LIME,
                    }
                } else if moss && depth == 0 {
                    if r(x, y, 53) < 0.35 {
                        TABLET_MOSS_LIGHT
                    } else {
                        TABLET_MOSS
                    }
                } else {
                    stone
                };
                g.set(x, y, z, c);
            }
        }
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
            open_below: true,
            open_behind: true,
        };
        let n = vertex_count(&super::ground(), ground);
        assert!(n <= u16::MAX as usize + 1, "ground: {n} vertices");
        let n = vertex_count(&tablet("y"), Build::default());
        assert!(n <= u16::MAX as usize + 1, "tablet y: {n} vertices");
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
        // the big peaks rise high into it, over the jungle in front, but stay
        // under its top, so the clouds still float above them, and the
        // valleys between dip below it, so the range is no wall.
        let tops: Vec<f32> = skyline(&mountains())
            .iter()
            .map(|&b| MOUNTAIN_BASE + b as f32 * MOUNTAIN_CELL)
            .collect();
        let tallest = tops.iter().cloned().fold(0.0, f32::max);
        assert!((7.0..=9.0).contains(&tallest), "tallest peak {tallest}");
        assert!(tops.iter().any(|&t| t < 3.3), "no valley under the clouds");
    }

    #[test]
    fn the_backdrops_have_no_gaps_and_fit_raylibs_indices() {
        use crate::bricks::{Build, vertex_count};
        let how = Build {
            wrap_x: true,
            open_below: true,
            open_behind: true,
        };
        let g = mountains();
        assert!(
            skyline(&g).iter().all(|&t| t > 0),
            "the mountains have a gap"
        );
        let n = vertex_count(&g, how);
        assert!(n <= u16::MAX as usize + 1, "mountains: {n} vertices");
        // The jungle is too leafy for one mesh; it builds into several, but
        // stays light enough for a family laptop (a tile or two is on screen).
        let g = jungle().grid;
        assert!(skyline(&g).iter().all(|&t| t > 0), "the jungle has a gap");
        let n = vertex_count(&g, JUNGLE_BUILD);
        assert!(n <= 2 * u16::MAX as usize, "jungle: {n} vertices");
    }

    #[test]
    fn the_jungle_is_round_not_a_cutout() {
        // Seen from the front, the nearest brick over each spot lies at many
        // depths (clumps in rows, each rounded), in many shades.
        let g = jungle().grid;
        let mut depths = std::collections::HashSet::new();
        let mut shades = std::collections::HashSet::new();
        for x in 0..g.w as isize {
            for y in 2..g.h as isize {
                if let Some(z) = (0..g.d as isize).rev().find(|&z| g.get(x, y, z) != 0) {
                    depths.insert(z);
                    shades.insert(g.get(x, y, z));
                }
            }
        }
        assert!(depths.len() >= 6, "front depths {depths:?}");
        assert!(shades.len() >= 7, "front shades {shades:?}");
    }

    #[test]
    fn the_jungle_is_glassy_in_front_and_solid_behind() {
        use crate::bricks::SOLID;
        let g = jungle().grid;
        // Per slice, back to front: its bricks, how many are see-through,
        // and how many let more than half through.
        let slices: Vec<(usize, usize, usize)> = (0..g.d as isize)
            .map(|z| {
                let mut s = (0, 0, 0);
                for y in 0..g.h as isize {
                    for x in 0..g.w as isize {
                        if g.get(x, y, z) != 0 {
                            let a = g.alpha(x, y, z);
                            s.0 += 1;
                            s.1 += usize::from(a < SOLID);
                            s.2 += usize::from(a < 128);
                        }
                    }
                }
                s
            })
            .collect();
        let share = |n: usize, of: usize| n as f32 / of.max(1) as f32;
        // The back half is solid: the glass is seen against it.
        assert!(slices[..g.d / 2].iter().all(|s| s.1 == 0), "{slices:?}");
        // The front is glassy in drifts: much of it clear, much of it not.
        let front = slices[g.d - 1];
        assert!(share(front.2, front.0) > 0.25, "{front:?}");
        assert!(share(front.1, front.0) < 0.8, "{front:?}");
        // Nearer is glassier.
        for p in slices.windows(2) {
            assert!(share(p[1].2, p[1].0) >= share(p[0].2, p[0].0), "{slices:?}");
        }
    }

    #[test]
    fn the_wind_moves_the_palms_and_nothing_far_from_them() {
        let j = jungle_plants();
        let s = Sway::of(&j);
        assert_eq!(j.palms.len(), 6);
        let (w, h, d) = (j.g.w as isize, j.g.h as isize, j.g.d as isize);
        let palm = |x: isize, y: isize, z: isize| {
            j.palm_of.get(x.rem_euclid(w), y, z) != 0 && j.g.get(x.rem_euclid(w), y, z) != 0
        };
        let (mut moving, mut still) = (0, 0);
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    if j.g.get(x, y, z) == 0 {
                        continue;
                    }
                    let b = [x as usize, y as usize, z as usize];
                    if palm(x, y, z) {
                        // A palm's foot hardly moves; the rest does.
                        assert!(
                            s.moves(b) || y <= FLOOR as isize,
                            "palm brick {b:?} stands still"
                        );
                        moving += 1;
                    } else if !(-1..=1).any(|dz| {
                        (-1..=1).any(|dy| (-1..=1).any(|dx| palm(x + dx, y + dy, z + dz)))
                    }) {
                        assert!(!s.moves(b), "{b:?} moves, far from any palm");
                        still += 1;
                    }
                }
            }
        }
        assert!(
            moving > 300 && still > 10 * moving,
            "{moving} moving, {still} still"
        );
        // The tile's two ends are one corner.
        for y in 0..=h as usize {
            for z in 0..=d as usize {
                assert_eq!(s.at([w as usize, y, z]), s.at([0, y, z]));
            }
        }
    }

    #[test]
    fn a_palm_bends_from_its_foot_and_its_fronds_flutter_at_the_tips() {
        let j = jungle_plants();
        let s = Sway::of(&j);
        let p = &j.palms[0];
        let [cx, cy, cz] = p.crown.map(|v| v.floor() as usize);
        // Up the trunk, from the foot to the crown, the bend only grows.
        let trunk: Vec<[usize; 3]> = (FLOOR..cy)
            .filter_map(|y| {
                (0..j.g.w).find_map(|x| {
                    let b = [x, y, cz];
                    let mine = j.palm_of.get(x as isize, y as isize, cz as isize) == 1;
                    (mine && j.g.get(x as isize, y as isize, cz as isize) == PALM_TRUNK)
                        .then_some(b)
                })
            })
            .collect();
        assert!(trunk.len() > 10, "{trunk:?}");
        let bends: Vec<u8> = trunk.iter().map(|&[x, y, z]| s.at([x, y, z])[0]).collect();
        // (Its lowest bricks may be a fern's now, painted over it.)
        assert!(bends[0] < 10, "the foot stands: {bends:?}");
        assert!(bends.windows(2).all(|b| b[0] <= b[1]), "{bends:?}");
        assert!(*bends.last().unwrap() > 200, "{bends:?}");
        // The crown goes all the way with the trunk and hardly flutters; a
        // frond's tip flutters the most.
        let crown = s.at([cx, cy, cz]);
        assert_eq!(crown[0], 255);
        assert!(crown[1] < 40, "{crown:?}");
        let leafiest = (0..j.g.w)
            .flat_map(|x| (0..j.g.h).flat_map(move |y| (0..j.g.d).map(move |z| [x, y, z])))
            .filter(|&[x, y, z]| j.palm_of.get(x as isize, y as isize, z as isize) == 1)
            .map(|b| s.at(b)[1])
            .max()
            .unwrap();
        assert!(leafiest > 200, "{leafiest}");
        // Its own beat, on every corner.
        assert!(trunk[3..].iter().all(|&b| s.at(b)[2] == crown[2]));
    }

    #[test]
    fn the_noise_is_smooth_and_runs_on_round_the_tile() {
        let period = 24;
        for i in 0..200 {
            let p = [
                hash(i, 0, 0, 5) * period as f32,
                hash(i, 1, 0, 5) * 4.0,
                hash(i, 2, 0, 5) * 2.0,
            ];
            let n = gradient_noise(p, period, 7);
            assert!(n.abs() <= 1.0, "{p:?} → {n}");
            // A whole period along, the same; a hair along, nearly so.
            let round = gradient_noise([p[0] + period as f32, p[1], p[2]], period, 7);
            assert!((n - round).abs() < 1e-4, "{p:?}: {n} vs {round}");
            let near = gradient_noise([p[0] + 0.01, p[1] + 0.01, p[2]], period, 7);
            assert!((n - near).abs() < 0.05, "{p:?}: {n} vs {near}");
        }
        // Through 0 on every lattice corner.
        assert_eq!(gradient_noise([3.0, 2.0, 1.0], period, 7), 0.0);
    }

    #[test]
    fn the_jungle_is_greenery_only() {
        // Leaves, lianas and the floor are all greens; the only other
        // colours are the trunks'.
        for (i, [r, g, b]) in JUNGLE_PALETTE.iter().enumerate() {
            if ![BARK, BARK_SHADE, PALM_TRUNK].contains(&(i as u8 + 1)) {
                assert!(g > r && g > b, "colour {} isn't a green", i + 1);
            }
        }
        let g = jungle().grid;
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
        {
            let (name, grove) = ("grove", &GROVE);
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
    fn every_palm_fits_its_grove() {
        {
            let grove = &GROVE;
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
                open_below: false,
                open_behind: false,
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
            let ink = tablet_letter(v);
            assert!(!ink.is_empty(), "{v}: no letter");
            for y in 0..TABLET_H {
                for x in 0..TABLET_W {
                    let (xi, yi) = (x as isize, y as isize);
                    let paint = (0..TABLET_D as isize)
                        .filter(|&z| g.get(xi, yi, z) == PAINT)
                        .count();
                    if ink.contains(&(x, y)) {
                        // Cut into the face, painted at the bottom of the
                        // cut, the whole letter inside the frame.
                        assert!(
                            tablet_ring(x, y) > TABLET_GROOVE,
                            "{v}: ink on the frame at {x},{y}"
                        );
                        assert_eq!(g.get(xi, yi, front), 0, "{v}: not carved at {x},{y}");
                        assert_eq!(paint, 1, "{v}: unpainted at {x},{y}");
                    } else {
                        // Nothing else is painted.
                        assert_eq!(paint, 0, "{v}: paint off the letter at {x},{y}");
                    }
                }
            }
        }
    }

    #[test]
    fn a_tablets_outline_is_whole_at_the_back_but_for_a_chip() {
        // The bevel and the carving only ever come out of the front: the
        // back is the whole outline, but for one top corner, knocked off.
        for v in ["a", "e", "i", "o", "u", "y"] {
            let g = tablet(v);
            let near_top_corner =
                |x: usize, y: usize| x.min(TABLET_W - 1 - x) + (TABLET_H - 1 - y) < 4;
            let mut chipped = 0;
            for y in 0..TABLET_H {
                for x in 0..TABLET_W {
                    let back = g.get(x as isize, y as isize, 0) != 0;
                    if near_top_corner(x, y) {
                        chipped += usize::from(on_tablet(x, y) && !back);
                    } else {
                        assert_eq!(on_tablet(x, y), back, "{v}: outline at {x},{y}");
                    }
                }
            }
            assert!(chipped > 3, "{v}: no chip");
        }
    }

    #[test]
    fn a_tablet_is_framed_and_weathered() {
        let front = TABLET_D as isize - 1;
        for v in ["a", "e", "i", "o", "u", "y"] {
            let g = tablet(v);
            let colours: Vec<u8> = (0..TABLET_D as isize)
                .flat_map(|z| {
                    let g = &g;
                    (0..TABLET_H as isize)
                        .flat_map(move |y| (0..TABLET_W as isize).map(move |x| g.get(x, y, z)))
                })
                .collect();
            for (c, what) in [
                (TABLET_MOSS, "moss"),
                (CRACK, "a crack"),
                (LIME_DARK, "carving"),
            ] {
                assert!(colours.contains(&c), "{v}: no {what}");
            }
            // The groove runs round unbroken, a brick deep.
            let r = TABLET_GROOVE as isize;
            let (w, h) = (TABLET_W as isize, TABLET_H as isize);
            for x in r..w - r {
                for y in [r, h - 1 - r] {
                    assert_eq!(g.get(x, y, front), 0, "{v}: groove at {x},{y}");
                    assert_ne!(g.get(x, y, front - 1), 0, "{v}: groove at {x},{y}");
                }
            }
        }
    }

    #[test]
    fn a_tablet_is_the_old_signs_size_scaled() {
        // The flat sign was 88 px square on a 720-px screen: about 1.27
        // world units at the hero's depth. At full size a tablet is a little
        // wider, a bit taller, and a slab: 0.2 thick. It's drawn at
        // TABLET_SCALE of that.
        let [w, h, d] = size(&tablet("a"), TABLET_CELL).map(|v| v / TABLET_SCALE);
        assert!(
            (1.2..=1.5).contains(&w) && (1.3..=1.6).contains(&h),
            "{w}×{h}"
        );
        assert!(
            (d - TABLET_D as f32 * 0.05).abs() < 1e-5 && d <= 0.2,
            "{d} thick"
        );
        // Unknown labels give a blank slab, no letter on it, rather than a
        // panic.
        let blank = tablet("x");
        assert!((0..TABLET_D as isize).all(|z| {
            (0..TABLET_H as isize)
                .all(|y| (0..TABLET_W as isize).all(|x| blank.get(x, y, z) != PAINT))
        }));
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
