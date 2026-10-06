//! Shader tuning as a table — flat-draw's arrangement, carried over.
//!
//! A shader's knobs are declared once, as rows of [`shader_params!`]: the Rust
//! field, the key (flat-draw's name for it, which is also what a tuning file
//! uses), the default and — for a number — its range. Everything else is derived
//! from that row:
//!
//! * the **uniform** it is pushed into: key `vibrance` → `uVibrance`, exactly as
//!   flat-draw's `Param.Uniform` derives it, so the GLSL copied from there works
//!   unchanged;
//! * **`set(key, value)`**, which reads flat-draw's config format — a float per
//!   key, a toggle as 0/1, a colour as three 0–255 channels `key.r`/`.g`/`.b` —
//!   and clamps a number to its range the way flat-draw's `SetParam` does;
//! * **`values()`**, the per-frame upload list, in row order.
//!
//! So adding a knob is one row plus one `uniform` in the shader, and a test per
//! shader (see `lampula.rs`, `brick_water.rs`) reads the GLSL source and refuses a row
//! the shader never declares — `GetShaderLocation` would answer −1 and the value
//! would vanish in silence.
//!
//! The fields are `pub`: to dabble, change a default here and rebuild, or set
//! `params.exposure = 2.4` in code, or point the shader's env var at a JSON file
//! ([`apply_overrides`]) and restart the game — no rebuild at all.

use raylib::prelude::{RaylibShader, Shader};
use std::path::Path;

/// One uniform's value, ready for `SetShaderValue`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Uniform {
    Float(f32),
    Vec3([f32; 3]),
}

/// A table's uniform locations in one shader, and the values last sent to
/// them. [`UniformRows::upload`] sends only the rows that changed since, so a
/// look that holds still costs nothing per frame (each send is a program bind
/// and a `glUniform`).
pub struct UniformRows {
    locs: Vec<i32>,
    sent: Vec<Option<Uniform>>,
}

impl UniformRows {
    /// Look `names` (a table's `uniform_names()`) up in `shader`. A row the
    /// compiler stripped, or one compiled in as a constant, answers −1 and is
    /// never sent.
    pub fn locate(shader: &Shader, names: &[String]) -> Self {
        let locs: Vec<i32> = names
            .iter()
            .map(|u| shader.get_shader_location(u))
            .collect();
        Self {
            sent: vec![None; locs.len()],
            locs,
        }
    }

    /// Send each of `values` (in row order) whose row changed since it was
    /// last sent.
    pub fn upload(&mut self, shader: &mut Shader, values: Vec<Uniform>) {
        let rows = self.locs.iter().zip(&mut self.sent).zip(values);
        for ((&loc, sent), value) in rows {
            if loc < 0 || *sent == Some(value) {
                continue;
            }
            match value {
                Uniform::Float(f) => shader.set_shader_value(loc, f),
                Uniform::Vec3(v) => shader.set_shader_value(loc, v),
            }
            *sent = Some(value);
        }
    }
}

/// A value a row can hold. Implemented for the three kinds flat-draw has: a
/// float, a toggle (uploaded as the 0 or 1 it multiplies by) and a colour.
pub trait ParamValue {
    /// Set from one number in flat-draw's config format. `channel` is `Some(i)`
    /// for `key.r`/`.g`/`.b`; `range` is the row's bounds, if it has any.
    /// Returns false when the key's shape doesn't fit this kind.
    fn set_from(&mut self, channel: Option<usize>, v: f32, range: Option<(f32, f32)>) -> bool;
    fn uniform(&self) -> Uniform;
    /// A switch's state; `None` for a row that isn't one.
    fn switch(&self) -> Option<bool> {
        None
    }
}

impl ParamValue for f32 {
    fn set_from(&mut self, channel: Option<usize>, v: f32, range: Option<(f32, f32)>) -> bool {
        if channel.is_some() {
            return false;
        }
        *self = match range {
            Some((lo, hi)) => v.clamp(lo, hi),
            None => v,
        };
        true
    }
    fn uniform(&self) -> Uniform {
        Uniform::Float(*self)
    }
}

impl ParamValue for bool {
    fn set_from(&mut self, channel: Option<usize>, v: f32, _: Option<(f32, f32)>) -> bool {
        if channel.is_some() {
            return false;
        }
        // flat-draw's ParamOnOf: on from one half up.
        *self = v >= 0.5;
        true
    }
    fn uniform(&self) -> Uniform {
        Uniform::Float(if *self { 1.0 } else { 0.0 })
    }
    fn switch(&self) -> Option<bool> {
        Some(*self)
    }
}

