//! Vowel Runner's brick water: the sea along the front, built of little glass
//! bricks like the clouds above it.
//!
//! One column of bricks per cell, from the bank's face out past the bottom of
//! the screen, all in one mesh and one draw call. The columns rise and fall
//! on a gentle swell rolling in toward the bank, so a column a little higher
//! than its neighbour shows its sides. Colours step from shallow at the bank to
//! deep further out, with white foam bricks lapping at the bank and riding the
//! crests. It scrolls with the ground and listens: while the child makes sound
//! the swell grows a little and the bricks' corners sparkle more.
//!
//! **The light is Lam::pula's**, the clouds' glass, through the very same
//! `lampula.fs` (flat-draw's, unchanged). Only the vertex stage is ours,
//! `brick_water.vs`: it lifts the columns and picks each brick's colour (its
//! header says how that fits a shader written for still drawings). The look is
//! a [`LampulaParams`] of its own (`water_glass()` in the runner), so the water
//! can wear different lamps than the clouds.
//!
//! This replaced the smooth toon water ([`crate::water`]) as the default on
//! 2026-10-02. That one is kept whole: `RONDELEK_WATER_STYLE=toon` brings it
//! back.
//!
//! Tuning: [`BrickWaterParams`] for the bricks and their motion, [`look`] for
//! the glass, both public. `RONDELEK_WATER=<file.json>` overrides keys of
//! either at launch (`{ "swellAmp": 0.1, "lamp0": "#FFFFFF" }`).
//!
//! [`look`]: BrickWater::look

use super::lampula::{Lampula, LampulaParams};
use super::shader_params::{self, Uniform, shader_params};
use super::water::{OVERRIDES_ENV, PERIOD, Placement};
use raylib::ffi;
use raylib::prelude::*;

shader_params! {
    /// The bricks and their motion. Keys double as uniform names
    /// (`swellAmp` → `uSwellAmp`), except the colours (they are the palette
    /// texture) and `voiceSpark` (it scales Lam::pula's `sparkGain`).
    pub struct BrickWaterParams {
        // Colour (the palette) ---------------------------------------------
        /// At the bank.
        shallow: [u8; 3] = "shallow", [72, 200, 216];
        /// Out in front.
        deep: [u8; 3] = "deep", [36, 102, 208];
        /// Foam bricks, at the bank and on the crests.
        foam: [u8; 3] = "foam", [250, 253, 255];
        /// How far out (world units) the water is fully deep.
        deep_dist: f32 = "deepDist", 4.5, 0.5 ..= 12.0;
        /// Colour steps between shallow and deep; 0 = a smooth blend.
        bands: f32 = "bands", 4.0, 0.0 ..= 8.0;
        /// How far a brick's colour may stray from its band, so the band
        /// edges come out dithered like pixel art.
        jitter: f32 = "jitter", 0.15, 0.0 ..= 1.0;

        // Swell ------------------------------------------------------------
        /// How far a column rises and falls, world units.
        swell_amp: f32 = "swellAmp", 0.06, 0.0 ..= 0.3;
        swell_speed: f32 = "swellSpeed", 0.9, 0.0 ..= 6.0;
        /// Wavelength of the roll toward the bank, world units.
        swell_length: f32 = "swellLength", 2.6, 0.5 ..= 8.0;
        /// Extra swell at full voice (a multiple of `swellAmp`).
        voice_swell: f32 = "voiceSwell", 1.2, 0.0 ..= 4.0;

        // Foam -------------------------------------------------------------
        /// How white the bricks on a crest go.
        crest_foam: f32 = "crestFoam", 0.35, 0.0 ..= 1.0;
        /// Foam width at the bank, world units.
        foam_width: f32 = "foamWidth", 0.3, 0.0 ..= 1.0;
        foam_wobble: f32 = "foamWobble", 0.45, 0.0 ..= 1.0;
        foam_speed: f32 = "foamSpeed", 0.6, 0.0 ..= 3.0;

        // Voice ------------------------------------------------------------
        /// Extra corner sparkle at full voice (a multiple of the glass's
        /// `sparkGain`).
        voice_spark: f32 = "voiceSpark", 1.0, 0.0 ..= 4.0;
    }
}

