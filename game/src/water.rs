//! Vowel Runner's water — the playful sea along the front of the world.
//!
//! A flat grid mesh laid in front of the meadow's bank, from the bank's face to
//! past the bottom of the screen, drawn through `water.vs`/`water.fs`: a gentle
//! swell that laps the bank, toon colour steps, a moving web of light-lines,
//! foam and bubbles at the shore, twinkles and a few goldfish. It scrolls with
//! the ground (so the world still reads as moving) and it listens: while the
//! child makes sound the swell grows a little and more twinkles light up —
//! babbling makes the water dance.
//!
//! Tuning works like Lam::pula's ([`crate::shader_params`]): [`WaterParams`] is
//! the table, its fields are public, and `RONDELEK_WATER=<file.json>` overrides
//! any of them at launch (`{ "deep": "#1E5AC8", "fishOn": 0 }`).

use super::models::draw_mesh_with;
use super::shader_params::{self, Uniform, shader_params};
use raylib::prelude::*;

// The 2026-09-27 calming pass: the water took too much of the child's
// attention, so its motion was slowed and its colours softened. Each row that
// changed says what it was ("was …") so the livelier look can come back.
shader_params! {
    /// The water's look. Keys double as uniform names (`deep` → `uDeep`).
    pub struct WaterParams {
        // Colour -----------------------------------------------------------
        /// At the bank. (Was [72, 214, 214].)
        shallow: [u8; 3] = "shallow", [112, 206, 212];
        /// Out in front. (Was [38, 128, 226].)
        deep: [u8; 3] = "deep", [70, 142, 210];
        /// How far out (world units) the water is fully deep.
        deep_dist: f32 = "deepDist", 4.5, 0.5 ..= 12.0;
        /// Toon colour steps between the two; 0 = a smooth blend.
        bands: f32 = "bands", 4.0, 0.0 ..= 8.0;

        // Swell (in the vertex shader) -------------------------------------
        swell_amp: f32 = "swellAmp", 0.06, 0.0 ..= 0.3;
        /// How fast the rolls come in. (Was 1.6.)
        swell_speed: f32 = "swellSpeed", 0.9, 0.0 ..= 6.0;
        /// Wavelength of the roll toward the bank, world units.
        swell_length: f32 = "swellLength", 2.6, 0.5 ..= 8.0;
        /// Extra swell at full voice (a multiple of `swellAmp`).
        voice_swell: f32 = "voiceSwell", 1.2, 0.0 ..= 4.0;

        // Light-lines and crests -------------------------------------------
        line: [u8; 3] = "line", [214, 248, 255];
        /// Web cells per world unit.
        cell_scale: f32 = "cellScale", 1.4, 0.3 ..= 4.0;
        /// How fast the light-line web shifts. (Was 0.9.)
        cell_speed: f32 = "cellSpeed", 0.35, 0.0 ..= 4.0;
        line_width: f32 = "lineWidth", 0.05, 0.0 ..= 0.3;
        /// How strongly the light-lines show. (Was 0.55.)
        line_gain: f32 = "lineGain", 0.38, 0.0 ..= 1.0;
        /// How pale the crests go. (Was 0.35.)
        crest_gain: f32 = "crestGain", 0.2, 0.0 ..= 1.0;

        // Shore ------------------------------------------------------------
        foam: [u8; 3] = "foam", [250, 253, 255];
        /// Foam band width at the bank, world units.
        foam_width: f32 = "foamWidth", 0.14, 0.0 ..= 0.6;
        foam_wobble: f32 = "foamWobble", 0.45, 0.0 ..= 1.0;
        /// Speed of the foam's wobble and the bubbles' pulse, as a multiple
        /// of the original motion. (Was fixed at 1.0.)
        foam_speed: f32 = "foamSpeed", 0.6, 0.0 ..= 3.0;
        bubble_gain: f32 = "bubbleGain", 0.9, 0.0 ..= 1.0;

        // Twinkles ---------------------------------------------------------
        sparkle: [u8; 3] = "sparkle", [255, 255, 240];
        /// Twinkle cells per world unit.
        sparkle_scale: f32 = "sparkleScale", 1.1, 0.2 ..= 4.0;
        /// Share of cells that twinkle when all is quiet… (Was 0.22.)
        sparkle_density: f32 = "sparkleDensity", 0.12, 0.0 ..= 1.0;
        /// …and how many more join in at full voice.
        voice_sparkle: f32 = "voiceSparkle", 0.6, 0.0 ..= 1.0;
        /// Arm length of a twinkle, world units.
        sparkle_size: f32 = "sparkleSize", 0.16, 0.02 ..= 0.6;
        /// (Was 1.0.)
        sparkle_gain: f32 = "sparkleGain", 0.85, 0.0 ..= 2.0;
        /// How fast twinkles flash, as a multiple of the original. (Was fixed
        /// at 1.0.)
        twinkle_speed: f32 = "twinkleSpeed", 0.7, 0.0 ..= 3.0;

        // Goldfish ---------------------------------------------------------
        fish_on: bool = "fishOn", true;
        fish: [u8; 3] = "fish", [255, 136, 32];
        /// How clearly they show through the water.
        fish_gain: f32 = "fishGain", 0.85, 0.0 ..= 1.0;
        fish_speed: f32 = "fishSpeed", 0.6, 0.0 ..= 3.0;
        /// Size, as a multiple of a ~1-unit fish.
        fish_size: f32 = "fishSize", 1.5, 0.3 ..= 4.0;

        // Reflection -------------------------------------------------------
        sky_tint: [u8; 3] = "skyTint", [184, 228, 255];
        fres_pow: f32 = "fresPow", 3.0, 0.5 ..= 8.0;
        refl_gain: f32 = "reflGain", 0.5, 0.0 ..= 1.0;

        alpha: f32 = "alpha", 1.0, 0.0 ..= 1.0;
    }
}

