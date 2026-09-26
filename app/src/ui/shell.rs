//! The "shell" look: everything outside the skinned sampler (who's playing,
//! the child's hub, games menu, profile form, calibration, grown-ups page).
//!
//! It borrows the pre-game screen's visual language: a cream→peach gradient,
//! soft pastel tiles, charcoal ink, one orange accent. Buttons are drawn as
//! chunky keycaps (a face on a darker lip that sinks when pressed), so the
//! shell feels like the same toy as the sampler. Every clickable widget here
//! reports an AccessKit label, which is also how the UI tests find it.

use egui::{
    Align2, Color32, CornerRadius, FontId, Mesh, Painter, Pos2, Rect, Response, Sense, Shape,
    Stroke, StrokeKind, Ui, Vec2, WidgetInfo, WidgetType,
};

// ---- palette -------------------------------------------------------------

/// Pastels shared with the game's pre-game screen (`game/src/lib.rs`).
pub mod palette {
    use egui::Color32;
    pub const CREAM: Color32 = Color32::from_rgb(251, 242, 228);
    pub const PEACH: Color32 = Color32::from_rgb(246, 220, 198);
    pub const MINT: Color32 = Color32::from_rgb(198, 229, 211);
    pub const ROSE: Color32 = Color32::from_rgb(245, 169, 188);
    pub const LILAC: Color32 = Color32::from_rgb(201, 184, 232);
    pub const SKY: Color32 = Color32::from_rgb(169, 212, 239);
    pub const BUTTER: Color32 = Color32::from_rgb(245, 226, 158);
    pub const CHARCOAL: Color32 = Color32::from_rgb(74, 68, 60);
    pub const STONE: Color32 = Color32::from_rgb(140, 124, 115);
    pub const PAPER: Color32 = Color32::from_rgb(255, 252, 247);
    pub const ORANGE: Color32 = Color32::from_rgb(0xFF, 0x6A, 0x1A);
    pub const DANGER: Color32 = Color32::from_rgb(0xD6, 0x3A, 0x2E);
    pub const OK_GREEN: Color32 = Color32::from_rgb(0x3C, 0xB0, 0x4B);
}
use palette::*;

/// Tile colours, picked per child so each one gets "their" colour.
const TILE_COLORS: [Color32; 5] = [SKY, LILAC, BUTTER, MINT, ROSE];

/// A stable pastel for `seed` (e.g. a profile uid).
pub fn tile_color(seed: &str) -> Color32 {
    let h = seed
        .bytes()
        .fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    TILE_COLORS[(h as usize) % TILE_COLORS.len()]
}