/// How deep each column goes below the surface, in bricks: enough that no
/// swell ever lifts a column's bottom above its neighbour's top.
const DEPTH_BRICKS: f32 = 4.0;

/// raylib's MAX_MATERIAL_MAPS: how many maps `DrawMesh` reads off a material.
const MAPS: usize = 12;

const VS: &str = include_str!("../../assets/shaders/brick_water.vs");

pub struct BrickWater {
    glass: Lampula,
    mesh: Mesh,
    /// Our own material, the palette as its diffuse map. Weak (it frees
    /// nothing): its maps live in `_maps`, the palette in `palette`.
    material: WeakMaterial,
    _maps: Box<[ffi::MaterialMap; MAPS]>,
    /// 2×2: shallow, deep / foam, foam. The vertex stage blends across it.
    palette: Texture2D,
    /// The colours the palette holds now, to re-upload only on a change.
    palette_colours: [[u8; 3]; 3],
    at: Placement,
    cell: f32,
    /// Model space → the brick lattice (bricks' corners on whole numbers).
    lattice: Matrix,
    /// The mesh's model-space box: where Lam::pula's lamps stand round.
    bounds: (Vector3, Vector3),
    loc_scroll: i32,
    loc_voice: i32,
    loc_cell: i32,
    loc_params: Vec<i32>,
    /// The bricks and their motion. Change any time; uploaded every frame.
    pub params: BrickWaterParams,
    /// The glass. Change any time; uploaded every frame.
    pub look: LampulaParams,
}

impl BrickWater {
    /// Build the bricks and compile the shader, starting from the glass `look`;
    /// `RONDELEK_WATER` applies on top of both tables. `None` (logged) if
    /// anything fails — the game then simply has no water, and the bank's
    /// earth shows as before.
    pub fn load(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        at: Placement,
        cell: f32,
        look: LampulaParams,
    ) -> Option<Self> {
        let mut params = BrickWaterParams::default();
        let mut look = look;
        if let Some(path) = std::env::var_os(OVERRIDES_ENV) {
            shader_params::apply_overrides(path.as_ref(), "water", &mut |k, v| {
                params.set(k, v) || look.set(k, v)
            });
        }

        let mut glass = Lampula::load_with_vs(rl, thread, VS, look.clone())?;
        let sh = glass.shader_mut();
        let loc_cell = sh.get_shader_location("uCell");
        if loc_cell < 0 {
            eprintln!("brick water shader did not compile; no water");
            return None;
        }
        let loc_scroll = sh.get_shader_location("uScroll");
        let loc_voice = sh.get_shader_location("uVoice");
        let loc_params = BrickWaterParams::uniform_names()
            .iter()
            .map(|u| sh.get_shader_location(u))
            .collect();

        let bricks = Bricks::new(at, cell);
        let mesh = match Mesh::gen_mesh(&bricks.vertices, &bricks.texcoords)
            .normals(&bricks.normals)
            .indices(&bricks.indices)
            .build(thread)
        {
            Ok(m) => m,
            Err(e) => {
                eprintln!("brick water mesh: {e}");
                return None;
            }
        };

        let colours = palette_colours(&params);
        let mut pixels = palette_pixels(colours);
        let image = ffi::Image {
            data: pixels.as_mut_ptr().cast(),
            width: 2,
            height: 2,
            mipmaps: 1,
            format: ffi::PixelFormat::PIXELFORMAT_UNCOMPRESSED_R8G8B8A8 as i32,
        };
        // SAFETY: LoadTextureFromImage only reads the pixels (they are copied
        // to the GPU, and `pixels` outlives the call); the texture is ours,
        // and Texture2D unloads it once, on drop.
        let raw = unsafe { ffi::LoadTextureFromImage(image) };
        if raw.id == 0 {
            eprintln!("brick water palette did not load; no water");
            return None;
        }
        let palette = unsafe { Texture2D::from_raw(raw) };
        // Blended, not snapped: the vertex stage's coordinates between the
        // texel centres mix the colours.
        palette.set_texture_filter(thread, TextureFilter::TEXTURE_FILTER_BILINEAR);
        palette.set_texture_wrap(thread, TextureWrap::TEXTURE_WRAP_CLAMP);

        let none = ffi::MaterialMap {
            texture: ffi::Texture::default(),
            color: Color::BLANK,
            value: 0.0,
        };
        let mut maps = Box::new([none; MAPS]);
        maps[0] = ffi::MaterialMap {
            texture: *palette.as_ref(),
            color: Color::WHITE,
            value: 0.0,
        };
        // SAFETY: a weak material over maps we own (boxed, so they never move)
        // and a texture we own; nothing is ever unloaded through it. The
        // shader is filled in per draw by `draw_mesh_with`.
        let material = unsafe {
            WeakMaterial::from_raw(ffi::Material {
                shader: *glass.shader_mut().as_ref(),
                maps: maps.as_mut_ptr(),
                params: [0.0; 4],
            })
        };

        Some(Self {
            glass,
            mesh,
            material,
            _maps: maps,
            palette,
            palette_colours: colours,
            at,
            cell,
            lattice: bricks.lattice,
            bounds: bricks.bounds,
            loc_scroll,
            loc_voice,
            loc_cell,
            loc_params,
            params,
            look,
        })
    }

