//! Lam::pula — flat-draw's tinted-glass shader, ported 1:1.
//!
//! The model becomes one piece of tinted glass in the artwork's own colours,
//! lit by three lamps standing round it (they can circle; they ship still),
//! with highlights, a reflected "room" gathered at the silhouette, and little
//! star-shaped rays that fire at brick corners when a lamp lines up on them.
//!
//! **The shader is flat-draw's, unchanged**: `assets/shaders/lampula.fs` is its
//! `lampFS` and `assets/shaders/flatdraw_model.vs` its shared `modelVS`, copied
//! verbatim (the files say from which commit). Everything that tunes it is a
//! uniform, set from [`LampulaParams`] — whose defaults are flat-draw's `Def`
//! column (`internal/gpu/lampulaparams.go`), so the game's glass is the glass in
//! flat-draw's 3D pane.
//!
//! **Anything can go through it**: [`Lampula::draw`] takes any [`FlatModel`]
//! and any transform. It needs a flat-draw drawing rather than any old mesh for
//! one reason — the corner rays and brick edges recover the drawing's pixel
//! lattice ([`FlatModel::lattice`]), the job flat-draw's `brickMatrix` does.
//!
//! **Its switches are compiled in.** Each `…On` uniform becomes a constant in
//! the copy of the shader handed to the driver ([`compiled_fs`]), so a term a
//! switch turns off is dropped by the compiler instead of being computed for
//! every fragment and multiplied by zero. The file itself stays verbatim.
//!
//! To dabble: edit a default below and rebuild, change `lampula.params` in code
//! (values only: the switches are set at load), or — no rebuild — start the game
//! with `RONDELEK_LAMPULA=<file.json>`. The file
//! may be flat-draw's own `~/.config/flatty/config.json` (its
//! `shaderParams.lampula` is used), a `saved-ideas/lampula-*.json` snapshot, or
//! a plain `{ "exposure": 2.4, "lamp1On": 0, "sky": "#88CCFF" }`. So a look
//! tuned live in flat-draw's Shaders pane carries straight over.

use super::models::{FlatModel, draw_mesh_with};
use super::shader_params::{self, UniformRows, shader_params};
use raylib::prelude::*;

shader_params! {
    /// Lam::pula's exposed values — flat-draw's `lampulaParams`, row for row,
    /// same keys, ranges and defaults. Groups as in flat-draw's Shaders pane.
    pub struct LampulaParams {
        // Material ---------------------------------------------------------
        /// Pushes colours away from grey, less so the more saturated they are.
        vibrance: f32 = "vibrance", 0.31, 0.0 ..= 1.0;
        /// Glass opacity, on top of the artwork's own alpha.
        alpha: f32 = "alpha", 0.80, 0.05 ..= 1.0;
        /// Near zero on purpose: everything you see is lit by a lamp.
        ambient: f32 = "ambient", 0.04, 0.0 ..= 1.0;
        exposure: f32 = "exposure", 1.93, 0.2 ..= 3.0;

        // Lamps: all three lit, all the same warm gold ---------------------
        lamp0_on: bool = "lamp0On", true;
        lamp0: [u8; 3] = "lamp0", [255, 199, 107];
        lamp1_on: bool = "lamp1On", true;
        lamp1: [u8; 3] = "lamp1", [255, 199, 107];
        lamp2_on: bool = "lamp2On", true;
        lamp2: [u8; 3] = "lamp2", [255, 199, 107];

        // Orbit: ships still ------------------------------------------------
        /// The lamps circle the model. Off: a fixed studio.
        orbit_on: bool = "orbitOn", false;
        orbit_rate: f32 = "orbitRate", 0.35, 0.0 ..= 2.0;
        /// Orbit radius, in multiples of the model's largest half-extent.
        orbit_dist: f32 = "orbitDist", 2.20, 1.05 ..= 6.0;
        orbit_lift: f32 = "orbitLift", 0.45, -2.0 ..= 2.0;
        falloff: f32 = "falloff", 0.60, 0.0 ..= 4.0;

        // Light ------------------------------------------------------------
        diffuse: f32 = "diffuse", 1.33, 0.0 ..= 2.0;
        wrap: f32 = "wrap", 0.16, 0.0 ..= 1.0;
        /// Light through the glass when a lamp is behind it.
        trans_on: bool = "transOn", true;
        trans_pow: f32 = "transPow", 6.52, 0.5 ..= 12.0;
        trans_gain: f32 = "transGain", 2.98, 0.0 ..= 3.0;

        // Reflection -------------------------------------------------------
        spec_on: bool = "specOn", true;
        spec_pow: f32 = "specPow", 106.35, 1.0 ..= 128.0;
        spec_gain: f32 = "specGain", 1.65, 0.0 ..= 3.0;
        /// The room, reflected at the silhouette.
        env_on: bool = "envOn", true;
        fres_pow: f32 = "fresPow", 6.85, 0.5 ..= 8.0;
        env_gain: f32 = "envGain", 1.35, 0.0 ..= 2.0;
        /// Room above.
        sky: [u8; 3] = "sky", [255, 199, 107];
        /// Room below.
        ground: [u8; 3] = "ground", [41, 33, 31];

        // Sparks: the corner rays ------------------------------------------
        spark_on: bool = "sparkOn", true;
        glint_pow: f32 = "glintPow", 52.77, 4.0 ..= 256.0;
        spark_gain: f32 = "sparkGain", 5.00, 0.0 ..= 12.0;
        corner_tilt: f32 = "cornerTilt", 0.3, 0.0 ..= 3.0;
        ray_fall: f32 = "rayFall", 1.93, 0.5 ..= 24.0;
        spike: f32 = "spike", 12.97, 0.5 ..= 16.0;

        // Bricks -----------------------------------------------------------
        edge_on: bool = "edgeOn", true;
        edge: f32 = "edge", 0.06, 0.0 ..= 0.5;
    }
}

