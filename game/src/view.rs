//! What the camera sees, as plain geometry: where the picture's edges fall in
//! the world. A game uses it to bring things into the picture and take them
//! out of it where nobody can see — a thing is placed just past the right
//! edge, fully out of sight, and dropped only once it is fully past the left
//! one — whatever the window's shape, and wherever the camera has drifted to.
//!
//! No GPU here (raylib's vectors are plain maths), so it is unit-tested.

use raylib::prelude::*;

/// How far out of sight, world units, a thing comes in and goes: a little
/// room, so nothing is ever caught on an edge.
pub const MARGIN: f32 = 0.5;

/// A box in the world: what a thing takes up, everything it carries included
/// (an obstacle's tablet, the suns over a ledge). x and y as ranges, z the
/// range of depths it stands at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: (f32, f32),
    pub y: (f32, f32),
    pub z: (f32, f32),
}

impl Bounds {
    /// The box round both.
    pub fn union(self, o: Bounds) -> Bounds {
        let lo = |a: (f32, f32), b: (f32, f32)| (a.0.min(b.0), a.1.max(b.1));
        Bounds {
            x: lo(self.x, o.x),
            y: lo(self.y, o.y),
            z: lo(self.z, o.z),
        }
    }

    /// The same box, moved `dx` along.
    pub fn shifted(self, dx: f32) -> Bounds {
        Bounds {
            x: (self.x.0 + dx, self.x.1 + dx),
            ..self
        }
    }
}

/// A perspective camera, as raylib's `Camera3D` (up is world up), and the
/// shape of the picture it draws into.
#[derive(Clone, Copy, Debug)]
pub struct Eye {
    pub position: Vector3,
    pub target: Vector3,
    /// Vertical field of view, degrees.
    pub fovy: f32,
    /// The picture's width over its height.
    pub aspect: f32,
}

impl Eye {
    pub fn camera(&self) -> Camera3D {
        Camera3D::perspective(self.position, self.target, Vector3::Y, self.fovy)
    }

    /// The same eye, moved `dy` up: both where it stands and where it looks.
    pub fn lifted(self, dy: f32) -> Eye {
        let up = Vector3::new(0.0, dy, 0.0);
        Eye {
            position: self.position + up,
            target: self.target + up,
            ..self
        }
    }

    /// The ray through the picture at `(sx, sy)`, each −1 (left, bottom) to
    /// 1 (right, top).
    fn ray(&self, sx: f32, sy: f32) -> Vector3 {
        let f = (self.target - self.position).normalize();
        let r = f.cross(Vector3::Y).normalize();
        let u = r.cross(f);
        let half_h = (self.fovy.to_radians() / 2.0).tan();
        f + r * (sx * half_h * self.aspect) + u * (sy * half_h)
    }

    /// Where the picture's left (`side` −1) or right (1) edge crosses the
    /// plane at depth `z`, as a line `x = a + b·y`. (The edges lean: the
    /// camera looks down a little and in from the side.)
    fn edge(&self, side: f32, z: f32) -> (f32, f32) {
        let at = |sy: f32| {
            let d = self.ray(side, sy);
            let t = (z - self.position.z) / d.z;
            self.position + d * t
        };
        let (p, q) = (at(-1.0), at(1.0));
        let b = (q.x - p.x) / (q.y - p.y);
        (p.x - b * p.y, b)
    }

    /// How far right the picture reaches, at any of the heights `y` and
    /// depths `z` of a box (the farthest of them).
    pub fn right_edge(&self, y: (f32, f32), z: (f32, f32)) -> f32 {
        self.corners(1.0, y, z).fold(f32::MIN, f32::max)
    }

    /// How far left the picture reaches, likewise.
    pub fn left_edge(&self, y: (f32, f32), z: (f32, f32)) -> f32 {
        self.corners(-1.0, y, z).fold(f32::MAX, f32::min)
    }

    fn corners(&self, side: f32, y: (f32, f32), z: (f32, f32)) -> impl Iterator<Item = f32> {
        [z.0, z.1].into_iter().flat_map(move |z| {
            let (a, b) = self.edge(side, z);
            [y.0, y.1].map(|y| a + b * y)
        })
    }

