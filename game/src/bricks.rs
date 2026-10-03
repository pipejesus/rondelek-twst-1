//! Brick models built in code: a grid of coloured bricks, turned into one
//! mesh and lit by Lam::pula, so they sit in the same world as the kid's
//! flat-draw drawings (the clouds, the sun) and the brick water.
//!
//! A [`Grid`] holds a palette colour per cell (0 = empty), and, if it wants,
//! how see-through each brick is. [`BrickModel::build`] walls in every filled
//! cell — one quad per face that borders an empty or see-through one — and
//! points each quad's texcoords at its colour (and opacity) in a small palette
//! texture, the way a flat-draw export points into its atlas, where a layer's
//! opacity is baked in too. Lam::pula then does the rest: the glass light, the
//! brick edges and the corner glints, found on the grid through
//! [`BrickModel::lattice`].
//!
//! The runner's ground and obstacles are built from these (see `props.rs`).

use super::lampula::Lampula;
use super::models::PaletteMaterial;
use raylib::prelude::*;

/// A box of brick cells, each a palette colour: `0` is empty, `n` is the
/// palette's `n`-th colour (1-based). x runs along, y up, z toward the camera.
#[derive(Clone, Debug)]
pub struct Grid {
    pub w: usize,
    pub h: usize,
    pub d: usize,
    cells: Vec<u8>,
    /// Each cell's opacity, 0 (clear) to [`SOLID`]; `None` until a brick is
    /// first made see-through, so a solid grid carries no second layer.
    alpha: Option<Vec<u8>>,
}

/// A brick's opacity when nothing else is said: it hides what is behind it.
pub const SOLID: u8 = 255;

impl Grid {
    pub fn new(w: usize, h: usize, d: usize) -> Self {
        Self {
            w,
            h,
            d,
            cells: vec![0; w * h * d],
            alpha: None,
        }
    }

    fn index(&self, x: usize, y: usize, z: usize) -> Option<usize> {
        (x < self.w && y < self.h && z < self.d).then(|| (z * self.h + y) * self.w + x)
    }

    /// The colour at a cell; 0 (empty) outside the grid.
    pub fn get(&self, x: isize, y: isize, z: isize) -> u8 {
        if x < 0 || y < 0 || z < 0 {
            return 0;
        }
        self.index(x as usize, y as usize, z as usize)
            .map_or(0, |i| self.cells[i])
    }

    /// Set one cell (a cell outside the grid is ignored).
    pub fn set(&mut self, x: usize, y: usize, z: usize, colour: u8) {
        if let Some(i) = self.index(x, y, z) {
            self.cells[i] = colour;
        }
    }

    /// Fill a box of cells, `from` inclusive to `to` exclusive.
    pub fn fill(&mut self, from: [usize; 3], to: [usize; 3], colour: u8) {
        for z in from[2]..to[2] {
            for y in from[1]..to[1] {
                for x in from[0]..to[0] {
                    self.set(x, y, z, colour);
                }
            }
        }
    }

    /// A cell's opacity; [`SOLID`] outside the grid and wherever none was set.
    pub fn alpha(&self, x: isize, y: isize, z: isize) -> u8 {
        if x < 0 || y < 0 || z < 0 {
            return SOLID;
        }
        let i = self.index(x as usize, y as usize, z as usize);
        self.alpha
            .as_ref()
            .zip(i)
            .map_or(SOLID, |(alpha, i)| alpha[i])
    }

    /// Make one cell's brick see-through by `alpha` (0 clear, [`SOLID`] not at
    /// all). Kept apart from its colour: refilling the cell leaves it be.
    pub fn set_alpha(&mut self, x: usize, y: usize, z: usize, alpha: u8) {
        let Some(i) = self.index(x, y, z) else {
            return;
        };
        if alpha == SOLID && self.alpha.is_none() {
            return;
        }
        let n = self.cells.len();
        self.alpha.get_or_insert_with(|| vec![SOLID; n])[i] = alpha;
    }

