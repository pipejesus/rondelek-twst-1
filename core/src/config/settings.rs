use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ponytail: codes only (no endonym/display name — that's a UI concern that
// lives in `app/src/i18n`'s `EUROPEAN_LANGS`), kept in sync with that list by
// hand. Needed here, not there, because Settings' default must not depend on
// the app crate (i18n is app-side; this crate stays GUI-toolkit-free so the
// game binary can link it without pulling raylib and winit into one exe).
const SUPPORTED_LANGS: &[&str] = &[
    "en", "pl", "de", "fr", "es", "it", "uk", "pt", "nl", "sv", "no", "da", "fi", "is", "et", "lv",
    "lt", "cs", "sk", "sl", "hu", "ro", "bg", "el", "hr", "sr", "bs", "mk", "sq", "ga", "mt", "be",
    "ru", "tr", "lb", "ca",
];

fn default_language() -> String {
    let locale = sys_locale::get_locale().unwrap_or_default();
    let primary = locale.split(['-', '_']).next().unwrap_or("").to_lowercase();
    if SUPPORTED_LANGS.contains(&primary.as_str()) {
        primary
    } else {
        "en".to_string()
    }
}

/// Lower dB floor of the visualizer's display window (display-only; never touches
/// the recorded signal). Lower = more sensitive = more movement.
fn default_visualizer_floor_db() -> f32 {
    -60.0
}

fn default_vowel_voicing_threshold() -> f32 {
    0.012
}
fn default_vowel_smoothing() -> f32 {
    0.5
}
fn default_vowel_show_threshold() -> f32 {
    0.4
}
fn default_vowel_margin_threshold() -> f32 {
    0.15
}
fn default_true() -> bool {
    true
}
fn default_game_reaction() -> f32 {
    0.5
}

/// User preferences, persisted as `settings.json` in the OS config dir.
///
/// **This file already exists on users' machines — never break it.** The
/// container-level `#[serde(default)]` means any missing field takes its value
/// from [`Settings::default`]; a field of the wrong type is dropped on its own
/// by [`Settings::load_from`] rather than resetting everything. New fields need
/// a default; renamed fields need `#[serde(alias = "old_name")]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub volume: f32,
    /// Installed skin folder name; None = the built-in base skin.
    #[serde(default)]
    pub skin: Option<String>,
    pub visualizer_smoothing: f32,
    pub visualizer_decay: f32,
    pub visualizer_num_bars: usize,
    /// Lower edge (dB) of the visualizer's display window; the response is
    /// logarithmic between this floor and a fixed ceiling. Display-only.
    #[serde(default = "default_visualizer_floor_db")]
    pub visualizer_floor_db: f32,
    /// Index of the active visualizer (cycled with the button next to REC).
    #[serde(default)]
    pub active_visualizer: usize,
    /// Vowel detector: RMS below this reads as silence / unvoiced.
    #[serde(default = "default_vowel_voicing_threshold")]
    pub vowel_voicing_threshold: f32,
    /// Vowel visualizer: match-meter smoothing (0 = snappy, → 1 = sluggish).
    #[serde(default = "default_vowel_smoothing")]
    pub vowel_smoothing: f32,
    /// Vowel visualizer: a vowel lights up only when its smoothed match clears this.
    #[serde(default = "default_vowel_show_threshold")]
    pub vowel_show_threshold: f32,
    /// Vowel visualizer: and only when it beats the runner-up by this margin
    /// (the firm separation between the child's own vowels).
    #[serde(default = "default_vowel_margin_threshold")]
    pub vowel_margin_threshold: f32,
    /// Steady detection: classify a rolling average of recent voiced frames
    /// (~0.25 s) instead of each frame alone. Much more stable on close vowel
    /// pairs; costs a touch of onset latency.
    #[serde(default = "default_true")]
    pub vowel_steady: bool,
    /// Games' pre-game Reaction slider (0 = turtle/steady, 1 = rabbit/snappy).
    /// Maps to the steady-window length and smoothing inside games only.
    #[serde(default = "default_game_reaction")]
    pub game_reaction: f32,
    /// UI language code. Defaults to the detected system language on first run.
    #[serde(default = "default_language")]
    pub language: String,
    /// Pinned input device name; None = follow system default.
    #[serde(default)]
    pub input_device: Option<String>,
    /// Pinned output device name; None = follow system default.
    #[serde(default)]
    pub output_device: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            skin: None,
            visualizer_smoothing: 0.7,
            visualizer_decay: 0.4,
            visualizer_num_bars: 36,
            visualizer_floor_db: default_visualizer_floor_db(),
            active_visualizer: 0,
            vowel_voicing_threshold: default_vowel_voicing_threshold(),
            vowel_smoothing: default_vowel_smoothing(),
            vowel_show_threshold: default_vowel_show_threshold(),
            vowel_margin_threshold: default_vowel_margin_threshold(),
            vowel_steady: true,
            game_reaction: 0.5,
            language: default_language(),
            input_device: None,
            output_device: None,
        }
    }
}

impl Settings {
    /// Load the user's settings from the OS config dir (see [`Self::load_from`]).
    pub fn load() -> (Self, PathBuf) {
        let path = settings_path();
        (Self::load_from(&path), path)
    }

