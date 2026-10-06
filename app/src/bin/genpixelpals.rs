//! Generates the "pixel pals": the eight character avatars as vivid 32×32
//! pixel art, into `assets/avatars/pixel-<name>.png` (saved 8× bigger, 256×256,
//! so every place that shows an avatar can use them as they are).
//!
//! Run from the repo root: `cargo run --bin genpixelpals`
//!
//! They sit **next to** the classic (smooth) avatars from `genavatars`, never
//! over them: the classic set stays, and this generator only writes `pixel-*`
//! files. Once a pixel pal has been touched up by hand, don't run it again.
//!
//! How a pal is made, the way a pixel artist would:
//!
//! 1. **Shapes** (head, ears, muzzle…) are filled pixel by pixel from the pixel
//!    *centres*, no anti-aliasing: every edge is a hard stair-step.
//! 2. **Shading** from a light at the top-left: a highlight rim where a shape
//!    faces the light, a shadow rim and a darker lower band where it turns away.
//! 3. **Selective outline**: only the silhouette gets an outline, in a dark
//!    tint of each part's own colour (never flat black).
//! 4. **Faces are hand-placed pixels**: eyes with a white catch-light, nose,
//!    mouth, blush.
//! 5. A **vivid background** that complements the animal, with a few pixel
//!    sparkles.

use std::path::Path;

const N: usize = 32; // sprite size
const SCALE: u32 = 8; // saved size = N × SCALE

type Rgb = [u8; 3];

const fn hex(v: u32) -> Rgb {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8]
}

/// Mix `a` toward `b` by `t` (0..1).
fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

const WHITE: Rgb = hex(0xFFFFFF);
/// The navy ink everything dark is tinted toward (the arcade palette's).
const INK: Rgb = hex(0x120F2A);
const EYE: Rgb = hex(0x1A1030);
const BLUSH: Rgb = hex(0xFF8FB4);

// ---- shapes -------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Shape {
    Ellipse(f32, f32, f32, f32),
    Tri([(f32, f32); 3]),
    Rect(f32, f32, f32, f32),
}

impl Shape {
    fn contains(&self, x: f32, y: f32) -> bool {
        match *self {
            Shape::Ellipse(cx, cy, rx, ry) => {
                let (dx, dy) = ((x - cx) / rx, (y - cy) / ry);
                dx * dx + dy * dy <= 1.0
            }
            Shape::Tri(p) => {
                let s = |a: (f32, f32), b: (f32, f32)| {
                    (b.0 - a.0) * (y - a.1) - (b.1 - a.1) * (x - a.0)
                };
                let (d1, d2, d3) = (s(p[0], p[1]), s(p[1], p[2]), s(p[2], p[0]));
                let neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
                let pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
                !(neg && pos)
            }
            Shape::Rect(x0, y0, x1, y1) => x >= x0 && x <= x1 && y >= y0 && y <= y1,
        }
    }

    /// Vertical extent, for the lower shadow band.
    fn y_range(&self) -> (f32, f32) {
        match *self {
            Shape::Ellipse(_, cy, _, ry) => (cy - ry, cy + ry),
            Shape::Tri(p) => {
                let ys = [p[0].1, p[1].1, p[2].1];
                (
                    ys.iter().cloned().fold(f32::MAX, f32::min),
                    ys.iter().cloned().fold(f32::MIN, f32::max),
                )
            }
            Shape::Rect(_, y0, _, y1) => (y0, y1),
        }
    }
}

/// One layer of a sprite.
struct Part {
    shape: Shape,
    color: Rgb,
    /// Shade it (highlight rim, shadow rim, lower band).
    shaded: bool,
    /// Outline it where it meets the background.
    outlined: bool,
}

fn part(shape: Shape, color: Rgb) -> Part {
    Part {
        shape,
        color,
        shaded: true,
        outlined: true,
    }
}

/// A soft inner part (muzzle, belly): shaded but never outlined.
fn patch(shape: Shape, color: Rgb) -> Part {
    Part {
        outlined: false,
        ..part(shape, color)
    }
}

/// A flat inner part (eye white, screen): one colour, no outline.
fn flat(shape: Shape, color: Rgb) -> Part {
    Part {
        shaded: false,
        outlined: false,
        ..part(shape, color)
    }
}

