//! "Vowel Runner" — the first voice game, now in 2.5D. The hero runs on their
//! own; the kid only speaks: say one vowel to jump over low blocks, hold
//! another to duck under high bars. Misses never kill — the obstacle just
//! bounces away; every cleared obstacle earns a star.
//!
//! Rendering: a fixed perspective camera slightly above and beside the action,
//! everything built from chunky 3D bricks ("pixels became big and 3-D").
//! Parallax layers scroll by hand-tuned factors of the travelled distance:
//! clouds 0.10, mountains 0.25 (fog shader), bushes 0.55, ground 1.0. The
//! hero placeholder brick is drawn under a comic-style toon shader — the same
//! slot the dragon GLB model will use later. HUD (letter signs, meter, stars)
//! stays crisp 2D, projected over the scene.
//!
//! Gameplay math is untouched from the 2D version: `update()` works in the
//! same 1280x720 logical space (100 logical px = 1 world unit at draw time),
//! so physics, collisions and their tests are identical.

use super::{VoiceGame, VoiceInput};
use raylib::prelude::*;
use raylib::rlgl::RaylibRlgl; // matrix stack for the salto flip
use rondelek_core::audio::vowel::VOWELS;

use super::{BUTTER, CHARCOAL, CREAM, LILAC, MINT_DARK, PEACH, ROSE, SKY};

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

// 2.5D palette additions.
const GRASS: Color = Color::new(139, 205, 130, 255);
const GRASS_DARK: Color = Color::new(110, 176, 105, 255);
const DIRT: Color = Color::new(173, 128, 94, 255);
const DIRT_DARK: Color = Color::new(128, 92, 66, 255);
const BUSH_A: Color = Color::new(96, 168, 110, 255);
const BUSH_B: Color = Color::new(74, 146, 92, 255);
const MOUNT_A: Color = Color::new(151, 165, 196, 255);
const MOUNT_B: Color = Color::new(126, 142, 178, 255);
const CLOUD_WHITE: Color = Color::new(255, 253, 247, 255);
// Wall: solid stone, deliberately un-pastel so it reads as "impassable".
const WALL_A: Color = Color::new(154, 136, 126, 255);
const WALL_B: Color = Color::new(122, 106, 98, 255);
const WALL_MORTAR: Color = Color::new(92, 80, 74, 255);

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

/// GPU-side resources, loaded in `init` (absent in unit tests — no window).
struct Gfx {
    camera: Camera3D,
    toon: Shader,
    fog: Shader,
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
    /// Therapist-chosen controls (indices into VOWELS).
    jump_vowel: usize,
    duck_vowel: usize,
    shoot_vowel: usize,
    /// Obstacle kinds allowed to spawn (therapist's difficulty pick; never
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
            speed: 260.0,
            spawn_timer: 1.2,
            squash: 0.0,
            t: 0.0,
            dist: 0.0,
            rng: 0x2545_F491_4F6C_DD1D,
            last_scores: [0.0; 6],
            last_held: None,
            last_level: 0.0,
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
        Kind::Wall => (o.x - WALL_W / 2.0, GROUND_Y - WALL_H + o.fly_y, WALL_W, WALL_H),
        // Tall slim pillar: only the ninja double-jump clears it.
        Kind::High => (o.x - HIGH_W / 2.0, GROUND_Y - HIGH_H + o.fly_y, HIGH_W, HIGH_H),
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
        let loc_color = fog.get_shader_location("fogColor");
        let loc_amount = fog.get_shader_location("fogAmount");
        // Fog toward the sky wash; constant per layer, so set once.
        fog.set_shader_value(loc_color, [0.965, 0.87, 0.8, 1.0f32]);
        fog.set_shader_value(loc_amount, 0.45f32);

