//! Twinkles: now and then a glint of sunlight on one of the jungle's bricks,
//! one or two at a time, never more. A star of light gleams up on a corner of
//! the brick and fades, the brick brightening under it: glass catching the
//! sun, the way Lam::pula's corner glints do on the meadow. (The far planes
//! wear no glints of their own, see `backdrop_glass`: a whole jungle of them
//! would fizz.)
//!
//! Where and when is plain state, raylib-free and unit-testable; only
//! [`Twinkles::draw`] touches the GPU.

use super::bricks::Grid;
use raylib::prelude::*;

/// How long one twinkle lasts, gleaming up and fading, in seconds.
const LIFE: f32 = 1.2;
/// The most twinkling at once.
const MOST: usize = 2;
/// The wait from one twinkle's start to the next one's, seconds: between
/// these, at random. Longer than [`LIFE`], so most of the time there are
/// none, and they come now and then.
const GAP: (f32, f32) = (1.6, 3.6);
/// How often a second twinkle follows the first close behind, and how soon
/// (seconds, at random): so they come one or two at a time.
const PAIR: f32 = 0.4;
const FOLLOW: (f32, f32) = (0.15, 0.5);
/// Sunlight through leaves: a pale, leafy green, so it belongs to the
/// jungle (and, added onto the green, it comes out bright, not white).
const LIGHT: Color = Color::new(208, 252, 170, 255);
/// The star's arms at their longest, in bricks: the four upright and level
/// ones, then the four slanting between them. Small enough to stay out of
/// the hero's way, big enough to be seen.
const ARM: (f32, f32) = (2.4, 1.05);
/// Each arm's half-width where it leaves the middle, in bricks, and how much
/// of it is left at its tip.
const ARM_WIDTH: f32 = 0.06;
const ARM_TIP: f32 = 0.5;
/// The soft round glow at the star's middle, its radius in bricks.
const CORE: f32 = 0.375;
/// How far the star turns over its life, radians: a slow quarter of a
/// slant, so it lives without spinning.
const TURN: f32 = 0.35;
/// How much the brick itself lights up at the gleam's height (0..1).
const FLARE: f32 = 0.4;

/// One glint.
struct Twinkle {
    /// The corner it shines from, on the plane's own track: model space,
    /// with x running on from tile to tile as the plane is laid (its world x
    /// is this less the plane's scroll).
    at: Vector3,
    /// The middle of the brick it lights, the same way.
    brick: Vector3,
    /// 0 → 1 over its life.
    age: f32,
    /// The star's slant at the start, radians.
    turn: f32,
}

/// The twinkles on one plane (the jungle's), and the bricks they may pick.
#[derive(Default)]
pub struct Twinkles {
    /// The bricks a twinkle may shine on, column by column along a tile
    /// (see [`Twinkles::on`]): each its front face's lower-left corner.
    columns: Vec<Vec<Vector3>>,
    /// One brick, world units.
    cell: f32,
    live: Vec<Twinkle>,
    /// Seconds until the next may start.
    wait: f32,
}

impl Twinkles {
    /// Twinkles for a plane built from `grid` with bricks `cell` units big
    /// (laid as `lay_tiles` lays it), shining on what the camera sees of it:
    /// each column's front-most brick, from layer `from` up (the lower ones
    /// stand behind the meadow, in the mist), if it stands `still` (a glint
    /// is drawn where its brick rests: on a brick the wind moves, it would
    /// drift off it).
    pub fn on(grid: &Grid, cell: f32, from: usize, still: impl Fn([usize; 3]) -> bool) -> Self {
        // `BrickModel::build`'s model space: x and z centred on 0, y from 0.
        let x0 = -(grid.w as f32) * cell / 2.0;
        let z0 = -(grid.d as f32) * cell / 2.0;
        let columns = (0..grid.w)
            .map(|x| {
                (from..grid.h)
                    .filter_map(|y| {
                        let filled = |z: &usize| grid.get(x as isize, y as isize, *z as isize) != 0;
                        let z = (0..grid.d).rev().find(filled)?;
                        if !still([x, y, z]) {
                            return None;
                        }
                        Some(Vector3::new(
                            x0 + x as f32 * cell,
                            y as f32 * cell,
                            z0 + (z + 1) as f32 * cell,
                        ))
                    })
                    .collect()
            })
            .collect();
        Self {
            columns,
            cell,
            ..Self::default()
        }
    }

