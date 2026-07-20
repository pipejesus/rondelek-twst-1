//! The vowel view: shows the detected Polish vowel as a large letter plus a
//! match meter per vowel. It runs the single [`rondelek_core::audio::vowel`] detector
//! against the active profile's calibration — there is no uncalibrated fallback,
//! so an uncalibrated profile is told to calibrate first.

use egui::{Align2, FontId, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

use rondelek_core::audio::vowel::{self, VOWELS, VowelDetector, VowelResult};
use rondelek_core::config::{Settings, Theme};
use crate::ui::visualizer::{AudioFrame, Visualizer};
use rondelek_core::util::lerp;

/// Analysis window (samples) — ~45 ms at 44.1 kHz, long enough for a stable
/// envelope, short enough to track a changing vowel.
const WINDOW: usize = 2048;

pub struct VowelVisualizer {
    /// Smoothed per-vowel match, indexed like [`VOWELS`].
    scores: [f32; 6],
    /// The single stateful detector (templates + running channel mean).
    detector: VowelDetector,
    /// Whether the active profile is calibrated (drives the status line).
    calibrated: bool,
}

impl VowelVisualizer {
    pub fn new() -> Self {
        Self {
            scores: [0.0; 6],
            detector: VowelDetector::new(),
            calibrated: false,
        }
    }

    /// Index of the strongest vowel — but only when it clears the show threshold
    /// *and* beats the runner-up by the margin threshold, so recognition is a
    /// firm decision rather than a flickering guess.
    fn best_index(&self, settings: &Settings) -> Option<usize> {
        let (idx, &val) = self
            .scores
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())?;
        let second = self
            .scores
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != idx)
            .map(|(_, &s)| s)
            .fold(0.0f32, f32::max);
        (val >= settings.vowel_show_threshold && (val - second) >= settings.vowel_margin_threshold)
            .then_some(idx)
    }
}

impl Default for VowelVisualizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Sum of squares over the last `WINDOW` samples — used to pick the louder tap.
fn tail_energy(s: &[f32]) -> f32 {
    let start = s.len().saturating_sub(WINDOW);
    s[start..].iter().map(|x| x * x).sum()
}

impl Visualizer for VowelVisualizer {
    fn update(&mut self, frame: &AudioFrame, settings: &Settings) {
        // Analyse whichever tap is louder (mic while speaking, playback while a
        // recording plays), using that tap's own sample rate.
        let (src, rate) = if tail_energy(frame.playback) > tail_energy(frame.input) {
            (frame.playback, frame.playback_rate)
        } else {
            (frame.input, frame.input_rate)
        };
        let window = &src[src.len().saturating_sub(WINDOW)..];

        self.detector.set_steady(settings.vowel_steady);
        let VowelResult { scores, .. } =
            self.detector
                .analyze(window, rate, settings.vowel_voicing_threshold);

        // Ease the meters toward the new match (toward zero when unvoiced).
        let a = 1.0 - settings.vowel_smoothing.clamp(0.0, 0.98);
        for (s, &target) in self.scores.iter_mut().zip(scores.iter()) {
            *s = lerp(*s, target, a);
        }
    }

    fn demo_fill(&mut self, _num_bars: usize) {
        // A representative "a" detection, so screenshots show the view alive.
        self.scores = [0.92, 0.34, 0.10, 0.52, 0.08, 0.22]; // a e i o u y
        self.calibrated = true;
    }

    fn set_calibration(&mut self, calibration: Option<vowel::VowelCalibration>) {
        self.detector.set_calibration(calibration.as_ref());
        self.calibrated = self.detector.is_calibrated();
    }

    fn draw(&self, painter: &Painter, rect: Rect, theme: &Theme, settings: &Settings) {
        if rect.width() <= 8.0 || rect.height() <= 8.0 {
            return;
        }
        let pad = (rect.width().min(rect.height()) * 0.06).clamp(6.0, 18.0);
        let inner = rect.shrink(pad);

        // Calibration status, top-left.
        let (status, status_color) = if self.calibrated {
            ("Calibrated ✓".to_string(), theme.visualizer_bar_high)
        } else {
            (
                "Not calibrated — set up the child's voice first".to_string(),
                theme.text_secondary,
            )
        };
        painter.text(
            inner.left_top(),
            Align2::LEFT_TOP,
            status,
            FontId::monospace((inner.height() * 0.06).clamp(9.0, 13.0)),
            status_color,
        );

        // Left third: the big detected letter.
        let split = inner.left() + inner.width() * 0.34;
        let left = Rect::from_min_max(inner.min, Pos2::new(split, inner.bottom()));
        let best = self.best_index(settings);

        let letter = best.map_or("–", |i| VOWELS[i].label());
        let letter_color = best.map_or(theme.text_secondary, |_| theme.visualizer_bar_high);
        painter.text(
            Pos2::new(left.center().x, left.center().y),
            Align2::CENTER_CENTER,
            letter,
            FontId::proportional((left.height() * 0.5).clamp(24.0, 140.0)),
            letter_color,
        );

        // Right two-thirds: one horizontal match meter per vowel.
        let bars = Rect::from_min_max(Pos2::new(split + pad, inner.top()), inner.max);
        let n = VOWELS.len();
        let row_h = bars.height() / n as f32;
        let bar_h = (row_h * 0.62).min(26.0);
        let label_w = (bars.width() * 0.12).clamp(16.0, 40.0);
        for (i, v) in VOWELS.iter().enumerate() {
            let cy = bars.top() + (i as f32 + 0.5) * row_h;
            painter.text(
                Pos2::new(bars.left(), cy),
                Align2::LEFT_CENTER,
                v.label(),
                FontId::proportional(bar_h * 0.9),
                theme.text_primary,
            );
            let track = Rect::from_min_size(
                Pos2::new(bars.left() + label_w, cy - bar_h * 0.5),
                Vec2::new(bars.width() - label_w, bar_h),
            );
            let radius = egui::CornerRadius::same((bar_h * 0.3) as u8);
            painter.rect_filled(track, radius, theme.visualizer_dot_off);
            let score = self.scores[i].clamp(0.0, 1.0);
            if score > 0.01 {
                let fill = Rect::from_min_size(track.min, Vec2::new(track.width() * score, bar_h));
                let color = if best == Some(i) {
                    theme.visualizer_bar_high
                } else {
                    theme.visualizer_bar_mid
                };
                painter.rect_filled(fill, radius, color);
            }
            if best == Some(i) {
                painter.rect_stroke(
                    track,
                    radius,
                    Stroke::new(1.5, theme.led_full),
                    StrokeKind::Inside,
                );
            }
        }
    }
}
