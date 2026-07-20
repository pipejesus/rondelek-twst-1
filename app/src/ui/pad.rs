use rondelek_core::config::{PadDef, PadKind, ROUNDING_PAD, Theme};
use crate::ui::skin::{Skin, uv_full};
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
}

pub struct Pad {
    pub state: PadState,
    pub mode: PadMode,
    pub kind: PadKind,
    pub key: Key,
    pub rect: Rect,
    pub sample_idx: usize,
    pub has_sample: bool,
    pub is_recording: bool,
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
            sample_idx: def.sample_idx,
            has_sample: false,
            is_recording: false,
            pulse_t: 0.0,
        }
    }

    /// No press animation by design: the key snaps between its idle and
    /// pressed artwork, like a real button.
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
                    self.state = PadState::Idle;
                    just_released = true;
                }
            }
        }

        self.pulse_t = (self.pulse_t + dt) % 1000.0;

        (just_activated, just_released)
    }

    pub fn draw(&self, painter: &Painter, theme: &Theme, skin: &Skin) {
        let rect = self.rect;
        let size = rect.width().min(rect.height());
        let rounding = CornerRadius::same(ROUNDING_PAD as u8);

        // Skin artwork: per-pad key for samples, the REC key for the function
        // pad. Press states are baked; the engine only picks and nudges them.
        let button = match self.kind {
            PadKind::Function => &skin.rec,
            PadKind::Sample => &skin.pads[self.sample_idx.min(skin.pads.len() - 1)],
        };
        let pressed = self.state == PadState::Pressed;
        let tex = if pressed {
            &button.pressed
        } else {
            &button.idle
        };

        // Soft drop shadow while the key is raised. Inset to stay behind the
        // artwork (skin images carry a transparent margin).
        if !pressed {
            painter.rect_filled(
                rect.shrink(size * 0.05)
                    .translate(Vec2::new(0.0, (size * 0.045).clamp(3.0, 9.0))),
                CornerRadius::same((size * 0.16).min(255.0) as u8),
                Color32::from_rgba_premultiplied(0, 0, 0, 36),
            );
        }

        // Record mode: tint sample pads toward the record colour.
        let tint = if self.kind == PadKind::Sample && self.mode == PadMode::Record {
            let r = theme.pad_record_bg;
            let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * 0.4) as u8;
            Color32::from_rgb(mix(255, r.r()), mix(255, r.g()), mix(255, r.b()))
        } else {
            Color32::WHITE
        };
        painter.image(tex.id(), rect, uv_full(), tint);

        // Status LED (sample pads only).
        if self.kind == PadKind::Sample {
            let led_r = (size * 0.045).clamp(3.0, 6.0);
            let inset = size * 0.14;
            let led_pos = Pos2::new(rect.right() - inset, rect.top() + inset);
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
            let r = theme.pad_record_bg;
            let ring =
                Color32::from_rgba_unmultiplied(r.r(), r.g(), r.b(), (90.0 + 140.0 * pulse) as u8);
            painter.rect_stroke(
                rect.expand(3.0),
                rounding,
                Stroke::new(3.0, ring),
                StrokeKind::Outside,
            );
        }
    }

    pub fn set_mode(&mut self, mode: PadMode) {
        self.mode = mode;
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }
}
