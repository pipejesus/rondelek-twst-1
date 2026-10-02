//! "Vowel Runner" — the first voice game, now in 2.5D. The hero runs on their
//! own; the kid only speaks: say one vowel to jump over low blocks, hold
//! another to duck under high bars. Misses never kill — the obstacle just
//! bounces away; every cleared obstacle earns a star.
//!
//! Rendering: a fixed perspective camera slightly above and beside the action,
//! everything built from chunky 3D bricks ("pixels became big and 3-D").
//! Parallax layers scroll by hand-tuned factors of the travelled distance:
//! clouds 0.10, mountains 0.25 (fog shader), bushes 0.55, ground 1.0. Clouds
//! and bushes are hand-drawn flat-draw props (GLB, see `models.rs`); the clouds
//! go through flat-draw's own Lam::pula glass shader (`lampula.rs`) and float,
//! bob and breathe. Mountains, ground and obstacles are still procedural
//! bricks. In front of the meadow's bank lies playful water that scrolls with
//! the ground and dances to the child's voice: little glass bricks in the
//! clouds' Lam::pula (`brick_water.rs`), or the first, smooth toon water
//! (`water.rs`, kept: `RONDELEK_WATER_STYLE=toon`). The hero placeholder brick
//! is drawn under a comic-style toon shader — the same slot the dragon GLB model
//! will use later. HUD (letter signs, meter, score) stays crisp 2D, projected
//! over the scene — except the score, a hand-drawn pixel sun (GLB) in
//! Lam::pula glass that floats just in front of the camera with the count
//! printed on its face like a coin's value; it whirls round once per point and
//! comes back showing the new number (see [`SunCoin`]).
//!
//! Gameplay math is untouched from the 2D version: `update()` works in the
//! same 1280x720 logical space (100 logical px = 1 world unit at draw time),
//! so physics, collisions and their tests are identical.

use super::brick_water::BrickWater;
use super::lampula::{Lampula, LampulaParams};
use super::models::FlatModel;
use super::water::{self, Water};
use super::{VoiceGame, VoiceInput};
use raylib::prelude::*;
use raylib::rlgl::RaylibRlgl; // matrix stack for the salto flip
use rondelek_core::audio::vowel::VOWELS;

use super::{BUTTER, CHARCOAL, LILAC, MINT_DARK, PEACH, ROSE, SKY};

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

// Logical px per world unit.
const PPU: f32 = 100.0;

// 2.5D palette: a clear, sunny-day world — saturated sky blue and meadow
// green, so the kid's own drawings (white clouds, the gold score sun) and the
// obstacles stand out against it instead of melting into a pastel wash.
const SKY_TOP: Color = Color::new(84, 176, 240, 255);
const SKY_LOW: Color = Color::new(184, 228, 255, 255);
const GRASS: Color = Color::new(112, 202, 92, 255);
const GRASS_DARK: Color = Color::new(78, 166, 68, 255);
const DIRT: Color = Color::new(184, 122, 78, 255);
// Far hills: meadow green, pushed back by the fog toward the horizon blue.
const MOUNT_A: Color = Color::new(96, 178, 120, 255);
const MOUNT_B: Color = Color::new(74, 154, 104, 255);
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
// Atmospheric haze over the cloud layer: how far the clouds are pulled toward
// the sky behind them (0 = not at all, 1 = gone). See the haze pass in draw().
const CLOUD_HAZE: f32 = 0.45;
const BUSH_H: f32 = 0.85;
const BUSH_H_VARY: f32 = 0.5;
const BUSH_PERIOD: f32 = 2.8;
// Wall: solid stone, deliberately un-pastel so it reads as "impassable".
const WALL_A: Color = Color::new(154, 136, 126, 255);
const WALL_B: Color = Color::new(122, 106, 98, 255);
const WALL_MORTAR: Color = Color::new(92, 80, 74, 255);

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

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Jump, // low block: jump over
    Duck, // high bar: duck under
    Wall, // tall wall: shoot a star to destroy
    High, // tall pillar: needs the ninja double-jump
}