/// The env var naming a tuning file (see the module doc).
pub const OVERRIDES_ENV: &str = "RONDELEK_LAMPULA";

impl LampulaParams {
    /// flat-draw's defaults, plus whatever `RONDELEK_LAMPULA` points at.
    pub fn from_env() -> Self {
        Self::default().with_env()
    }

    /// These values, with whatever `RONDELEK_LAMPULA` points at on top — so a
    /// scene's own look (e.g. the clouds' room colour) is still overridable.
    pub fn with_env(self) -> Self {
        let mut p = self;
        if let Some(path) = std::env::var_os(OVERRIDES_ENV) {
            shader_params::apply_overrides(path.as_ref(), "lampula", &mut |k, v| p.set(k, v));
        }
        p
    }
}

const VS: &str = include_str!("../../assets/shaders/flatdraw_model.vs");
pub(crate) const FS: &str = include_str!("../../assets/shaders/lampula.fs");

/// `lampula.fs` with `look`'s switches as constants: `uniform float uSparkOn;`
/// becomes `const float uSparkOn = 0.0;`, and so on for every `…On` row.
fn compiled_fs(look: &LampulaParams) -> String {
    let mut fs = FS.to_owned();
    for (name, on) in look.switches() {
        let uniform = format!("uniform float {name};");
        debug_assert!(fs.contains(&uniform), "lampula.fs has no {uniform}");
        fs = fs.replace(
            &uniform,
            &format!("const float {name} = {}.0;", u8::from(on)),
        );
    }
    fs
}

/// The compiled shader, its locations, and the values it draws with.
pub struct Lampula {
    shader: Shader,
    loc_brick: i32,
    loc_eye: i32,
    loc_centre: i32,
    loc_radius: i32,
    loc_time: i32,
    /// The rows of [`LampulaParams`] (the switches are compiled in instead).
    rows: UniformRows,
    /// The look. Public: change its values any time, a change is uploaded on
    /// the next [`Lampula::begin_frame`]; its switches were compiled in at
    /// load.
    pub params: LampulaParams,
}

