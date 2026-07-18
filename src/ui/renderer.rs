use crate::config::Theme;
use crate::ui::FaceLayout;
use crate::ui::skin::{Skin, draw_nine_slice, draw_tiled, slice_screen_inset};
use egui::{CornerRadius, Painter, Vec2};

pub struct Renderer;

impl Renderer {
    /// Draw the device case and the framed screen housing from the active
    /// skin. Pads and the visualizer are drawn separately on top.
    pub fn draw_case(painter: &Painter, layout: &FaceLayout, skin: &Skin, theme: &Theme) {
        let case = layout.case;

        // Drop shadow for the whole unit.
        painter.rect_filled(
            case.translate(Vec2::new(6.0, 6.0)),
            CornerRadius::same(18),
            theme.case_shadow,
        );

        // Body: nine-sliced so corners stay crisp at any window size, plus the
        // tiled matte grain over the flat centre (which stretching leaves
        // noise-free by design — see genskin).
        draw_nine_slice(painter, &skin.case, case, skin.slice.case);
        if let Some(grain) = &skin.grain {
            let d = slice_screen_inset(skin.slice.case, case);
            draw_tiled(painter, grain, case.shrink(d));
        }

        // Screen bezel and the screen itself.
        draw_nine_slice(painter, &skin.bezel, layout.screen_bezel, skin.slice.bezel);
        painter.rect_filled(layout.screen, CornerRadius::same(8), theme.visualizer_bg);
    }
}
