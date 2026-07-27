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
//!   (8 for `cloud9`), all sharing a single texture-atlas material. raylib does
//!   *not* batch or instance models on its own: `DrawModel` issues one
//!   `glDrawElements` per mesh per call, so a sky full of clouds would cost
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

/// A flat-draw prop, ready to draw many times: merged geometry plus the
/// material (texture atlas) that came with the GLB.
pub struct FlatModel {
    /// Kept alive purely to own the material: the atlas texture and its shader
    /// are freed when the model unloads, and `material` points into it.
    _model: Model,
    /// Borrowed view of `_model`'s single material — safe as long as the two
    /// live and die together, which they do (same struct).
    material: WeakMaterial,
    /// All parts of the drawing concatenated into one uploaded mesh.
    mesh: Mesh,
    /// Model-space extent of the drawing (`y` = height above the pivot).
    pub size: Vector3,
}

impl FlatModel {
    /// Load an embedded flat-draw GLB. `name` only labels the temp file.
    pub fn load(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        name: &str,
        glb: &[u8],
    ) -> anyhow::Result<Self> {
        // raylib picks its loader from the extension, so the temp file must
        // keep `.glb`; the pid keeps two running games from racing each other.
        let path = std::env::temp_dir().join(format!("rondelek-{name}-{}.glb", std::process::id()));
        std::fs::write(&path, glb)?;
        let loaded = rl.load_model(
            thread,
            path.to_str()
                .ok_or_else(|| anyhow::anyhow!("non-UTF-8 temp path"))?,
        );
        let _ = std::fs::remove_file(&path);
        let model = loaded?;

        let mut data = MeshData::default();
        for part in model.meshes() {
            data.push(
                part.vertices(),
                part.texcoords(),
                part.normals(),
                part.indices(),
            );
        }
        anyhow::ensure!(!data.vertices.is_empty(), "{name}.glb has no geometry");
        let size = data.size();
        let mesh = Mesh::gen_mesh(&data.vertices, &data.texcoords)
            .normals(&data.normals)
            .indices(&data.indices)
            .build(thread)?;

        // Careful: raylib's glTF loader keeps `materials[0]` as *its own*
        // default (plain white, no texture) and appends the file's materials
        // after it. The atlas is therefore at whatever index the mesh→material
        // table points to — taking the first material silently renders the
        // drawing blank white.
        let used = mesh_materials(&model);
        let index = used.first().copied().unwrap_or(0).max(0) as usize;
        anyhow::ensure!(
            used.iter().all(|&m| m as usize == index),
            "{name}.glb spreads its parts over several materials — merging them \
             into one mesh would lose all but one"
        );
        let material = model
            .materials()
            .get(index)
            .ok_or_else(|| anyhow::anyhow!("{name}.glb has no material {index}"))?
            .clone();

        Ok(Self {
            _model: model,
            material,
            mesh,
            size,
        })
    }

    /// Draw one instance with its base at `pos`, uniformly scaled. Colour comes
    /// from the drawing's own texture atlas, so there is no tint parameter.
    pub fn draw(&self, d: &mut impl RaylibDraw3D, pos: Vector3, scale: f32) {
        // Same order raylib uses in DrawModelEx: scale, then translate.
        let transform = Matrix::scale(scale, scale, scale) * Matrix::translate(pos.x, pos.y, pos.z);
        d.draw_mesh(&self.mesh, self.material.clone(), transform);
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
    /// hundreds: `cloud9` is 520.)
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

    /// Bounding-box extent of the merged geometry.
    fn size(&self) -> Vector3 {
        let mut min = Vector3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut max = Vector3::new(f32::MIN, f32::MIN, f32::MIN);
        for v in &self.vertices {
            min = Vector3::new(min.x.min(v.x), min.y.min(v.y), min.z.min(v.z));
            max = Vector3::new(max.x.max(v.x), max.y.max(v.y), max.z.max(v.z));
        }
        if self.vertices.is_empty() {
            Vector3::zero()
        } else {
            max - min
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f32, y: f32, z: f32) -> Vector3 {
        Vector3::new(x, y, z)
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
    fn size_spans_the_merged_geometry() {
        let mut data = MeshData::default();
        data.push(
            &[v(-2.0, 0.0, 0.0), v(2.0, 0.0, 0.0), v(0.0, 3.0, 0.5)],
            &[],
            &[],
            &[],
        );
        let s = data.size();
        assert_eq!(s.x, 4.0);
        assert_eq!(s.y, 3.0);
        assert_eq!(s.z, 0.5);
        assert!(MeshData::default().size().y.abs() < f32::EPSILON);
    }
}
