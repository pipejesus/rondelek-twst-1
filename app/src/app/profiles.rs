//! The profile picker (start screen) and language picker.

use super::*;

impl App {
    pub(super) fn draw_profiles(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);

        // Language picker, anchored top-right.
        let ctx = ui.ctx().clone();
        egui::Area::new(egui::Id::new("lang_picker"))
            .anchor(Align2::RIGHT_TOP, [-16.0, 16.0])
            .show(&ctx, |ui| {
                self.language_picker(ui);
            });

        // Settings entry (opens the F12 panel), anchored top-left.
        let settings_label = format!("⚙ {}", self.i18n.t("profiles.settings"));
        let mut open_settings = false;
        egui::Area::new(egui::Id::new("settings_entry"))
            .anchor(Align2::LEFT_TOP, [16.0, 16.0])
            .show(&ctx, |ui| {
                if ui.button(settings_label).clicked() {
                    open_settings = true;
                }
            });
        if open_settings {
            self.config_panel.visible = true;
        }

        // Preload avatar textures for the current filter so cards can read them.
        let query = self.profile_search.to_lowercase();
        let visible: Vec<usize> = self
            .profiles
            .iter()
            .enumerate()
            .filter(|(_, p)| query.is_empty() || p.name().to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        for &i in &visible {
            if let Some(path) = self.profiles[i].avatar_path() {
                let _ = self.texture_from_path(&ctx, &path);
            }
        }

        let mut clicked: Option<usize> = None;
        let mut new_profile = false;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((full.height() * 0.08).min(64.0));
                ui.label(
                    egui::RichText::new("RONDELEK · TWST-1")
                        .color(self.theme.text_primary)
                        .size(30.0)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(self.i18n.t("app.tagline"))
                        .color(self.theme.text_secondary)
                        .size(14.0),
                );
                ui.add_space(22.0);

                ui.add(
                    egui::TextEdit::singleline(&mut self.profile_search)
                        .hint_text(self.i18n.t("profiles.search"))
                        .desired_width(300.0),
                );
                ui.add_space(8.0);

                let new_btn = egui::Button::new(
                    egui::RichText::new(self.i18n.t("profiles.new"))
                        .size(16.0)
                        .color(Color32::WHITE),
                )
                .fill(ORANGE)
                .corner_radius(8.0);
                if ui.add_sized([300.0, 44.0], new_btn).clicked() {
                    new_profile = true;
                }

                ui.add_space(18.0);

                if visible.is_empty() {
                    ui.label(
                        egui::RichText::new(self.i18n.t("profiles.none"))
                            .color(self.theme.text_secondary),
                    );
                }

                for &i in &visible {
                    let name = self.profiles[i].name().to_string();
                    let tex = self.profiles[i]
                        .avatar_path()
                        .and_then(|p| self.tex_cache.get(&p).map(|t| t.id()));
                    if self.profile_card(ui, &name, tex) {
                        clicked = Some(i);
                    }
                    ui.add_space(8.0);
                }
            });
        });

        if new_profile {
            self.begin_new_profile();
        } else if let Some(i) = clicked {
            let profile = self.profiles[i].clone();
            self.select_profile(profile);
        }
    }

    /// One clickable profile row (avatar + name). Returns true when clicked.
    pub(super) fn profile_card(
        &self,
        ui: &mut Ui,
        name: &str,
        tex: Option<egui::TextureId>,
    ) -> bool {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(300.0, 60.0), Sense::click());
        let p = ui.painter();
        let bg = if resp.hovered() {
            self.theme.pad_play_hover
        } else {
            self.theme.pad_play_bg
        };
        p.rect_filled(rect, 10.0, bg);

        let pad = 8.0;
        let av = Rect::from_min_size(
            Pos2::new(rect.left() + pad, rect.top() + pad),
            Vec2::splat(rect.height() - pad * 2.0),
        );
        if let Some(id) = tex {
            p.image(id, av, uv_full(), Color32::WHITE);
            gloss_overlay(p, av, 8.0);
        } else {
            draw_kid_face(p, av, &self.theme);
        }
        p.text(
            Pos2::new(av.right() + 14.0, rect.center().y),
            Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(18.0),
            self.theme.text_primary,
        );
        resp.clicked()
    }

    pub(super) fn language_picker(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let current = self.i18n.lang().to_string();
        let mut chosen: Option<String> = None;

        egui::ComboBox::from_id_salt("lang_combo")
            .selected_text(i18n::endonym(&current))
            .width(150.0)
            .show_ui(ui, |ui| {
                for lang in EUROPEAN_LANGS {
                    let flag = self.flag_texture(&ctx, lang.code);
                    ui.horizontal(|ui| {
                        if let Some(id) = flag {
                            ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(
                                id,
                                egui::vec2(22.0, 15.0),
                            )));
                        }
                        if ui
                            .selectable_label(current == lang.code, lang.endonym)
                            .clicked()
                        {
                            chosen = Some(lang.code.to_string());
                        }
                    });
                }
            });

        if let Some(code) = chosen {
            self.set_language(&code);
        }
    }
}
