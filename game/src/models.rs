//! flat-draw scenery: the kid's own drawings, exported to GLB by our
//! `flat-draw` tool (2D pixel drawing → extruded 3D layers) and drawn here as
//! chunky world props.
//!
//! Two things this module does that plain `LoadModel` + `DrawModel` do not:
//!
//! * **Embedded, not shipped.** Like the fonts, translations and shaders, the
//!   GLB bytes live inside the binary (`include_bytes!`) — the app is a
//!   self-contained executable. raylib can only load a model from a *path*
//!   (there is no `LoadModelFromMemory`), so the bytes are spilled to a temp
//!   file for the duration of one `LoadModel` call and removed right after.
//!   The texture atlas travels inside the GLB, so that one file is enough.
//!
//! * **One mesh, one draw call.** flat-draw emits a mesh per drawn layer/part
//!   (3 for the cloud: back, middle and front), all sharing a single
//!   texture-atlas material. raylib does *not* batch or instance models on
//!   its own: `DrawModel` issues one `glDrawElements` per mesh per call, so a
//!   sky full of clouds would cost
//!   parts × instances draw calls. [`FlatModel::load`] concatenates the parts
//!   into one mesh (they share a material, so nothing is lost) and
//!   [`FlatModel::draw`] renders that with a per-instance transform — one call
//!   per prop, ~8x fewer. The vertex data is uploaded once and reused by every
//!   instance; only the model matrix changes.
//!
//! Beyond that, raylib has `DrawMeshInstanced` (one call for *all* instances),
//! but it needs an instancing-aware shader and only pays off in the hundreds —
//! a handful of clouds is nowhere near that.
//!
//! Pivot convention: flat-draw exports with `pivot: bottom-center`, so model
//! space runs y = 0 (bottom) upward and is centred on x. `pos` in
//! [`FlatModel::draw`] is therefore where the prop's *base* goes.

use raylib::ffi;
use raylib::prelude::*;
use std::rc::Rc;

/// A flat-draw prop, ready to draw many times: merged geometry plus the
/// material (texture atlas) that came with the GLB.
pub struct FlatModel {
    /// Kept alive purely to own the material: the atlas texture and its shader
    /// are freed when the model unloads, and `material` points into it.
    /// Shared by every part [`FlatModel::load_parts`] splits one file into.
    _model: Rc<Model>,
    /// Borrowed view of `_model`'s single material — safe as long as the two
    /// live and die together, which they do (same struct).
    material: WeakMaterial,
    /// All parts of the drawing concatenated into one uploaded mesh.
    mesh: Mesh,
    /// Model-space extent of the drawing (`y` = height above the pivot).
    pub size: Vector3,
    /// Model-space centre of the drawing's bounding box — the point to spin a
    /// prop about (the pivot sits at the bottom, and not every drawing fills
    /// its canvas down to the last row).
    pub center: Vector3,
    /// Model-space bounding box corners.
    pub min: Vector3,
    pub max: Vector3,
    /// The drawing's pixels per model unit (flat-draw's `pixelsPerUnit`),
    /// recovered from the geometry — see [`MeshData::pixel_size`].
    pub ppu: f32,
}

impl FlatModel {
    /// Load an embedded flat-draw GLB as one prop: every part merged into one
    /// mesh. `name` only labels the temp file and errors.
    pub fn load(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        name: &str,
        glb: &[u8],
    ) -> anyhow::Result<Self> {
        let model = Rc::new(load_glb(rl, thread, name, glb)?);
        let all: Vec<usize> = (0..model.meshes().len()).collect();
        Self::from_parts(thread, &model, &all, name)
    }

    /// Load an embedded flat-draw GLB as **several props, one per named mesh**
    /// (e.g. `phoneme_a` … `phoneme_y` in `letters.glb`), in file order. Parts
    /// that share a name are merged. Every prop is still one mesh and one draw
    /// call, and all of them share the file's single texture atlas — so one
    /// file of many drawings costs no more to draw than separate files, and
    /// loads one texture instead of many.
    pub fn load_parts(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        name: &str,
        glb: &[u8],
    ) -> anyhow::Result<Vec<(String, Self)>> {
        let names = glb_mesh_names(glb)?;
        let model = Rc::new(load_glb(rl, thread, name, glb)?);
        anyhow::ensure!(
            names.len() == model.meshes().len(),
            "{name}.glb: raylib loaded {} meshes, the file lists {}",
            model.meshes().len(),
            names.len()
        );
        group_by_name(&names)
            .into_iter()
            .map(|(part, idx)| {
                let label = format!("{name}/{part}");
                Ok((part, Self::from_parts(thread, &model, &idx, &label)?))
            })
            .collect()
    }

