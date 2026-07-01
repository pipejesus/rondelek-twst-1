use crate::config::{FONT_SIZE_LABEL, PadDef, PadKind, ROUNDING_PAD, Theme};
use egui::{Color32, CornerRadius, Key, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadMode {
    Play,
    Record,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadState {
    Idle,
    Pressed,
    Animating,
}

pub struct Pad {
    pub state: PadState,
    pub mode: PadMode,
    pub kind: PadKind,
    pub key: Key,
    pub rect: Rect,
    pub label: &'static str,
    pub sample_idx: usize,
    pub has_sample: bool,
    pub is_recording: bool,
    /// 0 = fully raised, 1 = fully pressed. Smoothly animated for a tactile feel.
    press: f32,
    /// Free-running phase used for the recording pulse.
    pulse_t: f32,
}

impl Pad {
    pub fn from_def(def: &PadDef, rect: Rect) -> Self {
        Self {
            state: PadState::Idle,
            mode: PadMode::Play,
            kind: def.kind,
            key: def.key,
            rect,
            label: def.label,
            sample_idx: def.sample_idx,
            has_sample: false,
            is_recording: false,
            press: 0.0,
            pulse_t: 0.0,
        }
    }

    pub fn update(&mut self, activated: bool, dt: f32) -> (bool, bool) {
        let mut just_activated = false;
        let mut just_released = false;

        match self.state {
            PadState::Idle => {
                if activated {
                    self.state = PadState::Pressed;
                    just_activated = true;
                }
            }
            PadState::Pressed => {
                if !activated {
                    self.state = PadState::Animating;
                    just_released = true;
                }
            }
            PadState::Animating => {
                if self.press <= 0.001 {
                    self.state = PadState::Idle;
                }
            }
        }

        // Animate the press depth toward its target (snappy on the way down).
        let target = if self.state == PadState::Pressed {
            1.0
        } else {
            0.0
        };
        let speed = if target > self.press { 22.0 } else { 14.0 };
        self.press += (target - self.press) * (speed * dt).min(1.0);

        self.pulse_t = (self.pulse_t + dt) % 1000.0;

        (just_activated, just_released)
    }

    pub fn draw(&self, painter: &Painter, theme: &Theme) {
        let bg = self.bg_color(theme);
        let rect = self.rect;
        let size = rect.width().min(rect.height());
        let rounding = CornerRadius::same(ROUNDING_PAD as u8);

        // Keycap geometry: a darker "rim" (the side walls) with a raised cap
        // that sinks down when pressed.
        let side = (size * 0.05).clamp(2.0, 6.0);
        let lift_max = (size * 0.09).clamp(3.0, 9.0);
        let lift = lift_max * (1.0 - self.press) + side * self.press;

        // Soft drop shadow, shrinking as the pad is pressed.
        let shadow_a = (60.0 * (1.0 - self.press)) as u8;
        painter.rect_filled(
            rect.translate(Vec2::new(0.0, lift * 0.6 + 2.0)),
            rounding,
            Color32::from_rgba_premultiplied(0, 0, 0, shadow_a),
        );

        // Rim (side walls).
        painter.rect_filled(rect, rounding, shade(bg, -0.22));

        // Raised cap surface.
        let cap = Rect::from_min_max(
            Pos2::new(rect.left() + side, rect.top() + side * 0.5),
            Pos2::new(rect.right() - side, rect.bottom() - lift),
        );
        painter.rect_filled(cap, rounding, bg);

        // Top highlight band for a glossy, moulded look.
        let hl = Rect::from_min_max(
            cap.min,
            Pos2::new(cap.right(), cap.top() + cap.height() * 0.42),
        );
        painter.rect_filled(hl, rounding, shade(bg, 0.10));
        // Re-draw the lower portion so the highlight stays at the top only.
        let lower = Rect::from_min_max(
            Pos2::new(cap.left(), cap.top() + cap.height() * 0.40),
            cap.max,
        );
        painter.rect_filled(lower, CornerRadius::same((ROUNDING_PAD * 0.6) as u8), bg);

        // Label.
        let fg = self.fg_color(theme);
        let font = (size * 0.34).clamp(12.0, FONT_SIZE_LABEL);
        painter.text(
            cap.center(),
            egui::Align2::CENTER_CENTER,
            self.label,
            egui::FontId::proportional(font),
            fg,
        );

        // Status LED (sample pads only).
        if self.kind == PadKind::Sample {
            let led_r = (size * 0.045).clamp(3.0, 6.0);
            let led_pos = Pos2::new(cap.right() - led_r * 2.4, cap.top() + led_r * 2.4);
            let led_color = if self.has_sample {
                theme.led_full
            } else {
                theme.led_empty
            };
            painter.circle_filled(led_pos, led_r, led_color);
        }

        // Pulsing ring while recording.
        if self.is_recording {
            let pulse = 0.5 + 0.5 * (self.pulse_t * std::f32::consts::TAU * 1.6).sin();
            let ring =
                Color32::from_rgba_premultiplied(0xFF, 0x6A, 0x1A, (90.0 + 140.0 * pulse) as u8);
            painter.rect_stroke(
                rect.expand(3.0),
                rounding,
                Stroke::new(3.0, ring),
                StrokeKind::Outside,
            );
        }
    }

    fn bg_color(&self, theme: &Theme) -> Color32 {
        match self.kind {
            PadKind::Function => theme.pad_function_bg,
            PadKind::Sample => match self.mode {
                PadMode::Play => theme.pad_play_bg,
                PadMode::Record => theme.pad_record_bg,
            },
        }
    }

    fn fg_color(&self, theme: &Theme) -> Color32 {
        match self.kind {
            PadKind::Function => theme.pad_function_fg,
            PadKind::Sample => match self.mode {
                PadMode::Play => theme.pad_play_fg,
                PadMode::Record => theme.pad_record_fg,
            },
        }
    }

    pub fn set_mode(&mut self, mode: PadMode) {
        self.mode = mode;
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }
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