fn ell(cx: f32, cy: f32, rx: f32, ry: f32) -> Shape {
    Shape::Ellipse(cx, cy, rx, ry)
}
fn circle(cx: f32, cy: f32, r: f32) -> Shape {
    Shape::Ellipse(cx, cy, r, r)
}
fn tri(a: (f32, f32), b: (f32, f32), c: (f32, f32)) -> Shape {
    Shape::Tri([a, b, c])
}
fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Shape {
    Shape::Rect(x0, y0, x1, y1)
}

// ---- the sprite ------------------------------------------------------------------

struct Sprite {
    bg: Rgb,
    parts: Vec<Part>,
    /// Hand-placed pixels, drawn last: (x, y, colour).
    pixels: Vec<(usize, usize, Rgb)>,
    /// Where the background sparkles go (centres; skipped if covered).
    sparkles: Vec<(usize, usize)>,
}

impl Sprite {
    fn new(bg: Rgb) -> Self {
        Self {
            bg,
            parts: Vec::new(),
            pixels: Vec::new(),
            sparkles: vec![(4, 4), (27, 5), (4, 25), (28, 23)],
        }
    }
    fn add(&mut self, p: Part) -> &mut Self {
        self.parts.push(p);
        self
    }
    fn px(&mut self, x: usize, y: usize, c: Rgb) -> &mut Self {
        self.pixels.push((x, y, c));
        self
    }
    fn block(&mut self, x: usize, y: usize, w: usize, h: usize, c: Rgb) -> &mut Self {
        for yy in y..y + h {
            for xx in x..x + w {
                self.px(xx, yy, c);
            }
        }
        self
    }
    /// A `w`×`h` eye with a white catch-light at its top-left.
    fn eye(&mut self, x: usize, y: usize, w: usize, h: usize) -> &mut Self {
        self.block(x, y, w, h, EYE).px(x, y, WHITE)
    }
    /// A pixel line through the given points (for mouths and whiskers).
    fn line(&mut self, pts: &[(usize, usize)], c: Rgb) -> &mut Self {
        for &(x, y) in pts {
            self.px(x, y, c);
        }
        self
    }

    fn render(&self) -> [[Rgb; N]; N] {
        // Which part owns each pixel (topmost wins).
        let mut owner: [[Option<usize>; N]; N] = [[None; N]; N];
        for (y, row) in owner.iter_mut().enumerate() {
            for (x, cell) in row.iter_mut().enumerate() {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                *cell = self.parts.iter().rposition(|p| p.shape.contains(fx, fy));
            }
        }

        let mut out = [[self.bg; N]; N];
        // Background sparkles: little "+" stars where nothing covers them.
        let spark = mix(self.bg, WHITE, 0.55);
        for &(cx, cy) in &self.sparkles {
            for (dx, dy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (x, y) = (cx as i32 + dx, cy as i32 + dy);
                if (0..N as i32).contains(&x) && (0..N as i32).contains(&y) {
                    let (x, y) = (x as usize, y as usize);
                    if owner[y][x].is_none() {
                        out[y][x] = if (dx, dy) == (0, 0) { WHITE } else { spark };
                    }
                }
            }
        }

        for y in 0..N {
            for x in 0..N {
                let Some(i) = owner[y][x] else { continue };
                let p = &self.parts[i];
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let mut c = p.color;
                if p.shaded {
                    let light = !p.shape.contains(fx - 1.4, fy - 1.4);
                    let dark = !p.shape.contains(fx + 1.4, fy + 1.4);
                    let (y0, y1) = p.shape.y_range();
                    let low = (fy - y0) / (y1 - y0).max(1.0) > 0.8;
                    c = if dark || low {
                        mix(p.color, INK, 0.28)
                    } else if light {
                        mix(p.color, WHITE, 0.3)
                    } else {
                        p.color
                    };
                }
                // Silhouette outline: a dark tint of the part's own colour,
                // wherever it meets the background (not the canvas edge).
                if p.outlined {
                    let edge = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|&(dx, dy)| {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            (0..N as i32).contains(&nx)
                                && (0..N as i32).contains(&ny)
                                && owner[ny as usize][nx as usize].is_none()
                        });
                    if edge {
                        c = mix(p.color, INK, 0.72);
                    }
                }
                out[y][x] = c;
            }
        }