    /// Merge the given meshes of `model` into one prop.
    fn from_parts(
        thread: &RaylibThread,
        model: &Rc<Model>,
        parts: &[usize],
        name: &str,
    ) -> anyhow::Result<Self> {
        let mut data = MeshData::default();
        for &i in parts {
            let part = &model.meshes()[i];
            data.push(
                part.vertices(),
                part.texcoords(),
                part.normals(),
                part.indices(),
            );
        }
        anyhow::ensure!(!data.vertices.is_empty(), "{name} has no geometry");
        let (min, max) = data.bounds();
        let size = max - min;
        let center = (min + max) * 0.5;
        let ppu = data.pixel_size().map_or(1.0, |px| 1.0 / px);
        let mesh = Mesh::gen_mesh(&data.vertices, &data.texcoords)
            .normals(&data.normals)
            .indices(&data.indices)
            .build(thread)?;

        // Careful: raylib's glTF loader keeps `materials[0]` as *its own*
        // default (plain white, no texture) and appends the file's materials
        // after it. The atlas is therefore at whatever index the mesh→material
        // table points to — taking the first material silently renders the
        // drawing blank white.
        let table = mesh_materials(model);
        let used: Vec<i32> = parts
            .iter()
            .filter_map(|&i| table.get(i).copied())
            .collect();
        let index = used.first().copied().unwrap_or(0).max(0) as usize;
        anyhow::ensure!(
            used.iter().all(|&m| m as usize == index),
            "{name} spreads its parts over several materials — merging them \
             into one mesh would lose all but one"
        );
        let material = model
            .materials()
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("{name} has no material {index}"))?
            .clone();

        Ok(Self {
            _model: Rc::clone(model),
            material,
            mesh,
            size,
            center,
            min,
            max,
            ppu,
        })
    }

    /// Draw one instance with its base at `pos`, uniformly scaled. Colour comes
    /// from the drawing's own texture atlas, so there is no tint parameter.
    pub fn draw(&self, d: &mut impl RaylibDraw3D, pos: Vector3, scale: f32) {
        // Same order raylib uses in DrawModelEx: scale, then translate.
        let transform = Matrix::scale(scale, scale, scale) * Matrix::translate(pos.x, pos.y, pos.z);
        self.draw_transformed(d, transform);
    }

    /// Draw one instance under an arbitrary model matrix (spins, tilts…). Still
    /// one draw call: only the matrix differs from [`FlatModel::draw`].
    pub fn draw_transformed(&self, d: &mut impl RaylibDraw3D, transform: Matrix) {
        d.draw_mesh(&self.mesh, self.material.clone(), transform);
    }

    /// Like [`FlatModel::draw_transformed`], but lit by `shader` instead of the
    /// material's own. The model's material is not touched — only copied for
    /// this one call — so the shader stays owned by the caller. (Setting it on
    /// the model would make raylib free it again when the model unloads.)
    pub fn draw_shaded(&self, d: &mut impl RaylibDraw3D, transform: Matrix, shader: &Shader) {
        draw_mesh_with(d, &self.mesh, &self.material, shader, transform);
    }

    /// The drawing's pixel lattice: model space → (x, y in canvas cells, z in
    /// pixels), the frame flat-draw's `brickMatrix` hands its brick shaders.
    ///
    /// Same shape as flat-draw's — scale by pixels-per-unit, rows flipped
    /// because canvas rows run down — with the pivot taken from the bounding
    /// box: the drawing's outer faces sit on cell boundaries, so its corner
    /// is a lattice point, and only the lattice's *fraction* is ever read.
    pub fn lattice(&self) -> Matrix {
        let s = self.ppu;
        Matrix {
            m0: s,
            m12: -s * self.min.x,
            m5: -s,
            m13: s * self.max.y,
            m10: s,
            m14: -s * self.min.z,
            m15: 1.0,
            ..Matrix::default()
        }
    }

    /// Uniform scale that makes the prop `height` world units tall.
    pub fn scale_for_height(&self, height: f32) -> f32 {
        if self.size.y > f32::EPSILON {
            height / self.size.y
        } else {
            1.0
        }
    }
}

