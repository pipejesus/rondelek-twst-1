use egui::{Pos2, Rect, Vec2};

use crate::config::{NUM_SAMPLES, PAD_COLS, PAD_ROWS};

/// Computed faceplate geometry for a single frame. Everything is derived from
/// the live window rectangle so the layout reflows fluidly on resize — there is
/// no fixed pixel grid.
pub struct FaceLayout {
    /// The device "case" — the whole bordered panel.
    pub case: Rect,
    /// Framed screen housing (the bezel).
    pub screen_bezel: Rect,
    /// Drawable area inside the screen bezel (where the visualizer renders).
    pub screen: Rect,
    /// The REC toggle button (header right).
    pub rec: Rect,
    /// The "back to profiles" button (header left), square.
    pub back: Rect,
    /// Profile picture frame: square, centred, flush with the top of the case.
    pub avatar: Rect,
    /// Sample-pad rectangles, row-major (`NUM_SAMPLES` of them), all square.
    pub pads: Vec<Rect>,
}

fn min_elem(v: Vec2) -> f32 {
    v.x.min(v.y)
}

/// Compute the faceplate layout for the given available rectangle.
pub fn compute(available: Rect) -> FaceLayout {
    let avail_size = available.size();
    let margin = (min_elem(avail_size) * 0.025).clamp(8.0, 28.0);
    let case = available.shrink(margin);

    let inner_pad = (min_elem(case.size()) * 0.04).clamp(12.0, 32.0);
    let content = case.shrink(inner_pad);

    let header_h = (content.height() * 0.10).clamp(40.0, 72.0);
    let gap_v = (content.height() * 0.025).clamp(8.0, 20.0);
    let screen_h = (content.height() * 0.22).clamp(70.0, 220.0);

    // Header strip.
    let header = Rect::from_min_size(content.min, Vec2::new(content.width(), header_h));

    // REC button: right-aligned within the header.
    let rec_h = header_h * 0.78;
    let rec_w = (rec_h * 2.4).min(content.width() * 0.4);
    let rec = Rect::from_min_size(
        Pos2::new(header.right() - rec_w, header.center().y - rec_h * 0.5),
        Vec2::new(rec_w, rec_h),
    );

    // Back-to-profiles button: a square keycap at the header's left.
    let back = Rect::from_min_size(
        Pos2::new(header.left(), header.center().y - rec_h * 0.5),
        Vec2::splat(rec_h),
    );

    // Profile avatar: a square centred horizontally, flush with the very top of
    // the case, reaching down to the bottom of the header strip.
    let avatar_side = (header.bottom() - case.top()).max(1.0);
    let avatar = Rect::from_min_size(
        Pos2::new(case.center().x - avatar_side * 0.5, case.top()),
        Vec2::splat(avatar_side),
    );

    // Screen housing below the header.
    let screen_top = header.bottom() + gap_v;
    let screen_bezel = Rect::from_min_size(
        Pos2::new(content.left(), screen_top),
        Vec2::new(content.width(), screen_h),
    );
    let bezel_thickness = (screen_h * 0.10).clamp(5.0, 14.0);
    let screen = screen_bezel.shrink(bezel_thickness);

    // Pad area: everything below the screen.
    let pad_area = Rect::from_min_max(
        Pos2::new(content.left(), screen_bezel.bottom() + gap_v),
        content.max,
    );

    let cols = PAD_COLS as f32;
    let rows = PAD_ROWS as f32;
    let gap = (min_elem(pad_area.size()) * 0.045).clamp(8.0, 26.0);

    let cell_w = (pad_area.width() - gap * (cols - 1.0)) / cols;
    let cell_h = (pad_area.height() - gap * (rows - 1.0)) / rows;
    // Square pads, capped so they don't become unwieldy on very large windows.
    let pad_size = cell_w.min(cell_h).clamp(1.0, 190.0);

    // Centre the grid within the pad area.
    let grid_w = cols * pad_size + (cols - 1.0) * gap;
    let grid_h = rows * pad_size + (rows - 1.0) * gap;
    let origin = Pos2::new(
        pad_area.center().x - grid_w * 0.5,
        pad_area.center().y - grid_h * 0.5,
    );

    let mut pads = Vec::with_capacity(NUM_SAMPLES);
    for r in 0..PAD_ROWS {
        for c in 0..PAD_COLS {
            let x = origin.x + c as f32 * (pad_size + gap);
            let y = origin.y + r as f32 * (pad_size + gap);
            pads.push(Rect::from_min_size(
                Pos2::new(x, y),
                Vec2::new(pad_size, pad_size),
            ));
        }
    }

    FaceLayout {
        case,
        screen_bezel,
        screen,
        rec,
        back,
        avatar,
        pads,
    }
}
