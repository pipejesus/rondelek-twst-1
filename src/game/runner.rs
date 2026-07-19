//! "Vowel Runner" — the first voice game. The hero runs on their own; the kid
//! only speaks: say **a** to jump over low blocks, hold **e** to duck under
//! high bars. Misses never kill — the obstacle just bounces away with a
//! wobble; every cleared obstacle earns a star. Visuals follow the Base
//! Pastel skin palette so the game feels like part of the toy.
//!
//! All positions are in a fixed 1280x720 logical canvas, uniformly scaled and
//! centred at draw time, so any monitor works.

use super::{VoiceGame, VoiceInput};
use raylib::prelude::*;

// Logical canvas.
const LW: f32 = 1280.0;
const LH: f32 = 720.0;
const GROUND_Y: f32 = 560.0;

// Hero.
const HERO_X: f32 = 280.0;
const HERO_W: f32 = 88.0;
const HERO_H: f32 = 110.0;
const DUCK_H: f32 = 58.0;
const GRAVITY: f32 = 2100.0;
const JUMP_V: f32 = 1000.0;

// Vowel indices in VOWELS order (a, e, i, o, u, y).
const VOWEL_JUMP: usize = 0; // "a"
const VOWEL_DUCK: usize = 1; // "e"

// Base Pastel palette.
const CREAM: Color = Color::new(251, 242, 228, 255);
const PEACH: Color = Color::new(246, 220, 198, 255);
const MINT: Color = Color::new(198, 229, 211, 255);
const MINT_DARK: Color = Color::new(154, 197, 172, 255);
const ROSE: Color = Color::new(245, 169, 188, 255);
const LILAC: Color = Color::new(201, 184, 232, 255);
const SKY: Color = Color::new(169, 212, 239, 255);
const BUTTER: Color = Color::new(245, 226, 158, 255);
const CHARCOAL: Color = Color::new(74, 68, 60, 255);

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Jump, // low block: jump over (say "a")
    Duck, // high bar: duck under (say "e")
}

struct Obstacle {
    x: f32,
    kind: Kind,
    /// Set on collision: the obstacle flies off instead of the hero failing.
    bounced: bool,
    counted: bool,
    fly_y: f32,
    fly_vy: f32,
    rot: f32,
}

struct Sparkle {
    x: f32,
    y: f32,
    age: f32,
}

pub struct Runner {
    hero_y: f32, // hero top edge when standing on ground = GROUND_Y - height
    vy: f32,
    on_ground: bool,
    ducking: bool,
    obstacles: Vec<Obstacle>,
    sparkles: Vec<Sparkle>,
    stars: u32,
    speed: f32,
    spawn_timer: f32,
    /// Hero squash feedback after a bump (seconds remaining).
    squash: f32,
    t: f32,
    rng: u64,
    // Copied from the latest VoiceInput so draw() can show live feedback.
    last_scores: [f32; 6],
    last_held: Option<usize>,
    last_level: f32,
}

