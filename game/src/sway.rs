//! The wind in the jungle's palms. Their trunks sway, slowly and unevenly,
//! each palm on its own beat, and their fronds are never quite still: they
//! rise and fall and flutter, each on its own. Every few seconds a gust
//! rolls along the jungle, leaning the palms over, blowing the fronds along
//! and lifting them, the flutter doubling, the palms trembling a little
//! while it blows.
//!
//! The jungle is one still mesh, like every brick prop; what may move, and
//! how much, rides in each vertex's colour (`props::Sway`, built with the
//! tile), and a vertex stage of its own does the moving
//! (`assets/shaders/jungle_sway.vs`), in front of Lam::pula's fragment stage,
//! unchanged.
//!
//! Tuning: [`WindParams`], public; `RONDELEK_WIND=<file.json>` overrides any
//! of its keys at launch (`{ "gustEvery": 5, "leafGust": 0.3 }`).

use super::lampula::{Lampula, LampulaParams};
use super::shader_params::{self, UniformRows, shader_params};
use raylib::prelude::*;

shader_params! {
    /// The wind. Keys double as uniform names (`trunkSway` → `uTrunkSway`).
    /// Distances are world units: a jungle brick is 0.25.
    pub struct WindParams {
        // The trunks -------------------------------------------------------
        /// How far a palm's crown sways to and fro.
        trunk_sway: f32 = "trunkSway", 0.08, 0.0 ..= 0.5;
        /// How far a gust leans it over.
        trunk_gust: f32 = "trunkGust", 0.16, 0.0 ..= 1.0;
        /// The sway's pace, radians a second (one sway in about 7 s).
        sway_speed: f32 = "swaySpeed", 0.9, 0.0 ..= 6.0;

        // The fronds: always moving -----------------------------------------
        /// How far a frond's tip rises and falls, slowly, all the time, each
        /// frond on its own beat.
        leaf_sway: f32 = "leafSway", 0.1, 0.0 ..= 0.5;
        /// How far a frond's tip flutters in calm air, and how fast
        /// (radians a second).
        leaf_flutter: f32 = "leafFlutter", 0.06, 0.0 ..= 0.5;
        flutter_speed: f32 = "flutterSpeed", 5.0, 0.0 ..= 12.0;

        // The fronds in a gust -----------------------------------------------
        /// How far a gust blows a frond's tip along...
        leaf_gust: f32 = "leafGust", 0.22, 0.0 ..= 1.0;
        /// ...and lifts it...
        leaf_lift: f32 = "leafLift", 0.07, 0.0 ..= 0.5;
        /// ...and how much more it flutters, at the gust's height (1 = twice
        /// the calm flutter).
        gust_flutter: f32 = "gustFlutter", 1.0, 0.0 ..= 6.0;

        // The gusts --------------------------------------------------------
        /// A gust comes every so many seconds...
        gust_every: f32 = "gustEvery", 6.0, 1.0 ..= 30.0;
        /// ...rolling along the jungle from the right this fast (world units
        /// a second)...
        gust_speed: f32 = "gustSpeed", 7.0, 0.5 ..= 40.0;
        /// ...and this short: higher, a shorter, sharper gust.
        gust_sharp: f32 = "gustSharp", 3.0, 1.0 ..= 12.0;
        /// How much it buffets while it blows: the palms tremble a little.
        buffet: f32 = "buffet", 0.15, 0.0 ..= 0.5;
    }
}

/// The env var naming a tuning file for [`WindParams`].
pub const WIND_ENV: &str = "RONDELEK_WIND";

const VS: &str = include_str!("../../assets/shaders/jungle_sway.vs");

/// Lam::pula with the wind in it: the jungle's glass.
pub struct WindyGlass {
    glass: Lampula,
    /// The rows of [`WindParams`].
    rows: UniformRows,
    /// The wind. Change it any time; a change is uploaded on the next
    /// [`WindyGlass::begin_frame`].
    pub wind: WindParams,
}

impl WindyGlass {
    /// Compile the jungle's vertex stage in front of Lam::pula, with `look`
    /// for the glass and the wind from [`WindParams::default`] plus
    /// `RONDELEK_WIND`. `None` (logged) if it didn't compile: the jungle then
    /// stands still, in the far planes' glass.
    pub fn load(rl: &mut RaylibHandle, thread: &RaylibThread, look: LampulaParams) -> Option<Self> {
        let mut wind = WindParams::default();
        if let Some(path) = std::env::var_os(WIND_ENV) {
            shader_params::apply_overrides(path.as_ref(), "wind", &mut |k, v| wind.set(k, v));
        }
        let mut glass = Lampula::load_with_vs(rl, thread, VS, look)?;
        let rows = UniformRows::locate(glass.shader_mut(), &WindParams::uniform_names());
        Some(Self { glass, rows, wind })
    }

    /// Once per frame, before drawing the jungle: Lam::pula's own
    /// ([`Lampula::begin_frame`]) and the wind. The glass to draw it with.
    pub fn begin_frame(&mut self, eye: Vector3, time: f32) -> &mut Lampula {
        self.glass.begin_frame(eye, time);
        self.rows
            .upload(self.glass.shader_mut(), self.wind.values());
        &mut self.glass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shader_params::declared_uniforms;

    #[test]
    fn every_wind_row_has_its_uniform_and_every_uniform_its_row() {
        let declared = declared_uniforms(VS);
        let rows = WindParams::uniform_names();
        for u in &rows {
            assert!(
                declared.contains(u),
                "row {u} has no uniform in jungle_sway.vs"
            );
        }
        // The rest: raylib's matrices, and the clock Lam::pula sets.
        for u in &declared {
            assert!(
                rows.contains(u) || ["mvp", "matModel", "matNormal", "uTime"].contains(&u.as_str()),
                "jungle_sway.vs declares {u}, which nothing sets"
            );
        }
    }

    #[test]
    fn the_stage_hands_lampula_what_it_reads() {
        // lampula.fs reads these; fragWorld must be the resting place.
        for out in [
            "out vec2 fragTexCoord;",
            "out vec3 fragNormal;",
            "out vec3 fragWorld;",
        ] {
            assert!(VS.contains(out), "{out}");
        }
        assert!(VS.contains("fragWorld    = rest;"));
    }
}