/// A colour row: RGB, 0–255 per channel, as flat-draw stores it.
impl ParamValue for [u8; 3] {
    fn set_from(&mut self, channel: Option<usize>, v: f32, _: Option<(f32, f32)>) -> bool {
        match channel {
            Some(i) if i < 3 => {
                self[i] = v.clamp(0.0, 255.0).round() as u8;
                true
            }
            _ => false,
        }
    }
    fn uniform(&self) -> Uniform {
        Uniform::Vec3(self.map(|c| c as f32 / 255.0))
    }
}

/// flat-draw's key → uniform rule: `vibrance` → `uVibrance`.
pub fn uniform_name(key: &str) -> String {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) => format!("u{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

/// Split flat-draw's colour-channel suffix off a key: `lamp0.g` → (`lamp0`, 1).
pub fn split_channel(key: &str) -> (&str, Option<usize>) {
    for (i, suffix) in [".r", ".g", ".b"].iter().enumerate() {
        if let Some(base) = key.strip_suffix(suffix) {
            return (base, Some(i));
        }
    }
    (key, None)
}

/// Declare a shader's tuning table. Each row:
///
/// ```text
/// /// doc
/// field: type = "key", default [, min ..= max];
/// ```
///
/// `type` is `f32` (a slider), `bool` (a switch) or `[u8; 3]` (a colour).
macro_rules! shader_params {
    (
        $(#[$meta:meta])*
        pub struct $name:ident {
            $(
                $(#[$fmeta:meta])*
                $field:ident : $ty:ty = $key:literal, $def:expr $(, $min:literal ..= $max:literal)? ;
            )*
        }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq)]
        pub struct $name {
            $( $(#[$fmeta])* pub $field: $ty, )*
        }

        impl Default for $name {
            fn default() -> Self {
                Self { $( $field: $def, )* }
            }
        }

        #[allow(dead_code)]
        impl $name {
            /// Every key, in row order (flat-draw's names).
            pub const KEYS: &'static [&'static str] = &[ $( $key ),* ];

            /// Every row's uniform name, in row order.
            pub fn uniform_names() -> Vec<String> {
                Self::KEYS.iter().map(|k| $crate::shader_params::uniform_name(k)).collect()
            }

            /// Every row's current value, in row order — the per-frame upload.
            pub fn values(&self) -> Vec<$crate::shader_params::Uniform> {
                use $crate::shader_params::ParamValue;
                vec![ $( self.$field.uniform() ),* ]
            }

            /// Every switch row (`bool`) as its uniform name and state, in row
            /// order.
            pub fn switches(&self) -> Vec<(String, bool)> {
                use $crate::shader_params::ParamValue;
                let mut out = Vec::new();
                $(
                    if let Some(on) = self.$field.switch() {
                        out.push(($crate::shader_params::uniform_name($key), on));
                    }
                )*
                out
            }

            /// Set one value from flat-draw's config format (see the module doc).
            /// False for a key this table doesn't have.
            pub fn set(&mut self, key: &str, v: f32) -> bool {
                use $crate::shader_params::ParamValue;
                let (base, channel) = $crate::shader_params::split_channel(key);
                $(
                    if base == $key {
                        #[allow(unused_mut, unused_assignments)]
                        let mut range: Option<(f32, f32)> = None;
                        $( range = Some(($min as f32, $max as f32)); )?
                        return self.$field.set_from(channel, v, range);
                    }
                )*
                false
            }
        }
    };
}
pub(crate) use shader_params;

/// Apply a JSON tuning file to a table, reporting problems on stderr rather than
/// failing: a typo in a file you are dabbling with should cost that one value,
/// not the game.
///
/// Accepts the shapes flat-draw writes, so values can be carried straight over:
///
/// * a whole flat-draw `config.json` — the object at `shaderParams.<id>` is used;
/// * a `saved-ideas/*.json` snapshot — its `configShaderParams`;
/// * a flat object of keys → numbers (booleans are 1/0, and a colour may also
///   be written `"#RRGGBB"`).
pub fn apply_overrides(path: &Path, shader_id: &str, set: &mut dyn FnMut(&str, f32) -> bool) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("{shader_id}: can't read {}: {e}", path.display());
            return;
        }
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(json) => apply_json(&json, shader_id, set),
        Err(e) => eprintln!("{shader_id}: {} is not JSON: {e}", path.display()),
    }
}