    /// Whether the brick in a cell covers the face its neighbour (`alpha`
    /// opaque) turns toward it, so that face is left out. A solid brick
    /// covers any face. A see-through one covers another see-through one's:
    /// touching glass bricks are one body, seen by its skin, not a stack of
    /// panes (which would add up to solid again). Only a solid brick's face
    /// shows through it.
    fn covers(&self, x: isize, y: isize, z: isize, alpha: u8) -> bool {
        self.get(x, y, z) != 0 && (self.alpha(x, y, z) == SOLID || alpha != SOLID)
    }

    /// Every opacity a brick of the grid has, in order: the palette
    /// texture's rows. Just [`SOLID`] for a grid that hides all it covers.
    fn alphas(&self) -> Vec<u8> {
        let Some(alpha) = &self.alpha else {
            return vec![SOLID];
        };
        let mut used = [false; 256];
        for (&c, &a) in self.cells.iter().zip(alpha) {
            if c != 0 {
                used[a as usize] = true;
            }
        }
        (0..=255u8).filter(|&a| used[a as usize]).collect()
    }
}

/// How [`BrickModel::build`] walls the grid in.
#[derive(Clone, Copy, Default)]
pub struct Build {
    /// The grid repeats along x (a tile laid end to end): the faces between
    /// its last column and its first, which a neighbouring copy covers, are
    /// left out.
    pub wrap_x: bool,
    /// Leave out the bottoms: for something the camera always sees from
    /// above (the ground), they are never seen.
    pub open_below: bool,
    /// Leave out the backs: the camera is in front of everything, so a back
    /// is only ever seen through a hole (the ground, the far planes).
    pub open_behind: bool,
}

/// The six face directions (each the step to the neighbour it borders, and
/// its normal): top, front, the sides, then bottom and back.
const FACES: [[isize; 3]; 6] = [
    [0, 1, 0],
    [0, 0, 1],
    [1, 0, 0],
    [-1, 0, 0],
    [0, -1, 0],
    [0, 0, -1],
];

/// A grid's faces as plain vectors, testable without a GPU. Model space: one
/// brick = `cell` units, x and z centred on 0, y = 0 at the grid's bottom
/// (flat-draw's bottom-centre pivot).
struct Faces {
    vertices: Vec<Vector3>,
    texcoords: Vec<Vector2>,
    normals: Vec<Vector3>,
    /// Into `vertices`, all of them: [`Faces::meshes`] splits them into runs
    /// raylib's 16-bit indices can hold.
    indices: Vec<u32>,
    /// The palette texture's rows: one opacity each (see [`Grid::alphas`]).
    alphas: Vec<u8>,
}

impl Faces {
    /// Every face that can be seen, back to front: the camera is always in
    /// front, so drawn in this order a see-through brick blends over what
    /// stands behind it, never the other way round.
    fn new(grid: &Grid, cell: f32, colours: usize, how: Build) -> Self {
        let alphas = grid.alphas();
        let x0 = -(grid.w as f32) * cell / 2.0;
        let z0 = -(grid.d as f32) * cell / 2.0;
        let (w, h, d) = (grid.w as isize, grid.h as isize, grid.d as isize);
        // Each face as (depth of its middle, low corner, normal, texcoords).
        let mut seen = Vec::new();
        for z in 0..d {
            for y in 0..h {
                for x in 0..w {
                    let c = grid.get(x, y, z);
                    if c == 0 {
                        continue;
                    }
                    // Its colour's texel centre, in the row of its opacity.
                    let alpha = grid.alpha(x, y, z);
                    let row = alphas.binary_search(&alpha).unwrap_or(0);
                    let uv = Vector2::new(
                        (c as f32 - 0.5) / colours.max(1) as f32,
                        (row as f32 + 0.5) / alphas.len() as f32,
                    );
                    let lo =
                        Vector3::new(x0 + x as f32 * cell, y as f32 * cell, z0 + z as f32 * cell);
                    for (k, step) in FACES.iter().enumerate() {
                        if (how.open_below && k == 4) || (how.open_behind && k == 5) {
                            continue;
                        }
                        let mut nx = x + step[0];
                        if how.wrap_x {
                            nx = nx.rem_euclid(w);
                        }
                        if grid.covers(nx, y + step[1], z + step[2], alpha) {
                            continue; // covered by its neighbour
                        }
                        let n = Vector3::new(step[0] as f32, step[1] as f32, step[2] as f32);
                        seen.push((lo.z + cell * (1.0 + n.z) / 2.0, lo, n, uv));
                    }
                }
            }
        }
        // The grid was walked back to front already, slice by slice; this
        // puts each slice's fronts after its tops and sides, and keeps the
        // walk's order otherwise.
        seen.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut f = Self {
            vertices: Vec::with_capacity(seen.len() * 4),
            texcoords: Vec::with_capacity(seen.len() * 4),
            normals: Vec::with_capacity(seen.len() * 4),
            indices: Vec::with_capacity(seen.len() * 6),
            alphas,
        };
        for (_, lo, n, uv) in seen {
            f.quad(lo, cell, n, uv);
        }
        f
    }

