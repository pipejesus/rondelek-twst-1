//! The games menu and launching the game child process.

use super::*;

impl App {
    pub(super) fn draw_games(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);

        let playing = self.game_child.is_some();
        let building = self.game_build.is_some();
        let mut back = false;
        let mut launch: Option<&'static str> = None;

        if ui
            .put(
                Rect::from_min_size(
                    Pos2::new(full.left() + 12.0, full.top() + 12.0),
                    egui::vec2(120.0, 34.0),
                ),
                egui::Button::new(format!("\u{2039} {}", self.i18n.t("sessions.title"))),
            )
            .clicked()
        {
            back = true;
        }

        ui.vertical_centered(|ui| {
            ui.add_space((full.height() * 0.10).min(72.0));
            ui.label(
                egui::RichText::new(self.i18n.t("games.title"))
                    .color(self.theme.text_primary)
                    .size(26.0)
                    .strong(),
            );
            ui.add_space(6.0);
            if self.current_calibration.is_none() {
                ui.label(
                    egui::RichText::new(self.i18n.t("games.calibrate_hint"))
                        .color(self.theme.text_secondary),
                );
            }
            ui.add_space(18.0);

            for game in rondelek_core::games::GAMES {
                let btn = egui::Button::new(
                    egui::RichText::new(self.i18n.t(game.name_key))
                        .size(18.0)
                        .color(Color32::WHITE),
                )
                .fill(ORANGE)
                .corner_radius(10.0);
                if ui
                    .add_enabled(!playing && !building, |ui: &mut Ui| {
                        ui.add_sized([320.0, 56.0], btn)
                    })
                    .clicked()
                {
                    launch = Some(game.id);
                }
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(self.i18n.t(game.tagline_key))
                        .color(self.theme.text_secondary),
                );
                ui.add_space(14.0);
            }

            let status = if playing {
                Some("games.playing")
            } else if building {
                Some("games.building")
            } else {
                None
            };
            if let Some(key) = status {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(self.i18n.t(key)).color(self.theme.text_primary));
            }
            if let Some(err) = &self.game_error {
                ui.add_space(8.0);
                ui.label(egui::RichText::new(err).color(self.theme.led_full));
            }
        });

        if back {
            self.screen = AppScreen::Sessions;
        } else if let Some(id) = launch {
            self.launch_game(id);
        }
    }

    /// Spawn the selected game as a child process (own fullscreen window),
    /// handing it the current profile dir for calibration. The sampler's mic
    /// is released first so the game can use it.
    ///
    /// The game runner is a separate executable (`rondelek-game`, built next
    /// to this one — see the workspace's `game` crate) rather than this same
    /// binary re-invoked with a flag: raylib and this app's windowing stack
    /// (eframe/winit) both define a `ShowCursor` symbol, which is a Windows
    /// linker error the moment both land in one binary.
    pub(super) fn launch_game(&mut self, id: &str) {
        if self.game_child.is_some() || self.game_build.is_some() {
            return;
        }
        self.game_error = None;
        // `cargo run` rebuilds only this app, so the game binary next to it can
        // be stale or missing. Under cargo, rebuild it first (a no-op when it's
        // up to date); `poll_game_build` spawns the game once that succeeds.
        if let Some(build) = dev_game_build() {
            match build {
                Ok(child) => self.game_build = Some((child, id.to_string())),
                Err(e) => {
                    self.game_error = Some(format!("{} ({e})", self.i18n.t("games.build_failed")))
                }
            }
            return;
        }
        self.spawn_game(id);
    }

    /// Poll the dev-mode game build started by `launch_game`; spawn the game
    /// when it succeeds, report when it fails.
    pub(super) fn poll_game_build(&mut self) {
        let Some((child, _)) = &mut self.game_build else {
            return;
        };
        let result = match child.try_wait() {
            Ok(None) => return,
            Ok(Some(status)) => Ok(status),
            Err(e) => Err(e),
        };
        let Some((_, id)) = self.game_build.take() else {
            return;
        };
        match result {
            Ok(status) if status.success() => self.spawn_game(&id),
            Ok(status) => {
                self.game_error = Some(format!("{} ({status})", self.i18n.t("games.build_failed")))
            }
            Err(e) => {
                self.game_error = Some(format!("{} ({e})", self.i18n.t("games.build_failed")))
            }
        }
    }

    /// Spawn the game binary that sits next to this one.
    pub(super) fn spawn_game(&mut self, id: &str) {
        self.capture = None;
        let profile_dir = self.current_profile.as_ref().map(|p| p.dir.clone());
        let game_bin = if cfg!(windows) {
            "rondelek-game.exe"
        } else {
            "rondelek-game"
        };
        let spawned = std::env::current_exe().and_then(|exe| {
            let sibling = exe.with_file_name(game_bin);
            let mut cmd = std::process::Command::new(sibling);
            cmd.arg(id);
            if let Some(dir) = profile_dir {
                cmd.arg("--profile").arg(dir);
            }
            cmd.spawn()
        });
        match spawned {
            Ok(child) => self.game_child = Some(child),
            Err(e) => {
                self.game_error = Some(format!("{} ({e})", self.i18n.t("games.launch_failed")))
            }
        }
    }
}