    /// Draw the water. `scroll` is the ground's travelled distance in world
    /// units, `voice` the child's smoothed voice level (0..1).
    pub fn draw(
        &mut self,
        d: &mut impl RaylibDraw3D,
        eye: Vector3,
        time: f32,
        scroll: f32,
        voice: f32,
    ) {
        let voice = voice.clamp(0.0, 1.0);
        let colours = palette_colours(&self.params);
        if colours != self.palette_colours {
            let _ = self.palette.update_texture(&palette_pixels(colours));
            self.palette_colours = colours;
        }

        // The glass, its corners sparkling more while the child makes sound.
        let mut look = self.look.clone();
        look.spark_gain *= 1.0 + self.params.voice_spark * voice;
        self.glass.params = look;
        self.glass.begin_frame(eye, time);

        // The bricks slide along with the ground by less than one brick, then
        // hop back by exactly one as the pattern moves on by one: the hop is
        // invisible, and the lattice never leaves the bricks.
        let wrapped = scroll.rem_euclid(PERIOD);
        let slide = wrapped.rem_euclid(self.cell);
        let sh = self.glass.shader_mut();
        sh.set_shader_value(self.loc_scroll, wrapped - slide);
        sh.set_shader_value(self.loc_voice, voice);
        sh.set_shader_value(self.loc_cell, self.cell);
        for (&loc, value) in self.loc_params.iter().zip(self.params.values()) {
            match value {
                Uniform::Float(f) if loc >= 0 => sh.set_shader_value(loc, f),
                _ => {} // a palette colour, or a row the compiler stripped
            }
        }

        let transform = Matrix::translate(-slide, self.at.y, self.at.shore_z);
        self.glass.draw_mesh(
            d,
            &self.mesh,
            &self.material,
            transform,
            self.lattice,
            self.bounds,
        );
    }
}

fn palette_colours(p: &BrickWaterParams) -> [[u8; 3]; 3] {
    [p.shallow, p.deep, p.foam]
}

/// The 2×2 palette's RGBA pixels, row by row: shallow, deep / foam, foam.
fn palette_pixels([shallow, deep, foam]: [[u8; 3]; 3]) -> Vec<u8> {
    [shallow, deep, foam, foam]
        .iter()
        .flat_map(|c| [c[0], c[1], c[2], 255])
        .collect()
}

/// The water's geometry, kept as plain vectors so it is testable without a
/// GPU. Model space: x along the shore, centred; y = 0 the resting surface;
/// z = 0 the bank's face, growing out toward the camera.
struct Bricks {
    vertices: Vec<Vector3>,
    /// Every vertex carries its column's centre (x, z): the vertex stage
    /// lifts and colours a whole column alike by it.
    texcoords: Vec<Vector2>,
    normals: Vec<Vector3>,
    indices: Vec<u16>,
    lattice: Matrix,
    bounds: (Vector3, Vector3),
}