impl Runner {
    pub fn new() -> Self {
        Self {
            hero_y: GROUND_Y - HERO_H,
            vy: 0.0,
            on_ground: true,
            ducking: false,
            obstacles: Vec::new(),
            sparkles: Vec::new(),
            stars: 0,
            speed: 260.0,
            spawn_timer: 1.2,
            squash: 0.0,
            t: 0.0,
            rng: 0x2545_F491_4F6C_DD1D,
            last_scores: [0.0; 6],
            last_held: None,
            last_level: 0.0,
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

impl VoiceGame for Runner {
    fn update(&mut self, input: &VoiceInput, dt: f32) {
        self.t += dt;
        self.squash = (self.squash - dt).max(0.0);
        self.last_scores = input.scores;
        self.last_held = input.held;
        self.last_level = input.level;

        // --- hero ---
        self.ducking = input.held == Some(VOWEL_DUCK) && self.on_ground;
        if input.onset == Some(VOWEL_JUMP) && self.on_ground {
            self.vy = -JUMP_V;
            self.on_ground = false;
        }
        if !self.on_ground {
            self.vy += GRAVITY * dt;
            self.hero_y += self.vy * dt;
            if self.hero_y >= GROUND_Y - HERO_H {
                self.hero_y = GROUND_Y - HERO_H;
                self.vy = 0.0;
                self.on_ground = true;
            }
        }

        // --- obstacles ---
        self.spawn_timer -= dt;
        if self.spawn_timer <= 0.0 {
            let kind = if self.rand() < 0.5 {
                Kind::Jump
            } else {
                Kind::Duck
            };
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
        self.obstacles.retain(|o| o.x > -200.0 && o.fly_y < LH);
        for s in &mut self.sparkles {
            s.age += dt;
            s.y -= 60.0 * dt;
        }
        self.sparkles.retain(|s| s.age < 1.0);
    }

    fn draw(&mut self, d: &mut RaylibDrawHandle, w: i32, h: i32) {
        d.clear_background(CREAM);
        let scale = (w as f32 / LW).min(h as f32 / LH);
        let ox = (w as f32 - LW * scale) / 2.0;
        let oy = (h as f32 - LH * scale) / 2.0;
        // Logical → screen helpers.
        let sx = |x: f32| (ox + x * scale) as i32;
        let sy = |y: f32| (oy + y * scale) as i32;
        let sl = |v: f32| (v * scale) as i32;
        let rect = |x: f32, y: f32, rw: f32, rh: f32| Rectangle {
            x: ox + x * scale,
            y: oy + y * scale,
            width: rw * scale,
            height: rh * scale,
        };

        // Sky wash + drifting clouds (parallax at half speed).
        d.draw_rectangle_gradient_v(0, 0, w, h, CREAM, PEACH);
        for (i, base_x) in [180.0f32, 620.0, 1030.0].iter().enumerate() {
            let cx = (base_x - (self.t * self.speed * 0.12) % (LW + 300.0) + LW + 300.0)
                % (LW + 300.0)
                - 150.0;
            let cy = 110.0 + i as f32 * 55.0;
            let c = Color::new(255, 255, 255, 170);
            d.draw_circle(sx(cx), sy(cy), 34.0 * scale, c);
            d.draw_circle(sx(cx + 34.0), sy(cy + 10.0), 26.0 * scale, c);
            d.draw_circle(sx(cx - 34.0), sy(cy + 12.0), 24.0 * scale, c);
        }

        // Ground: mint band with scrolling dashes for a sense of speed.
        d.draw_rectangle(0, sy(GROUND_Y), w, h - sy(GROUND_Y), MINT);
        d.draw_rectangle(0, sy(GROUND_Y), w, sl(6.0), MINT_DARK);
        let dash_shift = (self.t * self.speed) % 90.0;
        let mut x = -dash_shift;
        while x < LW {
            d.draw_rectangle_rounded(rect(x, GROUND_Y + 34.0, 44.0, 8.0), 1.0, 4, MINT_DARK);
            x += 90.0;
        }

        // Obstacles + their letter signs.
        for o in &self.obstacles {
            let (rx, ry, rw, rh) = obstacle_rect(o);
            let (body, letter) = match o.kind {
                Kind::Jump => (ROSE, "a"),
                Kind::Duck => (LILAC, "e"),
            };
            let alpha = if o.bounced { 180 } else { 255 };
            let body = Color::new(body.r, body.g, body.b, alpha);
            d.draw_rectangle_rounded(rect(rx, ry, rw, rh), 0.35, 6, body);
            if o.kind == Kind::Duck && !o.bounced {
                // Posts holding the bar, so "go under" reads visually.
                d.draw_rectangle_rounded(
                    rect(rx + 6.0, ry + rh, 10.0, GROUND_Y - ry - rh),
                    1.0,
                    4,
                    LILAC,
                );
                d.draw_rectangle_rounded(
                    rect(rx + rw - 16.0, ry + rh, 10.0, GROUND_Y - ry - rh),
                    1.0,
                    4,
                    LILAC,
                );
            }
            if !o.bounced {
                // Letter sign floating above: what to say.
                let sign = rect(rx + rw / 2.0 - 44.0, ry - 120.0, 88.0, 88.0);
                d.draw_rectangle_rounded(sign, 0.3, 6, Color::new(255, 255, 255, 235));
                d.draw_rectangle_rounded_lines(sign, 0.3, 6, CHARCOAL);
                let fs = sl(64.0);
                let tw = d.measure_text(letter, fs);
                d.draw_text(
                    letter,
                    sx(rx + rw / 2.0) - tw / 2,
                    sy(ry - 120.0 + 12.0),
                    fs,
                    CHARCOAL,
                );
            }
        }

        // Hero: chunky rounded body with a face; squashes on bump, leans when
        // ducking, legs scissor while running.
        let (hx, hy, hw, hh) = self.hero_rect();
        let squash = 1.0 - 0.18 * (self.squash / 0.35);
        let body = rect(hx, hy + hh * (1.0 - squash), hw, hh * squash);
        d.draw_rectangle_rounded(body, 0.45, 8, SKY);
        // Legs (only while grounded).
        if self.on_ground {
            let phase = (self.t * 10.0).sin();
            let leg =
                |off: f32, p: f32| rect(hx + hw / 2.0 + off - 8.0, GROUND_Y - 4.0 + p, 16.0, 12.0);
            d.draw_rectangle_rounded(leg(-20.0, phase.max(0.0) * -6.0), 1.0, 4, SKY);
            d.draw_rectangle_rounded(leg(20.0, (-phase).max(0.0) * -6.0), 1.0, 4, SKY);
        }
        // Face.
        let face_y = hy + hh * (1.0 - squash) + hh * squash * 0.32;
        let eye_r = 5.0 * scale;
        d.draw_circle(sx(hx + hw * 0.32), sy(face_y), eye_r, CHARCOAL);
        d.draw_circle(sx(hx + hw * 0.68), sy(face_y), eye_r, CHARCOAL);
        // Mouth: opens wide while a vowel registers — the hero "speaks" along.
        let mouth_y = face_y + 22.0;
        // Any sound at all makes the mouth stir — babbling is progress too.
        let mouth_r = if self.last_held.is_some() {
            13.0
        } else {
            5.0 + 5.0 * self.last_level.clamp(0.0, 1.0)
        };
        d.draw_circle(sx(hx + hw * 0.5), sy(mouth_y), mouth_r * scale, CHARCOAL);

        // Star sparkles.
        for s in &self.sparkles {
            let a = (255.0 * (1.0 - s.age)) as u8;
            let c = Color::new(BUTTER.r, BUTTER.g, BUTTER.b, a);
            let r = (10.0 + 8.0 * s.age) * scale;
            d.draw_circle(sx(s.x), sy(s.y), r, c);
        }

        // Star counter: big and centred, a little below the top edge — right
        // where the kid is already looking, so chasing points needs no focus
        // switch. Sits above the obstacle signs and the hero's jump apex.
        let txt = format!("{}", self.stars);
        let fs = sl(84.0);
        let tw = d.measure_text(&txt, fs);
        let star_r = 32.0 * scale;
        let gap = sl(24.0);
        let total = (star_r * 2.0) as i32 + gap + tw;
        let left = (w - total) / 2;
        let cy = sy(120.0);
        d.draw_circle(left + star_r as i32, cy, star_r, BUTTER);
        d.draw_circle_lines(left + star_r as i32, cy, star_r, CHARCOAL);
        d.draw_text(
            &txt,
            left + (star_r * 2.0) as i32 + gap,
            cy - fs / 2,
            fs,
            CHARCOAL,
        );

        // Vowel meter strip, bottom-centre: six mini bars with labels.
        let labels = ["a", "e", "i", "o", "u", "y"];
        let strip_w = 6.0 * 64.0;
        let strip_x = LW / 2.0 - strip_w / 2.0;
        for i in 0..6 {
            let bx = strip_x + i as f32 * 64.0;
            let level = self.last_scores[i].clamp(0.0, 1.0);
            let bh = 10.0 + 44.0 * level;
            let active = self.last_held == Some(i);
            let c = if active {
                BUTTER
            } else {
                Color::new(255, 255, 255, 160)
            };
            d.draw_rectangle_rounded(rect(bx, LH - 40.0 - bh, 40.0, bh), 0.5, 4, c);
            d.draw_text(labels[i], sx(bx + 14.0), sy(LH - 32.0), sl(22.0), CHARCOAL);
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
        let mut r = Runner::new();
        r.update(&input(Some(VOWEL_JUMP), Some(VOWEL_JUMP)), 1.0 / 60.0);
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
        let mut r = Runner::new();
        r.update(&input(Some(VOWEL_DUCK), None), 1.0 / 60.0);
        assert!(r.ducking);
        r.update(&input(None, None), 1.0 / 60.0);
        assert!(!r.ducking);
    }

    #[test]
    fn passed_obstacle_awards_star_and_bumped_one_does_not() {
        let mut r = Runner::new();
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
                input(Some(VOWEL_JUMP), Some(VOWEL_JUMP))
            } else {
                input(None, None)
            };
            r.update(&i, 1.0 / 60.0);
        }
        assert!(jumped);
        assert_eq!(r.stars, 1);

        // Same again but without jumping: collision bounces it, no star.
        let mut r = Runner::new();
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
}