        for &(x, y, c) in &self.pixels {
            if x < N && y < N {
                out[y][x] = c;
            }
        }
        out
    }

    fn save(&self, path: &Path) {
        let px = self.render();
        let size = N as u32 * SCALE;
        let img = image::RgbaImage::from_fn(size, size, |x, y| {
            let c = px[(y / SCALE) as usize][(x / SCALE) as usize];
            image::Rgba([c[0], c[1], c[2], 255])
        });
        img.save(path)
            .unwrap_or_else(|e| panic!("save {}: {e}", path.display()));
    }
}

// ---- the pals ---------------------------------------------------------------------

fn fox() -> Sprite {
    let orange = hex(0xFF7A1F);
    let cream = hex(0xFFF1DC);
    let mut s = Sprite::new(hex(0x3AA8F0));
    s.add(part(ell(16.0, 33.0, 12.0, 6.5), orange))
        .add(patch(ell(16.0, 33.5, 5.0, 5.0), cream))
        .add(part(tri((5.5, 3.5), (5.0, 15.0), (13.5, 10.0)), orange))
        .add(part(tri((26.5, 3.5), (27.0, 15.0), (18.5, 10.0)), orange))
        .add(flat(
            tri((7.0, 7.0), (7.0, 13.0), (11.5, 10.5)),
            hex(0x6A2A18),
        ))
        .add(flat(
            tri((25.0, 7.0), (25.0, 13.0), (20.5, 10.5)),
            hex(0x6A2A18),
        ))
        .add(part(ell(16.0, 17.5, 11.5, 9.5), orange))
        .add(patch(ell(9.5, 22.0, 5.0, 3.8), cream))
        .add(patch(ell(22.5, 22.0, 5.0, 3.8), cream))
        .add(patch(ell(16.0, 23.5, 4.5, 3.5), cream));
    s.eye(10, 15, 2, 3)
        .eye(20, 15, 2, 3)
        .block(15, 20, 2, 2, hex(0x2A1410))
        .px(15, 20, hex(0x7A4A40))
        .line(&[(14, 23), (15, 24), (16, 24), (17, 23)], hex(0x6A2A18))
        .line(&[(7, 20), (8, 20), (23, 20), (24, 20)], BLUSH);
    s
}

fn cat() -> Sprite {
    let violet = hex(0x9A6BF2);
    let light = hex(0xD2C0FF);
    let mut s = Sprite::new(hex(0xF7C83A));
    s.add(part(ell(16.0, 33.0, 12.0, 6.5), violet))
        .add(patch(ell(16.0, 34.0, 4.5, 4.5), light))
        .add(part(tri((5.0, 3.0), (5.5, 15.0), (13.0, 10.0)), violet))
        .add(part(tri((27.0, 3.0), (26.5, 15.0), (19.0, 10.0)), violet))
        .add(flat(
            tri((6.5, 6.5), (7.0, 12.5), (11.0, 10.5)),
            hex(0xFF8FC4),
        ))
        .add(flat(
            tri((25.5, 6.5), (25.0, 12.5), (21.0, 10.5)),
            hex(0xFF8FC4),
        ))
        .add(part(ell(16.0, 18.0, 11.5, 9.5), violet))
        .add(patch(ell(16.0, 22.5, 4.5, 3.0), light));
    let green = hex(0x3FD67A);
    s.block(9, 14, 3, 3, green)
        .block(20, 14, 3, 3, green)
        .line(&[(10, 14), (10, 15), (10, 16)], EYE)
        .line(&[(21, 14), (21, 15), (21, 16)], EYE)
        .px(9, 14, WHITE)
        .px(20, 14, WHITE)
        .block(15, 20, 2, 1, hex(0xFF5FA0))
        .line(&[(14, 22), (15, 23), (16, 23), (17, 22)], hex(0x3A1E70))
        .line(
            &[(4, 20), (5, 20), (6, 20), (4, 23), (5, 22), (6, 22)],
            hex(0x4A2A90),
        )
        .line(
            &[(25, 20), (26, 20), (27, 20), (27, 23), (26, 22), (25, 22)],
            hex(0x4A2A90),
        )
        .line(&[(8, 19), (23, 19)], BLUSH);
    s
}