impl Bricks {
    /// Columns of `cell`-sized bricks covering `at.width` along the shore,
    /// plus one more to slide into, and the bank to `at.far_z` out.
    ///
    /// Only faces that can ever be seen are built: the top, the front, and
    /// both sides. The camera stands in front of all of it, so the backs face
    /// away, and the bottoms are under water.
    fn new(at: Placement, cell: f32) -> Self {
        let cols = (at.width / cell).ceil() as i32 + 1;
        let rows = ((at.far_z - at.shore_z) / cell).ceil() as i32;
        // A whole number of bricks from x = 0, so the lattice is the same
        // wherever the mesh starts.
        let x0 = -((cols / 2) as f32) * cell;
        let bottom = -DEPTH_BRICKS * cell;
        let mut b = Self {
            vertices: Vec::new(),
            texcoords: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            // (x, y, z) / cell, with the bottom on a whole number too.
            lattice: Matrix {
                m0: 1.0 / cell,
                m5: 1.0 / cell,
                m10: 1.0 / cell,
                m13: DEPTH_BRICKS,
                m15: 1.0,
                ..Matrix::default()
            },
            bounds: (
                Vector3::new(x0, bottom, 0.0),
                Vector3::new(x0 + cols as f32 * cell, 0.0, rows as f32 * cell),
            ),
        };
        for j in 0..rows {
            for i in 0..cols {
                let (xa, za) = (x0 + i as f32 * cell, j as f32 * cell);
                let (xb, zb) = (xa + cell, za + cell);
                let centre = Vector2::new(xa + cell / 2.0, za + cell / 2.0);
                let v = Vector3::new;
                // Each face's corners counter-clockwise seen from outside.
                b.quad(
                    centre,
                    Vector3::Y,
                    [
                        v(xa, 0.0, za),
                        v(xa, 0.0, zb),
                        v(xb, 0.0, zb),
                        v(xb, 0.0, za),
                    ],
                );
                b.quad(
                    centre,
                    Vector3::Z,
                    [
                        v(xa, 0.0, zb),
                        v(xa, bottom, zb),
                        v(xb, bottom, zb),
                        v(xb, 0.0, zb),
                    ],
                );
                b.quad(
                    centre,
                    Vector3::X,
                    [
                        v(xb, 0.0, zb),
                        v(xb, bottom, zb),
                        v(xb, bottom, za),
                        v(xb, 0.0, za),
                    ],
                );
                b.quad(
                    centre,
                    -Vector3::X,
                    [
                        v(xa, 0.0, za),
                        v(xa, bottom, za),
                        v(xa, bottom, zb),
                        v(xa, 0.0, zb),
                    ],
                );
            }
        }
        b
    }