struct Obstacle {
    x: f32,
    kind: Kind,
    /// Set on collision (or a bullet, for walls): flies off instead of the
    /// hero failing.
    bounced: bool,
    counted: bool,
    fly_y: f32,
    fly_vy: f32,
    rot: f32,
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
    camera: Camera3D,
    toon: Shader,
    fog: Shader,
    /// The kid's own drawings (flat-draw GLB). `None` only if loading failed —
    /// that layer then stays empty rather than the game dying.
    cloud: Option<FlatModel>,
    bush: Option<FlatModel>,
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
    speed: f32,
    spawn_timer: f32,
    /// Hero squash feedback after a bump (seconds remaining).
    squash: f32,
    t: f32,
    /// Distance travelled (logical px) — drives all parallax offsets.
    dist: f32,
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
            speed: 260.0,
            spawn_timer: 1.2,
            squash: 0.0,
            t: 0.0,
            dist: 0.0,
            rng: 0x2545_F491_4F6C_DD1D,
            last_scores: [0.0; 6],
            last_held: None,
            last_level: 0.0,
            voice_glow: 0.0,
            jump_vowel,
            duck_vowel,
            shoot_vowel,
            kinds,
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

    fn hero_rect(&self) -> (f32, f32, f32, f32) {
        let h = if self.ducking { DUCK_H } else { HERO_H };
        let top = if self.on_ground && self.ducking {
            GROUND_Y - DUCK_H
        } else {
            self.hero_y
        };
        (HERO_X - HERO_W / 2.0, top, HERO_W, h)
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

/// Visible half-width (world units) of a layer at depth `z` for our camera —
/// used to know which procedural tiles are on screen.
fn half_span(z: f32) -> f32 {
    // camera z 12.5, fovy 45 deg, 16:9 -> half-width = (12.5 - z) * tan(22.5) * aspect
    (12.5 - z) * 0.4142 * (16.0 / 9.0) + 2.0
}

impl VoiceGame for Runner {
    fn init(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread) {
        let toon = rl.load_shader_from_memory(
            thread,
            Some(include_str!("../../assets/shaders/base.vs")),
            Some(include_str!("../../assets/shaders/toon.fs")),
        );
        let mut fog = rl.load_shader_from_memory(
            thread,
            None,
            Some(include_str!("../../assets/shaders/fog.fs")),
        );
        let sun_shader = rl.load_shader_from_memory(
            thread,
            Some(include_str!("../../assets/shaders/sun.vs")),
            Some(include_str!("../../assets/shaders/sun.fs")),
        );
        let loc_color = fog.get_shader_location("fogColor");
        let loc_amount = fog.get_shader_location("fogAmount");
        // Fog toward the horizon blue (SKY_LOW); constant per layer, so set once.
        let horizon = [SKY_LOW.r, SKY_LOW.g, SKY_LOW.b].map(|c| c as f32 / 255.0);
        fog.set_shader_value(loc_color, [horizon[0], horizon[1], horizon[2], 1.0f32]);
        fog.set_shader_value(loc_amount, 0.3f32);

        // Scenery drawn in flat-draw and exported as GLB; each drawing's parts
        // are merged into one mesh, so every prop on screen is one draw call.
        let cloud = load_prop(
            rl,
            thread,
            "cloud",
            include_bytes!("../../assets/models/cloud.glb"),
        );
        let bush = load_prop(
            rl,
            thread,
            "bush",
            include_bytes!("../../assets/models/bush.glb"),
        );
        let sun = load_prop(
            rl,
            thread,
            "sun",
            include_bytes!("../../assets/models/sun.glb"),
        );

        self.gfx = Some(Gfx {
            camera: Camera3D::perspective(
                Vector3::new(0.9, 2.2, 12.5),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::Y,
                45.0,
            ),
            toon,
            fog,
            cloud,
            bush,
            sun,
            sun_shader,
            lampula: Lampula::load(rl, thread, cloud_glass()),
            sun_glass: Lampula::load(rl, thread, sun_glass()),
            water: Sea::load(rl, thread, WaterStyle::from_env()),
        });
    }

    fn update(&mut self, input: &VoiceInput, dt: f32) {
        self.t += dt;
        self.squash = (self.squash - dt).max(0.0);
        self.salto = (self.salto - dt).max(0.0);
        self.coin.step(dt);
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
        if !self.on_ground {
            self.vy += GRAVITY * dt;
            self.hero_y += self.vy * dt;
            if self.hero_y >= GROUND_Y - HERO_H {
                self.hero_y = GROUND_Y - HERO_H;
                self.vy = 0.0;
                self.on_ground = true;
                self.air_jumped = false;
                self.salto = 0.0; // land upright
            }
        }

        // --- obstacles ---
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            let pick = (self.rand() * self.kinds.len() as f32) as usize;
            let kind = self.kinds[pick.min(self.kinds.len() - 1)];
            self.obstacles.push(Obstacle {
                x: LW + 120.0,
                kind,
                bounced: false,
                counted: false,
                fly_y: 0.0,
                fly_vy: 0.0,
                rot: 0.0,
            });
            // Faster game = slightly denser spawns, always with breathing room.
            self.spawn_timer = 2.6 - (self.speed - 260.0) / 240.0 * 0.8 + self.rand() * 0.6;
        }

        let hero = self.hero_rect();
        let mut starred = false;
        for o in &mut self.obstacles {
            o.x -= self.speed * dt;
            if o.bounced {
                o.fly_vy += GRAVITY * 0.5 * dt;
                o.fly_y += o.fly_vy * dt;
                o.rot += 360.0 * dt;
                continue;
            }
            if overlaps(hero, obstacle_rect(o), 0.2) {
                // No punishment: the obstacle is the one that gets launched.
                o.bounced = true;
                o.fly_vy = -520.0;
                self.squash = 0.35;
            } else if o.x < HERO_X - 90.0 && !o.counted {
                o.counted = true;
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
        self.obstacles.retain(|o| o.x > -200.0 && o.fly_y < 720.0);

        // --- bullets: fly right, spin, smash the first wall they touch ---
        for bi in 0..self.bullets.len() {
            self.bullets[bi].x += BULLET_VX * dt;
            self.bullets[bi].rot += 720.0 * dt;
            let (bx, by) = (self.bullets[bi].x, self.bullets[bi].y);
            if bx > LW + 200.0 {
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
        let Some(gfx) = &mut self.gfx else {
            return; // headless (unit tests) — nothing to draw with
        };
        let camera = gfx.camera;

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
                let span = half_span(z);
                let k0 = ((off - span) / CLOUD_PERIOD).floor() as i64;
                let k1 = ((off + span) / CLOUD_PERIOD).ceil() as i64;
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

        // --- haze: the clouds are far away, so they take on the sky -------
        // The sky's own gradient once more, see-through. Over bare sky it is
        // the same colour, so it vanishes; over a cloud it pulls the colours
        // toward the sky behind — how a painter pushes things into the
        // distance. The clouds stay a soft backdrop rather than something to
        // look at, and the Lam::pula shader itself is left as it is.
        let haze = |c: Color| Color::new(c.r, c.g, c.b, (255.0 * CLOUD_HAZE) as u8);
        d.draw_rectangle_gradient_v(0, 0, w, h, haze(SKY_TOP), haze(SKY_LOW));

        {
            let mut c3 = d.begin_mode3D(camera);

            // --- mountains (z -9, factor 0.25): stepped pyramids in fog ----
            {
                let mut fogm = c3.begin_shader_mode(&mut gfx.fog);
                let z = -9.0;
                let period = 5.2;
                let off = dist_u * 0.25;
                let span = half_span(z);
                let k0 = ((off - span) / period).floor() as i64;
                let k1 = ((off + span) / period).ceil() as i64;
                for k in k0..=k1 {
                    let cx = k as f32 * period - off + (hash01(k, 21) - 0.5) * 2.0;
                    let tiers = 2 + (hash01(k, 22) * 3.0) as i32;
                    let base_w = 3.0 + hash01(k, 23) * 2.2;
                    let tier_h = 0.85;
                    let col = if hash01(k, 24) > 0.5 {
                        MOUNT_A
                    } else {
                        MOUNT_B
                    };
                    for i in 0..tiers {
                        let tw = base_w * (1.0 - i as f32 / tiers as f32).max(0.25);
                        fogm.draw_cube(
                            Vector3::new(cx, tier_h * (i as f32 + 0.5), z),
                            tw,
                            tier_h,
                            1.6,
                            col,
                        );
                    }
                }
            }

            // --- bushes (z -4, factor 0.55): the kid's own drawing ----------
            // Same deal as the clouds: one instance per lane, one draw call
            // each, standing on the ground plane (y = 0).
            if let Some(bush) = &gfx.bush {
                let z = -4.0;
                let off = dist_u * 0.55;
                let span = half_span(z);
                let k0 = ((off - span) / BUSH_PERIOD).floor() as i64;
                let k1 = ((off + span) / BUSH_PERIOD).ceil() as i64;
                for k in k0..=k1 {
                    if hash01(k, 31) < 0.25 {
                        continue; // gaps between bushes
                    }
                    let cx = k as f32 * BUSH_PERIOD - off + (hash01(k, 32) - 0.5) * 1.2;
                    let scale = bush.scale_for_height(BUSH_H + hash01(k, 33) * BUSH_H_VARY);
                    bush.draw(&mut c3, Vector3::new(cx, 0.0, z), scale);
                }
            }

            // --- ground (factor 1.0): grass caps + dirt cross-section ------
            {
                let period = 1.0;
                let off = dist_u;
                let span = half_span(1.5);
                let k0 = ((off - span) / period).floor() as i64;
                let k1 = ((off + span) / period).ceil() as i64;
                for k in k0..=k1 {
                    let cx = k as f32 * period - off;
                    let g = 1.0 + (hash01(k, 41) - 0.5) * 0.12;
                    let grass = Color::new(
                        (GRASS.r as f32 * g) as u8,
                        (GRASS.g as f32 * g) as u8,
                        (GRASS.b as f32 * g) as u8,
                        255,
                    );
                    // Grass cap block.
                    c3.draw_cube(Vector3::new(cx, -0.14, 0.0), period, 0.28, 3.0, grass);
                    // Dirt body below (the Mario-style underground, its front
                    // face is the visible cross-section).
                    let dg = 1.0 + (hash01(k, 42) - 0.5) * 0.14;
                    let dirt = Color::new(
                        (DIRT.r as f32 * dg) as u8,
                        (DIRT.g as f32 * dg) as u8,
                        (DIRT.b as f32 * dg) as u8,
                        255,
                    );
                    // The bank: a strip of earth above the water, running on
                    // down beneath it (deep enough for the swell's troughs).
                    c3.draw_cube(Vector3::new(cx, -0.88, 0.0), period, 1.2, 3.0, dirt);
                    // Grass edge highlight on top, blocky dashes.
                    if hash01(k, 47) > 0.6 {
                        c3.draw_cube(
                            Vector3::new(cx, 0.02, 1.3),
                            period * 0.5,
                            0.06,
                            0.3,
                            GRASS_DARK,
                        );
                    }
                }
            }

            // --- water, in front of the bank ------------------------------
            if let Some(water) = gfx.water.as_mut() {
                water.draw(&mut c3, camera.position, self.t, dist_u, self.voice_glow);
            }

            // --- obstacles (hero plane z 0) --------------------------------
            for o in &self.obstacles {
                let alpha = if o.bounced { 200 } else { 255 };
                match o.kind {
                    Kind::Jump => {
                        let (pos, size) = wrect(obstacle_rect(o), 0.0, 0.6);
                        let c = Color::new(ROSE.r, ROSE.g, ROSE.b, alpha);
                        c3.draw_cube_v(pos, size, c);
                        c3.draw_cube_wires_v(pos, size, MINT_DARK);
                    }
                    Kind::Duck => {
                        let (pos, size) = wrect(obstacle_rect(o), 0.0, 0.6);
                        let c = Color::new(LILAC.r, LILAC.g, LILAC.b, alpha);
                        c3.draw_cube_v(pos, size, c);
                        c3.draw_cube_wires_v(pos, size, MINT_DARK);
                        if !o.bounced {
                            // Posts holding the bar.
                            let (rx, ry, rw, rh) = obstacle_rect(o);
                            for px in [rx + 8.0, rx + rw - 8.0] {
                                let post_top = ry + rh;
                                let post_h = GROUND_Y - post_top;
                                c3.draw_cube(
                                    Vector3::new(wx(px), wy(post_top + post_h / 2.0), 0.0),
                                    0.1,
                                    post_h / PPU,
                                    0.1,
                                    LILAC,
                                );
                            }
                        }
                    }
                    Kind::Wall => {
                        // A little masonry: staggered brick courses, mortar wires.
                        let (rx, ry, rw, rh) = obstacle_rect(o);
                        let (rows, cols) = (8i32, 2i32);
                        let (bw, bh) = (rw / cols as f32, rh / rows as f32);
                        for r in 0..rows {
                            let stagger = if r % 2 == 0 { 0.0 } else { bw * 0.5 };
                            for c in -1..=cols {
                                let bx = rx + stagger + c as f32 * bw;
                                let left = bx.max(rx);
                                let right = (bx + bw).min(rx + rw);
                                if right - left < 4.0 {
                                    continue;
                                }
                                let base = if (r + c).rem_euclid(2) == 0 {
                                    WALL_A
                                } else {
                                    WALL_B
                                };
                                let col = Color::new(base.r, base.g, base.b, alpha);
                                let (pos, size) = wrect(
                                    (left, ry + r as f32 * bh, right - left, bh - 3.0),
                                    0.0,
                                    0.7,
                                );
                                c3.draw_cube_v(pos, size, col);
                                c3.draw_cube_wires_v(pos, size, WALL_MORTAR);
                            }
                        }
                    }
                    Kind::High => {
                        // A slim rose totem: stacked blocks, jump-family colour.
                        let (rx, ry, rw, rh) = obstacle_rect(o);
                        let rows = 9i32;
                        let bh = rh / rows as f32;
                        for r in 0..rows {
                            let dark = r % 2 == 0;
                            let base = if dark { ROSE } else { PEACH };
                            let col = Color::new(base.r, base.g, base.b, alpha);
                            let (pos, size) =
                                wrect((rx, ry + r as f32 * bh, rw, bh - 3.0), 0.0, 0.55);
                            c3.draw_cube_v(pos, size, col);
                            c3.draw_cube_wires_v(pos, size, MINT_DARK);
                        }
                    }
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
                        GROUND_Y - DUCK_H
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
                                0.07 + p.max(0.0) * 0.07,
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

        // Letter signs above live obstacles, projected from world space.
        for o in &self.obstacles {
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
            d.draw_rectangle_rounded(sign, 0.3, 6, Color::new(255, 255, 255, 235));
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
        r.obstacles.push(Obstacle {
            x: HERO_X + 200.0,
            kind: Kind::Jump,
            bounced: false,
            counted: false,
            fly_y: 0.0,
            fly_vy: 0.0,
            rot: 0.0,
        });
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
        r.obstacles.push(Obstacle {
            x: HERO_X + 200.0,
            kind: Kind::Jump,
            bounced: false,
            counted: false,
            fly_y: 0.0,
            fly_vy: 0.0,
            rot: 0.0,
        });
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
        r.obstacles.push(Obstacle {
            x: HERO_X + 320.0,
            kind: Kind::Wall,
            bounced: false,
            counted: false,
            fly_y: 0.0,
            fly_vy: 0.0,
            rot: 0.0,
        });
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
        r.obstacles.push(Obstacle {
            x: HERO_X + 200.0,
            kind: Kind::Wall,
            bounced: false,
            counted: false,
            fly_y: 0.0,
            fly_vy: 0.0,
            rot: 0.0,
        });
        // Never shoot: the wall reaches the hero and bounces, no star.
        for _ in 0..240 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        assert_eq!(r.stars, 0);
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
        r.obstacles.push(Obstacle {
            x: HERO_X + 320.0,
            kind: Kind::Wall,
            bounced: false,
            counted: false,
            fly_y: 0.0,
            fly_vy: 0.0,
            rot: 0.0,
        });
        r.update(&input(None, Some(2)), 1.0 / 60.0);
        for _ in 0..120 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        assert_eq!(r.stars, 1);
        assert_eq!(r.coin.target, 360.0);
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