    /// Where along x to put a thing — the x its `shape` is drawn round —
    /// so that all of it is just out of sight past the right edge.
    pub fn entry(&self, shape: Bounds) -> f32 {
        self.right_edge(shape.y, shape.z) + MARGIN - shape.x.0
    }

    /// Whether a thing is all the way out past the left edge, so it can go.
    pub fn gone_left(&self, b: Bounds) -> bool {
        b.x.1 < self.left_edge(b.y, b.z) - MARGIN
    }

    /// Whether a thing is all the way out past the right edge.
    pub fn gone_right(&self, b: Bounds) -> bool {
        b.x.0 > self.right_edge(b.y, b.z) + MARGIN
    }

    /// The x range the picture spans at depth `z` over the heights `y`, for
    /// laying a plane's tiles across it.
    pub fn span(&self, z: f32, y: (f32, f32)) -> (f32, f32) {
        (self.left_edge(y, (z, z)), self.right_edge(y, (z, z)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The runner's camera, in a picture of the given shape.
    fn eye(aspect: f32) -> Eye {
        Eye {
            position: Vector3::new(0.9, 2.2, 12.5),
            target: Vector3::new(0.0, 1.0, 0.0),
            fovy: 45.0,
            aspect,
        }
    }

    /// Where a world point lands across the picture: −1 left edge, 1 right.
    fn screen_x(e: &Eye, p: Vector3) -> f32 {
        let f = (e.target - e.position).normalize();
        let r = f.cross(Vector3::Y).normalize();
        let v = p - e.position;
        v.dot(r) / v.dot(f) / ((e.fovy.to_radians() / 2.0).tan() * e.aspect)
    }

    #[test]
    fn the_edges_are_where_the_picture_ends() {
        for aspect in [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0] {
            let e = eye(aspect);
            for z in [-14.0, -5.0, 0.0, 1.5] {
                for y in [-1.0, 0.0, 3.0, 8.0] {
                    let (l, r) = e.span(z, (y, y));
                    let at = |x| screen_x(&e, Vector3::new(x, y, z));
                    assert!((at(l) + 1.0).abs() < 1e-3, "left at {aspect} {z} {y}");
                    assert!((at(r) - 1.0).abs() < 1e-3, "right at {aspect} {z} {y}");
                }
            }
        }
    }

    #[test]
    fn the_runners_old_spawn_and_drop_points_were_in_sight() {
        // Things came in at logical x 1400 (LW + 120: world x 7.6) and were
        // dropped at x -200 (world -8.4): both inside a 16:9 picture at the
        // hero's depth, which is how they popped up and vanished out of
        // nowhere.
        let e = eye(16.0 / 9.0);
        let (l, r) = e.span(0.0, (0.0, 3.0));
        assert!(r > 7.6 + 0.8 && l < -8.4 - 0.8, "{l}..{r}");
    }

    #[test]
    fn a_thing_enters_and_leaves_out_of_sight() {
        let e = eye(21.0 / 9.0);
        let shape = Bounds {
            x: (-0.5, 0.5),
            y: (0.0, 2.5),
            z: (-1.0, 0.5),
        };
        let placed = shape.shifted(e.entry(shape));
        // All of it just out of sight, by the margin, at its nearest corner.
        let out = placed.x.0 - e.right_edge(shape.y, shape.z);
        assert!((out - MARGIN).abs() < 1e-4, "{out}");
        assert!(!e.gone_right(placed), "not so far out it would be dropped");
        // Kept while any of it shows, dropped once all of it is past.
        let (l, _) = e.span(0.0, (0.0, 2.5));
        assert!(!e.gone_left(shape.shifted(l)));
        assert!(e.gone_left(shape.shifted(l - 30.0)));
    }

    #[test]
    fn lifting_moves_the_eye_and_where_it_looks() {
        let e = eye(16.0 / 9.0).lifted(0.75);
        assert_eq!(e.position.y, 2.95);
        assert_eq!(e.target.y, 1.75);
        let b = Bounds {
            x: (0.0, 1.0),
            y: (0.0, 1.0),
            z: (0.0, 0.0),
        };
        assert_eq!(b.union(b.shifted(2.0)).x, (0.0, 3.0));
    }
}