/// Mix toward white (`amount > 0`) or black (`amount < 0`).
pub fn shade(c: Color32, amount: f32) -> Color32 {
    let t = amount.abs().clamp(0.0, 1.0);
    let target = if amount >= 0.0 { 255.0 } else { 0.0 };
    let mix = |v: u8| (v as f32 + (target - v as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(mix(c.r()), mix(c.g()), mix(c.b()), c.a())
}

/// Charcoal or white, whichever reads better on `bg`.
pub fn ink_on(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum < 150.0 {
        Color32::WHITE
    } else {
        CHARCOAL
    }
}

// ---- global style --------------------------------------------------------

/// A light, roomy egui style for the shell's ordinary widgets (text fields,
/// sliders, combo boxes). Forces the light theme: egui otherwise follows the
/// OS and paints dark widgets onto our cream screens.
pub fn apply_style(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Light);
    ctx.all_styles_mut(|style| {
        use egui::{FontFamily::Proportional, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(28.0, Proportional)),
            (TextStyle::Body, FontId::new(17.0, Proportional)),
            (TextStyle::Button, FontId::new(17.0, Proportional)),
            (TextStyle::Small, FontId::new(13.0, Proportional)),
            (
                TextStyle::Monospace,
                FontId::new(15.0, egui::FontFamily::Monospace),
            ),
        ]
        .into();
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(14.0, 8.0);
        style.spacing.interact_size = Vec2::new(44.0, 36.0);
        style.spacing.slider_width = 260.0;
        style.spacing.combo_width = 280.0;

        let v = &mut style.visuals;
        *v = egui::Visuals::light();
        v.override_text_color = Some(CHARCOAL);
        v.panel_fill = CREAM;
        v.window_fill = PAPER;
        v.extreme_bg_color = PAPER; // text-edit background
        v.faint_bg_color = shade(PEACH, 0.5);
        v.selection.bg_fill = shade(ORANGE, 0.55);
        v.selection.stroke = Stroke::new(1.5, ORANGE);
        v.hyperlink_color = ORANGE;
        v.window_corner_radius = CornerRadius::same(16);
        v.menu_corner_radius = CornerRadius::same(12);
        v.window_stroke = Stroke::new(1.0, shade(PEACH, -0.1));
        let radius = CornerRadius::same(10);
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = radius;
            w.fg_stroke.color = CHARCOAL;
        }
        // bg_fill doubles as the slider rail / checkbox well; weak_bg_fill is
        // the button face.
        v.widgets.inactive.bg_fill = shade(PEACH, -0.04);
        v.widgets.inactive.weak_bg_fill = PAPER;
        v.slider_trailing_fill = true;
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, shade(PEACH, -0.15));
        v.widgets.hovered.bg_fill = shade(BUTTER, 0.4);
        v.widgets.hovered.weak_bg_fill = shade(BUTTER, 0.4);
        v.widgets.hovered.bg_stroke = Stroke::new(1.5, shade(PEACH, -0.3));
        v.widgets.active.bg_fill = BUTTER;
        v.widgets.active.weak_bg_fill = BUTTER;
        v.widgets.active.bg_stroke = Stroke::new(1.5, ORANGE);
        v.widgets.open.weak_bg_fill = shade(BUTTER, 0.4);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, shade(PEACH, -0.1));
    });
}

// ---- backgrounds & surfaces -----------------------------------------------

/// The shell's backdrop: the pre-game screen's cream→peach vertical gradient.
pub fn background(painter: &Painter, rect: Rect) {
    let mut mesh = Mesh::default();
    mesh.colored_vertex(rect.left_top(), CREAM);
    mesh.colored_vertex(rect.right_top(), CREAM);
    mesh.colored_vertex(rect.right_bottom(), PEACH);
    mesh.colored_vertex(rect.left_bottom(), PEACH);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// A white rounded card for grouping content (grown-ups page, forms).
pub fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(PAPER)
        .corner_radius(CornerRadius::same(20))
        .inner_margin(egui::Margin::same(22))
        .stroke(Stroke::new(1.0, shade(PEACH, -0.08)))
        .shadow(egui::epaint::Shadow {
            offset: [0, 4],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(18),
        })
}

// ---- keycaps --------------------------------------------------------------

/// Draw a chunky keycap into `rect`: a darker lip with the face on top. The
/// face sinks onto the lip while pressed and brightens a touch on hover.
/// Returns the face rect (where the label goes).
pub fn draw_keycap(p: &Painter, rect: Rect, face: Color32, hovered: bool, pressed: bool) -> Rect {
    let lip_h = (rect.height() * 0.09).clamp(4.0, 9.0);
    let r = CornerRadius::same((rect.height().min(rect.width()) * 0.22).clamp(8.0, 26.0) as u8);
    let lip = shade(face, -0.18);
    // Soft drop shadow under the whole key.
    p.rect_filled(
        rect.translate(Vec2::new(0.0, 3.0)).expand(1.0),
        r,
        Color32::from_black_alpha(16),
    );
    p.rect_filled(rect, r, lip);
    let sink = if pressed { lip_h * 0.75 } else { 0.0 };
    let face_rect = Rect::from_min_max(
        rect.min + Vec2::new(0.0, sink),
        Pos2::new(rect.max.x, rect.max.y - lip_h + sink),
    );
    let face_col = if hovered && !pressed {
        shade(face, 0.12)
    } else {
        face
    };
    p.rect_filled(face_rect, r, face_col);
    // Top highlight line, for a moulded look.
    p.rect_stroke(
        face_rect.shrink(1.0),
        r,
        Stroke::new(1.0, shade(face_col, 0.35)),
        StrokeKind::Inside,
    );
    face_rect
}

