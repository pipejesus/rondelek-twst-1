//! The games menu and launching the game child process.

use super::*;

impl App {
    pub(super) fn draw_games(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        shell::background(ui.painter(), full);

        let playing = self.game_child.is_some();
        let building = self.game_build.is_some();
        let (back, _) = self.top_bar(ui, full, true, false);
        if back {
            self.screen = AppScreen::Hub;
            return;
        }

        let mut launch: Option<&'static str> = None;
        let mut calibrate = false;
        let content = Rect::from_min_max(Pos2::new(full.left(), full.top() + 20.0), full.max);
        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content));
        egui::ScrollArea::vertical().show(&mut content_ui, |ui| {
            ui.vertical_centered(|ui| {
                shell::title(ui, self.i18n.t("games.title"), 36.0);
                ui.add_space(14.0);
                let width = (ui.available_width() - 48.0).min(640.0);

                if self.current_calibration.is_none() {
                    ui.allocate_ui(Vec2::new(width, 0.0), |ui| {
                        shell::card_frame()
                            .fill(shell::shade(palette::BUTTER, 0.35))
                            .show(ui, |ui| {
                                ui.set_width(width - 44.0);
                                ui.horizontal(|ui| {
                                    let (r, _) =
                                        ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
                                    shell::draw_icon(ui.painter(), Icon::Mic, r, palette::CHARCOAL);
                                    ui.label(
                                        egui::RichText::new(self.i18n.t("games.calibrate_hint"))
                                            .size(16.0),
                                    );
                                });
                                if ui
                                    .add(
                                        KeyButton::new(self.i18n.t("hub.voice"))
                                            .with_icon(Icon::Mic)
                                            .face(palette::BUTTER)
                                            .size(Vec2::new(240.0, 50.0))
                                            .font(17.0),
                                    )
                                    .clicked()
                                {
                                    calibrate = true;
                                }
                            });
                    });
                    ui.add_space(14.0);
                }

                for game in rondelek_core::games::GAMES {
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 176.0), Sense::hover());
                    let resp = ui.interact(rect, ui.id().with(("game", game.id)), Sense::click());
                    let name = self.i18n.t(game.name_key);
                    let enabled = !playing && !building;
                    resp.widget_info(|| {
                        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name)
                    });
                    let pressed = enabled && resp.is_pointer_button_down_on();
                    let face = if enabled {
                        palette::PAPER
                    } else {
                        shell::shade(palette::PAPER, -0.05)
                    };
                    let f = shell::draw_keycap(
                        ui.painter(),
                        rect,
                        face,
                        enabled && resp.hovered(),
                        pressed,
                    );
                    let scene = Rect::from_min_size(
                        f.min + Vec2::new(14.0, 14.0),
                        Vec2::new(190.0, f.height() - 28.0),
                    );
                    paint_game_scene(ui.painter(), scene);
                    let x = scene.right() + 22.0;
                    ui.painter().text(
                        Pos2::new(x, f.top() + 48.0),
                        Align2::LEFT_CENTER,
                        name,
                        egui::FontId::proportional(28.0),
                        palette::CHARCOAL,
                    );
                    let tagline = ui.painter().layout(
                        self.i18n.t(game.tagline_key).to_string(),
                        egui::FontId::proportional(16.0),
                        palette::STONE,
                        (f.right() - x - 90.0).max(120.0),
                    );
                    ui.painter()
                        .galley(Pos2::new(x, f.top() + 72.0), tagline, palette::STONE);
                    let play_c = Pos2::new(f.right() - 50.0, f.center().y);
                    ui.painter().circle_filled(
                        play_c,
                        30.0,
                        if enabled {
                            palette::ORANGE
                        } else {
                            palette::STONE
                        },
                    );
                    shell::draw_icon(
                        ui.painter(),
                        Icon::Play,
                        Rect::from_center_size(play_c + Vec2::new(3.0, 0.0), Vec2::splat(34.0)),
                        Color32::WHITE,
                    );
                    if enabled && resp.clicked() {
                        launch = Some(game.id);
                    }
                    ui.add_space(16.0);
                }

                let status = if playing {
                    Some("games.playing")
                } else if building {
                    Some("games.building")
                } else {
                    None
                };
                if let Some(key) = status {
                    ui.label(egui::RichText::new(self.i18n.t(key)).size(17.0));
                }
                if let Some(err) = &self.game_error {
                    ui.label(egui::RichText::new(err).color(palette::DANGER));
                }
            });
        });

        if calibrate {
            self.begin_calibration();
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

/// A tiny painted preview of the runner: sky, a cloud, grass, a hopping hero
/// and a star, in the game's palette.
fn paint_game_scene(p: &egui::Painter, r: Rect) {
    let radius = egui::CornerRadius::same(16);
    p.rect_filled(r, radius, palette::SKY);
    let ground = Rect::from_min_max(Pos2::new(r.left(), r.bottom() - r.height() * 0.26), r.max);
    p.rect_filled(
        ground,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: 16,
            se: 16,
        },
        Color32::from_rgb(139, 205, 130),
    );
    let cloud = Pos2::new(r.left() + r.width() * 0.3, r.top() + r.height() * 0.24);
    for (dx, rr) in [(-14.0, 11.0), (0.0, 15.0), (15.0, 11.0)] {
        p.circle_filled(cloud + Vec2::new(dx, 2.0), rr, Color32::WHITE);
    }
    let hero = Rect::from_center_size(
        Pos2::new(r.left() + r.width() * 0.35, ground.top() - 30.0),
        Vec2::new(30.0, 34.0),
    );
    p.rect_filled(hero, 8.0, palette::ROSE);
    p.circle_filled(hero.center() + Vec2::new(6.0, -6.0), 3.5, palette::CHARCOAL);
    let block = Rect::from_min_size(
        Pos2::new(r.left() + r.width() * 0.62, ground.top() - 26.0),
        Vec2::new(28.0, 26.0),
    );
    p.rect_filled(block, 5.0, palette::LILAC);
    shell::draw_icon(
        p,
        Icon::Star,
        Rect::from_center_size(
            Pos2::new(r.right() - r.width() * 0.2, r.top() + r.height() * 0.3),
            Vec2::splat(30.0),
        ),
        palette::BUTTER,
    );
}
