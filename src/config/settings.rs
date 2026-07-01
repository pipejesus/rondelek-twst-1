use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_language() -> String {
    crate::i18n::detect_system_lang()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub volume: f32,
    pub dark_mode: bool,
    pub visualizer_smoothing: f32,
    pub visualizer_decay: f32,
    pub visualizer_num_bars: usize,
    pub show_dev_panel: bool,
    /// UI language code. Defaults to the detected system language on first run.
    #[serde(default = "default_language")]
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            dark_mode: false,
            visualizer_smoothing: 0.7,
            visualizer_decay: 0.4,
            visualizer_num_bars: 36,
            show_dev_panel: false,
            language: default_language(),
        }
    }
}

impl Settings {
    pub fn load() -> (Self, PathBuf) {
        let path = settings_path();
        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(settings) => (settings, path),
                Err(_) => {
                    eprintln!("Failed to parse settings, using defaults");
                    let defaults = Self::default();
                    let _ = Self::save_to(&defaults, &path);
                    (defaults, path)
                }
            },
            Err(_) => {
                let defaults = Self::default();
                let _ = Self::save_to(&defaults, &path);
                (defaults, path)
            }
        }
    }

    pub fn save(&self, path: &PathBuf) {
        let _ = Self::save_to(self, path);
    }

    fn save_to(settings: &Self, path: &PathBuf) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create settings directory")?;
        }
        let json =
            serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;
        std::fs::write(path, json).context("Failed to write settings file")?;
        Ok(())
    }
}

fn settings_path() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("rondelek").join("settings.json")
}
