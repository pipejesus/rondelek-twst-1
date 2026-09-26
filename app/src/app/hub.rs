//! The child's hub (sampler / games / voice check), and opening or creating
//! sampler sessions.

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

    /// The sampler tile: carry on with the latest board, or start the first one.
    fn continue_sampler(&mut self) {
        if self.profile_sessions.is_empty() {
            self.start_new_session();
        } else {
            self.open_session_info(0);
        }
    }

    /// The child's hub: their picture and name, then three big tiles
    /// (sounds, games, voice check). No lists to browse: the sampler tile
    /// simply continues the latest board.
    pub(super) fn draw_hub(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        shell::background(ui.painter(), full);
        let ctx = ui.ctx().clone();
        let Some(profile) = self.current_profile.clone() else {
            self.screen = AppScreen::Home;
            return;
        };
        let avatar = self.avatar_texture(&ctx, &profile);
        let face_color = shell::tile_color(&profile.manifest.uid);

        let (back, gear) = self.top_bar(ui, full, true, true);
        if back {
            self.go_home();
            return;
        }
        if gear {
            self.open_settings(Some(Section::Child));
            return;
        }

        let calibrated = self.current_calibration.is_some();
        let mut action: Option<HubAction> = None;
        let content = Rect::from_min_max(Pos2::new(full.left(), full.top() + 20.0), full.max);
        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content));
        egui::ScrollArea::vertical().show(&mut content_ui, |ui| {
            ui.vertical_centered(|ui| {
                // Centre the whole block vertically when there's room.
                let block_h = 140.0 + 70.0 + 260.0 + 90.0;
                ui.add_space(((ui.available_height() - block_h) / 2.0).max(24.0));
                // Header: picture + name (+ a small edit key for grown-ups).
                let (pic, _) = ui.allocate_exact_size(Vec2::splat(140.0), Sense::hover());
                ui.painter()
                    .rect_filled(pic.expand(6.0), 34.0, shell::shade(face_color, 0.2));
                shell::paint_avatar(ui, pic, avatar, face_color, &self.theme);
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let name = egui::RichText::new(profile.name()).size(34.0).strong();
                    let name_w = ui.fonts_mut(|f| {
                        f.layout_no_wrap(
                            profile.name().to_string(),
                            egui::FontId::proportional(34.0),
                            palette::CHARCOAL,
                        )
                        .size()
                        .x
                    });
                    ui.add_space(((ui.available_width() - name_w) / 2.0 - 30.0).max(0.0));
                    ui.label(name);
                    if ui
                        .add(
                            KeyButton::icon(Icon::Pencil, self.i18n.t("settings.child.edit"))
                                .size(Vec2::splat(40.0)),
                        )
                        .clicked()
                    {
                        action = Some(HubAction::Edit);
                    }
                });
                ui.add_space(22.0);

                let voice_sub = if calibrated {
                    self.i18n.t("hub.voice_done")
                } else {
                    self.i18n.t("hub.voice_needed")
                };
                let tiles = [
                    (
                        HubAction::Sampler,
                        Icon::Pads,
                        self.i18n.t("hub.sampler"),
                        None,
                        palette::SKY,
                    ),
                    (
                        HubAction::Games,
                        Icon::Star,
                        self.i18n.t("hub.games"),
                        None,
                        palette::LILAC,
                    ),
                    (
                        HubAction::Voice,
                        Icon::Mic,
                        self.i18n.t("hub.voice"),
                        Some(voice_sub),
                        if calibrated {
                            palette::MINT
                        } else {
                            palette::BUTTER
                        },
                    ),
                ];
                let avail = ui.available_width() - 48.0;
                let gap = 24.0;
                if avail >= 3.0 * 200.0 + 2.0 * gap {
                    let w = ((avail - 2.0 * gap) / 3.0).min(250.0);
                    let size = Vec2::new(w, w * 0.95);
                    let row_w = 3.0 * w + 2.0 * gap;
                    let (row, _) = ui.allocate_exact_size(Vec2::new(row_w, size.y), Sense::hover());
                    for (k, (act, icon, label, sub, face)) in tiles.into_iter().enumerate() {
                        let r = Rect::from_min_size(
                            row.min + Vec2::new(k as f32 * (w + gap), 0.0),
                            size,
                        );
                        if big_tile(ui, r, icon, label, sub, face, true).clicked() {
                            action = Some(act);
                        }
                    }
                } else {
                    let size = Vec2::new(avail.min(480.0), 104.0);
                    for (act, icon, label, sub, face) in tiles {
                        let (r, _) = ui.allocate_exact_size(size, Sense::hover());
                        if big_tile(ui, r, icon, label, sub, face, false).clicked() {
                            action = Some(act);
                        }
                        ui.add_space(8.0);
                    }
                }

                // For grown-ups: start over with empty pads.
                if !self.profile_sessions.is_empty() {
                    ui.add_space(26.0);
                    if ui
                        .add(
                            KeyButton::new(self.i18n.t("hub.new_session"))
                                .with_icon(Icon::Refresh)
                                .size(Vec2::new(300.0, 48.0))
                                .font(16.0),
                        )
                        .clicked()
                    {
                        action = Some(HubAction::NewSession);
                    }
                }
                ui.add_space(24.0);
            });
        });

        match action {
            Some(HubAction::Sampler) => self.continue_sampler(),
            Some(HubAction::NewSession) => self.start_new_session(),
            Some(HubAction::Games) => {
                self.game_error = None;
                self.screen = AppScreen::Games;
            }
            Some(HubAction::Voice) => self.begin_calibration(),
            Some(HubAction::Edit) => self.begin_edit_profile(),
            None => {}
        }
    }
}