fn bear() -> Sprite {
    let brown = hex(0xB86A30);
    let tan = hex(0xF4D2A0);
    let mut s = Sprite::new(hex(0x35C77E));
    s.add(part(ell(16.0, 33.0, 12.5, 6.5), brown))
        .add(part(circle(7.0, 8.5, 4.5), brown))
        .add(part(circle(25.0, 8.5, 4.5), brown))
        .add(flat(circle(7.0, 8.5, 2.2), hex(0xE8A870)))
        .add(flat(circle(25.0, 8.5, 2.2), hex(0xE8A870)))
        .add(part(ell(16.0, 18.0, 11.5, 10.0), brown))
        .add(patch(ell(16.0, 22.5, 5.5, 4.0), tan));
    s.eye(10, 16, 2, 2)
        .eye(20, 16, 2, 2)
        .block(14, 20, 4, 2, hex(0x2A1810))
        .px(14, 20, hex(0x7A5040))
        .line(&[(14, 23), (15, 24), (16, 24), (17, 23)], hex(0x6A3A18))
        .line(&[(7, 20), (8, 20), (23, 20), (24, 20)], BLUSH);
    s
}

fn frog() -> Sprite {
    let green = hex(0x46D05A);
    let mut s = Sprite::new(hex(0xF25C9A));
    s.add(part(ell(16.0, 33.0, 12.5, 6.0), green))
        .add(patch(ell(16.0, 34.0, 6.0, 5.0), hex(0xD8F7A0)))
        .add(part(circle(9.5, 11.0, 5.0), green))
        .add(part(circle(22.5, 11.0, 5.0), green))
        .add(part(ell(16.0, 20.0, 13.0, 8.5), green))
        .add(flat(circle(9.5, 11.0, 3.3), WHITE))
        .add(flat(circle(22.5, 11.0, 3.3), WHITE));
    let mouth = hex(0x1E5A26);
    s.eye(9, 10, 2, 3)
        .eye(22, 10, 2, 3)
        .px(14, 17, mouth)
        .px(17, 17, mouth)
        .line(
            &[
                (8, 21),
                (9, 22),
                (10, 23),
                (11, 23),
                (12, 24),
                (13, 24),
                (14, 24),
                (15, 24),
                (16, 24),
                (17, 24),
                (18, 24),
                (19, 24),
                (20, 23),
                (21, 23),
                (22, 22),
                (23, 21),
            ],
            mouth,
        )
        .line(&[(6, 19), (7, 19), (24, 19), (25, 19)], BLUSH);
    s
}

fn owl() -> Sprite {
    let blue = hex(0x3A7CF0);
    let pale = hex(0xC2D8FF);
    let butter = hex(0xFFD84A);
    let mut s = Sprite::new(hex(0xFFD84A));
    s.sparkles = vec![(4, 4), (27, 4), (3, 24), (28, 24)];
    s.bg = hex(0xFF8A3A);
    s.add(part(ell(16.0, 32.0, 12.5, 7.0), blue))
        .add(patch(ell(16.0, 33.0, 6.0, 5.0), pale))
        .add(part(tri((5.0, 4.0), (6.0, 12.0), (12.0, 9.0)), blue))
        .add(part(tri((27.0, 4.0), (26.0, 12.0), (20.0, 9.0)), blue))
        .add(part(ell(16.0, 17.5, 12.0, 10.0), blue))
        .add(patch(ell(16.0, 18.0, 9.5, 7.5), pale))
        .add(flat(circle(11.0, 16.0, 4.2), butter))
        .add(flat(circle(21.0, 16.0, 4.2), butter))
        .add(flat(circle(11.0, 16.0, 3.0), WHITE))
        .add(flat(circle(21.0, 16.0, 3.0), WHITE))
        .add(flat(
            tri((14.5, 19.5), (17.5, 19.5), (16.0, 23.0)),
            hex(0xFF8A1A),
        ));
    let feather = hex(0x7FA6F0);
    s.eye(10, 15, 2, 2).eye(20, 15, 2, 2).line(
        &[(13, 30), (15, 31), (17, 30), (19, 31), (14, 33), (18, 33)],
        feather,
    );
    s
}

