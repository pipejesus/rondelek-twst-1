//! The child's hub (sampler / games / voice calibration), and opening or creating
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

    pub(super) fn enter_session(&mut self, mut session: Session) {
        // Remember it as the session in use, so Sounds carries on here.
        if let Err(e) = session.mark_opened() {
            eprintln!("Could not save session.json: {e:#}");
        }
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

    /// The sampler tile: carry on with the session used last, or start the
    /// first one.
    fn continue_sampler(&mut self) {
        match profile::most_recently_used(&self.profile_sessions) {
            Some(idx) => self.open_session_info(idx),
            None => self.start_new_session(),
        }
    }

    /// The child's hub: their picture and name, then three big tiles
    /// (sounds, games, voice calibration). No lists to browse: the sampler
    /// tile simply carries on with the session used last (grown-ups pick
    /// another one on the settings page).
    pub(super) fn draw_hub(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        shell::background(ui.painter(), full);
        let full = shell::inside_stripes(full);
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
                shell::panel(ui.painter(), pic.expand(12.0), shell::PX, face_color);
                shell::paint_avatar(ui, pic, avatar, face_color, &self.theme);
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    let name_w = ui.fonts_mut(|f| {
                        f.layout_no_wrap(
                            profile.name().to_string(),
                            shell::pixel_font(36.0),
                            palette::TEXT,
                        )
                        .size()
                        .x
                    });
                    ui.add_space(((ui.available_width() - name_w) / 2.0 - 30.0).max(0.0));
                    shell::title(ui, profile.name(), 36.0);
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
                        palette::BLUE,
                    ),
                    (
                        HubAction::Games,
                        Icon::Star,
                        self.i18n.t("hub.games"),
                        None,
                        palette::VIOLET,
                    ),
                    (
                        HubAction::Voice,
                        Icon::Mic,
                        self.i18n.t("hub.voice"),
                        Some(voice_sub),
                        if calibrated {
                            palette::OK
                        } else {
                            palette::SUNFLOWER
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
    if resp.hovered() {
        shell::highlight(ui.painter(), rect, shell::PX, palette::CYAN);
    }
    let f = shell::draw_keycap(ui.painter(), rect, face, resp.hovered(), pressed);
    let ink = shell::ink_on(face);
    let p = ui.painter();
    let sub_font = egui::FontId::proportional(15.0);
    let sub_gap = 6.0;
    let sub_h = if sub.is_some() {
        sub_gap
            + p.layout_no_wrap("Ag".into(), sub_font.clone(), ink)
                .size()
                .y
    } else {
        0.0
    };
    // The label (fitted, maybe two lines) and the status line as one block,
    // centred vertically in `area`; `align` is CENTER_TOP or LEFT_TOP.
    let draw_text = |area: Rect, align: Align2| {
        let (text, size) = fit_label(p, label, area.width());
        let font = shell::pixel_font(size);
        let line_h = p.layout_no_wrap("Ag".into(), font.clone(), ink).size().y;
        let lines: Vec<&str> = text.lines().collect();
        let label_h = line_h * lines.len() as f32;
        let top = (area.center().y - (label_h + sub_h) / 2.0).round();
        let x = if align == Align2::LEFT_TOP {
            area.left()
        } else {
            area.center().x
        };
        for (i, line) in lines.iter().enumerate() {
            let y = top + i as f32 * line_h;
            p.text(Pos2::new(x, y), align, *line, font.clone(), ink);
        }
        if let Some(sub) = sub {
            p.text(
                Pos2::new(x, top + label_h + sub_gap),
                align,
                sub,
                sub_font.clone(),
                ink.gamma_multiply(0.8),
            );
        }
    };
    if stacked {
        let icon_s = f.height() * 0.38;
        let icon_r = Rect::from_center_size(
            Pos2::new(f.center().x, f.top() + f.height() * 0.30),
            Vec2::splat(icon_s),
        );
        shell::draw_icon(p, icon, icon_r, ink);
        let text_area = Rect::from_min_max(
            Pos2::new(f.left() + 12.0, icon_r.bottom() + 6.0),
            Pos2::new(f.right() - 12.0, f.bottom() - 6.0),
        );
        draw_text(text_area, Align2::CENTER_TOP);
    } else {
        let icon_s = f.height() * 0.55;
        let icon_r = Rect::from_center_size(
            Pos2::new(f.left() + 22.0 + icon_s / 2.0, f.center().y),
            Vec2::splat(icon_s),
        );
        shell::draw_icon(p, icon, icon_r, ink);
        let text_area = Rect::from_min_max(
            Pos2::new(icon_r.right() + 22.0, f.top()),
            Pos2::new(f.right() - 12.0, f.bottom()),
        );
        draw_text(text_area, Align2::LEFT_TOP);
    }
    resp
}

/// A tile label that fits `max_w`: one line in the big pixel size, else two
/// lines (split at the most central space), else the same in the small size.
/// Returns the text (with a `\n` when split) and its pixel size.
fn fit_label(p: &egui::Painter, label: &str, max_w: f32) -> (String, f32) {
    let width = |text: &str, size: f32| {
        p.layout_no_wrap(text.to_string(), shell::pixel_font(size), Color32::WHITE)
            .size()
            .x
    };
    let two_lines = split_in_two(label);
    for size in [27.0, 18.0] {
        if width(label, size) <= max_w {
            return (label.to_string(), size);
        }
        if let Some(two) = &two_lines
            && width(two, size) <= max_w
        {
            return (two.clone(), size);
        }
    }
    (two_lines.unwrap_or_else(|| label.to_string()), 18.0)
}

/// `label` broken into two lines at the space nearest its middle.
fn split_in_two(label: &str) -> Option<String> {
    let mid = label.chars().count() / 2;
    let (at, _) = label
        .char_indices()
        .enumerate()
        .filter(|(_, (_, c))| *c == ' ')
        .min_by_key(|(n, _)| n.abs_diff(mid))
        .map(|(_, ci)| ci)?;
    Some(format!("{}\n{}", &label[..at], &label[at + 1..]))
}

#[cfg(test)]
mod tests {
    use super::split_in_two;

    #[test]
    fn labels_split_at_the_middle_space() {
        assert_eq!(
            split_in_two("Voice calibration").as_deref(),
            Some("Voice\ncalibration")
        );
        assert_eq!(
            split_in_two("Calibrage de la voix").as_deref(),
            Some("Calibrage\nde la voix")
        );
        // Multi-byte text splits on a character boundary.
        assert_eq!(
            split_in_two("Калібрування голосу").as_deref(),
            Some("Калібрування\nголосу")
        );
        assert_eq!(split_in_two("Stimmkalibrierung"), None);
    }
}
