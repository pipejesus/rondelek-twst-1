//! The grown-ups page as a screen: gathers what `ui::settings_page` needs
//! (devices, the child's info, flags), draws it, and carries out its requests.

use super::*;
use crate::ui::settings_page::{ChildInfo, SettingsCtx};

impl App {
    pub(super) fn draw_settings(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        shell::background(ui.painter(), full);
        let full = shell::inside_stripes(full);
        let ctx = ui.ctx().clone();
        let dt = ui.input(|i| i.unstable_dt);

        // Keep streams up so the level meter and the test sound work here too.
        self.maintain_audio(dt);
        let mic = self
            .capture
            .as_mut()
            .map(Capture::drain)
            .unwrap_or_default();
        if !mic.is_empty() {
            self.input_peak = mic.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        }
        self.refresh_devices(dt);
        for lang in EUROPEAN_LANGS {
            let _ = self.flag_texture(&ctx, lang.code);
        }

        let (back, _) = self.top_bar(ui, full, true, false);
        shell::pixel_text(
            ui.painter(),
            Pos2::new(full.center().x, full.top() + 50.0),
            Align2::CENTER_CENTER,
            self.i18n.t("settings.title"),
            36.0,
            palette::BUTTER,
            Some(palette::INK),
        );
        if back {
            self.screen = self.settings_return;
            return;
        }

        let child_avatar = match self.current_profile.clone() {
            Some(p) => self.avatar_texture(&ctx, &p),
            None => None,
        };
        let current_input = self
            .capture
            .as_ref()
            .map(|c| c.current_device().to_string());
        let content = Rect::from_min_max(Pos2::new(full.left(), full.top() + 92.0), full.max);
        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content));
        let outcome = egui::ScrollArea::vertical()
            .show(&mut content_ui, |ui| {
                ui.add_space(8.0);
                let child = self.current_profile.as_ref().map(|p| ChildInfo {
                    name: p.name(),
                    avatar: child_avatar,
                    tile: shell::tile_color(&p.manifest.uid),
                    calibrated_at: self.current_calibration.as_ref().map(|c| c.created),
                    calibration_mic: self
                        .current_calibration
                        .as_ref()
                        .and_then(|c| c.input_device.as_deref()),
                    sessions: &self.profile_sessions,
                    current_session: profile::most_recently_used(&self.profile_sessions),
                });
                self.settings_page.show(
                    ui,
                    SettingsCtx {
                        i18n: &self.i18n,
                        theme: &self.theme,
                        settings: &mut self.settings,
                        skins: &self.available_skins,
                        outputs: &self.devices.outputs,
                        inputs: &self.devices.inputs,
                        output_default: self.devices.output_default.as_deref(),
                        input_default: self.devices.input_default.as_deref(),
                        current_input: current_input.as_deref(),
                        input_peak: self.input_peak,
                        child,
                        flags: &self.flag_cache,
                    },
                )
            })
            .inner;

        if outcome.changed {
            self.save_settings();
        }
        if outcome.visualizer_changed {
            self.active_visualizer = self
                .settings
                .active_visualizer
                .min(self.visualizers.len() - 1);
        }
        if let Some(code) = outcome.chosen_language {
            self.set_language(&code);
        }
        if let Some(choice) = outcome.chosen_skin {
            self.settings.skin = choice;
            self.skin = Skin::load(&ctx, self.settings.skin.as_deref());
            self.theme = self.skin.theme.clone();
            self.save_settings();
        }
        if outcome.test_sound {
            self.play_test_sound();
        }
        if outcome.open_data_folder
            && let Some(dir) = profile::library_root().parent()
        {
            open_folder(dir);
        }
        if outcome.open_skins_folder {
            open_folder(&ui::skin::skins_dir());
        }
        if let Some(dir) = outcome.open_session {
            // Leave whatever session is open (saved) and carry on in this one.
            self.close_session();
            self.open_session_dir(dir);
        } else if outcome.new_session {
            self.close_session();
            self.start_new_session();
        } else if outcome.recalibrate {
            self.begin_calibration();
        } else if outcome.edit_child {
            self.begin_edit_profile();
        } else if outcome.delete_child {
            self.delete_current_profile();
        }
    }

    /// Re-enumerate audio devices about once a second while the page is open.
    fn refresh_devices(&mut self, dt: f32) {
        self.devices.age += dt;
        if self.devices.age < 1.0 {
            return;
        }
        self.devices = DeviceLists {
            outputs: device::list_output_devices(),
            inputs: device::list_input_devices(),
            output_default: device::default_output_name(),
            input_default: device::default_input_name(),
            age: 0.0,
        };
    }

    /// A short, friendly two-note chime so a grown-up can check the speaker.
    fn play_test_sound(&mut self) {
        let rate = 44_100u32;
        let len = 0.6f32;
        let n = (rate as f32 * len) as usize;
        let tau = std::f32::consts::TAU;
        let buf: Vec<f32> = (0..n)
            .map(|i| {
                let t = i as f32 / rate as f32;
                // First note, then the fifth above it, with soft attack/release.
                let f = if t < len / 2.0 { 523.25 } else { 783.99 };
                let local = t % (len / 2.0);
                let env = (local * 60.0).min(1.0) * (1.0 - local / (len / 2.0)).max(0.0);
                0.35 * env * (tau * f * t).sin()
            })
            .collect();
        self.play_buffer(buf, rate);
    }

    /// Move the current child's folder to the trash and go back home.
    fn delete_current_profile(&mut self) {
        let Some(profile) = self.current_profile.take() else {
            return;
        };
        if let Some(path) = profile.avatar_path() {
            self.tex_cache.remove(&path);
        }
        if let Err(e) = profile.move_to_trash() {
            eprintln!("Failed to delete profile: {e:#}");
        }
        self.current_calibration = None;
        self.go_home();
    }
}

/// Show a folder in the system file manager (best effort).
fn open_folder(path: &Path) {
    let _ = std::fs::create_dir_all(path);
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(opener).arg(path).spawn() {
        eprintln!("Could not open {}: {e}", path.display());
    }
}