        self.gfx = Some(Gfx {
            camera: Camera3D::perspective(
                Vector3::new(0.9, 2.2, 12.5),
                Vector3::new(0.0, 1.0, 0.0),
                Vector3::Y,
                45.0,
            ),
            toon,
            fog,
        });
    }

    fn update(&mut self, input: &VoiceInput, dt: f32) {
        self.t += dt;
        self.squash = (self.squash - dt).max(0.0);
        self.salto = (self.salto - dt).max(0.0);
        self.last_scores = input.scores;
        self.last_held = input.held;
        self.last_level = input.level;
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

        // Sky wash behind everything.
        d.clear_background(CREAM);
        d.draw_rectangle_gradient_v(0, 0, w, h, CREAM, PEACH);

        let dist_u = self.dist / PPU;
        {
            let mut c3 = d.begin_mode3D(camera);

            // --- clouds (z -14, factor 0.10): massive, gentle, blocky ------
            {
                let z = -14.0;
                let period = 16.0;
                let off = dist_u * 0.10 + self.t * 0.12;
                let span = half_span(z);
                let k0 = ((off - span) / period).floor() as i64;
                let k1 = ((off + span) / period).ceil() as i64;
                for k in k0..=k1 {
                    let cx = k as f32 * period - off;
                    let cy = 4.2 + hash01(k, 11) * 2.4;
                    let s = 1.0 + hash01(k, 12) * 0.8;
                    let c = CLOUD_WHITE;
                    c3.draw_cube(Vector3::new(cx, cy, z), 2.6 * s, 1.0 * s, 1.0, c);
                    c3.draw_cube(
                        Vector3::new(cx - 1.5 * s, cy - 0.3, z),
                        1.4 * s,
                        0.8 * s,
                        1.0,
                        c,
                    );
                    c3.draw_cube(
                        Vector3::new(cx + 1.5 * s, cy - 0.25, z),
                        1.2 * s,
                        0.7 * s,
                        1.0,
                        c,
                    );
                    c3.draw_cube(
                        Vector3::new(cx + 0.4 * s, cy + 0.55 * s, z),
                        1.3 * s,
                        0.7 * s,
                        1.0,
                        c,
                    );
                }
            }

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

            // --- bushes (z -4, factor 0.55): clusters of green blocks ------
            {
                let z = -4.0;
                let period = 2.4;
                let off = dist_u * 0.55;
                let span = half_span(z);
                let k0 = ((off - span) / period).floor() as i64;
                let k1 = ((off + span) / period).ceil() as i64;
                for k in k0..=k1 {
                    if hash01(k, 31) < 0.25 {
                        continue; // gaps between bushes
                    }
                    let cx = k as f32 * period - off + (hash01(k, 32) - 0.5) * 1.2;
                    let n = 3 + (hash01(k, 33) * 4.0) as i32;
                    for j in 0..n {
                        let jj = k.wrapping_mul(16).wrapping_add(j as i64);
                        let s = 0.35 + hash01(jj, 34) * 0.4;
                        let bx = cx + (hash01(jj, 35) - 0.5) * 1.3;
                        let by = s / 2.0 + hash01(jj, 36) * 0.5;
                        let col = if hash01(jj, 37) > 0.5 { BUSH_A } else { BUSH_B };
                        // Bush blocks funnel through one draw call so the
                        // user's textures can swap in later (mesh + material).
                        c3.draw_cube(Vector3::new(bx, by, z), s, s, s, col);
                    }
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
                    c3.draw_cube(Vector3::new(cx, -1.23, 0.0), period, 1.9, 3.0, dirt);
                    // Darker speckle stones embedded in the cross-section.
                    if hash01(k, 43) > 0.45 {
                        let sy = -0.55 - hash01(k, 44) * 1.2;
                        let ss = 0.14 + hash01(k, 45) * 0.16;
                        c3.draw_cube(
                            Vector3::new(cx + (hash01(k, 46) - 0.5) * 0.6, sy, 1.4),
                            ss,
                            ss,
                            0.25,
                            DIRT_DARK,
                        );
                    }
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
                                let (pos, size) =
                                    wrect((left, ry + r as f32 * bh, right - left, bh - 3.0), 0.0, 0.7);
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
        }

        // --- HUD: crisp 2D over the 3D scene -------------------------------
        let scale = (w as f32 / LW).min(h as f32 / 720.0);
        let sl = |v: f32| (v * scale) as i32;

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

        // Star counter: big and centred, where the kid is already looking.
        let txt = format!("{}", self.stars);
        let fs = sl(84.0);
        let tw = d.measure_text(&txt, fs);
        let star_r = 32.0 * scale;
        let gap = sl(24.0);
        let total = (star_r * 2.0) as i32 + gap + tw;
        let left = (w - total) / 2;
        let cy = sl(120.0);
        d.draw_circle(left + star_r as i32, cy, star_r, BUTTER);
        d.draw_circle_lines(left + star_r as i32, cy, star_r, CHARCOAL);
        d.draw_text(
            &txt,
            left + (star_r * 2.0) as i32 + gap,
            cy - fs / 2,
            fs,
            CHARCOAL,
        );

        // Vowel meter strip, bottom-centre.
        let labels = ["a", "e", "i", "o", "u", "y"];
        let strip_w = 6.0 * 64.0 * scale;
        let strip_x = w as f32 / 2.0 - strip_w / 2.0;
        for i in 0..6 {
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
                labels[i],
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
    fn distance_accumulates_for_parallax() {
        let mut r = Runner::new(0, 1, 2, vec![Kind::Jump, Kind::Duck]);
        for _ in 0..60 {
            r.update(&input(None, None), 1.0 / 60.0);
        }
        // One second at starting speed ~260 px/s.
        assert!((r.dist - 260.0).abs() < 5.0);
    }
}