/// Draw `mesh` through `shader` with `material`'s textures, leaving the material
/// itself untouched — the shader stays the caller's. (Putting a shader *on* a
/// model's material hands it to raylib, which frees it again when the model
/// unloads: a double free with our own `Shader`'s drop.)
pub fn draw_mesh_with(
    d: &mut impl RaylibDraw3D,
    mesh: impl AsRef<ffi::Mesh>,
    material: &WeakMaterial,
    shader: &Shader,
    transform: Matrix,
) {
    let mut raw: ffi::Material = *material.as_ref();
    raw.shader = *shader.as_ref();
    // SAFETY: a by-value copy of the material, used for one draw call.
    // `WeakMaterial` never unloads anything, and the textures and shader it
    // points to are owned by the caller, who outlives the call.
    let material = unsafe { WeakMaterial::from_raw(raw) };
    d.draw_mesh(mesh, material, transform);
}

/// Load a GLB through raylib. raylib can only load a model from a *path*, so
/// the embedded bytes are spilled to a temp file for the one call.
fn load_glb(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    name: &str,
    glb: &[u8],
) -> anyhow::Result<Model> {
    // raylib picks its loader from the extension, so the temp file must keep
    // `.glb`; the pid keeps two running games from racing each other.
    let path = std::env::temp_dir().join(format!("rondelek-{name}-{}.glb", std::process::id()));
    std::fs::write(&path, glb)?;
    let loaded = rl.load_model(
        thread,
        path.to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 temp path"))?,
    );
    let _ = std::fs::remove_file(&path);
    Ok(loaded?)
}

/// The name of every mesh raylib will load from a GLB, **in raylib's order**:
/// its glTF loader walks the nodes in file order and emits one mesh per
/// primitive of each node's mesh. The name is the node's (flat-draw names the
/// node that holds a layer's mesh, e.g. `phoneme_a`), else the mesh's.
fn glb_mesh_names(glb: &[u8]) -> anyhow::Result<Vec<String>> {
    anyhow::ensure!(glb.len() >= 20 && &glb[0..4] == b"glTF", "not a GLB");
    let len = u32::from_le_bytes(glb[12..16].try_into()?) as usize;
    anyhow::ensure!(
        &glb[16..20] == b"JSON" && glb.len() >= 20 + len,
        "GLB has no JSON chunk"
    );
    let json: serde_json::Value = serde_json::from_slice(&glb[20..20 + len])?;
    let meshes = json["meshes"].as_array().cloned().unwrap_or_default();
    let mut names = Vec::new();
    for (n, node) in json["nodes"].as_array().into_iter().flatten().enumerate() {
        let Some(m) = node["mesh"].as_u64() else {
            continue;
        };
        let mesh = meshes
            .get(m as usize)
            .ok_or_else(|| anyhow::anyhow!("node {n} points at missing mesh {m}"))?;
        let name = node["name"]
            .as_str()
            .or_else(|| mesh["name"].as_str())
            .map_or_else(|| format!("mesh{m}"), str::to_string);
        let prims = mesh["primitives"].as_array().map_or(0, Vec::len);
        names.extend(std::iter::repeat_n(name, prims));
    }
    Ok(names)
}

/// Mesh indices grouped by name, groups in order of first appearance.
fn group_by_name(names: &[String]) -> Vec<(String, Vec<usize>)> {
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (i, n) in names.iter().enumerate() {
        match groups.iter_mut().find(|(g, _)| g == n) {
            Some((_, idx)) => idx.push(i),
            None => groups.push((n.clone(), vec![i])),
        }
    }
    groups
}

/// Which material each of the model's meshes uses (raylib's `meshMaterial`
/// table, which raylib-rs does not expose).
fn mesh_materials(model: &Model) -> &[i32] {
    let raw: &ffi::Model = model.as_ref();
    if raw.meshMaterial.is_null() || raw.meshCount <= 0 {
        return &[];
    }
    // SAFETY: raylib allocates meshMaterial with exactly meshCount ints when
    // loading a model, and the borrow cannot outlive `model`, which owns it.
    unsafe { std::slice::from_raw_parts(raw.meshMaterial, raw.meshCount as usize) }
}