    /// One face of the brick whose low corner is `lo`, corners counter-
    /// clockwise seen from outside (raylib's front face).
    fn quad(&mut self, lo: Vector3, cell: f32, n: Vector3, uv: Vector2) {
        let hi = lo + Vector3::new(cell, cell, cell);
        let v = Vector3::new;
        let corners = if n.y > 0.5 {
            [
                v(lo.x, hi.y, lo.z),
                v(lo.x, hi.y, hi.z),
                v(hi.x, hi.y, hi.z),
                v(hi.x, hi.y, lo.z),
            ]
        } else if n.y < -0.5 {
            [
                v(lo.x, lo.y, lo.z),
                v(hi.x, lo.y, lo.z),
                v(hi.x, lo.y, hi.z),
                v(lo.x, lo.y, hi.z),
            ]
        } else if n.z > 0.5 {
            [
                v(lo.x, hi.y, hi.z),
                v(lo.x, lo.y, hi.z),
                v(hi.x, lo.y, hi.z),
                v(hi.x, hi.y, hi.z),
            ]
        } else if n.z < -0.5 {
            [
                v(hi.x, hi.y, lo.z),
                v(hi.x, lo.y, lo.z),
                v(lo.x, lo.y, lo.z),
                v(lo.x, hi.y, lo.z),
            ]
        } else if n.x > 0.5 {
            [
                v(hi.x, hi.y, hi.z),
                v(hi.x, lo.y, hi.z),
                v(hi.x, lo.y, lo.z),
                v(hi.x, hi.y, lo.z),
            ]
        } else {
            [
                v(lo.x, hi.y, lo.z),
                v(lo.x, lo.y, lo.z),
                v(lo.x, lo.y, hi.z),
                v(lo.x, hi.y, hi.z),
            ]
        };
        let base = self.vertices.len() as u32;
        self.vertices.extend(corners);
        self.texcoords.extend([uv; 4]);
        self.normals.extend([n; 4]);
        self.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
    }
}

/// The most faces one mesh can hold: raylib's indices are 16-bit, and every
/// face has four vertices of its own. (raylib-rs wants the vertex *count* to
/// fit 16 bits too, so a mesh holds 65535 vertices at most, not 65536.)
const FACES_PER_MESH: usize = u16::MAX as usize / 4;

impl Faces {
    /// The faces in runs that each fit one mesh: each run's first vertex and
    /// vertex count, and its indices, counted from that first vertex.
    fn meshes(&self) -> Vec<(usize, usize, Vec<u16>)> {
        let quads = self.vertices.len() / 4;
        (0..quads)
            .step_by(FACES_PER_MESH)
            .map(|first| {
                let last = (first + FACES_PER_MESH).min(quads);
                let indices = self.indices[first * 6..last * 6]
                    .iter()
                    .map(|&i| (i - first as u32 * 4) as u16)
                    .collect();
                (first * 4, (last - first) * 4, indices)
            })
            .collect()
    }
}

