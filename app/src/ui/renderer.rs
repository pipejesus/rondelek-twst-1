use rondelek_core::config::Theme;
use rondelek_core::config::atlas;
use crate::ui::FaceLayout;
use crate::ui::skin::Skin;
use egui::{CornerRadius, Painter};

pub struct Renderer;

impl Renderer {
    /// Draw the device case and the framed screen housing from the active
    /// skin. Pads and the visualizer are drawn separately on top.
    pub fn draw_case(painter: &Painter, layout: &FaceLayout, skin: &Skin, theme: &Theme) {
        let case = layout.case;

        // Body and screen bezel, both nine-sliced so corners stay crisp at any
        // window size. The bezel's inset matches `layout`'s screen inset, so the
        // visualizer fills exactly the opening and never overlaps the frame.
        skin.nine(painter, atlas::CASE, atlas::CASE_INSET, case);
        skin.nine(painter, atlas::BEZEL, atlas::BEZEL_INSET, layout.screen_bezel);
        painter.rect_filled(layout.screen, CornerRadius::same(6), theme.visualizer_bg);
    }
}