/// How a key button looks. Build with [`KeyButton::new`], then `.show(ui)`.
pub struct KeyButton<'a> {
    label: Option<&'a str>,
    icon: Option<Icon>,
    face: Color32,
    size: Vec2,
    font: f32,
    /// Accessible name when there is no visible label (icon-only keys).
    a11y: Option<&'a str>,
    selected: bool,
}

impl<'a> KeyButton<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label: Some(label),
            icon: None,
            face: PAPER,
            size: Vec2::new(200.0, 56.0),
            font: 19.0,
            a11y: None,
            selected: false,
        }
    }

    /// An icon-only square key; `name` is its accessible label.
    pub fn icon(icon: Icon, name: &'a str) -> Self {
        Self {
            label: None,
            icon: Some(icon),
            face: PAPER,
            size: Vec2::splat(52.0),
            font: 19.0,
            a11y: Some(name),
            selected: false,
        }
    }

    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn face(mut self, face: Color32) -> Self {
        self.face = face;
        self
    }
    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }
    pub fn font(mut self, font: f32) -> Self {
        self.font = font;
        self
    }
    /// Draw a ring around the key (the chosen option in a group).
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let (rect, resp) = ui.allocate_exact_size(self.size, Sense::click());
        let name = self.label.or(self.a11y).unwrap_or_default().to_string();
        let enabled = ui.is_enabled();
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, &name));
        if !ui.is_rect_visible(rect) {
            return resp;
        }
        let p = ui.painter();
        let face = if enabled {
            self.face
        } else {
            shade(self.face, 0.45)
        };
        if self.selected {
            p.rect_stroke(
                rect.expand(4.0),
                CornerRadius::same(18),
                Stroke::new(3.0, ORANGE),
                StrokeKind::Outside,
            );
        }
        let pressed = resp.is_pointer_button_down_on();
        let face_rect = draw_keycap(p, rect, face, resp.hovered() && enabled, pressed);
        let mut ink = ink_on(face);
        if !enabled {
            ink = ink.gamma_multiply(0.45);
        }
        match (self.icon, self.label) {
            (Some(icon), None) => {
                let s = face_rect.height().min(face_rect.width()) * 0.52;
                draw_icon(
                    p,
                    icon,
                    Rect::from_center_size(face_rect.center(), Vec2::splat(s)),
                    ink,
                );
            }
            (Some(icon), Some(label)) => {
                let s = (face_rect.height() * 0.46).min(34.0);
                let font = FontId::proportional(self.font);
                let galley = p.layout_no_wrap(label.to_string(), font, ink);
                let gap = 10.0;
                let total = s + gap + galley.size().x;
                let x0 = face_rect.center().x - total / 2.0;
                let icon_rect = Rect::from_center_size(
                    Pos2::new(x0 + s / 2.0, face_rect.center().y),
                    Vec2::splat(s),
                );
                draw_icon(p, icon, icon_rect, ink);
                p.galley(
                    Pos2::new(x0 + s + gap, face_rect.center().y - galley.size().y / 2.0),
                    galley,
                    ink,
                );
            }
            (None, Some(label)) => {
                p.text(
                    face_rect.center(),
                    Align2::CENTER_CENTER,
                    label,
                    FontId::proportional(self.font),
                    ink,
                );
            }
            (None, None) => {}
        }
        resp
    }
}

impl egui::Widget for KeyButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show(ui)
    }
}

/// A row of keycaps acting as radio buttons. Returns true when the choice changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    current: &mut T,
    options: &[(T, &str)],
    width: f32,
) -> bool {
    let mut changed = false;
    ui.horizontal_wrapped(|ui| {
        for (value, label) in options {
            let on = *current == *value;
            let face = if on { BUTTER } else { PAPER };
            if KeyButton::new(label)
                .face(face)
                .selected(on)
                .size(Vec2::new(width, 46.0))
                .font(16.0)
                .show(ui)
                .clicked()
                && !on
            {
                *current = *value;
                changed = true;
            }
        }
    });
    changed
}