/// The RGBA palette texture's pixels: the colours along, once per opacity
/// (a solid grid's palette is one row).
fn palette_pixels(colours: &[[u8; 3]], alphas: &[u8]) -> Vec<u8> {
    alphas
        .iter()
        .flat_map(|&a| colours.iter().flat_map(move |c| [c[0], c[1], c[2], a]))
        .collect()
}

/// A grid, built into a mesh and ready to draw any number of times. (Into
/// several, if it has more faces than one mesh can hold: a draw call each.)
pub struct BrickModel {
    meshes: Vec<Mesh>,
    palette: PaletteMaterial,
    /// Model space → the brick lattice: every brick corner on whole numbers.
    pub lattice: Matrix,
    /// Model-space box: x, z centred on 0, y from 0 up.
    pub min: Vector3,
    pub max: Vector3,
}

impl BrickModel {
    /// Build `grid` with bricks `cell` units big, its colours from `palette`
    /// (grid colour `n` = `palette[n - 1]`). `None` (logged) if the grid is
    /// empty, or a mesh or the palette won't load.
    pub fn build(
        thread: &RaylibThread,
        name: &str,
        grid: &Grid,
        palette: &[[u8; 3]],
        cell: f32,
        how: Build,
    ) -> Option<Self> {
        let faces = Faces::new(grid, cell, palette.len(), how);
        if faces.vertices.is_empty() {
            eprintln!("{name}: no bricks, no mesh");
            return None;
        }
        let mut meshes = Vec::new();
        for (first, count, indices) in faces.meshes() {
            let run = first..first + count;
            match Mesh::gen_mesh(&faces.vertices[run.clone()], &faces.texcoords[run.clone()])
                .normals(&faces.normals[run])
                .indices(&indices)
                .build(thread)
            {
                Ok(m) => meshes.push(m),
                Err(e) => {
                    eprintln!("{name} mesh: {e}");
                    return None;
                }
            }
        }
        // Snapped, not blended: each texel is one brick colour.
        let palette = PaletteMaterial::new(
            thread,
            palette.len() as i32,
            faces.alphas.len() as i32,
            &palette_pixels(palette, &faces.alphas),
            TextureFilter::TEXTURE_FILTER_POINT,
        )?;
        let (hw, hd) = (grid.w as f32 * cell / 2.0, grid.d as f32 * cell / 2.0);
        Some(Self {
            meshes,
            palette,
            lattice: lattice(grid, cell),
            min: Vector3::new(-hw, 0.0, -hd),
            max: Vector3::new(hw, grid.h as f32 * cell, hd),
        })
    }

    /// Draw one instance through Lam::pula, its lamps standing round `lamps`
    /// (world centre and half-extent; see [`Lampula::draw_mesh`]). Without a
    /// glass (it didn't compile) the bricks draw unlit, in their flat colours.
    pub fn draw(
        &self,
        d: &mut impl RaylibDraw3D,
        glass: Option<&mut Lampula>,
        transform: Matrix,
        lamps: (Vector3, Vector3),
    ) {
        let mut glass = glass;
        for mesh in &self.meshes {
            match glass.as_deref_mut() {
                Some(g) => g.draw_mesh(
                    d,
                    mesh,
                    self.palette.material(),
                    transform,
                    self.lattice,
                    lamps,
                ),
                None => d.draw_mesh(mesh, self.palette.material().clone(), transform),
            }
        }
    }
}

/// Model space → brick lattice: shift the grid's low corner to the origin,
/// then one brick per unit.
fn lattice(grid: &Grid, cell: f32) -> Matrix {
    Matrix {
        m0: 1.0 / cell,
        m5: 1.0 / cell,
        m10: 1.0 / cell,
        m12: grid.w as f32 / 2.0,
        m14: grid.d as f32 / 2.0,
        m15: 1.0,
        ..Matrix::default()
    }
}