    /// Load settings from `path`, salvaging as much as possible:
    ///
    /// - missing file → defaults (and the file is created);
    /// - missing fields → their defaults;
    /// - a field with an unusable value → only that field falls back;
    /// - not JSON at all → defaults.
    ///
    /// Whenever the file could not be read as-is, it is first copied to
    /// `settings.broken-<unix secs>.json` next to it, so nothing the user set is
    /// ever silently destroyed.
    pub fn load_from(path: &Path) -> Self {
        let Ok(content) = std::fs::read_to_string(path) else {
            let defaults = Self::default();
            let _ = defaults.save_to(path);
            return defaults;
        };
        if let Ok(settings) = serde_json::from_str::<Self>(&content) {
            return settings;
        }

        let backup =
            path.with_file_name(format!("settings.broken-{}.json", crate::util::now_secs()));
        let _ = std::fs::copy(path, &backup);
        eprintln!(
            "settings.json could not be read as-is; salvaged what was valid (original kept at {})",
            backup.display()
        );
        let recovered = Self::salvage(&content);
        let _ = recovered.save_to(path);
        recovered
    }

    /// Build settings from a partly-invalid document: start from the defaults
    /// and apply each field from `content` that still deserializes.
    fn salvage(content: &str) -> Self {
        let defaults = Self::default();
        let Ok(serde_json::Value::Object(fields)) =
            serde_json::from_str::<serde_json::Value>(content)
        else {
            return defaults;
        };
        let Ok(serde_json::Value::Object(mut merged)) = serde_json::to_value(&defaults) else {
            return defaults;
        };
        for (key, value) in fields {
            let previous = merged.insert(key.clone(), value);
            let ok =
                serde_json::from_value::<Self>(serde_json::Value::Object(merged.clone())).is_ok();
            if !ok {
                match previous {
                    Some(v) => merged.insert(key, v),
                    None => merged.remove(&key),
                };
            }
        }
        serde_json::from_value(serde_json::Value::Object(merged)).unwrap_or(defaults)
    }

    pub fn save(&self, path: &Path) {
        if let Err(e) = self.save_to(path) {
            eprintln!("Failed to save settings: {e:#}");
        }
    }

    /// Write atomically: a temp file next to the target, then rename over it,
    /// so a crash or a second writer (the game process) never leaves a
    /// half-written `settings.json`.
    fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create settings directory")?;
        }
        let json = serde_json::to_string_pretty(self).context("Failed to serialize settings")?;
        let tmp = path.with_extension(format!("json.tmp-{}", std::process::id()));
        std::fs::write(&tmp, json).context("Failed to write settings temp file")?;
        std::fs::rename(&tmp, path).context("Failed to replace settings file")?;
        Ok(())
    }
}

/// Named bundles of the five vowel-detection knobs, so a parent or therapist
/// can pick "how picky" recognition is without touching raw thresholds.
/// Nothing extra is stored: the settings page shows the preset whose values
/// match the current settings (or "custom").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DetectionPreset {
    /// Reacts to quieter, less clear sounds (more hits, more mix-ups).
    Relaxed,
    /// The shipped defaults.
    Normal,
    /// Needs a clear, confident sound (fewer mix-ups, fewer hits).
    Strict,
}

impl DetectionPreset {
    pub const ALL: [Self; 3] = [Self::Relaxed, Self::Normal, Self::Strict];

    /// (voicing, smoothing, show, margin, steady)
    fn values(self) -> (f32, f32, f32, f32, bool) {
        match self {
            Self::Relaxed => (0.008, 0.4, 0.3, 0.08, true),
            Self::Normal => (
                default_vowel_voicing_threshold(),
                default_vowel_smoothing(),
                default_vowel_show_threshold(),
                default_vowel_margin_threshold(),
                true,
            ),
            Self::Strict => (0.02, 0.6, 0.5, 0.25, true),
        }
    }

    pub fn apply(self, s: &mut Settings) {
        let (voicing, smoothing, show, margin, steady) = self.values();
        s.vowel_voicing_threshold = voicing;
        s.vowel_smoothing = smoothing;
        s.vowel_show_threshold = show;
        s.vowel_margin_threshold = margin;
        s.vowel_steady = steady;
    }

    /// The preset `s` currently matches, if any.
    pub fn matching(s: &Settings) -> Option<Self> {
        let close = |a: f32, b: f32| (a - b).abs() < 1e-4;
        Self::ALL.into_iter().find(|p| {
            let (voicing, smoothing, show, margin, steady) = p.values();
            close(s.vowel_voicing_threshold, voicing)
                && close(s.vowel_smoothing, smoothing)
                && close(s.vowel_show_threshold, show)
                && close(s.vowel_margin_threshold, margin)
                && s.vowel_steady == steady
        })
    }
}

