//! "Vowel Runner" — the first voice game, now in 2.5D. The hero runs on their
//! own; the kid only speaks: say one vowel to jump over low blocks, hold
//! another to duck under high bars. Misses never kill — the obstacle just
//! bounces away; every cleared obstacle earns a star.
//!
//! Rendering: a perspective camera slightly above and beside the action,
//! following the hero up and down, gently (`CAM_FOLLOW`), everything built
//! from chunky 3D bricks ("pixels became big and 3-D"). Things come into the
//! picture and leave it out of sight, wherever its edges fall (`view.rs`).
//! Parallax planes scroll by hand-tuned factors of the travelled distance,
//! 90s style: clouds 0.10, mountains 0.16, palms 0.23, jungle 0.36,
//! ground 1.0, each
//! farther plane hazed toward the sky and the mountains and jungle rising out
//! of valley mist. The clouds are a hand-drawn flat-draw prop (GLB, see
//! `models.rs`) through flat-draw's own Lam::pula glass shader (`lampula.rs`),
//! floating, bobbing and breathing; the mountains, palms, jungle, meadow and
//! obstacles are bricks built in code (`props.rs`) through the same glass. In front of the meadow's bank lies playful water that scrolls with
//! the ground and dances to the child's voice: little glass bricks in the
//! clouds' Lam::pula (`brick_water.rs`), or the first, smooth toon water
//! (`water.rs`, kept: `RONDELEK_WATER_STYLE=toon`). The hero placeholder brick
//! is drawn under a comic-style toon shader — the same slot the dragon GLB model
//! will use later. Each obstacle carries a white stone tablet with its vowel
//! carved in (`props::tablet`), which hops when the vowel is said and lights
//! up while it's held. HUD (meter, score) stays crisp 2D, projected over the
//! scene — except the score, a hand-drawn pixel sun (GLB) in
//! Lam::pula glass that floats just in front of the camera with the count
//! printed on its face like a coin's value; it whirls round once per point and
//! comes back showing the new number (see [`SunCoin`]).
//!
//! Gameplay math is untouched from the 2D version: `update()` works in the
//! same 1280x720 logical space (100 logical px = 1 world unit at draw time),
//! so physics, collisions and their tests are identical.

use super::brick_water::BrickWater;
use super::bricks::{BrickModel, Build};
use super::lampula::{Lampula, LampulaParams, world_box};
use super::models::FlatModel;
use super::props;
use super::view::{Bounds, Eye};
use super::water::{self, Water};
use super::{VoiceGame, VoiceInput};
use raylib::prelude::*;
use raylib::rlgl::RaylibRlgl; // matrix stack for the salto flip
use rondelek_core::audio::vowel::VOWELS;

use super::{BUTTER, CHARCOAL, SKY};

// Logical canvas (gameplay space, matches the 2D version).
const LW: f32 = 1280.0;
const GROUND_Y: f32 = 560.0;

// Hero.
const HERO_X: f32 = 280.0;
const HERO_W: f32 = 88.0;
const HERO_H: f32 = 110.0;
const DUCK_H: f32 = 58.0;
const GRAVITY: f32 = 2100.0;
const JUMP_V: f32 = 1000.0;
// Ninja double-jump ("salto"): a second jump-vowel while airborne, punchier
// than the first so the hero climbs clearly higher. Tuned (with HIGH_H) so that
// even a badly-timed second jump — pressed the instant the hero leaves the
// ground — still clears a High pillar: demanding, but not frustrating for kids.
const AIR_JUMP_V: f32 = 1150.0;
// Flip duration (s) — comfortably shorter than the double-jump's airtime, so
// the hero lands upright.
const SALTO_DUR: f32 = 0.45;

// Star bullet: flies right (obstacles come left) to smash walls. Fast enough
// that a shot fired when a wall is visible reaches it before the hero does.
const BULLET_VX: f32 = 1150.0;
// Wall: taller than the jump apex (~238 px) and floor-standing, so neither a
// jump nor a duck clears it — only a star bullet destroys it.
const WALL_W: f32 = 92.0;
const WALL_H: f32 = 300.0;
// High pillar: tall enough that a single jump can never clear it (even with the
// forgiving hitbox), but *any* ninja double-jump — however timed — does.
const HIGH_W: f32 = 64.0;
const HIGH_H: f32 = 360.0;

// Ledges: one-way platforms in meadow bricks (`props::ledge`). The hero jumps
// up through one from below and lands on it only on the way down, like Mario;
// when it slides out from under their feet, they drop off. Heights are of the
// top, above the ground. A single jump (apex ~238) lands on a low ledge with
// room to spare, and the low one floats clear of the hero's head (110); a
// high ledge needs the double jump, or a hop from a low ledge just before it.
const LEDGE_LOW: f32 = 180.0;
const LEDGE_HIGH: f32 = 300.0;
/// How often a ledge pattern takes an obstacle's turn (never twice running,
/// so ledges and obstacles never share a stretch).
const LEDGE_CHANCE: f32 = 0.3;
/// Logical px from a staircase's low ledge to its high one.
const STAIR_GAP: f32 = 70.0;
// Little suns: the score sun, small, floating over the ledges to be
// collected — a point each. Size (logical px), and how high the centre floats
// over the ledge: the standing hero's middle, so running along collects them.
const LITTLE_SUN_PX: f32 = 44.0;
const LITTLE_SUN_LIFT: f32 = 55.0;
/// Turns per second: a slow, calm spin.
const LITTLE_SUN_SPIN: f32 = 0.35;

// Logical px per world unit.
const PPU: f32 = 100.0;

// The camera, at rest: a little above and to the right of the action,
// looking at (0, 1, 0). It follows the hero up and down, aiming ahead (see
// `update`): at the top of the jump while the hero rises, at where they'll
// land once they fall. It rises by `CAM_FOLLOW` of that height past a small
// dead zone (`CAM_DEAD`, so a little hop leaves it still), never more than
// `CAM_LIFT_MAX`, on a critically damped spring (it never overshoots): up in
// step with the jump (`CAM_RATE_UP`), and back down unhurried
// (`CAM_RATE_DOWN`, Greg's pick: quicker snapped back too hard). A camera
// that chases the hero instead lags behind every jump and is still drifting
// back when the next one comes — it feels like a ship at sea. The scene is
// truly 3D, so
// rising shows it from a little higher — the near things slide down further
// than the far planes: vertical parallax, for free.
const CAMERA_AT: Vector3 = Vector3 {
    x: 0.9,
    y: 2.2,
    z: 12.5,
};
const CAMERA_LOOKS_AT: Vector3 = Vector3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
};
const CAMERA_FOVY: f32 = 45.0;
const CAM_FOLLOW: f32 = 0.35;
const CAM_DEAD: f32 = 0.5;
const CAM_LIFT_MAX: f32 = 1.2;
const CAM_RATE_UP: f32 = 5.0;
const CAM_RATE_DOWN: f32 = 3.0;

// 2.5D palette: a clear, sunny-day world — saturated sky blue (the meadow's
// greens and earths are bricks now, in `props`), so the kid's own drawings
// (white clouds, the gold score sun) and the obstacles stand out against it
// instead of melting into a pastel wash.
const SKY_TOP: Color = Color::new(84, 176, 240, 255);
const SKY_LOW: Color = Color::new(184, 228, 255, 255);
// flat-draw props: world height (plus its per-lane variation) and the lane
// spacing. Each drawing's own aspect decides the width; the height covers the
// *whole* drawing, rain drops and all.
const CLOUD_H: f32 = 2.7;
const CLOUD_H_VARY: f32 = 1.3;
const CLOUD_PERIOD: f32 = 12.0;
// The clouds' gentle life: a slow float up and down and a faint breath (a
// touch wider as they get a touch shorter, and back), each cloud out of step.
// Kept small and slow on purpose — they are scenery, and big or quick sky
// motion is what makes a child dizzy. (A sway was tried and dropped: the
// tilt was the most unsettling part, and it made the glass glints flicker.)
const CLOUD_BOB: f32 = 0.07;
const CLOUD_BOB_RATE: f32 = 0.35;
const CLOUD_BREATH: f32 = 0.015;
const CLOUD_BREATH_RATE: f32 = 0.5;
// The far planes, 90s style: each at its own depth and scroll rate (a
// fraction of the ground's), each built in bricks (`props.rs`).
const MOUNTAIN_Z: f32 = -12.0;
const MOUNTAIN_SCROLL: f32 = 0.16;
/// World y of the range's middle, top to bottom: where its lamps centre.
const MOUNTAIN_MID: f32 =
    props::MOUNTAIN_BASE + props::MOUNTAIN_LAYERS as f32 * props::MOUNTAIN_CELL / 2.0;
const PALM_Z: f32 = -10.25;
const PALM_SCROLL: f32 = 0.23;
const JUNGLE_Z: f32 = -5.0;
const JUNGLE_SCROLL: f32 = 0.36;
/// World y of the jungle's middle, top to bottom: where its lamps centre.
const JUNGLE_MID: f32 = props::JUNGLE_BASE + props::JUNGLE_LAYERS as f32 * props::JUNGLE_CELL / 2.0;
// Atmospheric haze: how far each plane ends up pulled toward the sky behind
// it (0 = not at all, 1 = gone) — the farther, the more. The clouds' is what
// it was before the planes came, so they look as they did. See `haze_step`.
const CLOUD_HAZE: f32 = 0.45;
const MOUNTAIN_HAZE: f32 = 0.3;
const PALM_HAZE: f32 = 0.24;
const JUNGLE_HAZE: f32 = 0.1;
// Valley mist: a white band rising from a plane's foot, clear `fade` units up
// and `alpha` thick at the foot (and below). It ties the planes together: the
// mountains stand in it, the jungle rises out of it, the meadow comes out of
// it in front.
const MIST: Color = Color::new(236, 244, 250, 255);
const MOUNTAIN_MIST: (f32, f32, f32) = (0.6, 2.2, 0.8); // (foot y, fade, alpha)
// The palms' mist stands high, up into their crowns: the tall jungle in front
// hides everything lower, so a mist at their feet would never be seen.
const PALM_MIST: (f32, f32, f32) = (3.0, 2.6, 0.42);
const JUNGLE_MIST: (f32, f32, f32) = (0.0, 0.8, 0.18);

// The water in front of the meadow: its resting surface sits a little below
// the grass, so a strip of the bank's earth shows above it; it runs from the
// bank's face (the ground blocks are 3 deep, centred on z = 0) to past the
// bottom of the screen.
const WATER: water::Placement = water::Placement {
    y: -0.5,
    shore_z: 1.5,
    far_z: 12.0,
    width: 40.0,
};
// The brick water needs less: every brick is geometry, so it stops just past
// the bottom of the screen (z ≈ 7.7 on the highest crest) and a little past
// the sides of a very wide window.
pub(crate) const BRICK_WATER: water::Placement = water::Placement {
    far_z: 8.5,
    width: 30.0,
    ..WATER
};
/// One water brick's size, world units: about a cloud brick on screen. A
/// whole number of them must fit the water's PERIOD (a test checks).
pub(crate) const WATER_CELL: f32 = 0.25;

/// Which water lies in front of the bank. The brick water is the one we play
/// with; the toon water (the first, smooth one) is kept whole and tested so
/// it can come back: change this, or launch with `RONDELEK_WATER_STYLE=toon`.
const WATER_STYLE: WaterStyle = WaterStyle::Bricks;
const WATER_STYLE_ENV: &str = "RONDELEK_WATER_STYLE";

#[derive(Clone, Copy, Debug, PartialEq)]
enum WaterStyle {
    Bricks,
    Toon,
}

impl WaterStyle {
    /// `bricks` or `toon` (any case); anything else is `None`.
    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bricks" | "brick" => Some(Self::Bricks),
            "toon" => Some(Self::Toon),
            _ => None,
        }
    }

    /// [`WATER_STYLE`], unless `RONDELEK_WATER_STYLE` says otherwise.
    fn from_env() -> Self {
        match std::env::var(WATER_STYLE_ENV) {
            Ok(s) => Self::parse(&s).unwrap_or_else(|| {
                eprintln!("{WATER_STYLE_ENV}={s}? (bricks or toon); using {WATER_STYLE:?}");
                WATER_STYLE
            }),
            Err(_) => WATER_STYLE,
        }
    }
}

/// The sea along the front, in whichever style was chosen.
enum Sea {
    Bricks(Box<BrickWater>),
    Toon(Box<Water>),
}

impl Sea {
    fn load(rl: &mut RaylibHandle, thread: &RaylibThread, style: WaterStyle) -> Option<Self> {
        match style {
            WaterStyle::Bricks => {
                BrickWater::load(rl, thread, BRICK_WATER, WATER_CELL, water_glass())
                    .map(|w| Sea::Bricks(Box::new(w)))
            }
            WaterStyle::Toon => Water::load(rl, thread, WATER).map(|w| Sea::Toon(Box::new(w))),
        }
    }

    fn draw(&mut self, d: &mut impl RaylibDraw3D, eye: Vector3, t: f32, scroll: f32, voice: f32) {
        match self {
            Sea::Bricks(w) => w.draw(d, eye, t, scroll, voice),
            Sea::Toon(w) => w.draw(d, eye, t, scroll, voice),
        }
    }
}

// Score sun (the HUD's point icon). Its on-screen height in logical px, and how
// far in front of the camera it floats — near enough that nothing in the scene
// (all of it at z ≤ 1.5, the camera at 12.5) can ever pass in front of it.
const SUN_PX: f32 = 104.0;
const SUN_DIST: f32 = 3.0;
// The spin spring: ω = 10 rad/s, damping ratio 0.6 — one point reads as a quick
// whirl (~0.35 s to come round) that swings a little past and settles back.
const SPIN_K: f32 = 100.0;
const SPIN_C: f32 = 12.0;
// The jelly "pop" (scale swell + sparks) that rides along with each point.
const POP_DUR: f32 = 0.7;
// The sun's own colours (its rays and its middle), for the sparks it throws.
const SUN_GOLD: Color = Color::new(232, 172, 72, 255);
const SUN_EMBER: Color = Color::new(214, 112, 64, 255);
// The count on the sun's face: light digits in raylib's pixel font (it suits
// the pixel sun) with a warm dark outline so they read on the orange middle.
// Height and the widest it may get, in the sun drawing's own units (the whole
// sun is ~1.33 tall and wide); more digits shrink to fit.
const SCORE_DIGIT_H: f32 = 0.46;
const SCORE_MAX_W: f32 = 0.95;
const SCORE_LIGHT: Color = Color::new(255, 250, 232, 255);
const SCORE_INK: Color = Color::new(110, 52, 24, 255);