/// [`apply_overrides`] on parsed JSON.
pub fn apply_json(
    json: &serde_json::Value,
    shader_id: &str,
    set: &mut dyn FnMut(&str, f32) -> bool,
) {
    let obj = json
        .pointer(&format!("/shaderParams/{shader_id}"))
        .or_else(|| json.get("configShaderParams"))
        .unwrap_or(json);
    let Some(map) = obj.as_object() else {
        eprintln!("{shader_id}: tuning file holds no object of values");
        return;
    };
    for (key, v) in map {
        let ok = match v {
            serde_json::Value::Number(n) => n.as_f64().is_some_and(|f| set(key, f as f32)),
            serde_json::Value::Bool(b) => set(key, if *b { 1.0 } else { 0.0 }),
            serde_json::Value::String(s) => match parse_hex(s) {
                Some(rgb) => [".r", ".g", ".b"]
                    .iter()
                    .zip(rgb)
                    .all(|(suffix, c)| set(&format!("{key}{suffix}"), c as f32)),
                None => false,
            },
            _ => false,
        };
        if !ok {
            eprintln!("{shader_id}: ignoring {key} = {v}");
        }
    }
}

fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([c(0)?, c(2)?, c(4)?])
}

/// Every `uniform` a GLSL source declares, by name — for the tests that hold a
/// table and its shader to each other.
#[cfg(test)]
pub fn declared_uniforms(glsl: &str) -> Vec<String> {
    glsl.lines()
        .filter_map(|l| l.trim().strip_prefix("uniform "))
        .filter_map(|rest| {
            let decl = rest.split(';').next()?;
            decl.split_whitespace().last().map(str::to_string)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    shader_params! {
        /// A tiny table for testing the macro itself.
        pub struct Demo {
            gain: f32 = "gain", 1.0, 0.0 ..= 2.0;
            glow_on: bool = "glowOn", true;
            tint: [u8; 3] = "tint", [10, 20, 30];
        }
    }

    #[test]
    fn keys_become_flat_draw_uniform_names() {
        assert_eq!(uniform_name("vibrance"), "uVibrance");
        assert_eq!(uniform_name("lamp0On"), "uLamp0On");
        assert_eq!(Demo::uniform_names(), ["uGain", "uGlowOn", "uTint"]);
    }

    #[test]
    fn set_reads_flat_draw_config_format_and_clamps() {
        let mut d = Demo::default();
        assert!(d.set("gain", 9.0));
        assert_eq!(d.gain, 2.0, "clamped to the row's range");
        assert!(d.set("glowOn", 0.0));
        assert!(!d.glow_on);
        assert!(d.set("tint.g", 255.0));
        assert_eq!(d.tint, [10, 255, 30]);
        assert!(!d.set("tint", 1.0), "a colour needs a channel");
        assert!(!d.set("nope", 1.0));
    }

    #[test]
    fn values_upload_in_row_order() {
        let d = Demo::default();
        assert_eq!(
            d.values(),
            vec![
                Uniform::Float(1.0),
                Uniform::Float(1.0),
                Uniform::Vec3([10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0]),
            ]
        );
    }

    #[test]
    fn overrides_accept_every_shape_flat_draw_writes() {
        let config = serde_json::json!({
            "modelStyle3d": "lampula",
            "shaderParams": { "demo": { "gain": 0.5, "tint.r": 1 } }
        });
        let snapshot = serde_json::json!({ "configShaderParams": { "gain": 0.25 } });
        let flat = serde_json::json!({ "glowOn": false, "tint": "#FFC76B" });

        let mut d = Demo::default();
        apply_json(&config, "demo", &mut |k, v| d.set(k, v));
        assert_eq!((d.gain, d.tint[0]), (0.5, 1));
        apply_json(&snapshot, "demo", &mut |k, v| d.set(k, v));
        assert_eq!(d.gain, 0.25);
        apply_json(&flat, "demo", &mut |k, v| d.set(k, v));
        assert!(!d.glow_on);
        assert_eq!(d.tint, [0xFF, 0xC7, 0x6B]);
    }

    #[test]
    fn declared_uniforms_parses_glsl() {
        let src = "uniform float uA;\nuniform vec3  uB; // x\n  uniform mat4 mvp;\nin vec3 p;";
        assert_eq!(declared_uniforms(src), ["uA", "uB", "mvp"]);
    }
}