// ---- avatar tiles ---------------------------------------------------------

/// A child's picture: a photo/character texture (centre-cropped, rounded) or,
/// without one, the drawn kid face on a pastel square.
pub fn paint_avatar(
    ui: &Ui,
    rect: Rect,
    tex: Option<(egui::TextureId, [usize; 2])>,
    bg: Color32,
    theme: &rondelek_core::config::Theme,
) {
    let r = CornerRadius::same((rect.width() * 0.2).clamp(8.0, 40.0) as u8);
    match tex {
        Some((id, size)) => {
            egui::Image::from_texture(egui::load::SizedTexture::new(id, rect.size()))
                .uv(cover_uv(size))
                .corner_radius(r)
                .paint_at(ui, rect);
        }
        None => {
            ui.painter().rect_filled(rect, r, shade(bg, 0.35));
            crate::ui::draw_kid_face(ui.painter(), rect.shrink(rect.width() * 0.12), theme);
        }
    }
}

/// UV rect that samples the centred square of a `size` texture (CSS
/// `object-fit: cover` into a square), so non-square photos crop, not stretch.
pub fn cover_uv(size: [usize; 2]) -> Rect {
    let (w, h) = (size[0] as f32, size[1] as f32);
    if w <= 0.0 || h <= 0.0 {
        return Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
    }
    let side = w.min(h);
    let ux = (w - side) / 2.0 / w;
    let uy = (h - side) / 2.0 / h;
    Rect::from_min_max(Pos2::new(ux, uy), Pos2::new(1.0 - ux, 1.0 - uy))
}

/// A round status badge (e.g. calibration) with an icon inside.
pub fn badge(p: &Painter, center: Pos2, radius: f32, fill: Color32, icon: Icon) {
    p.circle_filled(
        center + Vec2::new(0.0, 1.5),
        radius,
        Color32::from_black_alpha(30),
    );
    p.circle_filled(center, radius, fill);
    p.circle_stroke(center, radius, Stroke::new(2.0, PAPER));
    draw_icon(
        p,
        icon,
        Rect::from_center_size(center, Vec2::splat(radius * 1.1)),
        ink_on(fill),
    );
}

// ---- icons ------------------------------------------------------------------

/// Simple vector icons (no icon font needed, crisp at any size).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Back,
    Gear,
    Plus,
    Play,
    Mic,
    Pads,
    Star,
    Pencil,
    Trash,
    Camera,
    Folder,
    Check,
    Speaker,
    Close,
    Refresh,
}

