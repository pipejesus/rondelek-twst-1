//! A drawn kid-face placeholder avatar (used when a profile has no picture)
//! and the translucent gloss band the profile screens lay over avatar photos.

use crate::config::Theme;
use egui::{Color32, CornerRadius, Painter, Pos2, Rect, Stroke};

/// Overlay a translucent top-highlight band, e.g. on top of an avatar image so
/// it reads as "under glass".
pub fn gloss_overlay(painter: &Painter, rect: Rect, rounding: f32) {
    let band = Rect::from_min_max(
        rect.min,
        Pos2::new(rect.right(), rect.top() + rect.height() * 0.42),
    );
    painter.rect_filled(
        band,
        CornerRadius::same(rounding as u8),
        Color32::from_white_alpha(28),
    );
}

/// Lighten (`amount > 0`) or darken (`amount < 0`) a colour by mixing toward
/// white or black.
fn shade(c: Color32, amount: f32) -> Color32 {
    let mix = |ch: u8| {
        let target = if amount >= 0.0 { 255.0 } else { 0.0 };
        let t = amount.abs().clamp(0.0, 1.0);
        (ch as f32 + (target - ch as f32) * t).round() as u8
    };
    Color32::from_rgb(mix(c.r()), mix(c.g()), mix(c.b()))
}

/// Draw a friendly smiling-kid placeholder inside `rect` (used when a profile has
/// no picture).
pub fn draw_kid_face(painter: &Painter, rect: Rect, theme: &Theme) {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.5;
    let face = theme.text_secondary;

    // Head.
    painter.circle_filled(c, r * 0.72, shade(theme.panel_bg, 0.04));
    painter.circle_stroke(c, r * 0.72, Stroke::new(2.0, face));
    // Eyes.
    let eye_dx = r * 0.28;
    let eye_dy = r * 0.12;
    let eye_r = (r * 0.08).max(1.5);
    painter.circle_filled(Pos2::new(c.x - eye_dx, c.y - eye_dy), eye_r, face);
    painter.circle_filled(Pos2::new(c.x + eye_dx, c.y - eye_dy), eye_r, face);
    // Smile: a small arc approximated by a polyline.
    let smile_r = r * 0.34;
    let cy = c.y + r * 0.10;
    let pts: Vec<Pos2> = (0..=10)
        .map(|i| {
            let t = i as f32 / 10.0;
            let a = std::f32::consts::PI * (0.15 + 0.70 * t);
            Pos2::new(c.x - smile_r * a.cos(), cy + smile_r * a.sin())
        })
        .collect();
    painter.add(egui::Shape::line(pts, Stroke::new(2.0, face)));
}
