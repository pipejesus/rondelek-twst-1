use egui::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadKind {
    Sample,
    Function,
}

/// A pad's identity and keyboard binding. Geometry is *not* stored here — pad
/// rectangles are computed every frame from the real window size by the
/// `rondelek` app crate's `ui::layout`, so the faceplate reflows fluidly on
/// resize.
#[derive(Clone, Debug)]
pub struct PadDef {
    pub kind: PadKind,
    pub key: Key,
    pub sample_idx: usize,
}

/// Initial window size (logical points). The window is resizable; this is only
/// the starting size.
pub const WINDOW_WIDTH: u32 = 720;
pub const WINDOW_HEIGHT: u32 = 760;

/// Sample pad grid dimensions. 4×3 = 12 large, kid-friendly pads.
pub const PAD_COLS: usize = 4;
pub const PAD_ROWS: usize = 3;
pub const NUM_SAMPLES: usize = PAD_COLS * PAD_ROWS;

/// The 12 sample pads, in row-major order, mapped to an ergonomic keyboard
/// block (1-4 / Q-R / A-F).
pub const SAMPLE_PADS: &[PadDef] = &[
    PadDef {
        kind: PadKind::Sample,
        key: Key::Num1,
        sample_idx: 0,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::Num2,
        sample_idx: 1,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::Num3,
        sample_idx: 2,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::Num4,
        sample_idx: 3,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::Q,
        sample_idx: 4,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::W,
        sample_idx: 5,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::E,
        sample_idx: 6,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::R,
        sample_idx: 7,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::A,
        sample_idx: 8,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::S,
        sample_idx: 9,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::D,
        sample_idx: 10,
    },
    PadDef {
        kind: PadKind::Sample,
        key: Key::F,
        sample_idx: 11,
    },
];

/// The record-mode toggle, drawn in the header strip.
pub const REC_PAD: PadDef = PadDef {
    kind: PadKind::Function,
    key: Key::Space,
    sample_idx: 0,
};