impl Lampula {
    /// Compile the shader, starting from `look` (`LampulaParams::default()` is
    /// flat-draw's own) with `RONDELEK_LAMPULA` applied on top. `None` (logged)
    /// if it didn't compile — the caller then draws its props plain, as
    /// flat-draw falls back to its flat shader.
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, look: LampulaParams) -> Option<Self> {
        Self::load_with_vs(rl, thread, VS, look.with_env())
    }

    /// [`Lampula::load`] with `look` exactly as given, no `RONDELEK_LAMPULA`:
    /// for a look of our own (the meadow's) that a flat-draw tuning file
    /// shouldn't turn into see-through gold.
    pub fn load_exact(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        look: LampulaParams,
    ) -> Option<Self> {
        Self::load_with_vs(rl, thread, VS, look)
    }

    /// Lam::pula behind a vertex stage of your own, with `look` exactly as
    /// given (no `RONDELEK_LAMPULA`: whoever brings the stage brings its own
    /// tuning). The fragment stage is still flat-draw's, unchanged, so `vs` must
    /// hand it what `flatdraw_model.vs` does: `fragTexCoord`, `fragNormal` and
    /// `fragWorld`. The brick water's `brick_water.vs` is the one user.
    pub fn load_with_vs(
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        vs: &str,
        look: LampulaParams,
    ) -> Option<Self> {
        let shader = rl.load_shader_from_memory(thread, Some(vs), Some(&compiled_fs(&look)));
        // raylib quietly substitutes its default shader for one that failed to
        // compile.
        // SAFETY: a plain getter of rlgl's state.
        if shader.id == unsafe { raylib::ffi::rlGetShaderIdDefault() } {
            eprintln!("Lam::pula shader did not compile; its things draw plain");
            return None;
        }
        let loc_brick = shader.get_shader_location("uBrick");
        let rows = UniformRows::locate(&shader, &LampulaParams::uniform_names());
        Some(Self {
            loc_eye: shader.get_shader_location("uEye"),
            loc_centre: shader.get_shader_location("uBlobCentre"),
            loc_radius: shader.get_shader_location("uBlobRadius"),
            loc_time: shader.get_shader_location("uTime"),
            loc_brick,
            rows,
            shader,
            params: look,
        })
    }

    /// The compiled shader, for the extra uniforms a vertex stage of one's own
    /// declares (see [`Lampula::load_with_vs`]).
    pub fn shader_mut(&mut self) -> &mut Shader {
        &mut self.shader
    }

    /// Once per frame, before any [`Lampula::draw`]: where the eye is, the clock
    /// the lamps orbit by, and whichever of the look's values changed.
    pub fn begin_frame(&mut self, eye: Vector3, time: f32) {
        self.shader.set_shader_value(self.loc_eye, eye);
        self.shader.set_shader_value(self.loc_time, time);
        self.rows.upload(&mut self.shader, self.params.values());
    }

    /// Draw one instance of `model` under `transform`, as glass. The lamps
    /// stand round *this instance's* bounding box, so a big cloud and a small
    /// one are lit alike.
    pub fn draw(&mut self, d: &mut impl RaylibDraw3D, model: &FlatModel, transform: Matrix) {
        let lamps = world_box(model.min, model.max, transform);
        self.place(transform, model.lattice(), lamps);
        model.draw_shaded(d, transform, &self.shader);
    }

    /// Draw any mesh as glass, given what [`Lampula::draw`] reads off a
    /// drawing: `lattice` maps model space to the brick lattice (the bricks'
    /// corners on whole numbers, as [`FlatModel::lattice`] does), and `lamps`
    /// is the box the lamps stand round, in *world* space (centre,
    /// half-extent; see [`world_box`]). Fixed in the world rather than taken
    /// from the instance, so something scrolling (the water, the ground)
    /// slides under still lamps instead of carrying them along.
    pub fn draw_mesh(
        &mut self,
        d: &mut impl RaylibDraw3D,
        mesh: &Mesh,
        material: &WeakMaterial,
        transform: Matrix,
        lattice: Matrix,
        lamps: (Vector3, Vector3),
    ) {
        self.place(transform, lattice, lamps);
        draw_mesh_with(d, mesh, material, &self.shader, transform);
    }

    /// The per-instance uniforms: where the lamps stand, and the brick lattice.
    fn place(&mut self, transform: Matrix, lattice: Matrix, (centre, radius): (Vector3, Vector3)) {
        self.shader.set_shader_value(self.loc_centre, centre);
        self.shader.set_shader_value(self.loc_radius, radius);
        // World → the drawing's pixel lattice: undo this instance's transform,
        // then model → lattice (flat-draw: MatrixInvert(m) · brickMatrix).
        self.shader
            .set_shader_value_matrix(self.loc_brick, transform.invert() * lattice);
    }
}