    /// Age the twinkles and, when it's time, start one on a brick between
    /// track x `l` and `r` (what is in sight: see [`Twinkle::at`]). `rand`
    /// gives 0..1.
    pub fn step(&mut self, dt: f32, (l, r): (f32, f32), rand: &mut impl FnMut() -> f32) {
        for t in &mut self.live {
            t.age += dt / LIFE;
        }
        self.live.retain(|t| t.age < 1.0);
        self.wait -= dt;
        if self.wait > 0.0 || self.live.len() >= MOST || self.columns.is_empty() {
            return;
        }
        // The column under a spot in sight, in whichever tile that is.
        let tile = self.columns.len() as f32 * self.cell;
        let u = l + (r - l) * rand();
        let k = (u / tile).round();
        let col = ((u - k * tile) / self.cell + self.columns.len() as f32 / 2.0) as usize;
        let Some(bricks) = self.columns.get(col).filter(|b| !b.is_empty()) else {
            return; // nothing to shine on there: try again next frame
        };
        let face = bricks[((rand() * bricks.len() as f32) as usize).min(bricks.len() - 1)];
        let face = Vector3::new(face.x + k * tile, face.y, face.z);
        let c = self.cell;
        let corner = Vector3::new(
            if rand() < 0.5 { 0.0 } else { c },
            if rand() < 0.5 { 0.0 } else { c },
            0.0,
        );
        self.live.push(Twinkle {
            at: face + corner,
            brick: face + Vector3::new(c / 2.0, c / 2.0, -c / 2.0),
            age: 0.0,
            turn: (rand() - 0.5) * TURN,
        });
        let (lo, hi) = if self.live.len() < MOST && rand() < PAIR {
            FOLLOW
        } else {
            GAP
        };
        self.wait = lo + (hi - lo) * rand();
    }

    /// Draw them on a plane scrolled `off` world units and standing where
    /// `lay_tiles` puts it (bottom at `base_y`, middle at depth `z`), seen
    /// from `eye`. Right after the plane itself: the brick lights up in the
    /// depth test, and the star is drawn over everything near it, as a glint
    /// is.
    pub fn draw(
        &self,
        d: &mut (impl RaylibDraw + RaylibDraw3D),
        eye: Vector3,
        off: f32,
        base_y: f32,
        z: f32,
    ) {
        if self.live.is_empty() {
            return;
        }
        let place = |p: Vector3| Vector3::new(p.x - off, p.y + base_y, p.z + z);
        let c = self.cell;
        let mut d = d.begin_blend_mode(BlendMode::BLEND_ADDITIVE);
        for t in &self.live {
            let s = c * 1.04;
            let a = gleam(t.age) * FLARE;
            d.draw_cube(place(t.brick), s, s, s, LIGHT.alpha(a));
        }
        // The stars over what stands in front: a glint is light in the eye,
        // not a thing in the scene. (rlgl batches, so flush on both sides
        // to fence the state change in.)
        // SAFETY: plain rlgl state calls inside our 3D mode.
        unsafe {
            ffi::rlDrawRenderBatchActive();
            ffi::rlDisableDepthTest();
        }
        for t in &self.live {
            star(place(t.at), eye, c, gleam(t.age), t.turn + TURN * t.age);
        }
        // SAFETY: as above, putting the depth test back.
        unsafe {
            ffi::rlDrawRenderBatchActive();
            ffi::rlEnableDepthTest();
        }
    }
}

