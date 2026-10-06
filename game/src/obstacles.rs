//! Vowel Runner's obstacles, from the jungle: a mossy log and a sleeping
//! tortoise taking turns as the low one (jump over it), a tree fallen across
//! two boulders (duck under its trunk), a clump of bamboo (the double jump),
//! and the brick wall (shoot it). Each is a [`Grid`] of coloured bricks (see
//! `bricks.rs`), lit by Lam::pula like everything else in the scene.
//!
//! The jungle's obstacles are shaped in world units and laid in bricks by
//! [`sculpt`], so their brick size ([`OBSTACLE_CELL`]) is one number to
//! change. Like the other props they are deterministic, a hash of a brick's
//! place and never a random generator, so each looks the same every game
//! and a test can hold it to its hitbox in `runner.rs`.

use super::bricks::Grid;
use super::props::{gradient_noise, hash, smoothstep};
use std::f32::consts::{PI, TAU};

/// One brick of the jungle's obstacles, world units: a quarter of a meadow
/// brick, fine enough for a log's rings or a tortoise's closed eye.
pub const OBSTACLE_CELL: f32 = 0.0625;
/// One brick of the wall, world units: half a meadow brick.
pub const WALL_CELL: f32 = 0.125;

/// How far either side of the lane the hero's body reaches (it is 0.35
/// deep): what stands further back, the hero runs past in front of.
pub const HERO_HALF_DEPTH: f32 = 0.175;

// The wood: bark, the pale wood inside, its rings and heart, and moss.
const BARK: u8 = 1;
const BARK_DARK: u8 = 2;
const BARK_LIGHT: u8 = 3;
const WOOD: u8 = 4;
const WOOD_RING: u8 = 5;
const HEART: u8 = 6;
const CRACK: u8 = 7;
const MOSS: u8 = 8;
const MOSS_LIGHT: u8 = 9;
// The boulders.
const ROCK: u8 = 10;
const ROCK_LIGHT: u8 = 11;
const ROCK_DARK: u8 = 12;
// The tortoise: its shell's plates, their pale middles and the seams
// between, the rim; its skin, the yellow scales of a yellow-footed tortoise,
// a closed eye.
const SHELL: u8 = 13;
const SHELL_LIGHT: u8 = 14;
const SEAM: u8 = 15;
const RIM: u8 = 16;
const SKIN: u8 = 17;
const SKIN_DARK: u8 = 18;
const SCALE: u8 = 19;
const EYE: u8 = 20;
// The bamboo: golden, striped green, its nodes and the pale wax under each,
// its leaves and the shoots at its foot.
const CANE: u8 = 21;
const CANE_LIGHT: u8 = 22;
const CANE_SHADE: u8 = 23;
const STRIPE: u8 = 24;
const NODE: u8 = 25;
const WAX: u8 = 26;
const LEAF: u8 = 27;
const LEAF_DARK: u8 = 28;
const SHOOT: u8 = 29;
// A bromeliad's bloom on the fallen tree.
const BLOOM: u8 = 30;
// The wall.
const BRICK: u8 = 31;
const BRICK_LIGHT: u8 = 32;
const BRICK_DARK: u8 = 33;
const MORTAR: u8 = 34;
pub const OBSTACLE_PALETTE: [[u8; 3]; 34] = [
    [112, 78, 52],   // bark
    [78, 52, 36],    // bark, in a furrow
    [142, 104, 68],  // bark, sunlit
    [228, 190, 132], // wood
    [188, 142, 90],  // a ring in the wood
    [150, 104, 62],  // the heart
    [66, 46, 32],    // a crack
    [100, 160, 58],  // moss
    [138, 190, 74],  // moss, sunlit
    [146, 148, 138], // rock
    [178, 178, 166], // rock, sunlit
    [106, 110, 104], // rock, shaded
    [124, 82, 42],   // shell
    [236, 176, 74],  // shell, a plate's middle
    [56, 36, 22],    // shell, a seam
    [156, 108, 58],  // shell, the rim
    [172, 162, 124], // skin
    [124, 116, 90],  // skin, shaded
    [242, 184, 62],  // a yellow scale
    [40, 32, 26],    // a closed eye
    [240, 200, 70],  // cane
    [254, 226, 112], // cane, sunlit
    [206, 164, 52],  // cane, shaded
    [112, 158, 58],  // a green stripe down the cane
    [104, 112, 48],  // a node
    [232, 230, 178], // the wax under a node
    [84, 162, 68],   // a leaf
    [56, 126, 56],   // a leaf, its shaded side
    [160, 128, 70],  // a bamboo shoot
    [236, 92, 52],   // a bromeliad's bloom
    [200, 86, 64],   // brick red
    [218, 108, 80],  // brick, light
    [174, 70, 52],   // brick, dark
    [232, 220, 204], // mortar
];

