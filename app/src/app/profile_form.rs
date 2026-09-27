//! Create / edit a child: a name and a picture. The picture is one of the
//! built-in characters, a webcam photo (shown inline, not in a pop-up) or an
//! uploaded image.

use super::*;

impl App {
    pub(super) fn begin_new_profile(&mut self) {
        self.form_mode = FormMode::Create;
        self.form_name.clear();
        self.form_avatar = AvatarChoice::Keep;
        self.form_error = None;
        self.camera = None;
        self.screen = AppScreen::ProfileForm;
    }

    /// Enter the form pre-filled with the current profile's data for editing.
    pub(super) fn begin_edit_profile(&mut self) {
        let Some(profile) = self.current_profile.as_ref() else {
            return;
        };
        self.form_mode = FormMode::Edit;
        self.form_name = profile.name().to_string();
        self.form_avatar = AvatarChoice::Keep;
        self.form_error = None;
        self.camera = None;
        self.screen = AppScreen::ProfileForm;
    }

    pub(super) fn upload_avatar_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        {
            self.form_avatar = AvatarChoice::New(path);
        }
    }

    /// Submit the profile form: create a new profile or apply edits to the
    /// current one, depending on `form_mode`.
    pub(super) fn submit_profile_form(&mut self) {
        let name = profile::sanitize_name(&self.form_name);
        if name.is_empty() {
            self.form_error = Some(self.i18n.t("form.name_required").to_string());
            return;
        }
        self.camera = None;
        match self.form_mode {
            FormMode::Create => {
                let photo = match &self.form_avatar {
                    AvatarChoice::New(p) => Some(p.clone()),
                    _ => None,
                };
                let result =
                    Profile::create(&self.form_name, photo.as_deref()).and_then(|mut p| {
                        if let AvatarChoice::Character(c) = &self.form_avatar {
                            p.set_character(c)?;
                        }
                        Ok(p)
                    });
                match result {
                    Ok(profile) => {
                        self.refresh_profiles();
                        self.select_profile(profile);
                    }
                    Err(e) => self.form_error = Some(format!("{e}")),
                }
            }
            FormMode::Edit => {
                let Some(mut profile) = self.current_profile.take() else {
                    self.screen = AppScreen::Home;
                    return;
                };
                // Evict any cached texture for this avatar path before rewriting
                // it, so the new image (or default face) renders after save.
                if let Some(path) = profile.avatar_path() {
                    self.tex_cache.remove(&path);
                }
                let result =
                    profile
                        .set_name(&self.form_name)
                        .and_then(|()| match &self.form_avatar {
                            AvatarChoice::New(p) => profile.set_avatar(p),
                            AvatarChoice::Character(c) => profile.set_character(c),
                            AvatarChoice::Remove => profile.clear_avatar(),
                            AvatarChoice::Keep => Ok(()),
                        });
                match result {
                    Ok(()) => {
                        self.refresh_profiles();
                        self.select_profile(profile);
                    }
                    Err(e) => {
                        self.form_error = Some(format!("{e}"));
                        self.current_profile = Some(profile);
                    }
                }
            }
        }
    }

    /// Open the webcam for avatar capture, or report gracefully if unavailable.
    pub(super) fn open_camera(&mut self) {
        match crate::camera::CameraSession::open() {
            Ok(cam) => {
                self.camera = Some(cam);
                self.form_error = None;
            }
            Err(_) => self.form_error = Some(self.i18n.t("camera.unavailable").to_string()),
        }
    }

    /// The live webcam view, drawn inline in the form. On capture it stores a
    /// temp PNG as the pending avatar and closes the camera.
    fn camera_panel(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let frame = self.camera.as_mut().and_then(|c| c.grab().ok());
        let tex = frame.as_ref().map(|(rgba, w, h)| {
            let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
            ctx.load_texture("camera_preview", ci, egui::TextureOptions::LINEAR)
        });

        match (&tex, &frame) {
            (Some(t), Some((_, w, h))) => {
                // Fit the native frame without stretching, then overlay the
                // centred square crop guide (what becomes the avatar).
                let (fw, fh) = (*w as f32, *h as f32);
                let scale = (440.0 / fw).min(330.0 / fh);
                let disp = egui::vec2(fw * scale, fh * scale);
                let (rect, _) = ui.allocate_exact_size(disp, Sense::hover());
                egui::Image::from_texture(egui::load::SizedTexture::new(t.id(), disp))
                    .corner_radius(16)
                    .paint_at(ui, rect);
                let side = rect.width().min(rect.height());
                let sq = Rect::from_center_size(rect.center(), egui::vec2(side, side));
                draw_crop_guide(ui.painter(), sq);
            }
            _ => shell::hint(ui, self.i18n.t("camera.unavailable")),
        }
        let mut capture = false;
        let mut cancel = false;
        ui.horizontal(|ui| {
            if ui
                .add(
                    KeyButton::new(self.i18n.t("camera.capture"))
                        .with_icon(Icon::Camera)
                        .face(palette::ORANGE)
                        .size(Vec2::new(210.0, 56.0)),
                )
                .clicked()
            {
                capture = true;
            }
            if ui
                .add(KeyButton::new(self.i18n.t("camera.cancel")).size(Vec2::new(160.0, 56.0)))
                .clicked()
            {
                cancel = true;
            }
        });
        if capture {
            if let Some((rgba, w, h)) = &frame {
                match crate::camera::save_frame_png(rgba, *w, *h) {
                    Ok(path) => self.form_avatar = AvatarChoice::New(path),
                    Err(e) => self.form_error = Some(format!("{e}")),
                }
            }
            self.camera = None;
        } else if cancel {
            self.camera = None;
        }
    }

    /// Which picture the preview shows right now.
    fn form_preview(&mut self, ctx: &egui::Context) -> Option<AvatarTex> {
        match self.form_avatar.clone() {
            AvatarChoice::New(p) => {
                let id = self.texture_from_path(ctx, &p)?;
                Some((id, self.tex_cache.get(&p).map_or([1, 1], |t| t.size())))
            }
            AvatarChoice::Character(c) => self.character_texture(ctx, &c),
            AvatarChoice::Remove => None,
            AvatarChoice::Keep => match (self.form_mode, self.current_profile.clone()) {
                (FormMode::Edit, Some(p)) => self.avatar_texture(ctx, &p),
                _ => None,
            },
        }
    }

    pub(super) fn draw_profile_form(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        shell::background(ui.painter(), full);
        let full = shell::inside_stripes(full);
        let ctx = ui.ctx().clone();

        let (back, _) = self.top_bar(ui, full, true, false);
        let cancel_to = match self.form_mode {
            FormMode::Create => AppScreen::Home,
            FormMode::Edit => AppScreen::Hub,
        };
        if back {
            self.camera = None;
            self.screen = cancel_to;
            return;
        }

        let preview = self.form_preview(&ctx);
        let face_color = match (self.form_mode, &self.current_profile) {
            (FormMode::Edit, Some(p)) => shell::tile_color(&p.manifest.uid),
            _ => shell::tile_color(&self.form_name),
        };
        let chars: Vec<(&'static str, Option<AvatarTex>)> = ui::characters::CHARACTERS
            .iter()
            .map(|(name, _)| (*name, self.character_texture(&ctx, name)))
            .collect();
        let has_picture = preview.is_some();

        let mut do_upload = false;
        let mut do_camera = false;
        let mut do_remove = false;
        let mut do_submit = false;
        let mut do_cancel = false;
        let mut picked: Option<&'static str> = None;

        let content = Rect::from_min_max(Pos2::new(full.left(), full.top() + 20.0), full.max);
        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content));
        egui::ScrollArea::vertical().show(&mut content_ui, |ui| {
            ui.vertical_centered(|ui| {
                let title_key = match self.form_mode {
                    FormMode::Create => "form.title",
                    FormMode::Edit => "form.edit_title",
                };
                shell::title(ui, self.i18n.t(title_key), 34.0);
                ui.add_space(12.0);
                let width = (ui.available_width() - 40.0).min(720.0);
                ui.allocate_ui(Vec2::new(width, 0.0), |ui| {
                    shell::card(ui, |ui| {
                        ui.set_width(width - 48.0);
                        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                        // Picture + name.
                        ui.horizontal(|ui| {
                            let (pic, _) = ui.allocate_exact_size(Vec2::splat(150.0), Sense::hover());
                            shell::paint_avatar(ui, pic, preview, face_color, &self.theme);
                            ui.add_space(12.0);
                            ui.vertical(|ui| {
                                ui.add_space(20.0);
                                ui.label(egui::RichText::new(self.i18n.t("form.name")).size(18.0).strong());
                                let resp = ui.add(
                                    egui::TextEdit::singleline(&mut self.form_name)
                                        .hint_text(self.i18n.t("form.name_hint"))
                                        .font(egui::FontId::proportional(26.0))
                                        .desired_width(ui.available_width().min(380.0))
                                        .margin(Vec2::new(12.0, 10.0)),
                                );
                                if resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                    do_submit = true;
                                }
                                if let Some(err) = &self.form_error {
                                    ui.label(egui::RichText::new(err).color(palette::DANGER));
                                }
                            });
                        });

                        ui.add_space(14.0);
                        ui.label(egui::RichText::new(self.i18n.t("form.picture")).size(18.0).strong());
                        ui.add_space(4.0);
                        if self.camera.is_some() {
                            ui.vertical_centered(|ui| self.camera_panel(ui));
                        } else {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing = Vec2::splat(12.0);
                                for (name, tex) in &chars {
                                    let on = matches!(&self.form_avatar, AvatarChoice::Character(c) if c == name);
                                    if character_key(ui, name, *tex, on, &self.theme).clicked() {
                                        picked = Some(name);
                                    }
                                }
                            });
                            ui.add_space(8.0);
                            ui.horizontal_wrapped(|ui| {
                                if ui
                                    .add(
                                        KeyButton::new(self.i18n.t("form.take_photo"))
                                            .with_icon(Icon::Camera)
                                            .size(Vec2::new(200.0, 50.0))
                                            .font(16.0),
                                    )
                                    .clicked()
                                {
                                    do_camera = true;
                                }
                                if ui
                                    .add(
                                        KeyButton::new(self.i18n.t("form.upload"))
                                            .with_icon(Icon::Folder)
                                            .size(Vec2::new(200.0, 50.0))
                                            .font(16.0),
                                    )
                                    .clicked()
                                {
                                    do_upload = true;
                                }
                                if has_picture
                                    && ui
                                        .add(
                                            KeyButton::new(self.i18n.t("form.remove_photo"))
                                                .with_icon(Icon::Close)
                                                .size(Vec2::new(200.0, 50.0))
                                                .font(16.0),
                                        )
                                        .clicked()
                                {
                                    do_remove = true;
                                }
                            });
                        }
                        });
                    });
                });

                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    let row_w = 180.0 + 12.0 + 260.0;
                    ui.add_space(((ui.available_width() - row_w) / 2.0).max(0.0));
                    if ui
                        .add(KeyButton::new(self.i18n.t("form.cancel")).size(Vec2::new(180.0, 60.0)))
                        .clicked()
                    {
                        do_cancel = true;
                    }
                    let submit_key = match self.form_mode {
                        FormMode::Create => "form.create",
                        FormMode::Edit => "form.save",
                    };
                    if ui
                        .add(
                            KeyButton::new(self.i18n.t(submit_key))
                                .with_icon(Icon::Check)
                                .face(palette::ORANGE)
                                .size(Vec2::new(260.0, 60.0))
                                .font(21.0),
                        )
                        .clicked()
                    {
                        do_submit = true;
                    }
                });
                ui.add_space(24.0);
            });
        });

        if let Some(name) = picked {
            self.form_avatar = AvatarChoice::Character(name.to_string());
        }
        if do_upload {
            self.upload_avatar_dialog();
        }
        if do_camera {
            self.open_camera();
        }
        if do_remove {
            self.form_avatar = AvatarChoice::Remove;
        }
        if do_cancel {
            self.camera = None;
            self.screen = cancel_to;
        } else if do_submit {
            self.submit_profile_form();
        }
    }
}

/// One character in the picker: its picture on a keycap, ringed when chosen.
fn character_key(
    ui: &mut Ui,
    name: &str,
    tex: Option<AvatarTex>,
    selected: bool,
    theme: &rondelek_core::config::Theme,
) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(92.0), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    if selected || resp.hovered() {
        let c = if selected {
            palette::CYAN
        } else {
            palette::MIST
        };
        shell::highlight(ui.painter(), rect, shell::PX, c);
    }
    let pressed = resp.is_pointer_button_down_on();
    let face = shell::draw_keycap(
        ui.painter(),
        rect,
        palette::SURFACE,
        resp.hovered(),
        pressed,
    );
    shell::paint_avatar(ui, face.shrink(4.0), tex, palette::SURFACE, theme);
    resp
}