/// How bright a twinkle is, `age` 0..1 into its life: up quickly, then
/// fading slowly, smooth at both ends.
fn gleam(age: f32) -> f32 {
    const PEAK: f32 = 0.3;
    let t = if age < PEAK {
        age / PEAK
    } else {
        (1.0 - age) / (1.0 - PEAK)
    }
    .clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// A star of light at `at`, `cell`-sized bricks round it, turned square to
/// the eye: eight arms tapering from the middle (four long, upright and
/// level, four short between, all `turn` round), over a soft round core;
/// each bright at the middle, nothing at its ends. `bright` 0..1 sizes and
/// lights it.
fn star(at: Vector3, eye: Vector3, cell: f32, bright: f32, turn: f32) {
    let fwd = (at - eye).normalize();
    let right = fwd.cross(Vector3::Y).normalize();
    let up = right.cross(fwd);
    // A point `s` right and `t` up of the middle, square to the eye.
    let p = |s: f32, t: f32| at + right * s + up * t;
    let lit = LIGHT.alpha(bright);
    let none = LIGHT.alpha(0.0);
    let vertex = |v: Vector3, c: Color| {
        // SAFETY: inside the rlBegin below.
        unsafe {
            ffi::rlColor4ub(c.r, c.g, c.b, c.a);
            ffi::rlVertex3f(v.x, v.y, v.z);
        }
    };
    // SAFETY: an immediate-mode triangle list, closed by rlEnd.
    unsafe { ffi::rlBegin(ffi::RL_TRIANGLES as i32) };
    for i in 0..8 {
        let a = turn + i as f32 * std::f32::consts::FRAC_PI_4;
        let len = if i % 2 == 0 { ARM.0 } else { ARM.1 } * cell * (0.3 + 0.7 * bright);
        let (u, v) = ((a.cos(), a.sin()), (-a.sin(), a.cos()));
        // A strip, lit at the root and dark at the tip, only a little
        // narrower there: the fading light makes the point. (An arm that
        // narrows to nothing, thinner than a pixel toward its end, breaks
        // up into dashes.)
        let root = |side: f32| {
            let w = ARM_WIDTH * cell * side;
            p(v.0 * w, v.1 * w)
        };
        let tip = |side: f32| {
            let w = ARM_WIDTH * ARM_TIP * cell * side;
            p(u.0 * len + v.0 * w, u.1 * len + v.1 * w)
        };
        // Two triangles, counter-clockwise to the eye.
        vertex(root(-1.0), lit);
        vertex(tip(-1.0), none);
        vertex(tip(1.0), none);
        vertex(root(-1.0), lit);
        vertex(tip(1.0), none);
        vertex(root(1.0), lit);
    }
    let r = CORE * cell * bright;
    const ROUND: usize = 12;
    for k in 0..ROUND {
        let a0 = k as f32 / ROUND as f32 * std::f32::consts::TAU;
        let a1 = (k + 1) as f32 / ROUND as f32 * std::f32::consts::TAU;
        vertex(p(0.0, 0.0), lit);
        vertex(p(a0.cos() * r, a0.sin() * r), none);
        vertex(p(a1.cos() * r, a1.sin() * r), none);
    }
    // SAFETY: closes the rlBegin above.
    unsafe { ffi::rlEnd() };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small xorshift, so a test's run is the same every time.
    fn rng(seed: u64) -> impl FnMut() -> f32 {
        let mut s = seed;
        move || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s >> 40) as f32 / (1u64 << 24) as f32
        }
    }

    /// A 16-wide tile: a back wall 2 bricks high, and a bush in front of it
    /// from x 4 to 7, 4 high.
    fn tile() -> Grid {
        let mut g = Grid::new(16, 6, 4);
        g.fill([0, 0, 0], [16, 2, 1], 1);
        g.fill([4, 0, 2], [8, 4, 4], 2);
        g
    }

    #[test]
    fn a_twinkle_shines_on_what_the_camera_sees() {
        let t = Twinkles::on(&tile(), 0.5, 1, |_| true);
        assert_eq!(t.columns.len(), 16);
        // Behind the bush, its front; elsewhere, the wall's.
        let front = |x: usize| t.columns[x].iter().map(|v| v.z).collect::<Vec<_>>();
        assert_eq!(front(0), vec![-0.5]); // the wall, layer 1 only
        assert_eq!(front(5), vec![1.0, 1.0, 1.0]); // the bush, layers 1-3
        // The corners sit on the bricks: lower-left of each face.
        assert_eq!(t.columns[5][0], Vector3::new(-1.5, 0.5, 1.0));
        // A brick that moves (the bush's top, say) is no place for a glint,
        // and the one behind it is hidden: that spot is left out.
        let t = Twinkles::on(&tile(), 0.5, 1, |[_, y, _]| y != 3);
        assert_eq!(t.columns[5].len(), 2);
    }

    #[test]
    fn one_or_two_at_a_time_now_and_then() {
        let mut t = Twinkles::on(&tile(), 0.5, 0, |_| true);
        let mut rand = rng(9);
        let dt = 1.0 / 60.0;
        let (mut none, mut two, mut started) = (0, 0, 0);
        let frames = 60 * 120;
        for _ in 0..frames {
            let before = t.live.len();
            t.step(dt, (-3.0, 3.0), &mut rand);
            assert!(t.live.len() <= MOST);
            started += usize::from(t.live.len() > before);
            none += usize::from(t.live.is_empty());
            two += usize::from(t.live.len() == 2);
        }
        // Now and then: one (or a pair) every GAP's middle or so, about 30
        // twinkles a minute.
        let per_min = started as f32 / 2.0;
        assert!((22.0..42.0).contains(&per_min), "{per_min} a minute");
        // Mostly quiet; one at a time, and now and then two.
        assert!(none > frames / 3, "rarely quiet: {none}");
        assert!(
            two > frames / 30 && two < frames / 4,
            "{two} frames with two"
        );
    }

    #[test]
    fn a_twinkle_starts_in_sight_on_a_brick_corner() {
        let mut t = Twinkles::on(&tile(), 0.5, 0, |_| true);
        let mut rand = rng(3);
        let tile_w = 8.0;
        // Far along the track: tile 12 and the next are in sight.
        let (l, r) = (12.0 * tile_w - 1.0, 12.0 * tile_w + 5.0);
        for _ in 0..2000 {
            t.wait = 0.0;
            t.live.clear();
            t.step(0.0, (l, r), &mut rand);
            let tw = &t.live[0];
            assert!(tw.at.x >= l - 0.5 && tw.at.x <= r + 0.5, "{:?}", tw.at);
            // On the lattice, and on the brick it lights.
            let k = tw.at.x / 0.5;
            assert!((k - k.round()).abs() < 1e-3, "{:?}", tw.at);
            assert!((tw.at - tw.brick).length() < 0.5);
        }
    }

    #[test]
    fn a_gleam_rises_quickly_and_fades_slowly() {
        assert_eq!(gleam(0.0), 0.0);
        assert_eq!(gleam(1.0), 0.0);
        assert!((gleam(0.3) - 1.0).abs() < 1e-6);
        // Half up within the first sixth; half down only past two thirds.
        assert!(gleam(0.15) >= 0.5);
        assert!(gleam(0.6) > 0.5);
    }
}