fn settings_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("rondelek").join("settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_device_fields_default_to_none() {
        // A settings JSON written before this feature existed.
        let json = r#"{
            "volume": 0.8,
            "dark_mode": false,
            "visualizer_smoothing": 0.7,
            "visualizer_decay": 0.4,
            "visualizer_num_bars": 36,
            "language": "en"
        }"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.input_device, None);
        assert_eq!(s.output_device, None);
        // Field added later must fall back to its default, not fail the parse.
        assert_eq!(s.visualizer_floor_db, default_visualizer_floor_db());
    }

    /// A complete `settings.json` in the shape installs have on disk as of
    /// 2026-09, every field non-default. It must load exactly as written —
    /// this is the "never break users' settings" guard.
    #[test]
    fn current_settings_file_loads_unchanged() {
        let json = include_str!("../../tests/fixtures/settings-2026-09.json");
        let s: Settings = serde_json::from_str(json).expect("settings parse");
        assert_eq!(s.volume, 0.6);
        assert_eq!(s.skin.as_deref(), Some("sunny"));
        assert_eq!(s.visualizer_num_bars, 48);
        assert_eq!(s.visualizer_floor_db, -54.0);
        assert_eq!(s.active_visualizer, 1);
        assert_eq!(s.vowel_voicing_threshold, 0.02);
        assert!(!s.vowel_steady);
        assert_eq!(s.game_reaction, 0.75);
        assert_eq!(s.language, "pl");
        assert_eq!(s.input_device.as_deref(), Some("USB Microphone"));
        // And it survives a save/load cycle.
        let back: Settings = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn missing_original_fields_fall_back_instead_of_failing() {
        // `volume` and the first visualizer knobs used to be required.
        let s: Settings = serde_json::from_str(r#"{ "language": "de" }"#).unwrap();
        assert_eq!(s.language, "de");
        assert_eq!(s.volume, Settings::default().volume);
        assert_eq!(
            s.visualizer_num_bars,
            Settings::default().visualizer_num_bars
        );
    }

    fn temp_settings(tag: &str, content: Option<&str>) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rondelek-settings-test-{tag}-{}",
            crate::util::new_uid()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        if let Some(c) = content {
            std::fs::write(&path, c).unwrap();
        }
        path
    }

    fn broken_backups(path: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.to_string_lossy().contains("settings.broken-"))
            .collect()
    }

    #[test]
    fn one_bad_field_keeps_all_the_others() {
        let path = temp_settings(
            "badfield",
            Some(
                r#"{ "volume": "loud", "language": "pl", "input_device": "USB Mic", "visualizer_num_bars": 60 }"#,
            ),
        );
        let s = Settings::load_from(&path);
        assert_eq!(s.volume, Settings::default().volume, "bad value falls back");
        assert_eq!(s.language, "pl");
        assert_eq!(s.input_device.as_deref(), Some("USB Mic"));
        assert_eq!(s.visualizer_num_bars, 60);
        // The original is kept for the user, and the repaired file now loads cleanly.
        assert_eq!(broken_backups(&path).len(), 1);
        assert_eq!(Settings::load_from(&path), s);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn garbage_file_is_backed_up_not_lost() {
        let path = temp_settings("garbage", Some("{ this is not json"));
        let s = Settings::load_from(&path);
        assert_eq!(s.volume, Settings::default().volume);
        let backups = broken_backups(&path);
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(&backups[0]).unwrap(),
            "{ this is not json"
        );
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn missing_file_creates_defaults() {
        let path = temp_settings("missing", None);
        let s = Settings::load_from(&path);
        assert_eq!(s.volume, Settings::default().volume);
        assert!(path.exists());
        assert!(broken_backups(&path).is_empty());
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn save_is_atomic_and_leaves_no_temp_files() {
        let path = temp_settings("atomic", None);
        let s = Settings {
            game_reaction: 0.25,
            ..Settings::default()
        };
        s.save(&path);
        assert_eq!(Settings::load_from(&path), s);
        let leftovers: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name() != "settings.json")
            .collect();
        assert!(leftovers.is_empty(), "temp file left behind: {leftovers:?}");
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn defaults_are_the_normal_preset() {
        assert_eq!(
            DetectionPreset::matching(&Settings::default()),
            Some(DetectionPreset::Normal)
        );
    }

    #[test]
    fn presets_apply_and_are_recognised() {
        for preset in DetectionPreset::ALL {
            let mut s = Settings::default();
            preset.apply(&mut s);
            assert_eq!(DetectionPreset::matching(&s), Some(preset));
        }
    }

    #[test]
    fn hand_tuned_values_are_custom() {
        let mut s = Settings::default();
        s.vowel_margin_threshold = 0.33;
        assert_eq!(DetectionPreset::matching(&s), None);
    }

    #[test]
    fn applying_a_preset_touches_only_detection() {
        let mut s = Settings {
            volume: 0.3,
            language: "uk".into(),
            game_reaction: 0.9,
            ..Settings::default()
        };
        DetectionPreset::Strict.apply(&mut s);
        assert_eq!(s.volume, 0.3);
        assert_eq!(s.language, "uk");
        assert_eq!(s.game_reaction, 0.9);
    }

    #[test]
    fn device_fields_round_trip() {
        let s = Settings {
            output_device: Some("Speakers".to_string()),
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.output_device, Some("Speakers".to_string()));
        assert_eq!(back.input_device, None);
    }
}