fn bunny() -> Sprite {
    let white = hex(0xF7F3FF);
    let pink = hex(0xFF9EC8);
    let mut s = Sprite::new(hex(0x9A6BF2));
    s.sparkles = vec![(4, 5), (28, 6), (4, 25), (28, 24)];
    s.add(part(ell(16.0, 33.0, 12.0, 6.5), white))
        .add(part(ell(11.0, 8.5, 3.2, 7.5), white))
        .add(part(ell(21.0, 8.5, 3.2, 7.5), white))
        .add(flat(ell(11.0, 9.0, 1.4, 5.5), pink))
        .add(flat(ell(21.0, 9.0, 1.4, 5.5), pink))
        .add(part(ell(16.0, 20.0, 10.5, 8.5), white));
    s.eye(11, 17, 2, 3)
        .eye(19, 17, 2, 3)
        .block(15, 21, 2, 1, hex(0xFF5FA0))
        .line(&[(14, 22), (17, 22)], hex(0x6A5A9A))
        .block(15, 23, 2, 2, WHITE)
        .line(&[(15, 25), (16, 25)], hex(0xCFC6EA))
        .line(&[(9, 21), (10, 21), (21, 21), (22, 21)], BLUSH);
    s
}

fn robot() -> Sprite {
    let steel = hex(0xAEB8CC);
    let cyan = hex(0x3FE0FF);
    let mut s = Sprite::new(hex(0xFF6A1A));
    s.add(part(rect(5.0, 27.0, 27.0, 33.0), steel))
        .add(part(rect(15.0, 2.5, 17.0, 8.0), steel))
        .add(part(circle(16.0, 3.0, 2.3), hex(0xFF4F4F)))
        .add(part(rect(2.5, 13.5, 6.0, 20.5), steel))
        .add(part(rect(26.0, 13.5, 29.5, 20.5), steel))
        .add(part(rect(5.5, 7.5, 26.5, 26.5), steel))
        .add(flat(rect(8.5, 11.5, 23.5, 21.5), hex(0x1D1A3A)));
    s.block(11, 14, 3, 3, cyan)
        .block(18, 14, 3, 3, cyan)
        .px(11, 14, WHITE)
        .px(18, 14, WHITE)
        .line(
            &[
                (11, 18),
                (12, 19),
                (13, 19),
                (14, 19),
                (15, 19),
                (16, 19),
                (17, 19),
                (18, 19),
                (19, 19),
                (20, 18),
            ],
            cyan,
        )
        .px(8, 24, hex(0x5A6478))
        .px(23, 24, hex(0x5A6478))
        .px(12, 29, hex(0xFF4F4F))
        .px(15, 29, hex(0xFFD84A))
        .px(18, 29, hex(0x3FD67A));
    s
}

fn dino() -> Sprite {
    let green = hex(0x2FC48A);
    let spikes = hex(0xFF8A3A);
    let mut s = Sprite::new(hex(0x3FE0FF));
    s.add(part(ell(16.0, 33.0, 12.0, 6.5), green))
        .add(patch(ell(16.0, 34.0, 5.5, 5.0), hex(0xF7E08A)))
        .add(part(tri((8.5, 9.0), (11.5, 2.5), (15.0, 8.5)), spikes))
        .add(part(tri((13.5, 8.0), (16.5, 1.0), (19.5, 8.0)), spikes))
        .add(part(tri((18.5, 8.5), (22.0, 2.5), (24.5, 9.5)), spikes))
        .add(part(ell(16.0, 18.0, 11.5, 10.0), green))
        .add(patch(ell(16.0, 22.5, 7.0, 4.0), hex(0x7CEAB8)));
    let mouth = hex(0x14543A);
    s.eye(10, 14, 2, 3)
        .eye(20, 14, 2, 3)
        .px(14, 21, mouth)
        .px(17, 21, mouth)
        .line(
            &[
                (12, 24),
                (13, 25),
                (14, 25),
                (15, 25),
                (16, 25),
                (17, 25),
                (18, 25),
                (19, 24),
            ],
            mouth,
        )
        .line(&[(8, 20), (9, 20), (22, 20), (23, 20)], BLUSH);
    s
}

/// A pal: its name and how to draw it.
type Pal = (&'static str, fn() -> Sprite);

fn main() {
    let dir = Path::new("assets/avatars");
    std::fs::create_dir_all(dir).expect("create assets/avatars");
    let pals: [Pal; 8] = [
        ("fox", fox),
        ("cat", cat),
        ("bear", bear),
        ("frog", frog),
        ("owl", owl),
        ("bunny", bunny),
        ("robot", robot),
        ("dino", dino),
    ];
    for (name, make) in pals {
        let path = dir.join(format!("pixel-{name}.png"));
        make().save(&path);
        println!("wrote {}", path.display());
    }
}
