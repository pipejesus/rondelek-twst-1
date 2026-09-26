//! Create / edit a child profile: name, avatar upload, webcam capture.

use super::*;

impl App {
    pub(super) fn begin_new_profile(&mut self) {
        self.form_mode = FormMode::Create;
        self.form_name.clear();
        self.form_avatar = AvatarChoice::Keep;
        self.form_error = None;
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
        match self.form_mode {
            FormMode::Create => {
                let avatar = match &self.form_avatar {
                    AvatarChoice::New(p) => Some(p.clone()),
                    _ => None,
                };
                match Profile::create(&self.form_name, avatar.as_deref()) {
                    Ok(profile) => {
                        self.refresh_profiles();
                        self.select_profile(profile);
                    }
                    Err(e) => self.form_error = Some(format!("{e}")),
                }
            }
            FormMode::Edit => {
                let Some(mut profile) = self.current_profile.take() else {
                    self.screen = AppScreen::Profiles;
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

    /// Live camera modal shown over the new-profile form. Returns nothing; on
    /// capture it stores a temp PNG as the pending avatar and closes the camera.
    pub(super) fn camera_modal(&mut self, ui: &mut Ui) {
        if self.camera.is_none() {
            return;
        }
        let ctx = ui.ctx().clone();
        let frame = self.camera.as_mut().and_then(|c| c.grab().ok());
        let tex = frame.as_ref().map(|(rgba, w, h)| {
            let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
            ctx.load_texture("camera_preview", ci, egui::TextureOptions::LINEAR)
        });

        let mut capture = false;
        let mut cancel = false;
        egui::Window::new(self.i18n.t("camera.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(&ctx, |ui| {
                match (&tex, &frame) {
                    (Some(t), Some((_, w, h))) => {
                        // Fit the native frame into the preview box without
                        // stretching, then overlay the centred square crop guide.
                        let (fw, fh) = (*w as f32, *h as f32);
                        let scale = (400.0 / fw).min(300.0 / fh);
                        let disp = egui::vec2(fw * scale, fh * scale);
                        let (rect, _) = ui.allocate_exact_size(disp, Sense::hover());
                        let p = ui.painter();
                        p.image(t.id(), rect, uv_full(), Color32::WHITE);
                        let side = rect.width().min(rect.height());
                        let sq = Rect::from_center_size(rect.center(), egui::vec2(side, side));
                        draw_crop_guide(p, sq);
                    }
                    _ => {
                        ui.label(self.i18n.t("camera.unavailable"));
                    }
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let cap = egui::Button::new(
                        egui::RichText::new(self.i18n.t("camera.capture")).color(Color32::WHITE),
                    )
                    .fill(ORANGE);
                    if ui.add_sized([150.0, 36.0], cap).clicked() {
                        capture = true;
                    }
                    if ui
                        .add_sized(
                            [120.0, 36.0],
                            egui::Button::new(self.i18n.t("camera.cancel")),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                });
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

    pub(super) fn draw_profile_form(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);
        let ctx = ui.ctx().clone();

        // Resolve which image the preview should show:
        //  - New(p)  → the freshly chosen image
        //  - Remove  → none (default face)
        //  - Keep    → Edit: the profile's existing avatar; Create: none
        let preview_path = match &self.form_avatar {
            AvatarChoice::New(p) => Some(p.clone()),
            AvatarChoice::Remove => None,
            AvatarChoice::Keep => self
                .current_profile
                .as_ref()
                .filter(|_| self.form_mode == FormMode::Edit)
                .and_then(|p| p.avatar_path()),
        };
        // Load the preview texture and remember its pixel size so the preview can
        // centre-crop (cover) instead of stretching a non-square source.
        let avatar_tex = preview_path.as_ref().and_then(|p| {
            self.texture_from_path(&ctx, p)
                .map(|id| (id, self.tex_cache.get(p).map_or([1, 1], |t| t.size())))
        });

        let mut do_upload = false;
        let mut do_camera = false;
        let mut do_remove_photo = false;
        let mut do_submit = false;
        let mut do_cancel = false;

        ui.vertical_centered(|ui| {
            ui.add_space((full.height() * 0.10).min(72.0));
            let title_key = match self.form_mode {
                FormMode::Create => "form.title",
                FormMode::Edit => "form.edit_title",
            };
            ui.label(
                egui::RichText::new(self.i18n.t(title_key))
                    .color(self.theme.text_primary)
                    .size(24.0)
                    .strong(),
            );
            ui.add_space(16.0);

            // Avatar preview.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(140.0, 140.0), Sense::hover());
            let p = ui.painter();
            if let Some((id, size)) = avatar_tex {
                p.rect_filled(rect, 14.0, self.theme.panel_fg);
                p.image(id, rect.shrink(4.0), cover_uv(size), Color32::WHITE);
                gloss_overlay(p, rect.shrink(4.0), 12.0);
            } else {
                p.rect_filled(rect, 14.0, self.theme.pad_play_bg);
                draw_kid_face(p, rect.shrink(10.0), &self.theme);
            }

            ui.add_space(12.0);
            ui.allocate_ui_with_layout(
                egui::vec2(298.0, 36.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let w = 145.0;
                    if ui
                        .add_sized([w, 36.0], egui::Button::new(self.i18n.t("form.upload")))
                        .clicked()
                    {
                        do_upload = true;
                    }
                    if ui
                        .add_sized([w, 36.0], egui::Button::new(self.i18n.t("form.take_photo")))
                        .clicked()
                    {
                        do_camera = true;
                    }
                },
            );

            // Edit mode: allow clearing the photo back to the default face,
            // shown only when there is actually a photo to remove.
            let has_photo = matches!(self.form_avatar, AvatarChoice::New(_))
                || (self.form_mode == FormMode::Edit
                    && matches!(self.form_avatar, AvatarChoice::Keep)
                    && self
                        .current_profile
                        .as_ref()
                        .is_some_and(|p| p.avatar_path().is_some()));
            if self.form_mode == FormMode::Edit && has_photo {
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [298.0, 32.0],
                        egui::Button::new(self.i18n.t("form.remove_photo")),
                    )
                    .clicked()
                {
                    do_remove_photo = true;
                }
            }

            ui.add_space(16.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.form_name)
                    .hint_text(self.i18n.t("form.name_hint"))
                    .desired_width(300.0),
            );

            if let Some(err) = &self.form_error {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(err).color(self.theme.led_full));
            }

            ui.add_space(18.0);
            ui.allocate_ui_with_layout(
                egui::vec2(298.0, 40.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if ui
                        .add_sized([145.0, 40.0], egui::Button::new(self.i18n.t("form.cancel")))
                        .clicked()
                    {
                        do_cancel = true;
                    }
                    let submit_key = match self.form_mode {
                        FormMode::Create => "form.create",
                        FormMode::Edit => "form.save",
                    };
                    let submit = egui::Button::new(
                        egui::RichText::new(self.i18n.t(submit_key)).color(Color32::WHITE),
                    )
                    .fill(ORANGE);
                    if ui.add_sized([145.0, 40.0], submit).clicked() {
                        do_submit = true;
                    }
                },
            );
        });

        // Camera capture modal (over the form) when active.
        self.camera_modal(ui);

        if do_upload {
            self.upload_avatar_dialog();
        }
        if do_camera {
            self.open_camera();
        }
        if do_remove_photo {
            self.form_avatar = AvatarChoice::Remove;
        }
        if do_cancel {
            // Create returns to the profile list; edit returns to the profile's
            // sessions screen (the profile is still selected).
            self.screen = match self.form_mode {
                FormMode::Create => AppScreen::Profiles,
                FormMode::Edit => AppScreen::Sessions,
            };
        }
        if do_submit {
            self.submit_profile_form();
        }
    }
}
