use crate::config::Theme;
use crate::ui::FaceLayout;
use egui::{Align2, CornerRadius, FontId, Painter, Pos2, Stroke, StrokeKind, Vec2};

pub struct Renderer;

impl Renderer {
    /// Draw the device case and the framed screen housing. Pads and the
    /// visualizer are drawn separately on top.
    pub fn draw_case(painter: &Painter, layout: &FaceLayout, theme: &Theme) {
        let case = layout.case;

        // Drop shadow for the whole unit.
        painter.rect_filled(
            case.translate(Vec2::new(6.0, 6.0)),
            CornerRadius::same(18),
            theme.case_shadow,
        );

        // Body.
        painter.rect_filled(case, CornerRadius::same(18), theme.panel_bg);
        painter.rect_stroke(
            case,
            CornerRadius::same(18),
            Stroke::new(2.0, theme.case_border),
            StrokeKind::Inside,
        );

        // Screen bezel.
        let bezel = layout.screen_bezel;
        painter.rect_filled(bezel, CornerRadius::same(12), theme.panel_fg);
        painter.rect_stroke(
            bezel,
            CornerRadius::same(12),
            Stroke::new(1.0, theme.case_border),
            StrokeKind::Inside,
        );

        // Inner screen.
        painter.rect_filled(layout.screen, CornerRadius::same(8), theme.visualizer_bg);

        Self::draw_faceplate_marks(painter, layout, theme);
    }

    /// Small "future-retro" touches: the model wordmark and a row of
    /// registration tick marks above the pad grid.
    fn draw_faceplate_marks(painter: &Painter, layout: &FaceLayout, theme: &Theme) {
        let case = layout.case;

        // Model wordmark, bottom-right of the case.
        let wm_size = (case.height() * 0.022).clamp(10.0, 16.0);
        painter.text(
            Pos2::new(case.right() - 14.0, case.bottom() - 10.0),
            Align2::RIGHT_BOTTOM,
            "TWST·1",
            FontId::proportional(wm_size),
            theme.text_secondary,
        );
        painter.text(
            Pos2::new(case.left() + 16.0, case.bottom() - 10.0),
            Align2::LEFT_BOTTOM,
            "RONDELEK",
            FontId::proportional(wm_size),
            theme.text_secondary,
        );

        // Registration tick marks just under the screen bezel.
        let tick_y = layout.screen_bezel.bottom() + 5.0;
        let ticks = 24;
        let span = layout.screen_bezel.width();
        for i in 0..=ticks {
            let x = layout.screen_bezel.left() + span * (i as f32 / ticks as f32);
            let tall = i % 4 == 0;
            let h = if tall { 5.0 } else { 2.5 };
            let col = if tall {
                theme.text_secondary
            } else {
                theme.case_border
            };
            painter.line_segment(
                [Pos2::new(x, tick_y), Pos2::new(x, tick_y + h)],
                Stroke::new(1.0, col),
            );
        }
    }
}