// The vowel tablets (`props::tablet`): a stone with the vowel carved in it
// over every obstacle. Over a low one (block, bridge) it floats a little
// behind the hero's lane, so a jumping hero passes in front of it rather
// than through it; a tall one (wall, pillar) wears it on its front, over its
// upper part. The floating ones bob, slow and small, each out of step.
const TABLET_FLOAT_Z: f32 = -0.75;
const TABLET_BOB: f32 = 0.04;
const TABLET_BOB_RATE: f32 = 1.3;
// Say a tablet's vowel and it answers: a little hop (height, world units;
// length, s), and its letter lights up while the vowel is held.
const TABLET_HOP: f32 = 0.16;
const TABLET_HOP_DUR: f32 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Jump, // low block: jump over
    Duck, // high bar: duck under
    Wall, // tall wall: shoot a star to destroy
    High, // tall pillar: needs the ninja double-jump
}

impl Kind {
    /// Whether getting past this obstacle without a bump earns a star. Only
    /// the move it asks for counts, so no vowel can be skipped: a bar is for
    /// ducking under (leaping over it doesn't count), and a wall is for
    /// shooting — its star comes from the bullet, so leaping over it with the
    /// double jump doesn't count either. A wrong-way pass costs nothing; it
    /// just earns nothing.
    fn earns_a_star(self, leapt: bool, ducked: bool) -> bool {
        match self {
            Kind::Jump | Kind::High => leapt,
            Kind::Duck => ducked,
            Kind::Wall => false,
        }
    }

    /// The move that gets past it, and so the vowel on its tablet: 0 jump
    /// (the block and the pillar), 1 duck, 2 shoot.
    fn move_index(self) -> usize {
        match self {
            Kind::Jump | Kind::High => 0,
            Kind::Duck => 1,
            Kind::Wall => 2,
        }
    }
}

struct Obstacle {
    x: f32,
    kind: Kind,
    /// Set on collision (or a bullet, for walls): flies off instead of the
    /// hero failing.
    bounced: bool,
    counted: bool,
    /// How the hero went past, while the two overlapped along x: in the air
    /// at some moment, or ducking at some moment (see [`Kind::earns_a_star`]).
    leapt: bool,
    ducked: bool,
    fly_y: f32,
    fly_vy: f32,
    rot: f32,
    /// Where in its bob the tablet starts, so two on screen don't bob in step.
    phase: f32,
}

impl Obstacle {
    fn new(x: f32, kind: Kind) -> Self {
        Self {
            x,
            kind,
            bounced: false,
            counted: false,
            leapt: false,
            ducked: false,
            fly_y: 0.0,
            fly_vy: 0.0,
            phase: 0.0,
            rot: 0.0,
        }
    }
}

/// A ledge's length (the two `props::ledge` builds).
#[derive(Clone, Copy, Debug, PartialEq)]
enum LedgeSize {
    Short,
    Long,
}

impl LedgeSize {
    fn bricks(self) -> usize {
        match self {
            LedgeSize::Short => props::LEDGE_SHORT,
            LedgeSize::Long => props::LEDGE_LONG,
        }
    }

    /// Length in logical px.
    fn width(self) -> f32 {
        self.bricks() as f32 * props::GROUND_CELL * PPU
    }
}

/// A one-way platform (see `LEDGE_LOW`).
struct Ledge {
    /// Left edge, logical px.
    x: f32,
    /// Logical y of the top the hero stands on.
    top: f32,
    size: LedgeSize,
}

impl Ledge {
    fn w(&self) -> f32 {
        self.size.width()
    }
}

/// A little sun to collect, floating over a ledge.
struct LittleSun {
    /// Centre, logical px.
    x: f32,
    y: f32,
    /// Where in its spin it starts, so a row of them doesn't turn in step.
    phase: f32,
}

fn little_sun_rect(s: &LittleSun) -> (f32, f32, f32, f32) {
    let r = LITTLE_SUN_PX / 2.0;
    (s.x - r, s.y - r, LITTLE_SUN_PX, LITTLE_SUN_PX)
}

/// A spinning star fired by the shoot vowel — destroys walls.
struct Bullet {
    x: f32,
    y: f32,
    rot: f32,
    dead: bool,
}

struct Sparkle {
    x: f32,
    y: f32,
    age: f32,
}

/// The score sun's motion: each earned point adds one full turn to where the
/// sun is heading, and an underdamped spring chases that. So a point is a quick
/// whirl that swings a touch past and settles facing the kid again, and points
/// that come in fast simply whirl on (two points = two turns) instead of
/// restarting the animation. The count printed on the sun's face changes while
/// the face is turned away ([`SunCoin::shown`]), so the sun *comes back* with
/// the new number, like a coin flipping over. Raylib-free so the feel is
/// unit-testable.
#[derive(Default)]
struct SunCoin {
    /// Current spin about the vertical axis, degrees.
    angle: f32,
    /// Spin velocity, degrees/s.
    vel: f32,
    /// Where the spring is heading: 360 per point earned (minus wraps).
    target: f32,
    /// Pop animation, 1 → 0 over `POP_DUR` (0 = resting).
    pop: f32,
}

impl SunCoin {
    fn earn(&mut self) {
        self.target += 360.0;
        self.pop = 1.0;
    }

    fn step(&mut self, dt: f32) {
        // Semi-implicit Euler in small substeps: stable even if a frame hitches.
        let n = (dt * 240.0).ceil().max(1.0);
        let h = dt / n;
        for _ in 0..n as u32 {
            let acc = SPIN_K * (self.target - self.angle) - SPIN_C * self.vel;
            self.vel += acc * h;
            self.angle += self.vel * h;
        }
        // Whole turns look identical, so drop them before f32 loses precision.
        if self.angle > 3600.0 && self.target > 3600.0 {
            self.angle -= 3600.0;
            self.target -= 3600.0;
        }
        self.pop = (self.pop - dt / POP_DUR).max(0.0);
    }

    /// The count to print on the face, given the real one: points whose turn
    /// hasn't yet taken the face past its back-most moment (half a turn to go)
    /// are not shown yet. The swap therefore happens while nobody can see it.
    fn shown(&self, stars: u32) -> u32 {
        let pending = ((self.target - self.angle - 180.0) / 360.0).ceil().max(0.0) as u32;
        stars.saturating_sub(pending)
    }

    /// Scale factor: a jelly wobble — swell fast, dip a hair under, settle.
    fn scale(&self) -> f32 {
        if self.pop <= 0.0 {
            return 1.0;
        }
        let p = 1.0 - self.pop; // progress 0..1
        1.0 + 0.8 * (-6.0 * p).exp() * (p * 3.0 * std::f32::consts::PI).sin()
    }
}

/// GPU-side resources, loaded in `init` (absent in unit tests — no window).
struct Gfx {
    toon: Shader,
    /// The kid's own drawings (flat-draw GLB). `None` only if loading failed —
    /// that layer then stays empty rather than the game dying.
    cloud: Option<FlatModel>,
    /// The score icon. `None` → the HUD falls back to a plain gold disc.
    sun: Option<FlatModel>,
    /// Front-lit banded shading for the sun, turning with it (sun.vs/fs) —
    /// the fallback if Lam::pula didn't compile.
    sun_shader: Shader,
    /// Lam::pula for the sun: its own instance, so its look is tuned apart
    /// from the clouds'.
    sun_glass: Option<Lampula>,
    /// flat-draw's glass shader, for the clouds. `None` → they draw plain.
    lampula: Option<Lampula>,
    /// The sea along the front. `None` → the bank's earth shows instead.
    water: Option<Sea>,
    /// Lam::pula for the meadow and the obstacles. `None` → they draw unlit.
    world_glass: Option<Lampula>,
    /// Lam::pula for the far planes: calmer, no glints. `None` → unlit.
    backdrop_glass: Option<Lampula>,
    /// One tile each of the far planes (`props::mountains`,
    /// `props::palm_grove` twice, `props::jungle`).
    mountains: Option<BrickModel>,
    palms: Option<BrickModel>,
    jungle: Option<BrickModel>,
    /// One tile of the meadow (`props::ground`), laid end to end.
    ground: Option<BrickModel>,
    /// The four obstacles in bricks. `None` → plain boxes, so the game
    /// still plays.
    obstacle_models: Option<ObstacleModels>,
    /// The two ledge lengths in bricks. `None` → plain boxes.
    ledge_models: Option<(BrickModel, BrickModel)>,
    /// The vowel tablets in bricks. `None` → the old flat 2D signs.
    tablets: Option<Tablets>,
    /// Lam::pula for the tablets: white light. `None` → they draw unlit.
    stone_glass: Option<Lampula>,
}

/// Each move's stone tablet (`props::tablet`), plain and lit up, built once
/// the grown-up has picked the vowels.
struct Tablets {
    /// Indexed by [`Kind::move_index`]: (plain, lit).
    by_move: Vec<(BrickModel, BrickModel)>,
}

impl Tablets {
    fn build(thread: &RaylibThread, vowels: [usize; 3]) -> Option<Self> {
        let by_move = vowels
            .iter()
            .zip(props::TABLET_PAINT.into_iter().zip(props::TABLET_LIT_PAINT))
            .map(|(&v, (paint, lit))| {
                let grid = props::tablet(VOWELS[v].label());
                let model = |paint| {
                    BrickModel::build(
                        thread,
                        "tablet",
                        &grid,
                        &props::tablet_palette(paint),
                        props::TABLET_CELL,
                        Build::default(),
                    )
                };
                model(paint).zip(model(lit))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self { by_move })
    }
}

/// The obstacles, built in bricks once (`props.rs`) and drawn as many times
/// as they come.
struct ObstacleModels {
    block: BrickModel,
    bridge: BrickModel,
    wall: BrickModel,
    pillar: BrickModel,
}

impl ObstacleModels {
    fn build(thread: &RaylibThread) -> Option<Self> {
        let model = |name, grid| {
            BrickModel::build(
                thread,
                name,
                &grid,
                &props::OBSTACLE_PALETTE,
                props::OBSTACLE_CELL,
                Build::default(),
            )
        };
        Some(Self {
            block: model("block", props::block())?,
            bridge: model("bridge", props::bridge())?,
            wall: model("wall", props::wall())?,
            pillar: model("pillar", props::pillar())?,
        })
    }

    fn of(&self, kind: Kind) -> &BrickModel {
        match kind {
            Kind::Jump => &self.block,
            Kind::Duck => &self.bridge,
            Kind::Wall => &self.wall,
            Kind::High => &self.pillar,
        }
    }
}

/// Load a flat-draw prop, or complain and carry on without it.
fn load_prop(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    name: &str,
    glb: &[u8],
) -> Option<FlatModel> {
    match FlatModel::load(rl, thread, name, glb) {
        Ok(m) => Some(m),
        Err(e) => {
            eprintln!("{name} model unavailable: {e}");
            None
        }
    }
}

pub struct Runner {
    hero_y: f32, // hero top edge when standing on ground = GROUND_Y - height
    vy: f32,
    on_ground: bool,
    /// The mid-air jump has been spent this airtime (reset on landing).
    air_jumped: bool,
    /// Salto flip animation, seconds remaining (0 = not flipping).
    salto: f32,
    ducking: bool,
    obstacles: Vec<Obstacle>,
    bullets: Vec<Bullet>,
    sparkles: Vec<Sparkle>,
    stars: u32,
    /// The score icon's spin/pop, kicked by every point.
    coin: SunCoin,
    /// Each move's tablets hopping (1 → 0 over `TABLET_HOP_DUR`), kicked
    /// when its vowel is said. Indexed by [`Kind::move_index`].
    tablet_hop: [f32; 3],
    speed: f32,
    spawn_timer: f32,
    /// Hero squash feedback after a bump (seconds remaining).
    squash: f32,
    t: f32,
    /// Distance travelled (logical px) — drives all parallax offsets.
    dist: f32,
    /// The camera at rest, in the picture's latest shape (see `CAMERA_AT`).
    eye: Eye,
    /// How far the camera has risen after the hero, and how fast it moves.
    cam_lift: f32,
    cam_lift_v: f32,
    rng: u64,
    // Copied from the latest VoiceInput so draw() can show live feedback.
    last_scores: [f32; 6],
    last_held: Option<usize>,
    last_level: f32,
    /// The voice level, smoothed (quick up, slow down) — what the water
    /// dances to.
    voice_glow: f32,
    /// Grown-up-chosen controls (indices into VOWELS).
    jump_vowel: usize,
    duck_vowel: usize,
    shoot_vowel: usize,
    /// Obstacle kinds allowed to spawn (the grown-up's difficulty pick; never
    /// empty — falls back to all three).
    kinds: Vec<Kind>,
    ledges: Vec<Ledge>,
    little_suns: Vec<LittleSun>,
    /// The last spawn was a ledge pattern (the next one is an obstacle).
    last_ledges: bool,
    /// The obstacle kinds still to come this round (see `next_kind`).
    bag: Vec<Kind>,
    last_kind: Option<Kind>,
    gfx: Option<Gfx>,
}

impl Runner {
    pub fn new(jump_vowel: usize, duck_vowel: usize, shoot_vowel: usize, kinds: Vec<Kind>) -> Self {
        let kinds = if kinds.is_empty() {
            vec![Kind::Jump, Kind::Duck, Kind::Wall]
        } else {
            kinds
        };
        Self {
            hero_y: GROUND_Y - HERO_H,
            vy: 0.0,
            on_ground: true,
            air_jumped: false,
            salto: 0.0,
            ducking: false,
            obstacles: Vec::new(),
            bullets: Vec::new(),
            sparkles: Vec::new(),
            stars: 0,
            coin: SunCoin::default(),
            tablet_hop: [0.0; 3],
            speed: 260.0,
            // The first obstacle comes in at once: it enters out of sight, so it is
            // still a few seconds off.
            spawn_timer: 0.1,
            squash: 0.0,
            t: 0.0,
            dist: 0.0,
            eye: Eye {
                position: CAMERA_AT,
                target: CAMERA_LOOKS_AT,
                fovy: CAMERA_FOVY,
                aspect: 16.0 / 9.0,
            },
            cam_lift: 0.0,
            cam_lift_v: 0.0,
            rng: 0x2545_F491_4F6C_DD1D,
            last_scores: [0.0; 6],
            last_held: None,
            last_level: 0.0,
            voice_glow: 0.0,
            jump_vowel,
            duck_vowel,
            shoot_vowel,
            kinds,
            ledges: Vec::new(),
            little_suns: Vec::new(),
            last_ledges: false,
            bag: Vec::new(),
            last_kind: None,
            gfx: None,
        }
    }

