//! A tiny input-level meter: a horizontal bar coloured by how usable the signal
//! is. Shown during calibration and in the config panel's Audio tab so a helper
//! can see at a glance that the mic is neither clipping nor too quiet.
//!
//! There is deliberately no microphone *frequency* calibration (a sweep): vowel
//! detection compares the child to their own templates on the same mic, so the
//! mic's coloration cancels. Level, however, is not relative — clipping destroys
//! the spectrum and too-quiet audio is all noise — so that is what we surface.

use egui::{Color32, Sense, Vec2};

/// How usable the current input level is.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MicLevel {
    TooQuiet,
    Ok,
    Clipping,
}

/// Classify a peak amplitude (0..=1) from the last window.
pub fn classify_peak(peak: f32) -> MicLevel {
    if peak >= 0.98 {
        MicLevel::Clipping
    } else if peak < 0.03 {
        MicLevel::TooQuiet
    } else {
        MicLevel::Ok
    }
}

impl MicLevel {
    fn label(self) -> &'static str {
        match self {
            MicLevel::TooQuiet => "Too quiet — move closer or turn up the mic",
            MicLevel::Ok => "Level OK",
            MicLevel::Clipping => "Too loud — clipping! back off the mic",
        }
    }

    fn color(self) -> Color32 {
        match self {
            MicLevel::TooQuiet => Color32::from_rgb(0x9A, 0x9A, 0x9A),
            MicLevel::Ok => Color32::from_rgb(0x3C, 0xB0, 0x4B),
            MicLevel::Clipping => Color32::from_rgb(0xD6, 0x3A, 0x2E),
        }
    }
}

/// Draw a `width`-wide level meter for a peak amplitude (0..=1) plus a status
/// line. Cheap enough to call every frame.
pub fn level_meter(ui: &mut egui::Ui, peak: f32, width: f32) {
    let state = classify_peak(peak);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 16.0), Sense::hover());
    let painter = ui.painter();
    let radius = egui::CornerRadius::same(4);
    painter.rect_filled(rect, radius, Color32::from_gray(60));
    let fill_w = rect.width() * peak.clamp(0.0, 1.0);
    if fill_w > 0.5 {
        let fill = egui::Rect::from_min_size(rect.min, Vec2::new(fill_w, rect.height()));
        painter.rect_filled(fill, radius, state.color());
    }
    ui.add_space(2.0);
    ui.label(
        egui::RichText::new(state.label())
            .color(state.color())
            .size(12.0),
    );
}