/// Vertex data being concatenated from several meshes. Kept as plain vectors so
/// the merge is testable without a GPU (or a window).
#[derive(Default)]
struct MeshData {
    vertices: Vec<Vector3>,
    texcoords: Vec<Vector2>,
    normals: Vec<Vector3>,
    indices: Vec<u16>,
}

impl MeshData {
    /// Append one part, re-basing its indices onto the vertices already here.
    /// Missing normals/texcoords are filled in so all attribute arrays keep the
    /// one-per-vertex length raylib requires.
    ///
    /// raylib meshes index with `u16`, so a merge past 65536 vertices would wrap
    /// — `MeshBuilder::build` rejects such a mesh outright, so that shows up as
    /// a load error rather than as scrambled geometry. (Our drawings are in the
    /// hundreds.)
    fn push(
        &mut self,
        vertices: &[Vector3],
        texcoords: &[Vector2],
        normals: &[Vector3],
        indices: &[u16],
    ) {
        let base = self.vertices.len() as u16;
        self.vertices.extend_from_slice(vertices);
        for i in 0..vertices.len() {
            self.texcoords
                .push(texcoords.get(i).copied().unwrap_or(Vector2::zero()));
            self.normals.push(
                normals
                    .get(i)
                    .copied()
                    .unwrap_or(Vector3::new(0.0, 0.0, 1.0)),
            );
        }
        if indices.is_empty() {
            // Unindexed part: triangles are consecutive vertices.
            self.indices
                .extend((0..vertices.len() as u16).map(|i| base + i));
        } else {
            self.indices.extend(indices.iter().map(|i| base + i));
        }
    }

    /// The size of one drawn pixel in model units, recovered from the geometry:
    /// every vertex x and y sits on the pixel lattice, so the smallest step
    /// between two distinct coordinates is one pixel. (flat-draw writes
    /// `pixelsPerUnit` into `*.meshes.json`, but newer exports come without
    /// one, and the game must not depend on a side file anyway.) `None` for a
    /// drawing too small to have a step.
    fn pixel_size(&self) -> Option<f32> {
        let mut coords: Vec<f32> = self.vertices.iter().flat_map(|v| [v.x, v.y]).collect();
        // x and y share one lattice spacing, but not one origin, so step
        // within each axis separately.
        let step = |mut c: Vec<f32>| -> Option<f32> {
            c.sort_by(f32::total_cmp);
            c.windows(2)
                .map(|w| w[1] - w[0])
                .filter(|&g| g > 1e-4)
                .min_by(f32::total_cmp)
        };
        let ys: Vec<f32> = coords.iter().skip(1).step_by(2).copied().collect();
        coords = coords.into_iter().step_by(2).collect();
        match (step(coords), step(ys)) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Bounding box (min, max corners) of the merged geometry; all zero when
    /// empty.
    fn bounds(&self) -> (Vector3, Vector3) {
        let mut min = Vector3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vector3::new(f32::MIN, f32::MIN, f32::MIN);
        for v in &self.vertices {
            min = Vector3::new(min.x.min(v.x), min.y.min(v.y), min.z.min(v.z));
            max = Vector3::new(max.x.max(v.x), max.y.max(v.y), max.z.max(v.z));
        }
        if self.vertices.is_empty() {
            (Vector3::zero(), Vector3::zero())
        } else {
            (min, max)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32, z: f32) -> Vector3 {
        Vector3::new(x, y, z)
    }

    /// A minimal GLB: header + JSON chunk (no binary chunk needed to name).
    fn glb(json: &str) -> Vec<u8> {
        let mut body = json.as_bytes().to_vec();
        while !body.len().is_multiple_of(4) {
            body.push(b' ');
        }
        let mut out = b"glTF".to_vec();
        out.extend(2u32.to_le_bytes());
        out.extend(((20 + body.len()) as u32).to_le_bytes());
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(b"JSON");
        out.extend(body);
        out
    }

    #[test]
    fn mesh_names_follow_raylibs_node_order() {
        // Like letters.glb: mesh nodes first (named per letter), then the
        // layer nodes and the root, which hold no mesh.
        let g = glb(r#"{"meshes":[{"name":"m0","primitives":[{}]},
                          {"name":"m1","primitives":[{},{}]}],
                "nodes":[{"name":"phoneme_a","mesh":1},
                         {"name":"phoneme_e","mesh":0},
                         {"name":"a","children":[0]},
                         {"mesh":0}]}"#);
        assert_eq!(
            glb_mesh_names(&g).unwrap(),
            ["phoneme_a", "phoneme_a", "phoneme_e", "m0"]
        );
        assert!(glb_mesh_names(b"nope").is_err());
    }

    #[test]
    fn parts_group_by_name_in_first_seen_order() {
        let names: Vec<String> = ["b", "a", "b", "c"].map(String::from).to_vec();
        assert_eq!(
            group_by_name(&names),
            vec![
                ("b".to_string(), vec![0, 2]),
                ("a".to_string(), vec![1]),
                ("c".to_string(), vec![3]),
            ]
        );
    }

    #[test]
    fn the_letters_file_names_every_vowel() {
        let names = glb_mesh_names(include_bytes!("../../assets/models/letters.glb")).unwrap();
        for v in ["a", "e", "i", "o", "u", "y"] {
            assert!(names.contains(&format!("phoneme_{v}")), "no phoneme_{v}");
        }
    }

    #[test]
    fn merging_parts_rebases_their_indices() {
        let mut data = MeshData::default();
        let tri = [v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.0, 1.0, 0.0)];
        let uv = [Vector2::zero(); 3];
        let n = [v(0.0, 0.0, 1.0); 3];
        data.push(&tri, &uv, &n, &[0, 1, 2]);
        data.push(&tri, &uv, &n, &[0, 2, 1]);
        assert_eq!(data.vertices.len(), 6);
        // The second part's indices must point at its own (offset) vertices.
        assert_eq!(data.indices, vec![0, 1, 2, 3, 5, 4]);
        // Every attribute stays one-per-vertex, as raylib requires.
        assert_eq!(data.texcoords.len(), 6);
        assert_eq!(data.normals.len(), 6);
    }