    fn rand(&mut self) -> f32 {
        // xorshift64* — plenty for obstacle variety.
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        (self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }

    /// What the camera sees now: the eye at rest, risen after the hero.
    fn view(&self) -> Eye {
        self.eye.lifted(self.cam_lift)
    }

    /// Where to put a thing that takes up `shape` when it stands at logical
    /// x `at`, so that it comes in just out of sight on the right.
    fn entry_px(&self, shape: Bounds, at: f32) -> f32 {
        lx(self.view().entry(shape.shifted(-wx(at))))
    }

    /// The vowel of each move, indexed by [`Kind::move_index`].
    fn move_vowels(&self) -> [usize; 3] {
        [self.jump_vowel, self.duck_vowel, self.shoot_vowel]
    }

    fn hero_rect(&self) -> (f32, f32, f32, f32) {
        let h = if self.ducking { DUCK_H } else { HERO_H };
        // Ducking keeps the feet where they are: on the ground or a ledge.
        let top = if self.on_ground && self.ducking {
            self.hero_y + HERO_H - DUCK_H
        } else {
            self.hero_y
        };
        (HERO_X - HERO_W / 2.0, top, HERO_W, h)
    }

    /// The highest surface under the hero at or below `bottom` (a logical y;
    /// smaller is higher): the ground, or a ledge the hero overlaps along x
    /// whose top is no higher than the feet. A ledge above the feet doesn't
    /// count, which is what lets the hero jump up through one.
    /// Demo mode's pilot: this frame's input as a child who knows every
    /// vowel would give it. It jumps a block, ducks under a bar, shoots a
    /// wall, double-jumps a pillar, hops onto a ledge (and up a staircase)
    /// and runs along it for the little suns, saying the vowel as it goes,
    /// so the hero's mouth moves and the water dances. Every trigger is a
    /// lead *time*, so it keeps up as the game speeds up. A test holds it to
    /// clearing everything, which also proves every obstacle can be cleared.
    fn pilot(&self) -> VoiceInput {
        let (jump, duck, shoot) = (self.jump_vowel, self.duck_vowel, self.shoot_vowel);
        let lead = |secs: f32| self.speed * secs;
        let front = HERO_X + HERO_W / 2.0;
        let back = HERO_X - HERO_W / 2.0;
        let feet = self.hero_y + HERO_H;
        let near_apex = !self.on_ground && !self.air_jumped && self.vy > -250.0;
        let mut held = None;
        let mut onset = None;

        // The next obstacle not yet behind the hero.
        let next = self
            .obstacles
            .iter()
            .filter(|o| !o.bounced)
            .filter(|o| {
                let (ox, _, ow, _) = obstacle_rect(o);
                ox + ow > back
            })
            .min_by(|a, b| a.x.total_cmp(&b.x));
        if let Some(o) = next {
            let (ox, _, ow, _) = obstacle_rect(o);
            let ahead = o.x - HERO_X;
            match o.kind {
                Kind::Jump if self.on_ground && ahead < lead(0.42) => onset = Some(jump),
                Kind::Duck if ox < front + lead(0.2) && ox + ow > back => held = Some(duck),
                Kind::Wall if self.bullets.is_empty() && ahead < 700.0 => onset = Some(shoot),
                Kind::High if self.on_ground && ahead < lead(0.7) => onset = Some(jump),
                Kind::High if near_apex && ahead < lead(0.7) => onset = Some(jump),
                _ => {}
            }
        }

        // The next ledge ahead, higher than where the feet are now.
        let ledge = self
            .ledges
            .iter()
            .filter(|l| l.x + l.w() > front && l.top < feet - 1.0)
            .min_by(|a, b| a.x.total_cmp(&b.x));
        if let (None, Some(l)) = (onset, ledge) {
            let climb = feet - l.top;
            let single = JUMP_V * JUMP_V / (2.0 * GRAVITY) - 30.0;
            if self.on_ground && l.x - front < lead(0.3) {
                onset = Some(jump);
            } else if climb > single && near_apex && l.x - front < lead(0.6) {
                onset = Some(jump); // too high for one jump: the second
            }
        }

        // Saying it: a vowel is held while it's spoken.
        let held = held.or(onset);
        VoiceInput {
            held,
            onset,
            scores: [0.0; 6],
            level: if held.is_some() { 0.8 } else { 0.05 },
        }
    }

    fn floor_below(&self, bottom: f32) -> f32 {
        let (hx, _, hw, _) = self.hero_rect();
        self.ledges
            .iter()
            .filter(|l| hx < l.x + l.w() && l.x < hx + hw && l.top >= bottom - 0.5)
            .map(|l| l.top)
            .fold(GROUND_Y, f32::min)
    }

    /// The next obstacle kind, dealt from a shuffled bag that holds each
    /// allowed kind once and is refilled when empty: every kind (every vowel)
    /// comes round once a round, so none waits long and none crowds the
    /// others out, and a refill never repeats the kind just dealt.
    fn next_kind(&mut self) -> Kind {
        if self.bag.is_empty() {
            self.bag = self.kinds.clone();
            for i in (1..self.bag.len()).rev() {
                let j = ((self.rand() * (i + 1) as f32) as usize).min(i);
                self.bag.swap(i, j);
            }
            let n = self.bag.len();
            if n > 1 && self.bag.last() == self.last_kind.as_ref() {
                self.bag.swap(0, n - 1);
            }
        }
        // `kinds` is never empty (see `new`), so neither is a fresh bag.
        let kind = self.bag.pop().unwrap_or(Kind::Jump);
        self.last_kind = Some(kind);
        kind
    }

    /// Add a ledge whose left edge is at `x` and top `height` above the
    /// ground, little suns along it; its right edge.
    fn add_ledge(&mut self, x: f32, size: LedgeSize, height: f32) -> f32 {
        let w = size.width();
        let top = GROUND_Y - height;
        self.ledges.push(Ledge { x, top, size });
        let n = (w / 100.0).round().max(1.0) as usize;
        for i in 0..n {
            let phase = self.rand() * std::f32::consts::TAU;
            self.little_suns.push(LittleSun {
                x: x + (i as f32 + 0.5) * w / n as f32,
                y: top - LITTLE_SUN_LIFT,
                phase,
            });
        }
        x + w
    }

    /// Spawn a ledge pattern at the right edge: a long low ledge, a high one
    /// (double jump), or a staircase from a low one up to a high one. How far
    /// it reaches past the spawn point, logical px.
    fn spawn_ledges(&mut self) -> f32 {
        // In just out of sight on the right, for the tallest pattern.
        let probe = Ledge {
            x: 0.0,
            top: GROUND_Y - LEDGE_HIGH,
            size: LedgeSize::Long,
        };
        let x = self.entry_px(ledge_bounds(&probe), probe.x);
        let end = match (self.rand() * 3.0) as u32 {
            0 => self.add_ledge(x, LedgeSize::Long, LEDGE_LOW),
            1 => self.add_ledge(x, LedgeSize::Short, LEDGE_HIGH),
            _ => {
                let step = self.add_ledge(x, LedgeSize::Short, LEDGE_LOW);
                self.add_ledge(step + STAIR_GAP, LedgeSize::Short, LEDGE_HIGH)
            }
        };
        end - x
    }
}

fn obstacle_rect(o: &Obstacle) -> (f32, f32, f32, f32) {
    match o.kind {
        // Low block sitting on the ground.
        Kind::Jump => (o.x - 40.0, GROUND_Y - 88.0 + o.fly_y, 80.0, 88.0),
        // Floating bar: a ducking hero fits under, a standing one does not.
        Kind::Duck => (o.x - 70.0, GROUND_Y - 78.0 - 46.0 + o.fly_y, 140.0, 46.0),
        // Tall floor-standing wall: only a star bullet clears it.
        Kind::Wall => (
            o.x - WALL_W / 2.0,
            GROUND_Y - WALL_H + o.fly_y,
            WALL_W,
            WALL_H,
        ),
        // Tall slim pillar: only the ninja double-jump clears it.
        Kind::High => (
            o.x - HIGH_W / 2.0,
            GROUND_Y - HIGH_H + o.fly_y,
            HIGH_W,
            HIGH_H,
        ),
    }
}

/// The middle of an obstacle's tablet, world units, given how far the
/// obstacle's front stands out (`front`, its model's half depth): over a low
/// obstacle it floats behind the hero's lane, a fixed height above the top;
/// on a tall one it hangs on the front, over the upper part (their tops are
/// near or past the top of the screen). It flies off with a bumped obstacle.
fn tablet_centre(o: &Obstacle, front: f32) -> Vector3 {
    let (rx, ry, rw, _) = obstacle_rect(o);
    let hung_z = front + props::TABLET_D as f32 * props::TABLET_CELL / 2.0;
    let (y, z) = match o.kind {
        Kind::Wall => (wy(ry + WALL_H * 0.35), hung_z),
        Kind::High => (wy(ry + HIGH_H * 0.30), hung_z),
        Kind::Jump | Kind::Duck => (wy(ry) + 1.15, TABLET_FLOAT_Z),
    };
    Vector3::new(wx(rx + rw / 2.0), y, z)
}

/// How high a hopping tablet is lifted, `hop` going 1 → 0: one quick arc.
fn tablet_lift(hop: f32) -> f32 {
    if hop <= 0.0 {
        return 0.0;
    }
    TABLET_HOP * (std::f32::consts::PI * (1.0 - hop)).sin()
}

// ---- what things take up in the world, for coming and going out of sight --

/// How deep the obstacles stand, either side of the hero's lane: their brick
/// grids are at most five bricks deep (a test holds them to it).
const OBSTACLE_HALF_DEPTH: f32 = 2.5 * props::OBSTACLE_CELL;

/// An obstacle, its tablet included (hopping and bobbing at its highest).
fn obstacle_bounds(o: &Obstacle) -> Bounds {
    let (rx, ry, rw, rh) = obstacle_rect(o);
    let body = Bounds {
        x: (wx(rx), wx(rx + rw)),
        y: (wy(ry + rh), wy(ry)),
        z: (-OBSTACLE_HALF_DEPTH, OBSTACLE_HALF_DEPTH),
    };
    let c = tablet_centre(o, OBSTACLE_HALF_DEPTH);
    let half = |n: usize| n as f32 * props::TABLET_CELL / 2.0;
    let (hw, hh, hd) = (
        half(props::TABLET_W),
        half(props::TABLET_H),
        half(props::TABLET_D),
    );
    body.union(Bounds {
        x: (c.x - hw, c.x + hw),
        y: (c.y - hh - TABLET_BOB, c.y + hh + TABLET_BOB + TABLET_HOP),
        z: (c.z - hd, c.z + hd),
    })
}

/// A ledge, the little suns over it included.
fn ledge_bounds(l: &Ledge) -> Bounds {
    let thick = props::LEDGE_LAYERS as f32 * props::GROUND_CELL;
    let half_depth = 1.5 * props::GROUND_CELL; // three meadow bricks deep
    Bounds {
        x: (wx(l.x), wx(l.x + l.w())),
        y: (
            wy(l.top) - thick,
            wy(l.top - LITTLE_SUN_LIFT - LITTLE_SUN_PX),
        ),
        z: (-half_depth, half_depth),
    }
}

/// A little sun, bobbing.
fn little_sun_bounds(s: &LittleSun) -> Bounds {
    let r = LITTLE_SUN_PX / PPU / 2.0 + 0.03;
    Bounds {
        x: (wx(s.x) - r, wx(s.x) + r),
        y: (wy(s.y) - r, wy(s.y) + r),
        z: (-r, r),
    }
}

/// A star bullet: its core and the five cubes whirling round it.
fn bullet_bounds(b: &Bullet) -> Bounds {
    let r = 0.17 + 0.05;
    Bounds {
        x: (wx(b.x) - r, wx(b.x) + r),
        y: (wy(b.y) - r, wy(b.y) + r),
        z: (0.25, 0.35),
    }
}

fn overlaps(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32), shrink: f32) -> bool {
    // Shrink both rects for forgiving hitboxes.
    let s = |(x, y, w, h): (f32, f32, f32, f32)| {
        (
            x + w * shrink,
            y + h * shrink,
            w * (1.0 - 2.0 * shrink),
            h * (1.0 - 2.0 * shrink),
        )
    };
    let (ax, ay, aw, ah) = s(a);
    let (bx, by, bw, bh) = s(b);
    ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
}

// ---- logical px -> world units --------------------------------------------

/// World x of a logical-pixel x (screen centre -> 0).
fn wx(x_px: f32) -> f32 {
    (x_px - LW / 2.0) / PPU
}
/// Logical-pixel x of a world x (the other way round from [`wx`]).
fn lx(x: f32) -> f32 {
    x * PPU + LW / 2.0
}
/// World y of a logical-pixel y (ground top -> 0, up positive).
fn wy(y_px: f32) -> f32 {
    (GROUND_Y - y_px) / PPU
}
/// Centre + size of a logical rect as world vectors (at depth z, thickness d).
fn wrect(r: (f32, f32, f32, f32), z: f32, depth: f32) -> (Vector3, Vector3) {
    let (x, y, w, h) = r;
    (
        Vector3::new(wx(x + w / 2.0), wy(y + h / 2.0), z),
        Vector3::new(w / PPU, h / PPU, depth),
    )
}

/// Deterministic per-tile hash in 0..1 (world-stable procedural content).
/// A non-zero seed from the clock, for the obstacle generator.
fn clock_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    // splitmix64: spread the clock's low-entropy bits over the whole word.
    let mut z = nanos.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (z ^ (z >> 31)) | 1
}

