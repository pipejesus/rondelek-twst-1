//! "Who's playing?": the start screen. Big avatar tiles, one per child, plus a
//! "+" tile. Grown-up things (language, settings) sit in small corner keys.

use super::*;

/// Show the search box only once the list gets long.
const SEARCH_FROM: usize = 9;

impl App {
    pub(super) fn draw_home(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        shell::background(ui.painter(), full);
        let ctx = ui.ctx().clone();

        // Top bar: pixel wordmark (left), language + settings keys (right).
        shell::pixel_wordmark(
            ui.painter(),
            Pos2::new(full.left() + 28.0, full.top() + 46.0),
            "RONDELEK",
            4.0,
            palette::CHARCOAL,
        );
        let (_, open_settings) = self.top_bar(ui, full, false, true);
        let lang_rect = Rect::from_min_size(
            Pos2::new(full.right() - 20.0 - 56.0 - 12.0 - 84.0, full.top() + 18.0),
            Vec2::new(84.0, 56.0),
        );
        let lang = self.i18n.lang().to_string();
        let flag = self.flag_texture(&ctx, &lang);
        let lang_resp = ui.put(
            lang_rect,
            KeyButton::icon(Icon::Star, self.i18n.t("settings.look.language"))
                .size(lang_rect.size()),
        );
        if let Some(id) = flag {
            // Paint the flag over the key face (the star icon is just a fallback).
            let face = lang_rect
                .shrink2(Vec2::new(12.0, 14.0))
                .translate(Vec2::new(0.0, -3.0));
            ui.painter()
                .rect_filled(face.expand(2.0), 6.0, palette::PAPER);
            ui.painter().image(id, face, uv_full(), Color32::WHITE);
        }
        if open_settings {
            self.open_settings(None);
            return;
        }
        if lang_resp.clicked() {
            self.open_settings(Some(Section::Look));
            return;
        }

        let query = self.profile_search.to_lowercase();
        let visible: Vec<usize> = self
            .profiles
            .iter()
            .enumerate()
            .filter(|(_, p)| query.is_empty() || p.name().to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        let avatars: Vec<Option<AvatarTex>> = visible
            .iter()
            .map(|&i| {
                let p = self.profiles[i].clone();
                self.avatar_texture(&ctx, &p)
            })
            .collect();

        let mut clicked: Option<usize> = None;
        let mut new_profile = false;
        let content = Rect::from_min_max(Pos2::new(full.left(), full.top() + 92.0), full.max);
        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content));
        egui::ScrollArea::vertical().show(&mut content_ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                shell::title(ui, self.i18n.t("home.title"), 36.0);
                ui.add_space(4.0);
                if self.profiles.is_empty() {
                    shell::hint(ui, self.i18n.t("home.empty"));
                }
                if self.profiles.len() >= SEARCH_FROM {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.profile_search)
                            .hint_text(self.i18n.t("profiles.search"))
                            .desired_width(320.0)
                            .font(egui::FontId::proportional(18.0)),
                    );
                }
                ui.add_space(18.0);

                // Responsive grid of tiles.
                let avail = ui.available_width() - 48.0;
                let gap = 22.0;
                let tile_w = 176.0f32;
                let cols = (((avail + gap) / (tile_w + gap)).floor() as usize).max(1);
                let grid_w = cols as f32 * tile_w + (cols - 1) as f32 * gap;
                let tile = Vec2::new(tile_w, tile_w + 46.0);
                let count = visible.len() + 1; // + the "new child" tile
                let rows = count.div_ceil(cols);
                for row in 0..rows {
                    let (row_rect, _) =
                        ui.allocate_exact_size(Vec2::new(grid_w, tile.y), Sense::hover());
                    for col in 0..cols {
                        let k = row * cols + col;
                        if k >= count {
                            break;
                        }
                        let r = Rect::from_min_size(
                            row_rect.min + Vec2::new(col as f32 * (tile_w + gap), 0.0),
                            tile,
                        );
                        if k < visible.len() {
                            let i = visible[k];
                            if self.child_tile(ui, r, i, avatars[k]).clicked() {
                                clicked = Some(i);
                            }
                        } else if self.new_child_tile(ui, r).clicked() {
                            new_profile = true;
                        }
                    }
                    ui.add_space(gap);
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

    /// One child: their picture on their colour, name underneath, and a small
    /// voice-check badge for the grown-ups.
    fn child_tile(
        &self,
        ui: &mut Ui,
        rect: Rect,
        idx: usize,
        avatar: Option<AvatarTex>,
    ) -> egui::Response {
        let profile = &self.profiles[idx];
        let id = ui.id().with(("child_tile", &profile.dir));
        let resp = ui.interact(rect, id, Sense::click());
        resp.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, profile.name())
        });
        let face_color = shell::tile_color(&profile.manifest.uid);
        let pressed = resp.is_pointer_button_down_on();
        let face = shell::draw_keycap(ui.painter(), rect, face_color, resp.hovered(), pressed);
        let pic = Rect::from_min_size(
            face.min + Vec2::new(14.0, 14.0),
            Vec2::splat(face.width() - 28.0),
        );
        shell::paint_avatar(ui, pic, avatar, face_color, &self.theme);
        ui.painter().text(
            Pos2::new(face.center().x, (pic.bottom() + face.bottom()) / 2.0),
            Align2::CENTER_CENTER,
            truncate(profile.name(), 16),
            egui::FontId::proportional(20.0),
            palette::CHARCOAL,
        );
        let calibrated = self.calibrated.get(&profile.dir).copied().unwrap_or(false);
        let (fill, icon) = if calibrated {
            (palette::OK_GREEN, Icon::Check)
        } else {
            (palette::BUTTER, Icon::Mic)
        };
        shell::badge(
            ui.painter(),
            pic.right_top() + Vec2::new(-6.0, 6.0),
            15.0,
            fill,
            icon,
        );
        resp
    }

    fn new_child_tile(&self, ui: &mut Ui, rect: Rect) -> egui::Response {
        let label = self.i18n.t("home.new_child");
        let resp = ui.interact(rect, ui.id().with("new_child_tile"), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        let pressed = resp.is_pointer_button_down_on();
        let face = shell::draw_keycap(ui.painter(), rect, palette::PAPER, resp.hovered(), pressed);
        let pic = Rect::from_min_size(
            face.min + Vec2::new(14.0, 14.0),
            Vec2::splat(face.width() - 28.0),
        );
        ui.painter().rect_stroke(
            pic,
            egui::CornerRadius::same(28),
            egui::Stroke::new(3.0, shell::shade(palette::PEACH, -0.2)),
            egui::StrokeKind::Inside,
        );
        shell::draw_icon(
            ui.painter(),
            Icon::Plus,
            Rect::from_center_size(pic.center(), Vec2::splat(pic.width() * 0.45)),
            palette::STONE,
        );
        ui.painter().text(
            Pos2::new(face.center().x, (pic.bottom() + face.bottom()) / 2.0),
            Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(18.0),
            palette::STONE,
        );
        resp
    }
}

/// Shorten long names for tiles ("Alexandrina Maria" → "Alexandrina Ma…").
fn truncate(name: &str, max: usize) -> String {
    if name.chars().count() <= max {
        name.to_string()
    } else {
        let mut s: String = name.chars().take(max - 1).collect();
        s.push('…');
        s
    }
}