/// Lay a shape in bricks: a grid `size` world units big (in whole bricks of
/// `cell`), each brick the colour `paint` gives at its middle, 0 leaving it
/// empty. `paint` sees the brick as a brick model stands (x and z from the
/// middle, y up from the bottom, the camera toward +z) and its place.
fn sculpt(size: [f32; 3], cell: f32, paint: impl Fn([f32; 3], [usize; 3]) -> u8) -> Grid {
    let [w, h, d] = size.map(|s| (s / cell).round() as usize);
    let mut g = Grid::new(w, h, d);
    for z in 0..d {
        for y in 0..h {
            for x in 0..w {
                let p = [
                    (x as f32 + 0.5 - w as f32 / 2.0) * cell,
                    (y as f32 + 0.5) * cell,
                    (z as f32 + 0.5 - d as f32 / 2.0) * cell,
                ];
                g.set(x, y, z, paint(p, [x, y, z]));
            }
        }
    }
    g
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// `p` in an ellipsoid's own measure: 1 on its skin, less inside.
fn ellipsoid(p: [f32; 3], centre: [f32; 3], radii: [f32; 3]) -> [f32; 3] {
    let q = sub(p, centre);
    [q[0] / radii[0], q[1] / radii[1], q[2] / radii[2]]
}

/// How far `p` is from the segment `a`–`b`, and how far along it the nearest
/// point lies (0 at `a`, 1 at `b`).
fn from_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> (f32, f32) {
    let ab = sub(b, a);
    let t = (dot(sub(p, a), ab) / dot(ab, ab)).clamp(0.0, 1.0);
    let q = [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
    (len(sub(p, q)), t)
}

/// How wide a leaf is `t` of the way from its base to its tip, as a share
/// of its widest (a third of the way along): it comes to a point.
fn leaf_shape(t: f32) -> f32 {
    if t < 0.33 {
        0.5 + 0.5 * t / 0.33
    } else {
        (1.0 - t) / 0.67
    }
}

/// A leaf from `base` to `tip`, round, `width` across at its widest:
/// whether `p` is on it.
fn on_leaf(p: [f32; 3], base: [f32; 3], tip: [f32; 3], width: f32) -> bool {
    let (d, t) = from_segment(p, base, tip);
    d <= width / 2.0 * leaf_shape(t)
}

/// A flat leaf, its blade toward the camera and one brick thick (a round
/// one this thin would be a line of single bricks, lost from the front):
/// whether `p` is on it.
fn on_blade(p: [f32; 3], base: [f32; 3], tip: [f32; 3], width: f32, cell: f32) -> bool {
    let flat = |v: [f32; 3]| [v[0], v[1], 0.0];
    let (d, t) = from_segment(flat(p), flat(base), flat(tip));
    let z = base[2] + (tip[2] - base[2]) * t;
    (p[2] - z).abs() < cell / 2.0 && d <= width / 2.0 * leaf_shape(t)
}

/// Bark, the colour of a brick of it at angle `a` round its log (furrows run
/// along the log), sunlit or not.
fn bark(a: f32, sunlit: bool) -> u8 {
    match (a / TAU * 20.0).rem_euclid(1.0) {
        f if f < 0.3 => BARK_DARK,
        _ if sunlit => BARK_LIGHT,
        _ => BARK,
    }
}

/// Moss, in two shades by a hash of the brick's place.
fn moss([x, y, z]: [usize; 3], salt: u64) -> u8 {
    if hash(x, y, z, salt) < 0.4 {
        MOSS_LIGHT
    } else {
        MOSS
    }
}

/// How far down moss grows: on whatever faces up more than the first
/// number, and in its thickest clumps that much less than the second.
type Moss = (f32, f32);
/// A log's back, and a boulder's top, are mossy all over...
const LOG_MOSS: Moss = (0.75, 0.6);
const ROCK_MOSS: Moss = LOG_MOSS;
/// ...the fallen trunk's only here and there, so it reads as a trunk.
const TRUNK_MOSS: Moss = (0.9, 0.5);

/// Whether moss grows here: in clumps (a smooth noise along the wood), the
/// more the bark faces up (`up`, 1 on top), as far down as `reach` lets it.
fn mossy(p: [f32; 3], up: f32, (from, reach): Moss, salt: u64) -> bool {
    let n = gradient_noise(p.map(|v| v * 6.0 + 40.0), 1024, salt);
    up > from - reach * smoothstep(-0.3, 0.4, n)
}

/// A sawn end: rings round a dark heart, `r` from the middle.
fn rings(r: f32) -> u8 {
    if r < 0.06 {
        HEART
    } else if (r / 0.11).fract() > 0.68 {
        WOOD_RING
    } else {
        WOOD
    }
}

/// The log (jump over it): a length of fallen tree lying across the path,
/// its sawn end toward the camera, the rings showing a brick in from the
/// bark and a crack running in from its edge, moss on its back. 0.8 across
/// and 0.85 tall with the moss (the hitbox: 0.8 × 0.88).
pub fn log() -> Grid {
    const R: f32 = 0.4;
    const DEPTH: f32 = 0.625;
    let c = OBSTACLE_CELL;
    // The sawn face: the front brick is bark only, a lip round it.
    let face = (DEPTH / c).round() as usize - 2;
    sculpt([0.8, 0.875, DEPTH], c, |p @ [x, y, _], at @ [_, _, k]| {
        let dy = y - R;
        let r = x.hypot(dy);
        let up = dy / r.max(1e-3);
        let a = dy.atan2(x);
        if r > R {
            // A cushion of moss over the bark, a brick thick.
            return if r < R + c && mossy(p, up, LOG_MOSS, 1) {
                moss(at, 2)
            } else {
                0
            };
        }
        if r > R - 1.2 * c {
            return if mossy(p, up, LOG_MOSS, 1) {
                moss(at, 2)
            } else {
                bark(a, up > 0.2 && x < 0.0)
            };
        }
        if k > face {
            return 0;
        }
        if k < face {
            return WOOD;
        }
        // The crack: in from the bark at the upper right, narrowing.
        if (a - 0.95).abs() < 0.04 + 0.16 * r / R && r > 0.12 {
            return CRACK;
        }
        rings(r)
    })
}

/// The sleeping tortoise (jump over it, the log's turn about): a
/// yellow-footed tortoise, its domed shell in plates with pale middles, its
/// head out toward the hero, resting on the ground, eyes shut. 1.1 long
/// with its head and tail, 0.69 tall (the hitbox: 1.1 × 0.7).
pub fn tortoise() -> Grid {
    const SHELL_AT: [f32; 3] = [0.06, 0.18, 0.0];
    const SHELL_R: [f32; 3] = [0.4, 0.51, 0.29];
    const HEAD_AT: [f32; 3] = [-0.42, 0.13, 0.0];
    const HEAD_R: [f32; 3] = [0.14, 0.12, 0.115];
    let c = OBSTACLE_CELL;
    // The shell's top half, and which plate a point of it is on: hexagons,
    // laid out as the camera sees the shell (the pattern runs straight back,
    // so its edges stay crisp from the front).
    let in_shell = |p: [f32; 3]| p[1] >= SHELL_AT[1] && len(ellipsoid(p, SHELL_AT, SHELL_R)) <= 1.0;
    let plate = |p: [f32; 3]| {
        const SIZE: f32 = 0.13;
        let (x, y) = (p[0] - SHELL_AT[0], p[1] - SHELL_AT[1]);
        // Axial hexagon coordinates, rounded to the nearest hexagon.
        let q = (3f32.sqrt() / 3.0 * x - y / 3.0) / SIZE;
        let r = 2.0 / 3.0 * y / SIZE;
        let s = -q - r;
        let (mut rq, rs, mut rr) = (q.round(), s.round(), r.round());
        let (dq, ds, dr) = ((rq - q).abs(), (rs - s).abs(), (rr - r).abs());
        if dq > ds && dq > dr {
            rq = -rs - rr;
        } else if ds <= dr {
            rr = -rq - rs;
        }
        (rq as i32, rr as i32)
    };
    // Plates meet in a dark seam a brick wide (the brick on the near side
    // of the edge); the bricks along the edge are darker, the middles pale.
    let ahead = [[c, 0.0, 0.0], [0.0, c, 0.0]];
    let round = [[c, 0.0, 0.0], [-c, 0.0, 0.0], [0.0, c, 0.0], [0.0, -c, 0.0]];
    let meets_another = |p: [f32; 3], steps: &[[f32; 3]]| {
        let own = plate(p);
        steps.iter().any(|s| {
            let q = [p[0] + s[0], p[1] + s[1], p[2] + s[2]];
            in_shell(q) && plate(q) != own
        })
    };
    sculpt([1.25, 0.75, 0.625], c, |p @ [x, y, z], at| {
        if in_shell(p) {
            return if meets_another(p, &ahead) {
                SEAM
            } else if meets_another(p, &round) {
                SHELL
            } else {
                SHELL_LIGHT
            };
        }
        let flat = |grow: f32| {
            let q = ellipsoid(p, SHELL_AT, SHELL_R);
            (q[0] * SHELL_R[0] / (SHELL_R[0] + grow)).hypot(q[2] * SHELL_R[2] / (SHELL_R[2] + grow))
        };
        // The rim under it, flaring a little, in sixteen small plates.
        if (SHELL_AT[1] - 0.07..SHELL_AT[1]).contains(&y) && flat(0.02) <= 1.0 {
            let q = ellipsoid(p, SHELL_AT, SHELL_R);
            let sixteenth = ((q[2].atan2(q[0]) + PI) / (PI / 8.0)).fract();
            return if !(0.12..0.88).contains(&sixteenth) {
                SEAM
            } else {
                RIM
            };
        }
        // The body under the rim, in its shadow.
        if (0.06..SHELL_AT[1]).contains(&y) && flat(-0.05) <= 1.0 {
            return SKIN_DARK;
        }
        // Four stumpy legs, tucked in, scaly.
        for lx in [SHELL_AT[0] - 0.25, SHELL_AT[0] + 0.23] {
            for lz in [-0.18, 0.18] {
                if y < 0.18 && (x - lx).hypot(z - lz) <= 0.08 {
                    return if hash(at[0], at[1], at[2], 5) < 0.3 {
                        SCALE
                    } else {
                        SKIN
                    };
                }
            }
        }
        // The tail, a stub.
        if len(ellipsoid(
            p,
            [SHELL_AT[0] + 0.41, 0.12, 0.0],
            [0.06, 0.04, 0.04],
        )) <= 1.0
        {
            return SKIN;
        }
        // The head, resting on the ground: yellow on top, eyes shut.
        if len(ellipsoid(p, HEAD_AT, HEAD_R)) <= 1.0 {
            if (y - 0.15).abs() < c / 2.0 && (-0.52..-0.4).contains(&x) && z.abs() > 0.03 {
                return EYE;
            }
            return if y > 0.2 { SCALE } else { SKIN };
        }
        let (neck, _) = from_segment(p, [SHELL_AT[0] - 0.28, 0.16, 0.0], [-0.33, 0.14, 0.0]);
        if neck <= 0.07 {
            return SKIN;
        }
        0
    })
}

/// The fallen tree (duck under it): a trunk across the path, 0.78 to 1.32
/// above the ground and 1.4 long (the hitbox), broken off on the left and
/// sawn on the right, a snapped branch and a little moss on its back, and a
/// bromeliad in bloom. Its ends rest on two heaps of boulders that stand
/// behind the hero's lane, so the hero ducks under the trunk and runs past
/// in front of the rocks.
pub fn fallen_tree() -> Grid {
    const R: f32 = 0.27;
    const AXIS_Y: f32 = 1.05;
    const END: f32 = 0.7;
    const BROMELIAD: [f32; 3] = [0.22, AXIS_Y + R, 0.02];
    let c = OBSTACLE_CELL;
    // Each heap: a big boulder, a smaller one on it under the trunk's end,
    // a pebble by its side; all behind the lane. (Centre, radii, salt.)
    let boulders: Vec<([f32; 3], [f32; 3], u64)> = [(-1.0, 31), (1.0, 34)]
        .into_iter()
        .flat_map(|(side, salt)| {
            [
                ([side * 0.6, 0.24, -0.42], [0.34, 0.3, 0.2], salt),
                ([side * 0.55, 0.62, -0.44], [0.22, 0.22, 0.17], salt + 1),
                ([side * 0.88, 0.08, -0.38], [0.1, 0.1, 0.1], salt + 2),
            ]
        })
        .collect();
    sculpt([2.0, 1.625, 1.25], c, |p @ [x, y, z], at| {
        // The trunk, along x; `a` is round it, 0 toward the camera.
        let dy = y - AXIS_Y;
        let r = dy.hypot(z);
        let up = dy / r.max(1e-3);
        let a = dy.atan2(z);
        let sector = ((a + PI) / TAU * 9.0).floor() as usize;
        let broken_at = -END + 0.14 * hash(sector, 0, 0, 21);
        let along = (broken_at..=END).contains(&x);
        if along && r <= R {
            if r > R - 1.2 * c {
                return if mossy(p, up, TRUNK_MOSS, 22) {
                    moss(at, 23)
                } else {
                    bark(a, up > 0.2 && z > 0.0)
                };
            }
            if x > END - c {
                return rings(r);
            }
            return WOOD;
        }
        if along && r <= R + c && mossy(p, up, TRUNK_MOSS, 22) {
            return moss(at, 23);
        }
        // A branch that snapped off, a stub up and back.
        let (stub, _) = from_segment(
            p,
            [-0.24, AXIS_Y + 0.12, -0.08],
            [-0.38, AXIS_Y + 0.42, -0.18],
        );
        if stub <= 0.055 {
            return bark(0.0, x < -0.3);
        }
        // The bromeliad: a rosette of six leaves round a bloom.
        let (stalk, t) = from_segment(
            p,
            BROMELIAD,
            [BROMELIAD[0], BROMELIAD[1] + 0.26, BROMELIAD[2]],
        );
        if stalk <= 0.05 * (1.2 - t) && y > BROMELIAD[1] {
            return if t > 0.35 { BLOOM } else { LEAF };
        }
        for n in 0..6 {
            let az = n as f32 * TAU / 6.0 + 0.3;
            let tip = [
                BROMELIAD[0] + 0.22 * az.cos(),
                BROMELIAD[1] + 0.2,
                BROMELIAD[2] + 0.2 * az.sin(),
            ];
            if y > BROMELIAD[1] - c && on_leaf(p, BROMELIAD, tip, 0.13) {
                return if az.cos() < 0.0 { LEAF } else { LEAF_DARK };
            }
        }
        // The boulders: round, lumpy, lit from the upper left, moss on top.
        for &(centre, radii, salt) in &boulders {
            let q = ellipsoid(p, centre, radii);
            let n = len(q);
            let lump = 0.15 * gradient_noise(p.map(|v| v * 4.0 + 50.0), 1024, salt);
            if n <= 1.0 + lump {
                let s = q.map(|v| v / n.max(1e-3));
                if s[1] > 0.35 && mossy(p, s[1], ROCK_MOSS, salt + 10) {
                    return moss(at, salt);
                }
                let light = -0.5 * s[0] + 0.7 * s[1] + 0.4 * s[2];
                return match light + 0.25 * (hash(at[0], at[1], at[2], salt) - 0.5) {
                    l if l > 0.45 => ROCK_LIGHT,
                    l if l < -0.15 => ROCK_DARK,
                    _ => ROCK,
                };
            }
        }
        0
    })
}

/// One cane of bamboo: where it stands (x, z), how thick and tall, how it
/// leans (x per unit up), and where its nodes come.
struct Cane {
    x: f32,
    z: f32,
    r: f32,
    top: f32,
    lean: f32,
    first_node: f32,
    node_gap: f32,
}

const CANES: [Cane; 3] = [
    Cane {
        x: -0.19,
        z: -0.08,
        r: 0.11,
        top: 3.6,
        lean: -0.006,
        first_node: 0.3,
        node_gap: 0.46,
    },
    Cane {
        x: 0.02,
        z: 0.08,
        r: 0.12,
        top: 3.42,
        lean: 0.0,
        first_node: 0.18,
        node_gap: 0.5,
    },
    Cane {
        x: 0.2,
        z: -0.06,
        r: 0.1,
        top: 3.25,
        lean: 0.006,
        first_node: 0.38,
        node_gap: 0.42,
    },
];

/// The bamboo (double jump over it): three golden canes striped green, as
/// tall as the pillar's hitbox (0.64 × 3.6), with sprays of leaves at their
/// upper nodes and on top, and two shoots coming up at their foot.
pub fn bamboo() -> Grid {
    let c = OBSTACLE_CELL;
    // Each leafy node's spray: a twig out to one side, three leaves from it.
    let sprays: Vec<([f32; 3], [f32; 3], f32)> = CANES
        .iter()
        .enumerate()
        .flat_map(|(n, cane)| {
            let mut twigs = Vec::new();
            let mut y = cane.first_node;
            let mut k = 0;
            while y < cane.top {
                // The last node, on top, sprouts both ways.
                let last = y + cane.node_gap >= cane.top;
                let leafy = last || (y > 0.5 && hash(n, k, 0, 41) < 0.7);
                let one_way = if (n + k) % 2 == 0 { -1.0 } else { 1.0 };
                let sides: &[f32] = match (leafy, last) {
                    (false, _) => &[],
                    (true, false) => &[one_way],
                    (true, true) => &[-1.0, 1.0],
                };
                for &side in sides {
                    let cx = cane.x + cane.lean * y;
                    let base = [cx + side * cane.r, y, cane.z];
                    let end = [base[0] + side * 0.11, y + 0.1, cane.z];
                    twigs.push((base, end, side));
                }
                y += cane.node_gap;
                k += 1;
            }
            twigs
        })
        .collect();
    sculpt([1.5, 3.75, 0.4375], c, |p @ [x, y, z], at| {
        for (n, cane) in CANES.iter().enumerate() {
            if y > cane.top {
                continue;
            }
            let cx = cane.x + cane.lean * y;
            let (dx, dz) = (x - cx, z - cane.z);
            let d = dx.hypot(dz);
            let node =
                cane.first_node + ((y - cane.first_node) / cane.node_gap).round() * cane.node_gap;
            let from_node = y - node;
            if from_node.abs() < c / 2.0 && d <= cane.r + 0.025 {
                return NODE;
            }
            if d <= cane.r {
                if (-1.5 * c..-0.5 * c).contains(&from_node) {
                    return WAX;
                }
                let round = (dz.atan2(dx) / TAU * 3.0 + n as f32 * 0.37).rem_euclid(1.0);
                return if round < 0.12 {
                    STRIPE
                } else if dx < -0.35 * cane.r {
                    CANE_LIGHT
                } else if dx > 0.4 * cane.r {
                    CANE_SHADE
                } else {
                    CANE
                };
            }
        }
        for (k, &(base, end, side)) in sprays.iter().enumerate() {
            if from_segment(p, base, end).0 <= 0.035 {
                return NODE;
            }
            for (j, (out, drop)) in [(0.32, -0.04), (0.26, -0.2), (0.16, -0.3)]
                .iter()
                .enumerate()
            {
                let sway = 0.08 * (hash(k, j, 0, 43) - 0.5);
                let tip = [
                    end[0] + side * out,
                    end[1] + drop,
                    (end[2] + sway).clamp(-0.18, 0.18),
                ];
                if on_blade(p, end, tip, 0.13, c) {
                    return if side < 0.0 { LEAF } else { LEAF_DARK };
                }
            }
        }
        // Two shoots at the foot, behind the canes.
        for (sx, sz, r, h) in [(-0.06, -0.14, 0.06, 0.16), (0.12, 0.12, 0.05, 0.12)] {
            if y < h && (x - sx).hypot(z - sz) <= r * (1.0 - y / h) + c / 2.0 {
                return if hash(at[0], at[1], at[2], 47) < 0.3 {
                    BARK
                } else {
                    SHOOT
                };
            }
        }
        0
    })
}

/// The wall (shoot it): red bricks in staggered courses, 0.875 wide and 3
/// tall in [`WALL_CELL`]s (the hitbox: 0.92 × 3). The mortar is sunk one
/// brick in, so every brick stands out and catches the light at its corners.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bricks::{Build, vertex_count};

    /// The filled box of the bricks that `keep` lets through, world units:
    /// (min, max) per axis, a model's way round (x, z about the middle).
    fn extent(g: &Grid, cell: f32, keep: impl Fn(u8, [f32; 3]) -> bool) -> ([f32; 3], [f32; 3]) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for z in 0..g.d {
            for y in 0..g.h {
                for x in 0..g.w {
                    let colour = g.get(x as isize, y as isize, z as isize);
                    let low = [
                        (x as f32 - g.w as f32 / 2.0) * cell,
                        y as f32 * cell,
                        (z as f32 - g.d as f32 / 2.0) * cell,
                    ];
                    let mid = low.map(|v| v + cell / 2.0);
                    if colour != 0 && keep(colour, mid) {
                        for k in 0..3 {
                            lo[k] = lo[k].min(low[k]);
                            hi[k] = hi[k].max(low[k] + cell);
                        }
                    }
                }
            }
        }
        (lo, hi)
    }

    fn near(a: f32, b: f32, cell: f32) -> bool {
        (a - b).abs() <= cell
    }

    #[test]
    fn the_low_ones_are_their_hitboxes_size() {
        // (width, height) against runner.rs's hitboxes.
        let c = OBSTACLE_CELL;
        for (name, g, w, h) in [
            ("log", log(), 0.8, 0.88),
            ("tortoise", tortoise(), 1.1, 0.7),
        ] {
            let (lo, hi) = extent(&g, c, |_, _| true);
            let (gw, gh) = (hi[0] - lo[0], hi[1] - lo[1]);
            assert!(near(gw, w, c) && near(gh, h, c), "{name} {gw}×{gh}");
            assert!(near(lo[0], -hi[0], c), "{name} stands off centre");
        }
    }

    #[test]
    fn the_fallen_trunk_is_the_bar_and_the_hero_ducks_under_it() {
        // In the hero's lane there is nothing under the bar (0.78 up)...
        let c = OBSTACLE_CELL;
        let in_lane = |p: [f32; 3]| p[2].abs() < HERO_HALF_DEPTH;
        let (lo, _) = extent(&fallen_tree(), c, |_, p| in_lane(p));
        assert!(near(lo[1], 0.78, c), "something {} up", lo[1]);
        // ...and the trunk is the bar, 1.4 long, up to 1.32. (What grows on
        // top of it is free.)
        let (lo, hi) = extent(&fallen_tree(), c, |_, p| in_lane(p) && p[1] < 1.32);
        assert!(near(hi[0] - lo[0], 1.4, c), "{} long", hi[0] - lo[0]);
        assert!(near(hi[1], 1.32, c), "its top {} up", hi[1]);
    }

    #[test]
    fn the_boulders_stand_behind_the_hero() {
        let c = OBSTACLE_CELL;
        let rock = [ROCK, ROCK_LIGHT, ROCK_DARK];
        let (_, hi) = extent(&fallen_tree(), c, |colour, _| rock.contains(&colour));
        assert!(hi[2] <= -HERO_HALF_DEPTH, "a rock reaches z {}", hi[2]);
        // And they reach up to the trunk, mossy on top, so it rests on them.
        // (The trunk's own moss is all higher up, on its back.)
        let (_, hi) = extent(&fallen_tree(), c, |colour, p| {
            rock.contains(&colour) || ([MOSS, MOSS_LIGHT].contains(&colour) && p[1] < 1.0)
        });
        assert!(hi[1] >= 0.78, "the rocks are {} tall", hi[1]);
    }

    #[test]
    fn the_bamboo_is_the_pillars_size() {
        // Its canes, that is: the leaves are soft, and reach out past them.
        let c = OBSTACLE_CELL;
        let cane = [CANE, CANE_LIGHT, CANE_SHADE, STRIPE, WAX];
        let (lo, hi) = extent(&bamboo(), c, |colour, p| {
            cane.contains(&colour) && p[1] < 3.0
        });
        assert!(near(hi[0] - lo[0], 0.64, c), "{} wide", hi[0] - lo[0]);
        let (_, hi) = extent(&bamboo(), c, |_, _| true);
        assert!(near(hi[1], 3.6, 2.0 * c), "{} tall", hi[1]);
    }

    #[test]
    fn the_wall_is_its_hitboxes_size() {
        let (lo, hi) = extent(&wall(), WALL_CELL, |_, _| true);
        let (w, h) = (hi[0] - lo[0], hi[1] - lo[1]);
        assert!(
            near(w, 0.92, WALL_CELL) && near(h, 3.0, WALL_CELL),
            "wall {w}×{h}"
        );
    }

    #[test]
    fn the_tortoise_sleeps_facing_the_hero() {
        // Its head (with the closed eyes) is on the left, where the hero
        // comes from, low on the ground.
        let (lo, hi) = extent(&tortoise(), OBSTACLE_CELL, |colour, _| colour == EYE);
        assert!(hi[0] < -0.3, "the eyes are at x {}", lo[0]);
        assert!(hi[1] < 0.2, "the eyes are {} up", hi[1]);
    }

    #[test]
    fn every_obstacle_fits_raylibs_16_bit_indices() {
        for (name, g) in [
            ("log", log()),
            ("tortoise", tortoise()),
            ("fallen tree", fallen_tree()),
            ("bamboo", bamboo()),
            ("wall", wall()),
        ] {
            let n = vertex_count(&g, Build::default());
            assert!(n <= u16::MAX as usize + 1, "{name}: {n} vertices");
        }
    }
}