/// A model-space box under `m`, as a world-space centre and half-extent.
pub fn world_box(min: Vector3, max: Vector3, m: Matrix) -> (Vector3, Vector3) {
    let mut lo = Vector3::new(f32::MAX, f32::MAX, f32::MAX);
    let mut hi = Vector3::new(f32::MIN, f32::MIN, f32::MIN);
    for i in 0..8 {
        let c = Vector3::new(
            if i & 1 == 0 { min.x } else { max.x },
            if i & 2 == 0 { min.y } else { max.y },
            if i & 4 == 0 { min.z } else { max.z },
        )
        .transform(m);
        lo = Vector3::new(lo.x.min(c.x), lo.y.min(c.y), lo.z.min(c.z));
        hi = Vector3::new(hi.x.max(c.x), hi.y.max(c.y), hi.z.max(c.z));
    }
    ((lo + hi) * 0.5, (hi - lo) * 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_params::declared_uniforms;

    /// Uniforms the driver sets itself (or raylib does), not rows of the table.
    const ENGINE: &[&str] = &[
        "texture0",
        "colDiffuse",
        "uEye",
        "uTime",
        "uBlobCentre",
        "uBlobRadius",
        "uBrick",
    ];

    #[test]
    fn every_param_has_its_uniform_and_every_uniform_its_param() {
        // flat-draw's TestEveryExposedParamHasItsUniform, both ways round.
        let declared = declared_uniforms(FS);
        let rows = LampulaParams::uniform_names();
        for u in &rows {
            assert!(declared.contains(u), "row {u} has no uniform in lampula.fs");
        }
        for u in &declared {
            assert!(
                rows.contains(u) || ENGINE.contains(&u.as_str()),
                "lampula.fs declares {u}, which nothing sets"
            );
        }
        assert_eq!(rows.len(), 36);
    }

    #[test]
    fn defaults_are_flat_draws_def_column() {
        let p = LampulaParams::default();
        assert_eq!(
            (p.vibrance, p.alpha, p.ambient, p.exposure),
            (0.31, 0.80, 0.04, 1.93)
        );
        assert!(p.lamp0_on && p.lamp1_on && p.lamp2_on && !p.orbit_on);
        assert_eq!(p.lamp1, [255, 199, 107]);
        assert_eq!(p.ground, [41, 33, 31]);
        assert_eq!((p.spec_pow, p.glint_pow, p.spike), (106.35, 52.77, 12.97));
    }

    #[test]
    fn the_shader_is_flat_draws_verbatim() {
        // Guard the copy against "a quick tweak": the body starts where
        // flat-draw's does and keeps its one compiled-in constant.
        assert!(FS.contains("const int LAMPS = 3;"));
        assert!(FS.contains("finalColor = vec4(col, tex.a * uAlpha);"));
        assert!(VS.contains("fragWorld    = (matModel * vec4(vertexPosition, 1.0)).xyz;"));
    }

    #[test]
    fn every_switch_is_compiled_in_and_nothing_else() {
        let look = LampulaParams {
            spark_on: false,
            orbit_on: false,
            ..LampulaParams::default()
        };
        let fs = compiled_fs(&look);
        assert!(fs.contains("const float uSparkOn = 0.0;"));
        assert!(fs.contains("const float uLamp0On = 1.0;"));
        let switches = look.switches();
        assert_eq!(switches.len(), 9);
        for (name, _) in &switches {
            assert!(!fs.contains(&format!("uniform float {name};")), "{name}");
        }
        // Every other row is still a uniform.
        let declared = declared_uniforms(&fs);
        for u in LampulaParams::uniform_names() {
            let switch = switches.iter().any(|(name, _)| *name == u);
            assert_eq!(declared.contains(&u), !switch, "{u}");
        }
    }

    #[test]
    fn world_box_follows_the_transform() {
        let m = Matrix::scale(2.0, 2.0, 2.0) * Matrix::translate(10.0, 0.0, -1.0);
        let (c, r) = world_box(
            Vector3::new(-1.0, 0.0, -0.5),
            Vector3::new(1.0, 1.0, 0.5),
            m,
        );
        assert_eq!((c.x, c.y, c.z), (10.0, 1.0, -1.0));
        assert_eq!((r.x, r.y, r.z), (2.0, 1.0, 1.0));
    }
}