/// How many vertices `grid` builds into (for tests that hold a prop to
/// raylib's 16-bit indices without a GPU).
#[cfg(test)]
pub(crate) fn vertex_count(grid: &Grid, how: Build) -> usize {
    Faces::new(grid, 1.0, 1, how).vertices.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, d: usize) -> Grid {
        let mut g = Grid::new(w, h, d);
        g.fill([0, 0, 0], [w, h, d], 1);
        g
    }

    #[test]
    fn a_lone_brick_has_six_faces_a_block_only_its_skin() {
        let f = Faces::new(&solid(1, 1, 1), 1.0, 1, Build::default());
        assert_eq!(f.vertices.len(), 6 * 4);
        // A 3×2×2 block: 2·(3·2 + 3·2 + 2·2) = 32 outer faces, none inside.
        let f = Faces::new(&solid(3, 2, 2), 1.0, 1, Build::default());
        assert_eq!(f.indices.len() / 6, 32);
    }

    #[test]
    fn wrapping_and_opening_leave_out_what_is_never_seen() {
        let how = Build {
            wrap_x: true,
            open_below: true,
            open_behind: true,
        };
        // A 4×1×2 slab tiled along x: only its tops and fronts remain.
        let f = Faces::new(&solid(4, 1, 2), 1.0, 1, how);
        assert_eq!(f.indices.len() / 6, 4 * 2 + 4);
    }

    #[test]
    fn every_face_turns_outward() {
        let mut g = solid(3, 3, 3);
        g.set(1, 2, 1, 0); // a dent in the top
        g.set(5, 5, 5, 1); // outside: ignored
        let f = Faces::new(&g, 0.5, 1, Build::default());
        for tri in f.indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| f.vertices[tri[k] as usize]);
            let n = f.normals[tri[0] as usize];
            assert!((b - a).cross(c - a).dot(n) > 0.0, "{tri:?}");
        }
    }

    #[test]
    fn corners_sit_on_the_lattice_and_colours_on_their_texels() {
        let mut g = Grid::new(3, 2, 5); // odd sizes: centred on half bricks
        g.fill([0, 0, 0], [3, 1, 5], 2);
        g.set(1, 1, 2, 3);
        let cell = 0.125;
        let f = Faces::new(&g, cell, 3, Build::default());
        let m = lattice(&g, cell);
        for v in &f.vertices {
            let l = v.transform(m);
            for k in [l.x, l.y, l.z] {
                assert!((k - k.round()).abs() < 1e-4, "{v:?} → {l:?}");
                assert!(k > -1e-4, "the grid's low corner is the lattice origin");
            }
        }
        // Colour n sits at the middle of texel n - 1.
        assert!(f.texcoords.iter().any(|t| (t.x - 1.5 / 3.0).abs() < 1e-6));
        assert!(f.texcoords.iter().any(|t| (t.x - 2.5 / 3.0).abs() < 1e-6));
    }

    #[test]
    fn a_big_grid_splits_into_meshes_raylib_can_index() {
        // 128 × 4 × 40 lone bricks, every other one: 10240 bricks, six faces
        // each, too many for one mesh.
        let mut g = Grid::new(128, 4, 40);
        for z in (0..40).step_by(2) {
            for y in (0..4).step_by(2) {
                for x in (0..128).step_by(2) {
                    g.set(x, y, z, 1);
                }
            }
        }
        let f = Faces::new(&g, 1.0, 1, Build::default());
        let quads = f.vertices.len() / 4;
        assert_eq!(quads, 64 * 2 * 20 * 6);
        let meshes = f.meshes();
        assert_eq!(meshes.len(), quads.div_ceil(FACES_PER_MESH));
        let mut next = 0;
        for (first, count, indices) in &meshes {
            assert_eq!(*first, next, "runs follow on");
            next += count;
            assert!(*count <= u16::MAX as usize + 1);
            // Every index lands inside its own run, on the same vertex the
            // whole-grid index pointed at.
            assert!(indices.iter().all(|&i| (i as usize) < *count));
            let whole = &f.indices[first / 4 * 6..(first + count) / 4 * 6];
            for (&i, &w) in indices.iter().zip(whole) {
                assert_eq!(first + i as usize, w as usize);
            }
        }
        assert_eq!(next, f.vertices.len());
    }

    #[test]
    fn the_grid_answers_empty_outside() {
        let g = solid(2, 2, 2);
        assert_eq!(g.get(-1, 0, 0), 0);
        assert_eq!(g.get(0, 2, 0), 0);
        assert_eq!(g.get(1, 1, 1), 1);
        assert_eq!(g.alpha(-1, 0, 0), SOLID);
        assert_eq!(g.alpha(1, 1, 1), SOLID);
    }

    #[test]
    fn a_see_through_brick_shows_the_face_behind_it() {
        let faces = |g: &Grid| Faces::new(g, 1.0, 1, Build::default()).indices.len() / 6;
        // Two bricks, one behind the other: 10 faces between them.
        let mut g = solid(1, 1, 2);
        assert_eq!(faces(&g), 10);
        // The front one see-through: the back one's front shows through it
        // (its own back still hides against the solid one).
        g.set_alpha(0, 0, 1, 128);
        assert_eq!(faces(&g), 11);
        // Both: one body of glass again, seen by its skin.
        g.set_alpha(0, 0, 0, 200);
        assert_eq!(faces(&g), 10);
    }

    #[test]
    fn each_opacity_is_a_row_of_the_palette() {
        let mut g = Grid::new(3, 1, 1);
        g.set(0, 0, 0, 1);
        g.set(1, 0, 0, 2);
        g.set_alpha(1, 0, 0, 64);
        // An empty cell's opacity is no brick's: it makes no row.
        g.set_alpha(2, 0, 0, 7);
        assert_eq!(g.alphas(), vec![64, SOLID]);
        let f = Faces::new(&g, 1.0, 2, Build::default());
        for (q, t) in f.vertices.chunks(4).zip(f.texcoords.chunks(4)) {
            // Brick 1 (colour 2, see-through) in the first row; brick 0
            // (colour 1, solid; x -1.5..-0.5) in the second.
            let mid = q.iter().map(|v| v.x).sum::<f32>() / 4.0;
            let want = if mid > -0.25 {
                (0.75, 0.25)
            } else {
                (0.25, 0.75)
            };
            let t = t[0];
            assert!(
                (t.x - want.0).abs() < 1e-6 && (t.y - want.1).abs() < 1e-6,
                "{q:?} {t:?}"
            );
        }
        let px = palette_pixels(&[[1, 2, 3], [4, 5, 6]], &f.alphas);
        assert_eq!(px, [1, 2, 3, 64, 4, 5, 6, 64, 1, 2, 3, 255, 4, 5, 6, 255]);
        // A solid grid keeps its one-row palette.
        assert_eq!(solid(2, 2, 2).alphas(), vec![SOLID]);
    }

    #[test]
    fn faces_come_back_to_front() {
        let mut g = Grid::new(6, 4, 6);
        for z in 0..6 {
            for y in 0..4 {
                for x in 0..6 {
                    if (x * 7 + y * 3 + z * 5) % 4 != 0 {
                        g.set(x, y, z, 1);
                    }
                }
            }
        }
        g.set_alpha(2, 1, 3, 100);
        let f = Faces::new(&g, 1.0, 1, Build::default());
        let mid = |q: &[Vector3]| q.iter().map(|v| v.z).sum::<f32>() / 4.0;
        let depths: Vec<f32> = f.vertices.chunks(4).map(mid).collect();
        assert!(depths.windows(2).all(|p| p[0] <= p[1]), "{depths:?}");
    }
}