    #[test]
    fn missing_attributes_are_filled_per_vertex() {
        let mut data = MeshData::default();
        let quad = [
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(1.0, 1.0, 0.0),
            v(0.0, 1.0, 0.0),
        ];
        // A part with neither normals, texcoords nor indices.
        data.push(&quad, &[], &[], &[]);
        assert_eq!(data.texcoords.len(), 4);
        assert_eq!(data.normals.len(), 4);
        assert_eq!(data.indices, vec![0, 1, 2, 3]);
    }

    #[test]
    fn pixel_size_is_the_smallest_lattice_step() {
        // A 14-px-per-unit drawing (like the cloud): coordinates on k/14, with
        // merged runs of several cells mixed in.
        let s = 1.0 / 14.0;
        let mut data = MeshData::default();
        data.push(
            &[
                v(-8.0 * s, 4.0 * s, 0.0),
                v(-5.0 * s, 4.0 * s, 0.0),
                v(-4.0 * s, 11.0 * s, 0.1),
                v(3.0 * s, 9.0 * s, 0.1),
            ],
            &[],
            &[],
            &[],
        );
        let px = data.pixel_size().unwrap();
        assert!((1.0 / px - 14.0).abs() < 1e-3, "ppu {}", 1.0 / px);
        assert!(MeshData::default().pixel_size().is_none());
    }

    #[test]
    fn bounds_span_the_merged_geometry() {
        let mut data = MeshData::default();
        data.push(
            &[v(-2.0, 0.0, 0.0), v(2.0, 0.0, 0.0), v(0.0, 3.0, 0.5)],
            &[],
            &[],
            &[],
        );
        let (min, max) = data.bounds();
        let s = max - min;
        assert_eq!(s.x, 4.0);
        assert_eq!(s.y, 3.0);
        assert_eq!(s.z, 0.5);
        // The centre is what a spinning prop turns about.
        let c = (min + max) * 0.5;
        assert_eq!((c.x, c.y, c.z), (0.0, 1.5, 0.25));
        let (min, max) = MeshData::default().bounds();
        assert!((max - min).y.abs() < f32::EPSILON);
    }
}
