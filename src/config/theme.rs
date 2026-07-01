use egui::Color32;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Theme {
    pub panel_bg: Color32,
    pub panel_fg: Color32,
    pub pad_play_bg: Color32,
    pub pad_play_fg: Color32,
    pub pad_play_hover: Color32,
    pub pad_play_pressed: Color32,
    pub pad_record_bg: Color32,
    pub pad_record_fg: Color32,
    pub pad_function_bg: Color32,
    pub pad_function_fg: Color32,
    pub led_empty: Color32,
    pub led_full: Color32,
    pub case_shadow: Color32,
    pub case_border: Color32,
    pub text_primary: Color32,
    pub text_secondary: Color32,
    pub visualizer_bg: Color32,
    /// Colour of unlit dots in the dot-matrix screen.
    pub visualizer_dot_off: Color32,
    pub visualizer_bar_low: Color32,
    pub visualizer_bar_mid: Color32,
    pub visualizer_bar_high: Color32,
}

#[allow(dead_code)]
pub const SPACING_INNER: f32 = 16.0;
#[allow(dead_code)]
pub const ROUNDING_MD: f32 = 8.0;
/// Pad corner radius — a soft squircle, not a pill, à la EP-133 buttons.
pub const ROUNDING_PAD: f32 = 12.0;
pub const FONT_SIZE_LABEL: f32 = 22.0;
#[allow(dead_code)]
pub const FONT_SIZE_SMALL: f32 = 11.0;
#[allow(dead_code)]
pub const FONT_SIZE_HEADING: f32 = 20.0;

// The signature orange, used deliberately for the record state and accents.
const ORANGE: Color32 = Color32::from_rgb(0xFF, 0x6A, 0x1A);

pub fn theme_light() -> Theme {
    Theme {
        // Bone / cream device body.
        panel_bg: Color32::from_rgb(0xE6, 0xE1, 0xD5),
        panel_fg: Color32::from_rgb(0xD5, 0xD0, 0xC2),
        // Warm off-white tactile pads with charcoal labels.
        pad_play_bg: Color32::from_rgb(0xF3, 0xEE, 0xE4),
        pad_play_fg: Color32::from_rgb(0x26, 0x23, 0x1E),
        pad_play_hover: Color32::from_rgb(0xFF, 0xF5, 0xE8),
        pad_play_pressed: Color32::from_rgb(0xFF, 0xDF, 0xBF),
        pad_record_bg: ORANGE,
        pad_record_fg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        // REC control: dark with orange lettering.
        pad_function_bg: Color32::from_rgb(0x2E, 0x2A, 0x24),
        pad_function_fg: ORANGE,
        led_empty: Color32::from_rgb(0xBE, 0xB8, 0xAA),
        led_full: ORANGE,
        case_shadow: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x30),
        case_border: Color32::from_rgb(0xCF, 0xC9, 0xBB),
        text_primary: Color32::from_rgb(0x26, 0x23, 0x1E),
        text_secondary: Color32::from_rgb(0x8A, 0x84, 0x75),
        // Dark screen so the amber dot-matrix glows.
        visualizer_bg: Color32::from_rgb(0x1A, 0x18, 0x14),
        visualizer_dot_off: Color32::from_rgb(0x2C, 0x29, 0x22),
        visualizer_bar_low: Color32::from_rgb(0xB8, 0x5A, 0x12),
        visualizer_bar_mid: ORANGE,
        visualizer_bar_high: Color32::from_rgb(0xFF, 0xC0, 0x6A),
    }
}

pub fn theme_dark() -> Theme {
    Theme {
        panel_bg: Color32::from_rgb(0x20, 0x1E, 0x1A),
        panel_fg: Color32::from_rgb(0x2B, 0x28, 0x23),
        pad_play_bg: Color32::from_rgb(0x33, 0x2F, 0x29),
        pad_play_fg: Color32::from_rgb(0xEC, 0xE7, 0xDB),
        pad_play_hover: Color32::from_rgb(0x40, 0x3A, 0x31),
        pad_play_pressed: Color32::from_rgb(0x52, 0x3C, 0x24),
        pad_record_bg: ORANGE,
        pad_record_fg: Color32::from_rgb(0xFF, 0xFF, 0xFF),
        pad_function_bg: Color32::from_rgb(0x16, 0x14, 0x11),
        pad_function_fg: ORANGE,
        led_empty: Color32::from_rgb(0x4A, 0x46, 0x3E),
        led_full: ORANGE,
        case_shadow: Color32::from_rgba_premultiplied(0x00, 0x00, 0x00, 0x60),
        case_border: Color32::from_rgb(0x3A, 0x36, 0x2F),
        text_primary: Color32::from_rgb(0xEC, 0xE7, 0xDB),
        text_secondary: Color32::from_rgb(0x8A, 0x84, 0x75),
        visualizer_bg: Color32::from_rgb(0x0F, 0x0E, 0x0B),
        visualizer_dot_off: Color32::from_rgb(0x24, 0x21, 0x1B),
        visualizer_bar_low: Color32::from_rgb(0xB8, 0x5A, 0x12),
        visualizer_bar_mid: ORANGE,
        visualizer_bar_high: Color32::from_rgb(0xFF, 0xC0, 0x6A),
    }
}
