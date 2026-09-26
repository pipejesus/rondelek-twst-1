//! A profile's sessions screen, and opening/creating sessions.

use super::*;

impl App {
    pub(super) fn start_new_session(&mut self) {
        let result = self.current_profile.as_ref().map(|p| p.new_session());
        match result {
            Some(Ok(session)) => self.enter_session(session),
            Some(Err(e)) => self.audio_status = format!("Could not create session: {e}"),
            None => {}
        }
    }

    pub(super) fn open_session_info(&mut self, idx: usize) {
        let Some(dir) = self.profile_sessions.get(idx).map(|s| s.dir.clone()) else {
            return;
        };
        match Session::open(dir) {
            Ok(session) => self.enter_session(session),
            Err(e) => self.audio_status = format!("Could not open session: {e}"),
        }
    }

    /// Open a session directly from a path (screenshot/kiosk harness).
    pub(super) fn open_session_dir(&mut self, path: PathBuf) {
        match Session::open(path) {
            Ok(session) => self.enter_session(session),
            Err(e) => self.audio_status = format!("Could not open session: {e}"),
        }
    }

    pub(super) fn enter_session(&mut self, session: Session) {
        self.samples = session.load_samples(self.capture_rate);
        for (idx, sample) in self.samples.iter().enumerate() {
            if let Some(pad) = self.pads.get_mut(idx) {
                pad.has_sample = sample.has_data;
            }
        }
        self.record_mode = false;
        self.recording_active = false;
        for pad in &mut self.pads {
            pad.set_mode(PadMode::Play);
            pad.is_recording = false;
        }
        self.session = Some(session);
        self.screen = AppScreen::Session;
    }

    pub(super) fn draw_sessions(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);
        let ctx = ui.ctx().clone();

        let avatar_tex = self
            .current_profile
            .as_ref()
            .and_then(|p| p.avatar_path())
            .and_then(|p| self.texture_from_path(&ctx, &p));
        let profile_name = self
            .current_profile
            .as_ref()
            .map(|p| p.name().to_string())
            .unwrap_or_default();

        let mut back = false;
        let mut edit_profile = false;
        let mut calibrate = false;
        let mut games = false;
        let mut new_session = false;
        let mut open_idx: Option<usize> = None;

        // Back button, top-left.
        if ui
            .put(
                Rect::from_min_size(
                    Pos2::new(full.left() + 12.0, full.top() + 12.0),
                    egui::vec2(120.0, 34.0),
                ),
                egui::Button::new(format!("‹ {}", self.i18n.t("sessions.back"))),
            )
            .clicked()
        {
            back = true;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((full.height() * 0.08).min(56.0));

                // Avatar + name heading.
                let (rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 96.0), Sense::hover());
                let p = ui.painter();
                if let Some(id) = avatar_tex {
                    p.rect_filled(rect, 12.0, self.theme.panel_fg);
                    p.image(id, rect.shrink(3.0), uv_full(), Color32::WHITE);
                    gloss_overlay(p, rect.shrink(3.0), 10.0);
                } else {
                    p.rect_filled(rect, 12.0, self.theme.pad_play_bg);
                    draw_kid_face(p, rect.shrink(8.0), &self.theme);
                }
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(&profile_name)
                        .color(self.theme.text_primary)
                        .size(22.0)
                        .strong(),
                );
                ui.add_space(10.0);
                if ui
                    .add_sized(
                        [200.0, 32.0],
                        egui::Button::new(self.i18n.t("sessions.edit")),
                    )
                    .clicked()
                {
                    edit_profile = true;
                }
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [200.0, 32.0],
                        egui::Button::new(self.i18n.t("sessions.calibrate")),
                    )
                    .clicked()
                {
                    calibrate = true;
                }
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [200.0, 32.0],
                        egui::Button::new(self.i18n.t("sessions.games")),
                    )
                    .clicked()
                {
                    games = true;
                }
                ui.add_space(16.0);

                let new_btn = egui::Button::new(
                    egui::RichText::new(self.i18n.t("sessions.new"))
                        .size(16.0)
                        .color(Color32::WHITE),
                )
                .fill(ORANGE)
                .corner_radius(8.0);
                if ui.add_sized([300.0, 44.0], new_btn).clicked() {
                    new_session = true;
                }
                ui.add_space(16.0);

                if self.profile_sessions.is_empty() {
                    ui.label(
                        egui::RichText::new(self.i18n.t("sessions.none"))
                            .color(self.theme.text_secondary),
                    );
                }
                for (i, info) in self.profile_sessions.iter().enumerate() {
                    let label = info.folder.replace('_', "  ").replace('-', ".");
                    if ui
                        .add_sized([300.0, 36.0], egui::Button::new(label))
                        .clicked()
                    {
                        open_idx = Some(i);
                    }
                    ui.add_space(6.0);
                }
            });
        });

        if back {
            self.go_to_profiles();
        } else if edit_profile {
            self.begin_edit_profile();
        } else if calibrate {
            self.begin_calibration();
        } else if games {
            self.screen = AppScreen::Games;
        } else if new_session {
            self.start_new_session();
        } else if let Some(i) = open_idx {
            self.open_session_info(i);
        }
    }
}