/// The env var naming a tuning file.
pub const OVERRIDES_ENV: &str = "RONDELEK_WATER";

impl WaterParams {
    /// The defaults, plus whatever `RONDELEK_WATER` points at.
    pub fn from_env() -> Self {
        let mut p = Self::default();
        if let Some(path) = std::env::var_os(OVERRIDES_ENV) {
            shader_params::apply_overrides(path.as_ref(), "water", &mut |k, v| p.set(k, v));
        }
        p
    }
}

/// Distance (world units) the pattern repeats on — the shaders' PERIOD. The
/// scroll is wrapped on it, which is why every x-frequency there fits it.
pub const PERIOD: f32 = 64.0;

const VS: &str = include_str!("../../assets/shaders/water.vs");
const FS: &str = include_str!("../../assets/shaders/water.fs");

/// Where the water lies, in world units.
#[derive(Clone, Copy)]
pub struct Placement {
    /// The surface's resting height.
    pub y: f32,
    /// The bank's face: the water starts here…
    pub shore_z: f32,
    /// …and runs out to here (past the bottom of the screen).
    pub far_z: f32,
    /// Width, centred on x = 0.
    pub width: f32,
}

pub struct Water {
    shader: Shader,
    /// Owns the grid (and its default material, whose textures we borrow).
    model: Model,
    material: WeakMaterial,
    at: Placement,
    loc_eye: i32,
    loc_time: i32,
    loc_scroll: i32,
    loc_voice: i32,
    loc_shore: i32,
    loc_params: Vec<i32>,
    pub params: WaterParams,
}

impl Water {
    /// Build the grid and compile the shader; `None` (logged) if either fails —
    /// the game then simply has no water, and the dirt shows as before.
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, at: Placement) -> Option<Self> {
        let shader = rl.load_shader_from_memory(thread, Some(VS), Some(FS));
        let loc_shore = shader.get_shader_location("uShoreZ");
        if loc_shore < 0 {
            eprintln!("water shader did not compile; no water");
            return None;
        }
        let length = at.far_z - at.shore_z;
        // Fine enough along z for the swell (~2.6 units) to stay round.
        let mesh = Mesh::gen_mesh_plane(thread, at.width, length, 96, 48);
        // SAFETY: the model takes ownership of the mesh and unloads it once.
        let model = match rl.load_model_from_mesh(thread, unsafe { mesh.make_weak() }) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("water mesh: {e}");
                return None;
            }
        };
        let material = model.materials().first()?.clone();
        let loc_params = WaterParams::uniform_names()
            .iter()
            .map(|u| shader.get_shader_location(u))
            .collect();
        Some(Self {
            loc_eye: shader.get_shader_location("uEye"),
            loc_time: shader.get_shader_location("uTime"),
            loc_scroll: shader.get_shader_location("uScroll"),
            loc_voice: shader.get_shader_location("uVoice"),
            loc_shore,
            loc_params,
            shader,
            model,
            material,
            at,
            params: WaterParams::from_env(),
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
        let sh = &mut self.shader;
        sh.set_shader_value(self.loc_eye, eye);
        sh.set_shader_value(self.loc_time, time);
        sh.set_shader_value(self.loc_scroll, scroll.rem_euclid(PERIOD));
        sh.set_shader_value(self.loc_voice, voice.clamp(0.0, 1.0));
        sh.set_shader_value(self.loc_shore, self.at.shore_z);
        for (&loc, value) in self.loc_params.iter().zip(self.params.values()) {
            if loc < 0 {
                continue;
            }
            match value {
                Uniform::Float(f) => sh.set_shader_value(loc, f),
                Uniform::Vec3(v) => sh.set_shader_value(loc, v),
            }
        }
        let mid_z = (self.at.shore_z + self.at.far_z) / 2.0;
        let transform = Matrix::translate(0.0, self.at.y, mid_z);
        let mesh = &self.model.meshes()[0];
        draw_mesh_with(d, mesh, &self.material, &self.shader, transform);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_params::declared_uniforms;

    const ENGINE: &[&str] = &[
        "mvp", "matModel", "uEye", "uTime", "uScroll", "uVoice", "uShoreZ",
    ];

    #[test]
    fn every_param_has_its_uniform_and_every_uniform_its_param() {
        let mut declared = declared_uniforms(VS);
        declared.extend(declared_uniforms(FS));
        let rows = WaterParams::uniform_names();
        for u in &rows {
            assert!(
                declared.contains(u),
                "row {u} has no uniform in water.vs/fs"
            );
        }
        for u in &declared {
            assert!(
                rows.contains(u) || ENGINE.contains(&u.as_str()),
                "water shaders declare {u}, which nothing sets"
            );
        }
    }

    #[test]
    fn the_shaders_period_is_the_scroll_wrap() {
        let line = format!("const float PERIOD = {PERIOD:.1};");
        assert!(VS.contains(&line) && FS.contains(&line), "{line}");
    }

    #[test]
    fn x_frequencies_are_whole_turns_per_period() {
        // Every kx(n) must be a whole number of turns, or the surface jumps
        // each time the scroll wraps.
        for src in [VS, FS] {
            for part in src.split("kx(").skip(1) {
                let arg = part.split(')').next().unwrap();
                if arg == "float turns" {
                    continue; // the definition
                }
                let n: f32 = arg.parse().unwrap_or_else(|_| panic!("kx({arg})"));
                assert_eq!(n.fract(), 0.0, "kx({arg}) isn't whole");
            }
        }
    }
}