pub fn draw_icon(p: &Painter, icon: Icon, r: Rect, ink: Color32) {
    let c = r.center();
    let s = r.width().min(r.height());
    let w = (s * 0.11).max(2.0);
    let stroke = Stroke::new(w, ink);
    let at = |x: f32, y: f32| Pos2::new(c.x + x * s, c.y + y * s);
    match icon {
        Icon::Back => {
            p.line_segment([at(0.18, -0.32), at(-0.16, 0.0)], stroke);
            p.line_segment([at(-0.16, 0.0), at(0.18, 0.32)], stroke);
            p.circle_filled(at(-0.16, 0.0), w / 2.0, ink);
        }
        Icon::Close => {
            p.line_segment([at(-0.3, -0.3), at(0.3, 0.3)], stroke);
            p.line_segment([at(0.3, -0.3), at(-0.3, 0.3)], stroke);
        }
        Icon::Plus => {
            p.line_segment([at(0.0, -0.36), at(0.0, 0.36)], stroke);
            p.line_segment([at(-0.36, 0.0), at(0.36, 0.0)], stroke);
        }
        Icon::Check => {
            p.add(Shape::line(
                vec![at(-0.32, 0.02), at(-0.08, 0.26), at(0.34, -0.24)],
                stroke,
            ));
        }
        Icon::Play => {
            p.add(Shape::convex_polygon(
                vec![at(-0.24, -0.34), at(0.36, 0.0), at(-0.24, 0.34)],
                ink,
                Stroke::NONE,
            ));
        }
        Icon::Gear => {
            for k in 0..8 {
                let a = k as f32 * std::f32::consts::TAU / 8.0;
                let (sn, cs) = a.sin_cos();
                p.line_segment(
                    [at(cs * 0.24, sn * 0.24), at(cs * 0.44, sn * 0.44)],
                    Stroke::new(w * 1.5, ink),
                );
            }
            p.circle_stroke(c, s * 0.28, Stroke::new(w * 1.2, ink));
            p.circle_filled(c, s * 0.1, ink);
        }
        Icon::Mic => {
            p.rect_filled(
                Rect::from_center_size(at(0.0, -0.12), Vec2::new(s * 0.3, s * 0.48)),
                CornerRadius::same((s * 0.15) as u8),
                ink,
            );
            let arc: Vec<Pos2> = (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI * (i as f32 / 12.0);
                    at(-0.28 * a.cos(), 0.02 + 0.24 * a.sin())
                })
                .collect();
            p.add(Shape::line(arc, stroke));
            p.line_segment([at(0.0, 0.26), at(0.0, 0.42)], stroke);
            p.line_segment([at(-0.16, 0.42), at(0.16, 0.42)], stroke);
        }
        Icon::Pads => {
            let cell = s * 0.24;
            for row in 0..2 {
                for col in 0..2 {
                    let x = c.x + (col as f32 - 0.5) * cell * 1.3;
                    let y = c.y + (row as f32 - 0.5) * cell * 1.3;
                    p.rect_filled(
                        Rect::from_center_size(Pos2::new(x, y), Vec2::splat(cell)),
                        CornerRadius::same((cell * 0.28) as u8),
                        ink,
                    );
                }
            }
        }
        Icon::Star => {
            let pts: Vec<Pos2> = (0..10)
                .map(|i| {
                    let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                    let rr = if i % 2 == 0 { 0.44 } else { 0.19 };
                    at(rr * a.cos(), rr * a.sin() + 0.03)
                })
                .collect();
            // A star isn't convex: fill it as one un-feathered triangle fan
            // (no AA seams between triangles), then stroke the outline for AA.
            let mut mesh = Mesh::default();
            mesh.colored_vertex(c + Vec2::new(0.0, s * 0.03), ink);
            for pt in &pts {
                mesh.colored_vertex(*pt, ink);
            }
            for i in 0..10u32 {
                mesh.add_triangle(0, 1 + i, 1 + (i + 1) % 10);
            }
            p.add(Shape::mesh(mesh));
            p.add(Shape::closed_line(pts, Stroke::new(1.0, ink)));
        }
        Icon::Pencil => {
            p.line_segment(
                [at(-0.28, 0.28), at(0.24, -0.24)],
                Stroke::new(w * 2.0, ink),
            );
            p.add(Shape::convex_polygon(
                vec![at(-0.36, 0.36), at(-0.33, 0.2), at(-0.2, 0.33)],
                ink,
                Stroke::NONE,
            ));
        }
        Icon::Trash => {
            p.line_segment([at(-0.34, -0.26), at(0.34, -0.26)], stroke);
            p.line_segment([at(-0.1, -0.36), at(0.1, -0.36)], stroke);
            p.add(Shape::line(
                vec![
                    at(-0.26, -0.18),
                    at(-0.2, 0.38),
                    at(0.2, 0.38),
                    at(0.26, -0.18),
                ],
                stroke,
            ));
        }
        Icon::Camera => {
            p.rect_stroke(
                Rect::from_center_size(at(0.0, 0.05), Vec2::new(s * 0.8, s * 0.56)),
                CornerRadius::same((s * 0.1) as u8),
                stroke,
                StrokeKind::Middle,
            );
            p.circle_stroke(at(0.0, 0.07), s * 0.15, stroke);
            p.line_segment([at(-0.14, -0.3), at(0.14, -0.3)], stroke);
        }
        Icon::Folder => {
            p.add(Shape::line(
                vec![
                    at(-0.4, 0.3),
                    at(-0.4, -0.28),
                    at(-0.12, -0.28),
                    at(-0.04, -0.18),
                    at(0.4, -0.18),
                    at(0.4, 0.3),
                    at(-0.4, 0.3),
                ],
                stroke,
            ));
        }
        Icon::Speaker => {
            p.add(Shape::convex_polygon(
                vec![
                    at(-0.36, -0.12),
                    at(-0.18, -0.12),
                    at(0.04, -0.32),
                    at(0.04, 0.32),
                    at(-0.18, 0.12),
                    at(-0.36, 0.12),
                ],
                ink,
                Stroke::NONE,
            ));
            let arc = |rr: f32| -> Vec<Pos2> {
                (0..=8)
                    .map(|i| {
                        let a = -0.8 + 1.6 * i as f32 / 8.0;
                        at(0.04 + rr * a.cos(), rr * a.sin())
                    })
                    .collect()
            };
            p.add(Shape::line(arc(0.2), stroke));
            p.add(Shape::line(arc(0.34), stroke));
        }
        Icon::Refresh => {
            let arc: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = 0.6 + 4.6 * i as f32 / 16.0;
                    at(0.3 * a.cos(), 0.3 * a.sin())
                })
                .collect();
            let end = *arc.last().unwrap_or(&c);
            p.add(Shape::line(arc, stroke));
            p.add(Shape::convex_polygon(
                vec![
                    end + Vec2::new(-s * 0.14, -s * 0.04),
                    end + Vec2::new(s * 0.1, -s * 0.12),
                    end + Vec2::new(s * 0.04, s * 0.14),
                ],
                ink,
                Stroke::NONE,
            ));
        }
    }
}