    fn quad(&mut self, centre: Vector2, normal: Vector3, corners: [Vector3; 4]) {
        let base = self.vertices.len() as u16;
        self.vertices.extend(corners);
        self.texcoords.extend([centre; 4]);
        self.normals.extend([normal; 4]);
        self.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_params::declared_uniforms;

    /// Uniforms the driver (or Lam::pula, or raylib) sets, not rows.
    const ENGINE: &[&str] = &["mvp", "matModel", "uTime", "uScroll", "uVoice", "uCell"];

    /// Rows that don't go to a uniform of their own (see [`BrickWaterParams`]).
    const OFF_SHADER: &[&str] = &["shallow", "deep", "foam", "voiceSpark"];

    /// Where the runner lays the water.
    const AT: Placement = crate::runner::BRICK_WATER;

    #[test]
    fn every_param_has_its_uniform_and_every_uniform_its_param() {
        let declared = declared_uniforms(VS);
        let rows = BrickWaterParams::uniform_names();
        for (key, u) in BrickWaterParams::KEYS.iter().zip(&rows) {
            if !OFF_SHADER.contains(key) {
                assert!(
                    declared.contains(u),
                    "row {u} has no uniform in brick_water.vs"
                );
            }
        }
        for u in &declared {
            assert!(
                rows.contains(u) || ENGINE.contains(&u.as_str()),
                "brick_water.vs declares {u}, which nothing sets"
            );
        }
        for key in OFF_SHADER {
            assert!(BrickWaterParams::KEYS.contains(key), "{key} is no row");
        }
    }

    #[test]
    fn the_vertex_stage_hands_lampula_what_it_reads() {
        let fs = include_str!("../../assets/shaders/lampula.fs");
        for var in ["vec2 fragTexCoord;", "vec3 fragNormal;", "vec3 fragWorld;"] {
            assert!(fs.contains(&format!("in {var}")), "lampula.fs reads {var}");
            assert!(
                VS.contains(&format!("out {var}")),
                "brick_water.vs writes {var}"
            );
        }
    }

    #[test]
    fn the_shaders_period_is_the_scroll_wrap() {
        assert!(VS.contains(&format!("const float PERIOD = {PERIOD:.1};")));
    }

    #[test]
    fn x_frequencies_are_whole_turns_per_period() {
        for part in VS.split("kx(").skip(1) {
            let arg = part.split(')').next().unwrap();
            if arg == "float turns" {
                continue; // the definition
            }
            let n: f32 = arg.parse().unwrap_or_else(|_| panic!("kx({arg})"));
            assert_eq!(n.fract(), 0.0, "kx({arg}) isn't whole");
        }
    }

    #[test]
    fn the_brick_fits_the_period() {
        // The slide hops back by one brick as the pattern moves on by one,
        // and the pattern wraps on PERIOD: both must agree.
        let n = PERIOD / crate::runner::WATER_CELL;
        assert_eq!(n.fract(), 0.0, "{n} bricks per period");
    }

    #[test]
    fn the_mesh_fits_raylibs_16_bit_indices() {
        let b = Bricks::new(AT, crate::runner::WATER_CELL);
        assert!(
            b.vertices.len() <= u16::MAX as usize + 1,
            "{}",
            b.vertices.len()
        );
        assert_eq!(b.texcoords.len(), b.vertices.len());
        assert_eq!(b.normals.len(), b.vertices.len());
    }

    #[test]
    fn every_face_turns_outward() {
        let b = Bricks::new(AT, 0.5);
        for tri in b.indices.chunks(3) {
            let [a, p, q] = [0, 1, 2].map(|k| b.vertices[tri[k] as usize]);
            let n = b.normals[tri[0] as usize];
            // Counter-clockwise seen from outside = raylib's front face.
            assert!((p - a).cross(q - a).dot(n) > 0.0, "{tri:?}");
        }
    }

    #[test]
    fn a_column_carries_its_centre_and_its_corners_sit_on_the_lattice() {
        let cell = 0.25;
        let b = Bricks::new(AT, cell);
        for (v, c) in b.vertices.iter().zip(&b.texcoords) {
            // The vertex belongs to the column whose centre it carries…
            assert!((v.x - c.x).abs() <= cell / 2.0 + 1e-4);
            assert!((v.z - c.y).abs() <= cell / 2.0 + 1e-4);
            // …and every corner is a lattice point.
            let l = v.transform(b.lattice);
            for k in [l.x, l.y, l.z] {
                assert!((k - k.round()).abs() < 1e-3, "{v:?} → {l:?}");
            }
            // Centres sit mid-cell, which is what the vertex stage's floor()
            // relies on.
            let f = (c.x / cell).rem_euclid(1.0);
            assert!((f - 0.5).abs() < 1e-3, "centre {c:?}");
        }
    }

    #[test]
    fn the_palette_is_shallow_deep_over_foam() {
        let px = palette_pixels([[1, 2, 3], [4, 5, 6], [7, 8, 9]]);
        assert_eq!(px, [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 7, 8, 9, 255]);
    }

    #[test]
    fn overrides_reach_both_tables() {
        let mut params = BrickWaterParams::default();
        let mut look = LampulaParams::default();
        let json = serde_json::json!({ "swellAmp": 0.1, "lamp0": "#FFFFFF", "nope": 1 });
        shader_params::apply_json(&json, "water", &mut |k, v| {
            params.set(k, v) || look.set(k, v)
        });
        assert_eq!(params.swell_amp, 0.1);
        assert_eq!(look.lamp0, [255, 255, 255]);
    }
}
