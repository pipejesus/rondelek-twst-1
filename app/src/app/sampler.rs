//! The sampler screen itself: faceplate, visualizer, pads.

use super::*;

impl App {
    pub(super) fn draw_session(&mut self, ui: &mut Ui) {
        let dt = ui.input(|i| i.unstable_dt);
        self.maintain_audio(dt);
        self.drain_capture();

        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        let primary_down = ui.input(|i| i.pointer.primary_down());

        if ui.input(|i| i.key_pressed(Key::Escape)) && self.recording_active {
            self.stop_recording();
        }

        let full_bounds = ui.max_rect();
        let layout = compute_layout(full_bounds);
        let ctx = ui.ctx().clone();

        // Avatar texture for the header (current profile).
        let avatar_path = self.current_profile.as_ref().and_then(|p| p.avatar_path());
        let avatar_id = avatar_path
            .as_ref()
            .and_then(|p| self.texture_from_path(&ctx, p));

        let painter = ui.painter().clone();
        self.skin.cover(&painter, atlas::BG, full_bounds);
        Renderer::draw_case(&painter, &layout, &self.skin, &self.theme);

        let active = self.active_visualizer;
        if self.auto_shot.is_some() {
            self.visualizers[active].demo_fill(self.settings.visualizer_num_bars);
        }
        self.visualizers[active].draw(&painter, layout.screen, &self.theme, &self.settings);

        // Pads (sample pads use the grid; REC uses the header-right rect).
        for (i, rect) in layout.pads.iter().enumerate() {
            self.pads[i].set_rect(*rect);
        }
        self.pads[REC_PAD_IDX].set_rect(layout.rec);

        let pointer = pointer_pos.unwrap_or(Pos2::ZERO);
        for idx in 0..self.pads.len() {
            let kind = self.pads[idx].kind;
            let key = self.pads[idx].key;
            let sample_idx = self.pads[idx].sample_idx;
            let rect = self.pads[idx].rect;

            let key_held = ui.input(|i| i.key_down(key));
            let mouse_over = rect.contains(pointer);
            let allowed = match kind {
                PadKind::Function => true,
                PadKind::Sample => {
                    !(self.record_mode
                        && self.recording_active
                        && sample_idx != self.recording_sample_idx)
                }
            };
            let activated = allowed && (key_held || (mouse_over && primary_down));
            let (just_activated, just_released) = self.pads[idx].update(activated, dt);

            if just_activated {
                match kind {
                    PadKind::Function => {
                        self.stop_recording();
                        self.toggle_record_mode();
                    }
                    PadKind::Sample => {
                        if self.record_mode && !self.recording_active {
                            self.start_recording(sample_idx);
                        } else if !self.record_mode {
                            self.play_sample(sample_idx);
                        }
                    }
                }
            }
            if just_released
                && kind == PadKind::Sample
                && self.recording_active
                && sample_idx == self.recording_sample_idx
            {
                self.stop_recording();
            }
            self.pads[idx].draw(&painter, &self.theme, &self.skin);
        }

        // Header: skinned "back to profiles" key (left).
        let back_resp = ui.interact(
            layout.back,
            egui::Id::new("back_to_profiles"),
            Sense::click(),
        );
        draw_cap_button(
            &self.skin,
            &painter,
            atlas::BACK_CAP,
            layout.back,
            &back_resp,
        );
        if back_resp.clicked() {
            self.go_to_profiles();
            return;
        }

        // Header: cycle-visualizer key (square, just left of REC).
        let cycle_resp = ui.interact(layout.cycle, egui::Id::new("cycle_viz"), Sense::click());
        draw_cap_button(
            &self.skin,
            &painter,
            atlas::CYCLE_CAP,
            layout.cycle,
            &cycle_resp,
        );
        if cycle_resp.clicked() {
            self.cycle_visualizer();
        }

        // Header: profile avatar under the skin's frame, flush to the top edge.
        let hole = layout.avatar.shrink(layout.avatar.width() * 0.10);
        if let Some(id) = avatar_id {
            painter.image(id, hole, uv_full(), Color32::WHITE);
        } else {
            painter.rect_filled(
                hole,
                egui::CornerRadius::same(ROUNDING_PAD as u8),
                self.theme.panel_fg,
            );
            draw_kid_face(&painter, hole.shrink(hole.width() * 0.08), &self.theme);
        }
        self.skin
            .sprite(&painter, atlas::AVATAR, layout.avatar, Color32::WHITE);

        // Bottom status line: record state, else any audio error.
        let status = if self.record_mode {
            if self.recording_active {
                format!(
                    "{} {}",
                    self.i18n.t("session.recording"),
                    self.recording_sample_idx + 1
                )
            } else {
                self.i18n.t("session.rec_hold").to_string()
            }
        } else {
            self.audio_status.clone()
        };
        if !status.is_empty() {
            let color = if self.record_mode {
                self.theme.pad_record_bg
            } else {
                self.theme.led_full
            };
            painter.text(
                Pos2::new(full_bounds.center().x, full_bounds.bottom() - 8.0),
                Align2::CENTER_BOTTOM,
                status,
                egui::FontId::proportional(13.0),
                color,
            );
        }
    }
}