// ---- text -------------------------------------------------------------------

/// A big friendly heading.
pub fn title(ui: &mut Ui, text: &str, size: f32) {
    ui.label(
        egui::RichText::new(text)
            .size(size)
            .strong()
            .color(CHARCOAL),
    );
}

/// Secondary text.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(15.0).color(STONE));
}

/// The 5×7 pixel wordmark (same glyphs as the skin's key labels), drawn as
/// little rounded squares. ASCII only; `cell` is one pixel's size.
pub fn pixel_wordmark(p: &Painter, left_center: Pos2, text: &str, cell: f32, ink: Color32) -> f32 {
    let mut x = left_center.x;
    let top = left_center.y - cell * 3.5;
    for ch in text.chars() {
        if ch != ' ' {
            let bits = crate::pixelart::char_bits(ch);
            for (row, bits_row) in bits.iter().enumerate() {
                for col in 0..5 {
                    if (bits_row >> (4 - col)) & 1 == 1 {
                        let r = Rect::from_min_size(
                            Pos2::new(x + col as f32 * cell, top + row as f32 * cell),
                            Vec2::splat(cell * 0.9),
                        );
                        p.rect_filled(r, CornerRadius::same((cell * 0.25) as u8), ink);
                    }
                }
            }
        }
        x += cell * 6.0;
    }
    x - cell
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_color_is_stable_and_pastel() {
        assert_eq!(tile_color("abc"), tile_color("abc"));
        assert!(TILE_COLORS.contains(&tile_color("anything")));
    }

    #[test]
    fn ink_contrasts_with_background() {
        assert_eq!(ink_on(PAPER), CHARCOAL);
        assert_eq!(ink_on(CHARCOAL), Color32::WHITE);
    }

    #[test]
    fn cover_uv_crops_to_centre_square() {
        let uv = cover_uv([600, 200]);
        assert!((uv.min.x - 1.0 / 3.0).abs() < 1e-6);
        assert_eq!(uv.min.y, 0.0);
        assert_eq!(
            cover_uv([256, 256]),
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0))
        );
    }
}