#[derive(Clone, Copy)]
enum HubAction {
    Sampler,
    NewSession,
    Games,
    Voice,
    Edit,
}

/// A big hub tile: a keycap with a large icon and a label (and an optional
/// small status line). `stacked` = icon above text; otherwise icon left.
pub(super) fn big_tile(
    ui: &mut Ui,
    rect: Rect,
    icon: Icon,
    label: &str,
    sub: Option<&str>,
    face: Color32,
    stacked: bool,
) -> egui::Response {
    let resp = ui.interact(rect, ui.id().with(("big_tile", label)), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    let pressed = resp.is_pointer_button_down_on();
    let f = shell::draw_keycap(ui.painter(), rect, face, resp.hovered(), pressed);
    let ink = shell::ink_on(face);
    let p = ui.painter();
    if stacked {
        let icon_s = f.height() * 0.38;
        let icon_r = Rect::from_center_size(
            Pos2::new(f.center().x, f.top() + f.height() * 0.36),
            Vec2::splat(icon_s),
        );
        shell::draw_icon(p, icon, icon_r, ink);
        p.text(
            Pos2::new(f.center().x, f.top() + f.height() * 0.72),
            Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(24.0),
            ink,
        );
        if let Some(sub) = sub {
            p.text(
                Pos2::new(f.center().x, f.top() + f.height() * 0.86),
                Align2::CENTER_CENTER,
                sub,
                egui::FontId::proportional(15.0),
                ink.gamma_multiply(0.8),
            );
        }
    } else {
        let icon_s = f.height() * 0.55;
        let icon_r = Rect::from_center_size(
            Pos2::new(f.left() + 22.0 + icon_s / 2.0, f.center().y),
            Vec2::splat(icon_s),
        );
        shell::draw_icon(p, icon, icon_r, ink);
        let x = icon_r.right() + 22.0;
        let y = if sub.is_some() {
            f.center().y - 11.0
        } else {
            f.center().y
        };
        p.text(
            Pos2::new(x, y),
            Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(24.0),
            ink,
        );
        if let Some(sub) = sub {
            p.text(
                Pos2::new(x, f.center().y + 16.0),
                Align2::LEFT_CENTER,
                sub,
                egui::FontId::proportional(15.0),
                ink.gamma_multiply(0.8),
            );
        }
    }
    resp
}