fn hash01(k: i64, salt: u64) -> f32 {
    let mut h = (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ salt;
    h ^= h >> 33;
    h = h.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h ^= h >> 29;
    (h & 0xFFFF) as f32 / 65535.0
}

/// Lam::pula as the clouds wear it: flat-draw's look, with one change — the
/// "room below" it reflects at the silhouette is our horizon blue instead of
/// flat-draw's dark floor, which read as mud on a sunny sky. (The warm gold
/// lamps and "room above" stay: that is the glass's character.)
fn cloud_glass() -> LampulaParams {
    LampulaParams {
        ground: [SKY_LOW.r, SKY_LOW.g, SKY_LOW.b],
        ..LampulaParams::default()
    }
}

/// Lam::pula as the score sun wears it: flat-draw's look with the same sky
/// "room below" as the clouds, so the glass sits in our sky.
fn sun_glass() -> LampulaParams {
    LampulaParams {
        ground: [SKY_LOW.r, SKY_LOW.g, SKY_LOW.b],
        ..LampulaParams::default()
    }
}

/// Lam::pula as the water wears it: the clouds' glass, with only the light
/// changed. The clouds' warm gold lamps turned the blue murky green and white
/// ones washed it pale, so the lamps are a clear sky blue; the room it
/// reflects is our sky above and a deep sea below.
fn water_glass() -> LampulaParams {
    const LAMP: [u8; 3] = [140, 196, 255];
    LampulaParams {
        lamp0: LAMP,
        lamp1: LAMP,
        lamp2: LAMP,
        sky: [SKY_LOW.r, SKY_LOW.g, SKY_LOW.b],
        ground: [38, 96, 170],
        ..cloud_glass()
    }
}

/// Lam::pula as the meadow and the obstacles wear it: the clouds' glass, made
/// solid (they are things to stand on and bump into, not to see through),
/// under warm daylight lamps rather than the clouds' gold, and toned down.
/// At the clouds' full strength coloured bricks went pastel: the grass is
/// seen nearly edge-on, where the reflected room and the highlights are
/// strongest, and the exposure curve flattened what was left. So less
/// exposure, a faint room, softer highlights, and more vibrance to keep the
/// colours bold.
fn world_glass() -> LampulaParams {
    const LAMP: [u8; 3] = [255, 238, 206];
    LampulaParams {
        lamp0: LAMP,
        lamp1: LAMP,
        lamp2: LAMP,
        alpha: 1.0,
        exposure: 1.1,
        env_gain: 0.3,
        vibrance: 0.7,
        spec_gain: 0.6,
        trans_gain: 1.5,
        ..cloud_glass()
    }
}

/// Lam::pula as the vowel tablets wear it: the meadow's glass made chalk.
/// White lamps, so the white stone stays white (the meadow's warm daylight
/// turned it cream, the colour of the earth), and light all round — a high
/// ambient, a soft wrap — with little sheen and almost none of the blue room
/// at the edges, which together turned it silver. Bright as it is lit, the
/// letters' paints are deep (`props::TABLET_PAINT`) to come out strong.
fn stone_glass() -> LampulaParams {
    const LAMP: [u8; 3] = [255, 255, 255];
    LampulaParams {
        lamp0: LAMP,
        lamp1: LAMP,
        lamp2: LAMP,
        ambient: 1.0,
        diffuse: 0.9,
        wrap: 0.8,
        spec_gain: 0.2,
        env_gain: 0.05,
        ..world_glass()
    }
}

/// Lam::pula as the far planes wear it: the meadow's glass without the corner
/// glints and highlights. Far things don't sparkle, and nothing out there
/// should pull the eye from the hero.
fn backdrop_glass() -> LampulaParams {
    LampulaParams {
        spark_on: false,
        spec_on: false,
        ..world_glass()
    }
}

/// A palm grove's tile, every face built: the crowns rise above the camera,
/// so their undersides show.
fn grove_model(thread: &RaylibThread, name: &str, grove: &props::Grove) -> Option<BrickModel> {
    BrickModel::build(
        thread,
        name,
        &props::palm_grove(grove),
        &grove.palette,
        grove.cell,
        Build {
            wrap_x: true,
            open_below: false,
            open_behind: false,
        },
    )
}

/// The see-through sky pass drawn just in front of a plane hazed `far`, so
/// that after the passes in front of it (which add up to `near`) it ends up
/// hazed `far` in all: passes multiply what shows through, (1 − a)(1 − near)
/// = 1 − far.
fn haze_step(far: f32, near: f32) -> f32 {
    1.0 - (1.0 - far) / (1.0 - near)
}

/// Lay a plane's tile end to end across what `eye` sees at depth `z`,
/// scrolled by `off` world units, its bottom at `base_y`; each tile one draw
/// call, all lit by lamps fixed in the world (`lamps`), so the plane slides
/// under them.
#[allow(clippy::too_many_arguments)]
fn lay_tiles(
    d: &mut impl RaylibDraw3D,
    eye: &Eye,
    model: &BrickModel,
    glass: Option<&mut Lampula>,
    tile: f32,
    base_y: f32,
    z: f32,
    off: f32,
    lamps: (Vector3, Vector3),
) {
    let (l, r) = eye.span(z, (base_y, base_y + model.max.y));
    let k0 = ((off + l) / tile - 0.5).floor() as i64;
    let k1 = ((off + r) / tile + 0.5).ceil() as i64;
    let mut glass = glass;
    for k in k0..=k1 {
        let t = Matrix::translate(k as f32 * tile - off, base_y, z);
        model.draw(d, glass.as_deref_mut(), t, lamps);
    }
}

/// The env var naming a tuning file for [`world_glass`] (any Lam::pula key).
const PROPS_ENV: &str = "RONDELEK_PROPS";

/// [`world_glass`], plus whatever `RONDELEK_PROPS` points at.
fn world_glass_from_env() -> LampulaParams {
    let mut look = world_glass();
    if let Some(path) = std::env::var_os(PROPS_ENV) {
        crate::shader_params::apply_overrides(path.as_ref(), "props", &mut |k, v| look.set(k, v));
    }
    look
}

/// Where the meadow's lamps stand round (world centre, half-extent): the
/// stretch of meadow on screen. Fixed, so the meadow slides under still
/// lamps.
fn ground_lamps() -> (Vector3, Vector3) {
    (Vector3::new(0.0, -0.75, 0.0), Vector3::new(12.0, 0.75, 1.5))
}

/// A cloud's model matrix: standing at `base` (its pivot, bottom-centre),
/// `s` times its drawn size, gently floating and breathing about its own
/// middle, each cloud (lane `k`) out of step with the others.
fn cloud_transform(cloud: &FlatModel, base: Vector3, s: f32, t: f32, k: i64) -> Matrix {
    let phase = hash01(k, 13) * std::f32::consts::TAU;
    let breath = (t * CLOUD_BREATH_RATE + phase).sin() * CLOUD_BREATH;
    let bob = (t * CLOUD_BOB_RATE + phase).sin() * CLOUD_BOB;
    let c = cloud.center;
    Matrix::translate(-c.x, -c.y, -c.z)
        * Matrix::scale(s * (1.0 + breath), s * (1.0 - 0.8 * breath), s)
        * Matrix::translate(base.x + c.x * s, base.y + c.y * s + bob, base.z + c.z * s)
}

impl VoiceGame for Runner {
    fn autoplay(&mut self) -> Option<VoiceInput> {
        Some(self.pilot())
    }

    fn init(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        // Every game its own order of obstacles and ledges (the unit tests,
        // which never init, keep `new`'s fixed seed).
        self.rng = clock_seed();
        let (w, h) = (rl.get_screen_width(), rl.get_screen_height());
        if w > 0 && h > 0 {
            self.eye.aspect = w as f32 / h as f32;
        }

        let toon = rl.load_shader_from_memory(
            thread,
            Some(include_str!("../../assets/shaders/base.vs")),
            Some(include_str!("../../assets/shaders/toon.fs")),
        );
        let sun_shader = rl.load_shader_from_memory(
            thread,
            Some(include_str!("../../assets/shaders/sun.vs")),
            Some(include_str!("../../assets/shaders/sun.fs")),
        );
        // Scenery drawn in flat-draw and exported as GLB; each drawing's parts
        // are merged into one mesh, so every prop on screen is one draw call.
        let cloud = load_prop(
            rl,
            thread,
            "cloud",
            include_bytes!("../../assets/models/cloud.glb"),
        );
        let sun = load_prop(
            rl,
            thread,
            "sun",
            include_bytes!("../../assets/models/sun.glb"),
        );

        self.gfx = Some(Gfx {
            toon,
            cloud,
            sun,
            sun_shader,
            lampula: Lampula::load(rl, thread, cloud_glass()),
            sun_glass: Lampula::load(rl, thread, sun_glass()),
            water: Sea::load(rl, thread, WaterStyle::from_env()),
            world_glass: Lampula::load_exact(rl, thread, world_glass_from_env()),
            ground: BrickModel::build(
                thread,
                "ground",
                &props::ground(),
                &props::GROUND_PALETTE,
                props::GROUND_CELL,
                Build {
                    wrap_x: true,
                    open_below: true,
                    open_behind: true,
                },
            ),
            obstacle_models: ObstacleModels::build(thread),
            backdrop_glass: Lampula::load_exact(rl, thread, backdrop_glass()),
            mountains: BrickModel::build(
                thread,
                "mountains",
                &props::mountains(),
                &props::MOUNTAIN_PALETTE,
                props::MOUNTAIN_CELL,
                Build {
                    wrap_x: true,
                    open_below: true,
                    open_behind: true,
                },
            ),
            // Every face built: the crowns rise above the camera, so their
            // undersides show.
            palms: grove_model(thread, "palms", &props::GROVE),
            jungle: BrickModel::build(
                thread,
                "jungle",
                &props::jungle(),
                &props::JUNGLE_PALETTE,
                props::JUNGLE_CELL,
                props::JUNGLE_BUILD,
            ),
            ledge_models: {
                let ledge = |size: LedgeSize| {
                    BrickModel::build(
                        thread,
                        "ledge",
                        &props::ledge(size.bricks()),
                        &props::GROUND_PALETTE,
                        props::GROUND_CELL,
                        Build::default(),
                    )
                };
                ledge(LedgeSize::Short).zip(ledge(LedgeSize::Long))
            },
            tablets: Tablets::build(thread, self.move_vowels()),
            stone_glass: Lampula::load_exact(rl, thread, stone_glass()),
        });
    }

    fn update(&mut self, input: &VoiceInput, dt: f32) {
        self.t += dt;
        self.squash = (self.squash - dt).max(0.0);
        self.salto = (self.salto - dt).max(0.0);
        self.coin.step(dt);
        let vowels = self.move_vowels();
        for (hop, vowel) in self.tablet_hop.iter_mut().zip(vowels) {
            *hop = if input.onset == Some(vowel) {
                1.0
            } else {
                (*hop - dt / TABLET_HOP_DUR).max(0.0)
            };
        }
        self.last_scores = input.scores;
        self.last_held = input.held;
        self.last_level = input.level;
        // A recognised vowel counts as full voice; any other sound by its level.
        let voice = if input.held.is_some() {
            1.0
        } else {
            input.level.clamp(0.0, 1.0)
        };
        let rate = if voice > self.voice_glow { 8.0 } else { 1.5 };
        self.voice_glow += (voice - self.voice_glow) * (1.0 - (-rate * dt).exp());
        self.dist += self.speed * dt;

        // --- hero ---
        self.ducking = input.held == Some(self.duck_vowel) && self.on_ground;
        if input.onset == Some(self.jump_vowel) {
            if self.on_ground {
                self.vy = -JUMP_V;
                self.on_ground = false;
                self.air_jumped = false;
            } else if !self.air_jumped {
                // Ninja-jump: a second, punchier jump + a forward salto.
                self.vy = -AIR_JUMP_V;
                self.air_jumped = true;
                self.salto = SALTO_DUR;
            }
        }
        // Shoot: a spinning star leaves the hero's front at their current height.
        if input.onset == Some(self.shoot_vowel) {
            let (hx, hy, hw, hh) = self.hero_rect();
            self.bullets.push(Bullet {
                x: hx + hw,
                y: hy + hh / 2.0,
                rot: 0.0,
                dead: false,
            });
        }
        // Standing on a ledge that has slid out from under the feet: drop.
        let feet = self.hero_y + HERO_H;
        if self.on_ground && self.floor_below(feet) > feet + 0.5 {
            self.on_ground = false;
            self.vy = 0.0;
        }
        if !self.on_ground {
            self.vy += GRAVITY * dt;
            self.hero_y += self.vy * dt;
            // Land only on the way down, and only on a surface the feet were
            // above a moment ago: a ledge is jumped up through from below.
            let floor = self.floor_below(feet);
            if self.vy >= 0.0 && self.hero_y + HERO_H >= floor {
                self.hero_y = floor - HERO_H;
                self.vy = 0.0;
                self.on_ground = true;
                self.air_jumped = false;
                self.salto = 0.0; // land upright
            }
        }

        // --- camera: up and down with the hero ---
        // It aims ahead rather than chasing: while the hero rises, at where
        // this jump tops out; once they fall (or stand), at where they come
        // to rest — the ground, or a ledge. So it sets off at the take-off,
        // turns at the top and is coming down as the hero lands, in step,
        // with nothing left to catch up on afterwards.
        let feet = self.hero_y + HERO_H;
        let aim = if self.vy < 0.0 {
            (GROUND_Y - feet + self.vy * self.vy / (2.0 * GRAVITY)) / PPU
        } else {
            (GROUND_Y - self.floor_below(feet)) / PPU
        };
        let goal = ((aim - CAM_DEAD).max(0.0) * CAM_FOLLOW).min(CAM_LIFT_MAX);
        // Semi-implicit Euler in small substeps: stable even if a frame hitches.
        let n = (dt * 240.0).ceil().max(1.0);
        let h = dt / n;
        for _ in 0..n as u32 {
            let rate = if goal < self.cam_lift {
                CAM_RATE_DOWN
            } else {
                CAM_RATE_UP
            };
            let pull = rate * rate * (goal - self.cam_lift);
            self.cam_lift_v += (pull - 2.0 * rate * self.cam_lift_v) * h;
            self.cam_lift += self.cam_lift_v * h;
        }
        let view = self.view();

        // --- obstacles ---
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            // Faster game = slightly denser spawns, always with breathing room.
            let gap = 2.6 - (self.speed - 260.0) / 240.0 * 0.8 + self.rand() * 0.6;
            if !self.last_ledges && self.rand() < LEDGE_CHANCE {
                // The next spawn waits until the whole pattern is in.
                let reach = self.spawn_ledges();
                self.spawn_timer = gap + reach / self.speed;
                self.last_ledges = true;
            } else {
                let kind = self.next_kind();
                let mut o = Obstacle::new(0.0, kind);
                o.x = self.entry_px(obstacle_bounds(&o), o.x);
                o.phase = hash01(self.dist as i64, 14) * std::f32::consts::TAU;
                self.obstacles.push(o);
                self.spawn_timer = gap;
                self.last_ledges = false;
            }
        }

        let hero = self.hero_rect();
        let (airborne, ducking) = (!self.on_ground, self.ducking);
        let mut starred = false;
        for o in &mut self.obstacles {
            o.x -= self.speed * dt;
            if o.bounced {
                o.fly_vy += GRAVITY * 0.5 * dt;
                o.fly_y += o.fly_vy * dt;
                o.rot += 360.0 * dt;
                continue;
            }
            let (ox, _, ow, _) = obstacle_rect(o);
            if hero.0 < ox + ow && ox < hero.0 + hero.2 {
                o.leapt |= airborne;
                o.ducked |= ducking;
            }
            if overlaps(hero, obstacle_rect(o), 0.2) {
                // No punishment: the obstacle is the one that gets launched.
                o.bounced = true;
                o.fly_vy = -520.0;
                self.squash = 0.35;
            } else if o.x < HERO_X - 90.0 && !o.counted {
                o.counted = true;
                if !o.kind.earns_a_star(o.leapt, o.ducked) {
                    continue;
                }
                self.stars += 1;
                self.coin.earn();
                starred = true;
                self.sparkles.push(Sparkle {
                    x: o.x,
                    y: GROUND_Y - 150.0,
                    age: 0.0,
                });
            }
        }
        if starred {
            self.speed = (self.speed + 8.0).min(500.0);
        }
        // Gone once all of it is out past the left edge (or, bumped, fallen
        // far below the picture).
        self.obstacles
            .retain(|o| !view.gone_left(obstacle_bounds(o)) && o.fly_y < 720.0);

        // --- ledges and their little suns ---
        for l in &mut self.ledges {
            l.x -= self.speed * dt;
        }
        self.ledges.retain(|l| !view.gone_left(ledge_bounds(l)));
        for s in &mut self.little_suns {
            s.x -= self.speed * dt;
        }
        // A little sun the hero touches is a point (it spins the score sun,
        // but unlike an obstacle it doesn't speed the game up).
        let hero = self.hero_rect();
        let (taken, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut self.little_suns)
            .into_iter()
            .partition(|s| overlaps(hero, little_sun_rect(s), 0.0));
        self.little_suns = kept
            .into_iter()
            .filter(|s| !view.gone_left(little_sun_bounds(s)))
            .collect();
        for s in taken {
            self.stars += 1;
            self.coin.earn();
            self.sparkles.push(Sparkle {
                x: s.x,
                y: s.y,
                age: 0.0,
            });
        }

        // --- bullets: fly right, spin, smash the first wall they touch ---
        for bi in 0..self.bullets.len() {
            self.bullets[bi].x += BULLET_VX * dt;
            self.bullets[bi].rot += 720.0 * dt;
            let (bx, by) = (self.bullets[bi].x, self.bullets[bi].y);
            if view.gone_right(bullet_bounds(&self.bullets[bi])) {
                self.bullets[bi].dead = true;
                continue;
            }
            for oi in 0..self.obstacles.len() {
                let o = &self.obstacles[oi];
                if o.kind != Kind::Wall || o.bounced {
                    continue;
                }
                let (rx, ry, rw, rh) = obstacle_rect(o);
                if bx >= rx && bx <= rx + rw && by >= ry && by <= ry + rh {
                    // Shatter the wall: launch it, count the star, spark it up.
                    let o = &mut self.obstacles[oi];
                    o.bounced = true;
                    o.counted = true;
                    o.fly_vy = -640.0;
                    self.bullets[bi].dead = true;
                    self.stars += 1;
                    self.coin.earn();
                    self.speed = (self.speed + 8.0).min(500.0);
                    for k in 0..7 {
                        let jitter = (self.rand() - 0.5) * rw;
                        self.sparkles.push(Sparkle {
                            x: rx + rw / 2.0 + jitter,
                            y: ry + rh * 0.25 + k as f32 * 18.0,
                            age: 0.0,
                        });
                    }
                    break;
                }
            }
        }
        self.bullets.retain(|b| !b.dead);

        for s in &mut self.sparkles {
            s.age += dt;
            s.y -= 60.0 * dt;
        }
        self.sparkles.retain(|s| s.age < 1.0);
    }

    fn draw(&mut self, d: &mut RaylibDrawHandle, w: i32, h: i32) {
        if w > 0 && h > 0 {
            self.eye.aspect = w as f32 / h as f32;
        }
        let eye = self.view();
        let Some(gfx) = &mut self.gfx else {
            return; // headless (unit tests) — nothing to draw with
        };
        let camera = eye.camera();

        // Sky behind everything: deep blue overhead, paler toward the horizon.
        d.clear_background(SKY_LOW);
        d.draw_rectangle_gradient_v(0, 0, w, h, SKY_TOP, SKY_LOW);

        let dist_u = self.dist / PPU;

        // The score: the sun, centred at the top where the kid is already
        // looking, with the count on its face.
        let scale = (w as f32 / LW).min(h as f32 / 720.0);
        let sl = |v: f32| (v * scale) as i32;
        let shown = self.coin.shown(self.stars).to_string();
        let pop = self.coin.scale();
        let icon_r = 40.0 * scale;
        let icon_c = Vector2::new(w as f32 / 2.0, sl(110.0) as f32);
        // Where the sun floats in the world: SUN_DIST along the camera ray
        // through the icon's screen spot, turned to face the camera square-on
        // so it reads like a HUD icon — but a real 3D one when it whirls.
        let sun_place = gfx.sun.as_ref().map(|sun| {
            let ray = d.get_screen_to_world_ray(icon_c, camera);
            let pos = ray.position + ray.direction * SUN_DIST;
            let fwd = (camera.target - camera.position).normalize();
            // World units per screen px at that depth (fovy 45°).
            let depth = SUN_DIST * ray.direction.dot(fwd);
            let per_px = 2.0 * depth * (22.5f32).to_radians().tan() / h as f32;
            let bob = (self.t * 2.2).sin() * 4.0 * scale * per_px;
            let size = SUN_PX * scale * per_px * pop;
            let s = size / sun.size.y.max(f32::EPSILON);
            let to_cam = -ray.direction;
            // Idle: a slow, shy sway; on top of it, the point whirls.
            let spin = self.coin.angle + (self.t * 1.3).sin() * 10.0;
            let c = sun.center;
            let m = Matrix::translate(-c.x, -c.y, -c.z)
                * Matrix::scale(s, s, s)
                * Matrix::rotate_y(spin.to_radians())
                * Matrix::rotate_x(-to_cam.y.asin())
                * Matrix::rotate_y(to_cam.x.atan2(to_cam.z))
                * Matrix::translate(pos.x, pos.y + bob, pos.z);
            (m, spin)
        });
        // The count, laid out in the pixel font's own px (size 20 = the
        // 10-px font at 2x), then mapped onto the sun's face: x right, y down
        // → the drawing's x right, y up, just in front of its front layer.
        // It turns with the sun because it is drawn with the sun's matrix.
        let score_plate = gfx.sun.as_ref().zip(sun_place).map(|(sun, (m, _))| {
            let fs = 20;
            let tw = d.measure_text(&shown, fs).max(1) as f32;
            let k = (SCORE_DIGIT_H / fs as f32).min(SCORE_MAX_W / tw);
            let c = sun.center;
            Matrix::scale(k, -k, k)
                * Matrix::translate(
                    c.x - tw * k / 2.0,
                    c.y + fs as f32 * k / 2.0,
                    sun.max.z + 0.03,
                )
                * m
        });

        {
            let mut c3 = d.begin_mode3D(camera);

            // --- clouds (z -14, factor 0.10): the kid's own drawing ---------
            // One instance per sky lane, each a single draw call of the merged
            // flat-draw mesh, through Lam::pula; only the transform differs.
            if let Some(cloud) = &gfx.cloud {
                if let Some(lamp) = gfx.lampula.as_mut() {
                    lamp.begin_frame(camera.position, self.t);
                }
                let z = -14.0;
                let off = dist_u * 0.10 + self.t * 0.12;
                // Every lane whose cloud reaches into the picture, at its
                // biggest and highest.
                let tallest = CLOUD_H + CLOUD_H_VARY;
                let (l, r) = eye.span(z, (3.3, 3.3 + 2.2 + tallest));
                let reach = cloud.size.x * cloud.scale_for_height(tallest) * 0.55;
                let k0 = ((off + l - reach) / CLOUD_PERIOD).floor() as i64;
                let k1 = ((off + r + reach) / CLOUD_PERIOD).ceil() as i64;
                for k in k0..=k1 {
                    let cx = k as f32 * CLOUD_PERIOD - off;
                    // Vary size and altitude per lane so the repeat is invisible.
                    let s = cloud.scale_for_height(CLOUD_H + hash01(k, 12) * CLOUD_H_VARY);
                    let base = 3.3 + hash01(k, 11) * 2.2;
                    let m = cloud_transform(cloud, Vector3::new(cx, base, z), s, self.t, k);
                    match gfx.lampula.as_mut() {
                        Some(lamp) => lamp.draw(&mut c3, cloud, m),
                        None => cloud.draw_transformed(&mut c3, m),
                    }
                }
            }
        }

        // --- haze and mist: the far planes, painter's distance -----------
        // The sky's own gradient once more, see-through: over bare sky it is
        // the same colour, so it vanishes; over a plane it pulls the colours
        // toward the sky behind, the more the farther the plane (how a
        // painter pushes things into the distance). One pass in front of each
        // plane; a plane behind gets every pass in front of it as well.
        let haze = |d: &mut RaylibDrawHandle, a: f32| {
            let k = |c: Color| Color::new(c.r, c.g, c.b, (255.0 * a) as u8);
            d.draw_rectangle_gradient_v(0, 0, w, h, k(SKY_TOP), k(SKY_LOW));
        };
        // Valley mist rising from a plane's foot (see MOUNTAIN_MIST).
        let mist = |d: &mut RaylibDrawHandle, z: f32, (foot, fade, alpha): (f32, f32, f32)| {
            let at = |y: f32| d.get_world_to_screen(Vector3::new(0.0, y, z), camera).y as i32;
            let (clear_y, thick_y) = (at(foot + fade), at(foot));
            let thick = Color::new(MIST.r, MIST.g, MIST.b, (255.0 * alpha) as u8);
            let clear = Color::new(MIST.r, MIST.g, MIST.b, 0);
            d.draw_rectangle_gradient_v(0, clear_y, w, thick_y - clear_y, clear, thick);
            d.draw_rectangle(0, thick_y, w, h - thick_y, thick);
        };
        haze(d, haze_step(CLOUD_HAZE, MOUNTAIN_HAZE));

        // --- mountains (z -12, factor 0.16): in front of the clouds --------
        // Their tallest peaks rise into the clouds' band, so now and then one
        // stands in front of a cloud (the depth test does it).
        if let Some(mountains) = &gfx.mountains {
            let mut c3 = d.begin_mode3D(camera);
            if let Some(glass) = gfx.backdrop_glass.as_mut() {
                glass.begin_frame(camera.position, self.t);
            }
            lay_tiles(
                &mut c3,
                &eye,
                mountains,
                gfx.backdrop_glass.as_mut(),
                props::MOUNTAIN_TILE as f32 * props::MOUNTAIN_CELL,
                props::MOUNTAIN_BASE,
                MOUNTAIN_Z,
                dist_u * MOUNTAIN_SCROLL,
                (
                    Vector3::new(0.0, MOUNTAIN_MID, MOUNTAIN_Z),
                    Vector3::new(24.0, MOUNTAIN_MID - props::MOUNTAIN_BASE, 0.6),
                ),
            );
        }
        haze(d, haze_step(MOUNTAIN_HAZE, PALM_HAZE));
        mist(d, MOUNTAIN_Z, MOUNTAIN_MIST);

        // --- palm grove (z -10.25, factor 0.23) ----------------------------
        // A step less hazed than the mountains behind; the jungle hides
        // their feet.
        if let Some(model) = &gfx.palms {
            let grove = &props::GROVE;
            let mut c3 = d.begin_mode3D(camera);
            lay_tiles(
                &mut c3,
                &eye,
                model,
                gfx.backdrop_glass.as_mut(),
                grove.tile as f32 * grove.cell,
                grove.base,
                PALM_Z,
                dist_u * PALM_SCROLL,
                (Vector3::new(0.0, 2.5, PALM_Z), Vector3::new(17.0, 3.0, 0.2)),
            );
        }
        haze(d, haze_step(PALM_HAZE, JUNGLE_HAZE));
        // The palms' trunks fade into mist, well up toward their crowns.
        mist(d, PALM_Z, PALM_MIST);

        // --- jungle (z -5, factor 0.36): rising out of the mist -----------
        if let Some(jungle) = &gfx.jungle {
            let mut c3 = d.begin_mode3D(camera);
            lay_tiles(
                &mut c3,
                &eye,
                jungle,
                gfx.backdrop_glass.as_mut(),
                props::JUNGLE_TILE as f32 * props::JUNGLE_CELL,
                props::JUNGLE_BASE,
                JUNGLE_Z,
                dist_u * JUNGLE_SCROLL,
                (
                    Vector3::new(0.0, JUNGLE_MID, JUNGLE_Z),
                    Vector3::new(
                        16.0,
                        JUNGLE_MID - props::JUNGLE_BASE,
                        props::JUNGLE_DEPTH as f32 * props::JUNGLE_CELL / 2.0,
                    ),
                ),
            );
        }
        haze(d, JUNGLE_HAZE);
        mist(d, JUNGLE_Z, JUNGLE_MIST);

        {
            let mut c3 = d.begin_mode3D(camera);

            if let Some(glass) = gfx.world_glass.as_mut() {
                glass.begin_frame(camera.position, self.t);
            }

            // --- ground (factor 1.0): the meadow, in bricks ----------------
            // Tile after tile of the same meadow (each one draw call), its
            // grass top on y = 0 and its front, the bank, at z = 1.5.
            if let Some(ground) = &gfx.ground {
                lay_tiles(
                    &mut c3,
                    &eye,
                    ground,
                    gfx.world_glass.as_mut(),
                    props::GROUND_TILE as f32 * props::GROUND_CELL,
                    -(props::GROUND_LAYERS as f32) * props::GROUND_CELL,
                    0.0,
                    dist_u,
                    ground_lamps(),
                );
            }

            // --- water, in front of the bank ------------------------------
            if let Some(water) = gfx.water.as_mut() {
                water.draw(&mut c3, camera.position, self.t, dist_u, self.voice_glow);
            }

            // --- ledges (hero plane z 0), in meadow bricks -----------------
            // Each lit by lamps round itself, like the obstacles.
            for l in &self.ledges {
                let model = gfx.ledge_models.as_ref().map(|(short, long)| match l.size {
                    LedgeSize::Short => short,
                    LedgeSize::Long => long,
                });
                match model {
                    Some(model) => {
                        let at =
                            Matrix::translate(wx(l.x + l.w() / 2.0), wy(l.top) - model.max.y, 0.0);
                        let lamps = world_box(model.min, model.max, at);
                        model.draw(&mut c3, gfx.world_glass.as_mut(), at, lamps);
                    }
                    None => {
                        let thick = props::LEDGE_LAYERS as f32 * props::GROUND_CELL * PPU;
                        let (pos, size) = wrect((l.x, l.top, l.w(), thick), 0.0, 0.75);
                        c3.draw_cube_v(pos, size, Color::GREEN);
                    }
                }
            }

            // --- obstacles (hero plane z 0), in bricks ---------------------
            // Each stands on the ground at its x (or flies off, bumped), lit
            // by lamps round itself, so every one is lit alike.
            for o in &self.obstacles {
                let at = Matrix::translate(wx(o.x), wy(GROUND_Y + o.fly_y), 0.0);
                match &gfx.obstacle_models {
                    Some(models) => {
                        let model = models.of(o.kind);
                        let lamps = world_box(model.min, model.max, at);
                        model.draw(&mut c3, gfx.world_glass.as_mut(), at, lamps);
                    }
                    None => {
                        let (pos, size) = wrect(obstacle_rect(o), 0.0, 0.6);
                        c3.draw_cube_v(pos, size, Color::GRAY);
                    }
                }
            }

            // --- vowel tablets: carved stone over (or on) each obstacle ----
            // Alive: the floating ones bob, and every tablet hops when its
            // vowel is said and lights up while it is held. A bumped
            // obstacle carries its tablet off, still.
            if let Some(tablets) = &gfx.tablets {
                if let Some(glass) = gfx.stone_glass.as_mut() {
                    glass.begin_frame(camera.position, self.t);
                }
                let vowels = [self.jump_vowel, self.duck_vowel, self.shoot_vowel];
                for o in &self.obstacles {
                    let m = o.kind.move_index();
                    let lit = !o.bounced && self.last_held == Some(vowels[m]);
                    let (plain, glowing) = &tablets.by_move[m];
                    let model = if lit { glowing } else { plain };
                    let front = gfx
                        .obstacle_models
                        .as_ref()
                        .map_or(OBSTACLE_HALF_DEPTH, |models| models.of(o.kind).max.z);
                    let c = tablet_centre(o, front);
                    let floats = c.z == TABLET_FLOAT_Z && !o.bounced;
                    let bob = if floats {
                        (self.t * TABLET_BOB_RATE + o.phase).sin() * TABLET_BOB
                    } else {
                        0.0
                    };
                    let hop = if o.bounced {
                        0.0
                    } else {
                        tablet_lift(self.tablet_hop[m])
                    };
                    let at = Matrix::translate(c.x, c.y - model.max.y / 2.0 + bob + hop, c.z);
                    let lamps = world_box(model.min, model.max, at);
                    model.draw(&mut c3, gfx.stone_glass.as_mut(), at, lamps);
                }
            }

            // --- star bullets: spinning 2.5D gold bursts -------------------
            for b in &self.bullets {
                let (cx, cy, z) = (wx(b.x), wy(b.y), 0.3);
                let rot = b.rot.to_radians();
                c3.draw_cube(Vector3::new(cx, cy, z), 0.13, 0.13, 0.1, BUTTER);
                for k in 0..5 {
                    let a = rot + k as f32 * std::f32::consts::TAU / 5.0;
                    c3.draw_cube(
                        Vector3::new(cx + 0.17 * a.cos(), cy + 0.17 * a.sin(), z),
                        0.1,
                        0.1,
                        0.08,
                        BUTTER,
                    );
                }
            }

            // --- hero: cardboard brick under the toon shader ---------------
            {
                let (hx, hy, hw, hh) = {
                    let h = if self.ducking { DUCK_H } else { HERO_H };
                    let top = if self.on_ground && self.ducking {
                        self.hero_y + HERO_H - DUCK_H
                    } else {
                        self.hero_y
                    };
                    (HERO_X - HERO_W / 2.0, top, HERO_W, h)
                };
                // Ninja salto: spin the whole hero about its centre (rotation
                // about world X = a forward flip) while the mid-air jump plays
                // out. rlgl matrix push/rotate/pop; the pop is on guard drop.
                let angle = if self.salto > 0.0 {
                    360.0 * (1.0 - self.salto / SALTO_DUR)
                } else {
                    0.0
                };
                let (pcx, pcy) = (wx(HERO_X), wy(hy + hh / 2.0));
                let mut m = c3.rl_push_matrix();
                m.rl_translatef(pcx, pcy, 0.0);
                m.rl_rotatef(angle, 1.0, 0.0, 0.0);
                m.rl_translatef(-pcx, -pcy, 0.0);
                let mut toonm = m.begin_shader_mode(&mut gfx.toon);
                let squash = 1.0 - 0.18 * (self.squash / 0.35);
                let vis_h = hh * squash;
                let body = (hx, hy + hh - vis_h, hw, vis_h);
                let (pos, size) = wrect(body, 0.0, 0.35);
                toonm.draw_cube_v(pos, size, SKY);

                // Face: small dark blocks on the front of the brick.
                let front = 0.35 / 2.0 + 0.03;
                let eye_y = wy(body.1 + vis_h * 0.30);
                for ex in [hx + hw * 0.30, hx + hw * 0.70] {
                    toonm.draw_cube(
                        Vector3::new(wx(ex), eye_y, front),
                        0.09,
                        0.09,
                        0.06,
                        CHARCOAL,
                    );
                }
                // Mouth grows when the kid's voice registers (or stirs with
                // any sound at all — babbling is progress too).
                let mouth = if self.last_held.is_some() {
                    0.22
                } else {
                    0.09 + 0.10 * self.last_level.clamp(0.0, 1.0)
                };
                toonm.draw_cube(
                    Vector3::new(wx(hx + hw * 0.5), wy(body.1 + vis_h * 0.58), front),
                    mouth,
                    mouth,
                    0.06,
                    CHARCOAL,
                );
                // Scissoring feet while grounded.
                if self.on_ground {
                    let phase = (self.t * 10.0).sin();
                    for (side, p) in [(-1.0f32, phase), (1.0, -phase)] {
                        toonm.draw_cube(
                            Vector3::new(
                                wx(hx + hw * 0.5) + side * 0.22,
                                wy(hy + hh) + 0.07 + p.max(0.0) * 0.07,
                                0.12,
                            ),
                            0.2,
                            0.14,
                            0.3,
                            SKY,
                        );
                    }
                }
            }

            // --- little suns over the ledges: the score sun, small ---------
            // Turning slowly (each out of step) and bobbing a touch, through
            // the sun's own glass.
            if let Some(sun) = &gfx.sun {
                if let Some(glass) = gfx.sun_glass.as_mut() {
                    glass.begin_frame(camera.position, self.t);
                }
                let k = LITTLE_SUN_PX / PPU / sun.size.y.max(f32::EPSILON);
                let c = sun.center;
                for s in &self.little_suns {
                    let spin = self.t * LITTLE_SUN_SPIN * std::f32::consts::TAU + s.phase;
                    let bob = (self.t * 1.6 + s.phase).sin() * 0.03;
                    let m = Matrix::translate(-c.x, -c.y, -c.z)
                        * Matrix::scale(k, k, k)
                        * Matrix::rotate_y(spin)
                        * Matrix::translate(wx(s.x), wy(s.y) + bob, 0.0);
                    match gfx.sun_glass.as_mut() {
                        Some(glass) => glass.draw(&mut c3, sun, m),
                        None => sun.draw_transformed(&mut c3, m),
                    }
                }
            }

            // --- star sparkles as rising blocks ----------------------------
            for s in &self.sparkles {
                let a = (255.0 * (1.0 - s.age)) as u8;
                let c = Color::new(BUTTER.r, BUTTER.g, BUTTER.b, a);
                let sz = 0.18 + 0.22 * s.age;
                c3.draw_cube(Vector3::new(wx(s.x), wy(s.y), 0.4), sz, sz, sz, c);
            }

            // --- score sun: last, and nearest the camera -------------------
            if let (Some(sun), Some((m, spin))) = (&gfx.sun, sun_place) {
                match gfx.sun_glass.as_mut() {
                    Some(glass) => {
                        glass.begin_frame(camera.position, self.t);
                        glass.draw(&mut c3, sun, m);
                    }
                    None => sun.draw_shaded(&mut c3, m, &gfx.sun_shader),
                }
                // The count on its face — only while the face is towards us
                // (from behind, the glyphs would read mirrored).
                if let (Some(plate), true) = (score_plate, spin.to_radians().cos() > 0.0) {
                    // Without depth testing: the glyph quads' see-through
                    // corners would otherwise hide the light face behind its
                    // own outline. Safe, because the sun is the frontmost
                    // thing on screen. (rlgl batches these quads, so flush
                    // before and after to fence the state change in.)
                    // SAFETY: plain rlgl state calls inside our 3D mode.
                    unsafe {
                        raylib::ffi::rlDrawRenderBatchActive();
                        raylib::ffi::rlDisableDepthTest();
                    }
                    {
                        let mut mm = c3.rl_push_matrix();
                        // Transposed on purpose: raylib-rs 6's rl_mult_matrixf
                        // casts the Matrix struct straight to the float[16]
                        // rlMultMatrixf reads column-major, but the struct's
                        // fields are laid out m0, m4, m8, m12, … (row order),
                        // so the matrix would arrive transposed and lose its
                        // translation. raylib's own callers use MatrixToFloat.
                        mm.rl_mult_matrixf(plate.transpose());
                        // A one-font-pixel outline all round, then the light face.
                        for (dx, dy) in [
                            (-2, 0),
                            (2, 0),
                            (0, -2),
                            (0, 2),
                            (-2, -2),
                            (2, -2),
                            (-2, 2),
                            (2, 2),
                        ] {
                            mm.draw_text(&shown, dx, dy, 20, SCORE_INK);
                        }
                        mm.draw_text(&shown, 0, 0, 20, SCORE_LIGHT);
                    }
                    // SAFETY: as above.
                    unsafe {
                        raylib::ffi::rlDrawRenderBatchActive();
                        raylib::ffi::rlEnableDepthTest();
                    }
                }
            }
        }

        // --- HUD: crisp 2D over the 3D scene -------------------------------

        // Flat letter signs above live obstacles, projected from world
        // space — only if the stone tablets didn't build.
        let flat_signs = if gfx.tablets.is_some() {
            &[][..]
        } else {
            &self.obstacles[..]
        };
        for o in flat_signs {
            if o.bounced {
                continue;
            }
            let (rx, ry, rw, _) = obstacle_rect(o);
            // Tall obstacles (wall/pillar) get their sign over the upper part,
            // not a fixed height above the (off-screen-high) top edge.
            let sign_y = match o.kind {
                Kind::Wall => wy(ry + WALL_H * 0.35),
                Kind::High => wy(ry + HIGH_H * 0.30),
                _ => wy(ry) + 1.15,
            };
            let anchor = Vector3::new(wx(rx + rw / 2.0), sign_y, 0.0);
            let sp = d.get_world_to_screen(anchor, camera);
            if sp.x < -100.0 || sp.x > w as f32 + 100.0 {
                continue;
            }
            let letter = match o.kind {
                Kind::Jump | Kind::High => VOWELS[self.jump_vowel].label(),
                Kind::Duck => VOWELS[self.duck_vowel].label(),
                Kind::Wall => VOWELS[self.shoot_vowel].label(),
            };
            let side = 88.0 * scale;
            let sign = Rectangle {
                x: sp.x - side / 2.0,
                y: sp.y - side / 2.0,
                width: side,
                height: side,
            };
            d.draw_rectangle_rounded(sign, 0.3, 6, Color::WHITE);
            d.draw_rectangle_rounded_lines(sign, 0.3, 6, CHARCOAL);
            let fs = sl(64.0);
            let tw = d.measure_text(letter, fs);
            d.draw_text(
                letter,
                sp.x as i32 - tw / 2,
                (sign.y + 12.0 * scale) as i32,
                fs,
                CHARCOAL,
            );
        }

        // Score: the sun (drawn in 3D above) or, if its model failed to load,
        // a plain gold disc with the count on it.
        if sun_place.is_none() {
            let r = icon_r * 0.8 * pop;
            d.draw_circle_v(icon_c, r, BUTTER);
            d.draw_circle_lines(icon_c.x as i32, icon_c.y as i32, r, CHARCOAL);
            let fs = sl(40.0);
            let tw = d.measure_text(&shown, fs);
            d.draw_text(
                &shown,
                icon_c.x as i32 - tw / 2,
                icon_c.y as i32 - fs / 2,
                fs,
                CHARCOAL,
            );
        }
        // Sparks thrown off by a fresh point: a ring of little pixel-art
        // "+" twinkles (the same shape as the ones in the sun's own drawing),
        // flying outward, shrinking and fading as the pop settles.
        if self.coin.pop > 0.0 {
            let p = 1.0 - self.coin.pop;
            let alpha = (255.0 * (1.0 - p)) as u8;
            for k in 0..8 {
                let long = k % 2 == 0;
                let a = k as f32 * std::f32::consts::TAU / 8.0 - 0.3 + p * 0.5;
                let reach = icon_r * (0.9 + (if long { 1.6 } else { 1.1 }) * p.sqrt());
                let arm = (if long { 26.0 } else { 18.0 }) * scale * (1.0 - 0.5 * p);
                let bar = (arm / 3.0).max(2.0);
                let base = if long { SUN_GOLD } else { SUN_EMBER };
                let c = Color::new(base.r, base.g, base.b, alpha);
                let (sx, sy) = (icon_c.x + reach * a.cos(), icon_c.y + reach * a.sin());
                d.draw_rectangle_v(
                    Vector2::new(sx - arm / 2.0, sy - bar / 2.0),
                    Vector2::new(arm, bar),
                    c,
                );
                d.draw_rectangle_v(
                    Vector2::new(sx - bar / 2.0, sy - arm / 2.0),
                    Vector2::new(bar, arm),
                    c,
                );
            }
        }

        // Vowel meter strip, bottom-centre.
        let labels = ["a", "e", "i", "o", "u", "y"];
        let strip_w = 6.0 * 64.0 * scale;
        let strip_x = w as f32 / 2.0 - strip_w / 2.0;
        for (i, label) in labels.iter().enumerate() {
            let bx = strip_x + i as f32 * 64.0 * scale;
            let level = self.last_scores[i].clamp(0.0, 1.0);
            let bh = (10.0 + 44.0 * level) * scale;
            let active = self.last_held == Some(i);
            let c = if active {
                BUTTER
            } else {
                Color::new(255, 255, 255, 170)
            };
            d.draw_rectangle_rounded(
                Rectangle {
                    x: bx,
                    y: h as f32 - 40.0 * scale - bh,
                    width: 40.0 * scale,
                    height: bh,
                },
                0.5,
                4,
                c,
            );
            d.draw_text(
                label,
                (bx + 14.0 * scale) as i32,
                h - sl(32.0),
                sl(22.0),
                CHARCOAL,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(held: Option<usize>, onset: Option<usize>) -> VoiceInput {
        VoiceInput {
            held,
            onset,
            scores: [0.0; 6],
            level: 0.0,
        }
    }

    #[test]
    fn onset_a_jumps_and_lands() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump, Kind::Duck]);
        r.update(&input(Some(0), Some(0)), 1.0 / 60.0);
        assert!(!r.on_ground);
        // Simulate ~1.5 s: must land again.
        for _ in 0..90 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        assert!(r.on_ground);
        assert_eq!(r.hero_y, GROUND_Y - HERO_H);
    }

    /// Every way the picture can be shaped that a family's screen might have.
    const ASPECTS: [f32; 4] = [4.0 / 3.0, 16.0 / 10.0, 16.0 / 9.0, 21.0 / 9.0];

    #[test]
    fn everything_comes_in_out_of_sight() {
        for aspect in ASPECTS {
            let mut r = Runner::new(0, 1, 2, vec![Kind::Jump]);
            r.eye.aspect = aspect;
            let view = r.view();
            let unseen = |b: Bounds| b.x.0 > view.right_edge(b.y, b.z);
            // Each obstacle kind, as the spawner places it.
            for kind in [Kind::Jump, Kind::Duck, Kind::Wall, Kind::High] {
                let mut o = Obstacle::new(0.0, kind);
                o.x = r.entry_px(obstacle_bounds(&o), o.x);
                assert!(unseen(obstacle_bounds(&o)), "{kind:?} at {aspect}");
            }
            // Each ledge pattern, little suns and all.
            for _ in 0..12 {
                r.ledges.clear();
                r.little_suns.clear();
                r.spawn_ledges();
                assert!(r.ledges.iter().all(|l| unseen(ledge_bounds(l))));
                assert!(r.little_suns.iter().all(|s| unseen(little_sun_bounds(s))));
            }
        }
    }

    #[test]
    fn things_stay_while_any_of_them_shows() {
        for aspect in ASPECTS {
            let mut r = Runner::new(0, 1, 2, vec![Kind::Jump]);
            r.eye.aspect = aspect;
            r.spawn_timer = 999.0;
            let view = r.view();
            // A wall whose right side just pokes into the picture on the
            // left: it stays. Once all of it is past the edge, it goes.
            let mut o = Obstacle::new(0.0, Kind::Wall);
            let b = obstacle_bounds(&o);
            let edge = view.left_edge(b.y, b.z);
            o.x = lx(edge - (b.x.1 - wx(o.x)) + 0.05);
            r.obstacles.push(o);
            r.update(&input(None, None), 1e-4);
            assert_eq!(r.obstacles.len(), 1, "dropped in sight at {aspect}");
            for _ in 0..120 {
                r.update(&input(None, None), 1.0 / 60.0);
            }
            assert!(r.obstacles.is_empty(), "never dropped at {aspect}");
        }
    }

    #[test]
    fn the_obstacles_fit_their_half_depth() {
        for g in [
            props::block(),
            props::bridge(),
            props::wall(),
            props::pillar(),
        ] {
            assert!(g.d as f32 * props::OBSTACLE_CELL / 2.0 <= OBSTACLE_HALF_DEPTH);
        }
    }

    #[test]
    fn the_camera_follows_a_jump_in_step_and_settles_on_landing() {
        let dt = 1.0 / 60.0;
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump]);
        r.spawn_timer = 999.0;
        r.update(&input(Some(0), Some(0)), dt);
        let (mut highest, mut prev, mut steepest) = (0.0f32, 0.0f32, 0.0f32);
        let (mut hero_top_at, mut cam_top_at, mut hero_top) = (0, 0, f32::MAX);
        let mut landed_at = None;
        for i in 0..90 {
            r.update(&input(None, None), dt);
            if r.hero_y < hero_top {
                (hero_top, hero_top_at) = (r.hero_y, i);
            }
            if r.cam_lift > highest {
                (highest, cam_top_at) = (r.cam_lift, i);
            }
            steepest = steepest.max((r.cam_lift - prev).abs());
            prev = r.cam_lift;
            if r.on_ground && landed_at.is_none() {
                landed_at = Some(i);
            }
        }
        // Visibly up, never more than its share of the jump, and in step
        // with it: the camera tops out within 0.15 s of the hero (the gentle
        // settling spring carries it on a moment past the top; the chasing
        // camera this replaced trailed by a quarter of a second and more).
        assert!(highest > 0.3, "rose {highest}");
        assert!(highest <= (2.4 - CAM_DEAD) * CAM_FOLLOW, "rose {highest}");
        assert!(
            cam_top_at <= hero_top_at + 9,
            "lags: {cam_top_at} vs {hero_top_at}"
        );
        // Smooth: never faster than a third of the hero's own take-off.
        assert!(steepest < JUMP_V / PPU / 3.0 * dt, "a jolt of {steepest}");
        // Back down unhurried after landing — still on its way a tenth of a
        // second in, most of the way there by one second, at rest within
        // three — and for good, without dipping below it.
        let landed = landed_at.expect("never landed");
        let mut r2 = Runner::new(0, 1, 2, vec![Kind::Jump]);
        r2.spawn_timer = 999.0;
        r2.update(&input(Some(0), Some(0)), dt);
        let mut lowest = f32::MAX;
        for i in 0..400 {
            r2.update(&input(None, None), dt);
            lowest = lowest.min(r2.cam_lift);
            let lift = r2.cam_lift;
            if i == landed + 6 {
                assert!(lift > 0.1, "already down to {lift}");
            }
            if i == landed + 60 {
                assert!(lift < 0.1, "still {lift} a second after landing");
            }
            if i == landed + 180 {
                assert!(lift < 0.01, "still {lift} three seconds after landing");
            }
        }
        assert!(r2.cam_lift.abs() < 0.001, "still {}", r2.cam_lift);
        assert!(lowest > -0.01, "dipped to {lowest}");

        // A little hop leaves it still.
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump]);
        r.hero_y -= CAM_DEAD * PPU * 0.9;
        r.on_ground = false;
        r.update(&input(None, None), dt);
        assert_eq!(r.cam_lift, 0.0);

        // A double jump takes it higher, but never past the cap.
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump]);
        r.spawn_timer = 999.0;
        r.update(&input(Some(0), Some(0)), dt);
        let mut double = 0.0f32;
        for i in 0..150 {
            let again = if i == 20 { Some(0) } else { None };
            r.update(&input(again, again), dt);
            double = double.max(r.cam_lift);
        }
        assert!(double > highest && double <= CAM_LIFT_MAX, "{double}");
    }

    #[test]
    fn saying_a_vowel_hops_its_own_tablets_only() {
        // Jump is "u", duck "o", shoot "y": the hop follows the vowel, not a
        // fixed a/e/i.
        let mut r = Runner::new(4, 3, 5, vec![Kind::Jump]);
        r.update(&input(Some(3), Some(3)), 1.0 / 60.0);
        assert_eq!(r.tablet_hop, [0.0, 1.0, 0.0]);
        // Up and back down in one quick arc, then at rest.
        let mut highest: f32 = 0.0;
        for _ in 0..30 {
            r.update(&input(Some(3), None), 1.0 / 60.0);
            highest = highest.max(tablet_lift(r.tablet_hop[1]));
        }
        assert!(highest > TABLET_HOP * 0.9, "hopped {highest}");
        assert_eq!(r.tablet_hop[1], 0.0);
        assert_eq!(tablet_lift(r.tablet_hop[1]), 0.0);
        // Every kind carries the vowel of the move that gets past it.
        let moves = [Kind::Jump, Kind::Duck, Kind::Wall, Kind::High].map(Kind::move_index);
        assert_eq!(moves, [0, 1, 2, 0]);
    }

    #[test]
    fn a_tablet_floats_behind_a_low_obstacle_and_hangs_on_a_tall_one() {
        let front = 0.3;
        for kind in [Kind::Jump, Kind::Duck] {
            let o = Obstacle::new(HERO_X, kind);
            let (_, top, _, _) = obstacle_rect(&o);
            let c = tablet_centre(&o, front);
            assert!(c.z < -0.5, "{kind:?}: in the hero's way");
            // Its bottom clears the obstacle's top.
            let half = props::TABLET_H as f32 * props::TABLET_CELL / 2.0;
            assert!(c.y - half > wy(top), "{kind:?}");
        }
        for kind in [Kind::Wall, Kind::High] {
            let o = Obstacle::new(HERO_X, kind);
            let (_, top, _, _) = obstacle_rect(&o);
            let c = tablet_centre(&o, front);
            let half = props::TABLET_D as f32 * props::TABLET_CELL / 2.0;
            assert_eq!(c.z - half, front, "{kind:?}: not on its front");
            // Within its height, its top under the obstacle's.
            let top_half = props::TABLET_H as f32 * props::TABLET_CELL / 2.0;
            assert!(c.y + top_half < wy(top) && c.y - top_half > 0.0, "{kind:?}");
        }
    }

    #[test]
    fn holding_e_ducks_only_while_held() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump, Kind::Duck]);
        r.update(&input(Some(1), None), 1.0 / 60.0);
        assert!(r.ducking);
        r.update(&input(None, None), 1.0 / 60.0);
        assert!(!r.ducking);
    }

    #[test]
    fn passed_obstacle_awards_star_and_bumped_one_does_not() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump, Kind::Duck]);
        // Plant a low block just right of the hero, ducked out of spawn flow.
        r.spawn_timer = 999.0;
        r.obstacles.push(Obstacle::new(HERO_X + 200.0, Kind::Jump));
        // Jump when the block gets close, like a real player would.
        let mut jumped = false;
        for _ in 0..240 {
            let close = !jumped && r.obstacles.first().is_some_and(|o| o.x < HERO_X + 110.0);
            let i = if close {
                jumped = true;
                input(Some(0), Some(0))
            } else {
                input(None, None)
            };
            r.update(&i, 1.0 / 60.0);
        }
        assert!(jumped);
        assert_eq!(r.stars, 1);

        // Same again but without jumping: collision bounces it, no star.
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump, Kind::Duck]);
        r.spawn_timer = 999.0;
        r.obstacles.push(Obstacle::new(HERO_X + 200.0, Kind::Jump));
        for _ in 0..240 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        assert_eq!(r.stars, 0);
        assert!(r.squash > 0.0 || r.obstacles.is_empty() || r.obstacles[0].bounced);
    }

    #[test]
    fn shooting_a_wall_destroys_it_for_a_star() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Wall]);
        r.spawn_timer = 999.0;
        r.obstacles.push(Obstacle::new(HERO_X + 320.0, Kind::Wall));
        // Fire on the shoot vowel's onset.
        r.update(&input(None, Some(2)), 1.0 / 60.0);
        assert_eq!(r.bullets.len(), 1);
        // Let the star fly into the wall.
        for _ in 0..120 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        assert_eq!(r.stars, 1);
        assert!(r.obstacles.is_empty() || r.obstacles[0].bounced);
    }

    #[test]
    fn an_unshot_wall_bumps_without_a_star() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Wall]);
        r.spawn_timer = 999.0;
        r.obstacles.push(Obstacle::new(HERO_X + 200.0, Kind::Wall));
        // Never shoot: the wall reaches the hero and bounces, no star.
        for _ in 0..240 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        assert_eq!(r.stars, 0);
    }

    /// Run `r` for `frames` frames, feeding `pick(frame, runner)` as input;
    /// whether the hero bumped into anything on the way.
    fn play(r: &mut Runner, frames: u32, mut pick: impl FnMut(u32, &Runner) -> VoiceInput) -> bool {
        let mut bumped = false;
        for f in 0..frames {
            let i = pick(f, r);
            r.update(&i, 1.0 / 60.0);
            bumped |= r.squash > 0.0;
        }
        bumped
    }

    #[test]
    fn only_the_asked_move_earns_a_star() {
        use Kind::*;
        // (kind, leapt, ducked) → star?
        assert!(Jump.earns_a_star(true, false));
        assert!(High.earns_a_star(true, false));
        assert!(Duck.earns_a_star(false, true));
        assert!(!Duck.earns_a_star(true, false), "leaping over a bar");
        assert!(!Wall.earns_a_star(true, false), "leaping over a wall");
        assert!(!Wall.earns_a_star(false, false));
    }

    #[test]
    fn ducking_under_a_bar_earns_a_star_leaping_over_it_does_not() {
        // Hold the duck vowel all the way: under it, untouched, one star.
        let mut r = Runner::new(0, 1, 2, vec![Kind::Duck]);
        r.spawn_timer = 999.0;
        r.obstacles.push(Obstacle::new(HERO_X + 200.0, Kind::Duck));
        let bumped = play(&mut r, 240, |_, _| input(Some(1), None));
        assert!(!bumped);
        assert_eq!(r.stars, 1);

        // Jump as it comes: over it, untouched, but no star.
        let mut r = Runner::new(0, 1, 2, vec![Kind::Duck]);
        r.spawn_timer = 999.0;
        r.obstacles.push(Obstacle::new(HERO_X + 200.0, Kind::Duck));
        let mut jumped = false;
        let bumped = play(&mut r, 240, |_, r| {
            if !jumped && r.obstacles.first().is_some_and(|o| o.x < HERO_X + 125.0) {
                jumped = true;
                input(Some(0), Some(0))
            } else {
                input(None, None)
            }
        });
        assert!(jumped);
        assert!(!bumped, "the jump should clear the bar");
        assert_eq!(r.stars, 0);
    }

    #[test]
    fn leaping_over_a_wall_earns_no_star() {
        // The double jump climbs over a wall, but a wall is for shooting.
        let mut r = Runner::new(0, 1, 2, vec![Kind::Wall]);
        r.spawn_timer = 999.0;
        r.obstacles.push(Obstacle::new(HERO_X + 260.0, Kind::Wall));
        let mut first = None;
        let bumped = play(&mut r, 240, |f, r| {
            let close = r.obstacles.first().is_some_and(|o| o.x < HERO_X + 185.0);
            match first {
                None if close => {
                    first = Some(f);
                    input(Some(0), Some(0))
                }
                Some(f0) if f == f0 + 22 => input(Some(0), Some(0)), // the double jump
                _ => input(None, None),
            }
        });
        assert!(first.is_some());
        assert!(!bumped, "the double jump should clear the wall");
        assert_eq!(r.stars, 0);
    }

    /// A runner with no spawns and one ledge (and its little suns).
    fn on_a_ledge_course(x: f32, size: LedgeSize, height: f32) -> Runner {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump]);
        r.spawn_timer = 999.0;
        r.add_ledge(x, size, height);
        r
    }

    fn feet(r: &Runner) -> f32 {
        r.hero_y + HERO_H
    }

    #[test]
    fn ledge_heights_fit_the_hero_and_the_jumps() {
        let single_apex = JUMP_V * JUMP_V / (2.0 * GRAVITY);
        let thick = props::LEDGE_LAYERS as f32 * props::GROUND_CELL * PPU;
        assert!(
            LEDGE_LOW - thick > HERO_H,
            "the hero runs under a low ledge"
        );
        assert!(
            single_apex > LEDGE_LOW + 40.0,
            "a single jump lands on a low ledge"
        );
        assert!(
            single_apex < LEDGE_HIGH,
            "a high ledge needs the double jump"
        );
        assert!(
            LEDGE_LOW + single_apex > LEDGE_HIGH + 40.0,
            "a low ledge is a step up to a high one"
        );
    }

    #[test]
    fn a_jump_lands_on_a_low_ledge_collects_its_suns_and_drops_off_the_end() {
        let mut r = on_a_ledge_course(HERO_X + 150.0, LedgeSize::Long, LEDGE_LOW);
        let top = GROUND_Y - LEDGE_LOW;
        let mut stood = false;
        let bumped = play(&mut r, 240, |f, r| {
            stood |= r.on_ground && (feet(r) - top).abs() < 0.01;
            if f == 0 {
                input(Some(0), Some(0))
            } else {
                input(None, None)
            }
        });
        assert!(!bumped);
        assert!(stood, "never stood on the ledge");
        // Ridden off the end and back on the ground.
        assert!(r.on_ground && feet(&r) == GROUND_Y);
        // Its four little suns were collected, and didn't speed the game up.
        assert_eq!(r.stars, 4);
        assert_eq!(r.speed, 260.0);
        assert!(r.little_suns.is_empty());
    }

    #[test]
    fn a_jump_goes_up_through_a_ledge_and_lands_on_it() {
        // The ledge is right overhead: jumping, the hero rises through it.
        let mut r = on_a_ledge_course(HERO_X - 100.0, LedgeSize::Long, LEDGE_LOW);
        let top = GROUND_Y - LEDGE_LOW;
        r.update(&input(Some(0), Some(0)), 1.0 / 60.0);
        let mut rose_through = false;
        let mut landed = false;
        for _ in 0..60 {
            r.update(&input(None, None), 1.0 / 60.0);
            rose_through |= r.vy < 0.0 && feet(&r) < top;
            landed |= r.on_ground && (feet(&r) - top).abs() < 0.01;
        }
        assert!(rose_through && landed);
    }

    #[test]
    fn a_high_ledge_needs_the_double_jump() {
        let top = GROUND_Y - LEDGE_HIGH;
        let stood_on = |double_at: Option<u32>| {
            let mut r = on_a_ledge_course(HERO_X + 100.0, LedgeSize::Long, LEDGE_HIGH);
            let mut stood = false;
            for f in 0..120 {
                let jump = f == 0 || double_at == Some(f);
                let i = if jump {
                    input(Some(0), Some(0))
                } else {
                    input(None, None)
                };
                r.update(&i, 1.0 / 60.0);
                stood |= r.on_ground && (feet(&r) - top).abs() < 0.01;
            }
            stood
        };
        assert!(!stood_on(None), "a single jump can't reach it");
        assert!(stood_on(Some(22)), "a double jump can");
    }

    #[test]
    fn ducking_on_a_ledge_stays_on_it() {
        let mut r = on_a_ledge_course(HERO_X - 100.0, LedgeSize::Long, LEDGE_LOW);
        let top = GROUND_Y - LEDGE_LOW;
        r.hero_y = top - HERO_H; // standing on it
        r.update(&input(Some(1), None), 1.0 / 60.0);
        assert!(r.ducking && r.on_ground);
        let (_, y, _, h) = r.hero_rect();
        assert!((y + h - top).abs() < 0.01, "ducked down to the ground");
    }

    #[test]
    fn running_under_a_low_ledge_touches_nothing() {
        let mut r = on_a_ledge_course(HERO_X + 100.0, LedgeSize::Long, LEDGE_LOW);
        let mut always_down = true;
        let bumped = play(&mut r, 180, |_, r| {
            always_down &= r.on_ground && feet(r) == GROUND_Y;
            input(None, None)
        });
        assert!(!bumped && always_down);
        assert_eq!(r.stars, 0, "the suns float out of reach overhead");
    }

    #[test]
    fn the_demo_pilot_clears_everything_without_a_bump() {
        // Four minutes with every obstacle kind, the game speeding up as it
        // goes: not one bump, a star for every obstacle, ledges landed on and
        // their suns collected. (Which also proves every obstacle can be
        // cleared at every speed the game reaches.)
        let mut r = Runner::new(
            0,
            1,
            2,
            vec![Kind::Jump, Kind::Duck, Kind::Wall, Kind::High],
        );
        let (mut bumps, mut on_ledges, mut squash) = (0, 0, 0.0);
        for _ in 0..60 * 240 {
            let i = r.pilot();
            r.update(&i, 1.0 / 60.0);
            // A bump is the frame the hero's squash jumps up.
            bumps += usize::from(r.squash > squash);
            squash = r.squash;
            on_ledges += usize::from(r.on_ground && r.hero_y + HERO_H < GROUND_Y - 1.0);
        }
        assert_eq!(bumps, 0, "the pilot bumped into something");
        assert!(r.stars > 40, "only {} points in four minutes", r.stars);
        assert!(on_ledges > 0, "never stood on a ledge");
        assert!(r.speed > 400.0, "the game never sped up ({})", r.speed);
    }

    #[test]
    fn every_kind_comes_round_once_a_round() {
        let all = vec![Kind::Jump, Kind::Duck, Kind::Wall, Kind::High];
        let mut r = Runner::new(0, 1, 2, all.clone());
        let dealt: Vec<Kind> = (0..40).map(|_| r.next_kind()).collect();
        for round in dealt.chunks(4) {
            for k in &all {
                assert!(round.contains(k), "{k:?} missing from a round: {round:?}");
            }
        }
        for pair in dealt.windows(2) {
            assert_ne!(pair[0], pair[1], "the same kind twice running");
        }
    }

    #[test]
    fn a_wall_comes_within_the_first_round() {
        // The regression: with a fixed seed and a plain random pick, the
        // first wall came 18th — a minute or more into every game.
        let mut r = Runner::new(
            0,
            1,
            2,
            vec![Kind::Jump, Kind::Duck, Kind::Wall, Kind::High],
        );
        let mut kinds = Vec::new();
        while kinds.len() < 4 {
            let before = r.obstacles.len();
            r.update(&input(None, None), 1.0 / 60.0);
            if r.obstacles.len() > before {
                kinds.push(r.obstacles.last().unwrap().kind);
            }
        }
        assert!(kinds.contains(&Kind::Wall), "{kinds:?}");
    }

    #[test]
    fn a_single_kind_still_deals() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Wall]);
        assert!((0..5).all(|_| r.next_kind() == Kind::Wall));
    }

    #[test]
    fn ledges_and_obstacles_never_share_a_stretch() {
        // A long run with every obstacle kind: whenever a ledge is on the
        // course, no obstacle overlaps it along x.
        let mut r = Runner::new(
            0,
            1,
            2,
            vec![Kind::Jump, Kind::Duck, Kind::Wall, Kind::High],
        );
        let mut ledges_seen = 0;
        for _ in 0..60 * 120 {
            r.update(&input(None, None), 1.0 / 60.0);
            ledges_seen = ledges_seen.max(r.ledges.len());
            for l in &r.ledges {
                for o in r.obstacles.iter().filter(|o| !o.bounced) {
                    let (ox, _, ow, _) = obstacle_rect(o);
                    assert!(
                        ox + ow < l.x || ox > l.x + l.w(),
                        "a {:?} under a ledge",
                        o.kind
                    );
                }
            }
        }
        assert!(ledges_seen > 0, "no ledges in two minutes");
    }

    #[test]
    fn ninja_double_jump_climbs_higher_than_a_single() {
        // Peak hero-bottom height (px above ground). `second_at` = frame in the
        // air to fire the second jump (None = single jump).
        fn peak(second_at: Option<u32>) -> f32 {
            let mut r = Runner::new(0, 1, 2, vec![Kind::Jump]);
            r.spawn_timer = 999.0;
            r.update(&input(None, Some(0)), 1.0 / 60.0); // leave the ground
            let mut highest = 0.0f32;
            for f in 0..120 {
                let i = if second_at == Some(f) {
                    input(None, Some(0))
                } else {
                    input(None, None)
                };
                r.update(&i, 1.0 / 60.0);
                highest = highest.max(GROUND_Y - (r.hero_y + HERO_H));
            }
            highest
        }
        let single = peak(None);
        let double_early = peak(Some(0)); // worst timing: pressed immediately
        let double_apex = peak(Some(22)); // good timing: near the first apex
        // A single jump can never clear a High pillar…
        assert!(single < 0.8 * HIGH_H, "single {single} vs {}", 0.8 * HIGH_H);
        // …but *any* double-jump does, even the worst-timed one.
        assert!(
            double_early > 0.8 * HIGH_H,
            "worst-timed double {double_early} must still clear {}",
            0.8 * HIGH_H
        );
        assert!(double_apex > double_early); // better timing → higher
    }

    #[test]
    fn only_one_air_jump_per_airtime() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump]);
        r.update(&input(None, Some(0)), 1.0 / 60.0); // ground jump
        r.update(&input(None, Some(0)), 1.0 / 60.0); // air jump (allowed)
        assert!(r.air_jumped);
        let vy = r.vy;
        // A third jump-vowel mid-air must be ignored (no re-launch).
        r.update(&input(None, Some(0)), 1.0 / 60.0);
        assert!(r.vy > vy, "vy should decay under gravity, not re-launch");
    }

    #[test]
    fn a_point_spins_the_sun_one_full_turn() {
        let mut c = SunCoin::default();
        c.earn();
        let mut peak = 0.0f32;
        for _ in 0..180 {
            c.step(1.0 / 60.0);
            peak = peak.max(c.angle);
        }
        // Comes to rest facing the kid again, one whole turn later…
        assert!((c.angle - 360.0).abs() < 1.0, "angle {}", c.angle);
        assert!(c.vel.abs() < 5.0);
        // …with a springy swing past it, but not a second lap.
        assert!(peak > 365.0 && peak < 420.0, "peak {peak}");
        assert_eq!(c.pop, 0.0);
        assert_eq!(c.scale(), 1.0);
    }

    #[test]
    fn quick_points_add_turns_instead_of_restarting() {
        let mut c = SunCoin::default();
        c.earn();
        for _ in 0..6 {
            c.step(1.0 / 60.0); // a tenth of a second into the whirl…
        }
        c.earn(); // …another point
        for _ in 0..240 {
            c.step(1.0 / 60.0);
        }
        assert!((c.angle - 720.0).abs() < 1.0, "angle {}", c.angle);
    }

    #[test]
    fn the_face_shows_the_new_count_only_once_turned_away() {
        let mut c = SunCoin::default();
        assert_eq!(c.shown(0), 0);
        c.earn(); // stars is now 1
        assert_eq!(c.shown(1), 0, "still facing us: the old number");
        let mut swapped_at = None;
        for _ in 0..120 {
            c.step(1.0 / 60.0);
            if swapped_at.is_none() && c.shown(1) == 1 {
                swapped_at = Some(c.angle);
            }
        }
        // It swapped while the face was turned away (90°..270° into the turn)…
        let at = swapped_at.expect("never showed the new count");
        assert!((90.0..=270.0).contains(&at), "swapped at {at}°");
        // …and stays swapped through the springy settle.
        assert_eq!(c.shown(1), 1);
    }

    #[test]
    fn quick_points_are_revealed_one_turn_each() {
        let mut c = SunCoin::default();
        c.earn();
        c.earn(); // two points at once: stars = 2
        assert_eq!(c.shown(2), 0);
        let mut seen = vec![0];
        for _ in 0..240 {
            c.step(1.0 / 60.0);
            let s = c.shown(2);
            if *seen.last().unwrap() != s {
                seen.push(s);
            }
        }
        assert_eq!(seen, vec![0, 1, 2], "one reveal per turn, in order");
    }

    #[test]
    fn sun_pop_swells_then_settles() {
        let mut c = SunCoin::default();
        c.earn();
        c.step(0.1);
        assert!(c.scale() > 1.15, "swell {}", c.scale());
        c.step(POP_DUR);
        assert_eq!(c.scale(), 1.0);
    }

    #[test]
    fn long_runs_wrap_whole_turns() {
        let mut c = SunCoin::default();
        for _ in 0..12 {
            c.earn();
            for _ in 0..120 {
                c.step(1.0 / 60.0);
            }
        }
        // 12 turns = 4320°; the whole 10-turn block is dropped.
        assert!((c.angle - 720.0).abs() < 1.0, "angle {}", c.angle);
    }

    #[test]
    fn earning_a_star_kicks_the_sun() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Wall]);
        r.spawn_timer = 999.0;
        r.obstacles.push(Obstacle::new(HERO_X + 320.0, Kind::Wall));
        r.update(&input(None, Some(2)), 1.0 / 60.0);
        for _ in 0..120 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        assert_eq!(r.stars, 1);
        assert_eq!(r.coin.target, 360.0);
    }

    #[test]
    fn each_plane_ends_up_hazed_as_set() {
        // The passes drawn in front of each plane multiply what shows through.
        let (c, m, p, j) = (
            haze_step(CLOUD_HAZE, MOUNTAIN_HAZE),
            haze_step(MOUNTAIN_HAZE, PALM_HAZE),
            haze_step(PALM_HAZE, JUNGLE_HAZE),
            JUNGLE_HAZE,
        );
        let through = |passes: &[f32]| 1.0 - passes.iter().map(|a| 1.0 - a).product::<f32>();
        assert!((through(&[c, m, p, j]) - CLOUD_HAZE).abs() < 1e-5);
        assert!((through(&[m, p, j]) - MOUNTAIN_HAZE).abs() < 1e-5);
        assert!((through(&[p, j]) - PALM_HAZE).abs() < 1e-5);
        assert!((through(&[j]) - JUNGLE_HAZE).abs() < 1e-5);
        // The farther, the hazier: every pass is a real (non-negative) one.
        assert!([c, m, p, j].iter().all(|&a| a >= 0.0));
    }

    #[test]
    fn water_style_reads_both_names() {
        assert_eq!(WaterStyle::parse("toon"), Some(WaterStyle::Toon));
        assert_eq!(WaterStyle::parse(" Bricks\n"), Some(WaterStyle::Bricks));
        assert_eq!(WaterStyle::parse("brick"), Some(WaterStyle::Bricks));
        assert_eq!(WaterStyle::parse("lava"), None);
        // The default stays the bricks: the toon water is the one kept aside.
        assert_eq!(WATER_STYLE, WaterStyle::Bricks);
    }

    #[test]
    fn voice_glow_rises_fast_and_settles_slowly() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump]);
        r.spawn_timer = 999.0;
        let loud = VoiceInput {
            held: None,
            onset: None,
            scores: [0.0; 6],
            level: 1.0,
        };
        for _ in 0..15 {
            r.update(&loud, 1.0 / 60.0); // a quarter second of sound
        }
        assert!(r.voice_glow > 0.8, "rise {}", r.voice_glow);
        for _ in 0..15 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        // A quarter second of quiet later it is still clearly up: it fades
        // like a ripple, not like a switch.
        assert!(r.voice_glow > 0.5, "decay {}", r.voice_glow);
        for _ in 0..300 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        assert!(r.voice_glow < 0.01);
    }

    #[test]
    fn distance_accumulates_for_parallax() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump, Kind::Duck]);
        for _ in 0..60 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        // One second at starting speed ~260 px/s.
        assert!((r.dist - 260.0).abs() < 5.0);
    }
}
