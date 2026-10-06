//! The "shell" look: everything outside the skinned sampler (who's playing,
//! the child's hub, games menu, profile form, calibration, grown-ups page).
//!
//! **Arcade style**, the same 8-bit "chrome" as the games' entrance screens and
//! the README pictures: a navy night with a slowly twinkling starfield and 80s
//! stripe bands, chunky notched keycaps with a hard ink outline and a bevel,
//! pixel titles in the Tiny5 font, vivid colours that sit calmly on the deep
//! navy. The palette and the pixel bitmaps are shared with the game through
//! `rondelek_core::arcade`, so the app and the games can't drift apart.
//!
//! Kid-facing words (titles, big keys, names) are in the **pixel font**;
//! grown-up body text (descriptions, hints) stays in Space Grotesk, which
//! reads better in sentences. Every clickable widget reports an AccessKit
//! label, which is also how the UI tests find it.

use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Mesh, Painter, Pos2, Rect, Response, Sense,
    Shape, Stroke, StrokeKind, Ui, Vec2, WidgetInfo, WidgetType,
};
use rondelek_core::arcade as data;

// ---- palette ---------------------------------------------------------------

/// The arcade palette (`rondelek_core::arcade`) as egui colours, plus the
/// roles the screens use.
pub mod palette {
    use egui::Color32;
    use rondelek_core::arcade as data;

    const fn rgb(c: data::Rgb) -> Color32 {
        Color32::from_rgb(c[0], c[1], c[2])
    }

    pub const NIGHT: Color32 = rgb(data::NIGHT);
    pub const NIGHT_HI: Color32 = rgb(data::NIGHT_HI);
    pub const NIGHT_LO: Color32 = rgb(data::NIGHT_LO);
    pub const INK: Color32 = rgb(data::INK);
    pub const ORANGE: Color32 = rgb(data::ORANGE);
    pub const BUTTER: Color32 = rgb(data::BUTTER);
    pub const PINK: Color32 = rgb(data::PINK);
    pub const CYAN: Color32 = rgb(data::CYAN);
    pub const CREAM: Color32 = rgb(data::CREAM);
    pub const GREEN: Color32 = rgb(data::GREEN);
    pub const MIST: Color32 = rgb(data::MIST);
    pub const RED: Color32 = rgb(data::RED);
    /// The card colours by name (`data::TILE_COLORS`).
    pub const BLUE: Color32 = rgb(data::TILE_COLORS[0]);
    pub const VIOLET: Color32 = rgb(data::TILE_COLORS[1]);
    pub const SUNFLOWER: Color32 = rgb(data::TILE_COLORS[2]);

    // Roles.
    /// Main text on the night.
    pub const TEXT: Color32 = CREAM;
    /// Secondary text (hints, captions).
    pub const TEXT_DIM: Color32 = MIST;
    /// Raised surfaces: cards, plain keys.
    pub const SURFACE: Color32 = NIGHT_HI;
    /// Sunken wells: text fields, slider rails, name plates.
    pub const WELL: Color32 = NIGHT_LO;
    /// The one "do it" colour (Create, Save, Play).
    pub const ACCENT: Color32 = ORANGE;
    /// "Chosen" (the selected option in a group).
    pub const CHOSEN: Color32 = BUTTER;
    pub const OK: Color32 = GREEN;
    pub const DANGER: Color32 = RED;
}
use palette::*;

/// A child's stable colour, from their uid (same as in the games).
pub fn tile_color(seed: &str) -> Color32 {
    let [r, g, b] = data::tile_color(seed);
    Color32::from_rgb(r, g, b)
}

/// Mix toward white (`amount > 0`) or black (`amount < 0`).
pub fn shade(c: Color32, amount: f32) -> Color32 {
    let t = amount.abs().clamp(0.0, 1.0);
    let target = if amount >= 0.0 { 255.0 } else { 0.0 };
    let mix = |v: u8| (v as f32 + (target - v as f32) * t).round() as u8;
    Color32::from_rgba_unmultiplied(mix(c.r()), mix(c.g()), mix(c.b()), c.a())
}

/// Ink (near-black) or cream, whichever reads better on `bg`.
pub fn ink_on(bg: Color32) -> Color32 {
    let lum = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
    if lum < 140.0 { CREAM } else { INK }
}

// ---- the pixel grid & font ----------------------------------------------------

/// One 8-bit "pixel" of the shell's chrome, in points.
pub const PX: f32 = 3.0;

/// The pixel font family (Tiny5, installed in `App::new`'s font setup).
pub fn pixel_family() -> FontFamily {
    FontFamily::Name("pixel".into())
}

/// Tiny5 at a crisp size: its glyphs sit on a 9-row grid, so sizes snap to
/// whole multiples of 9 points (2× = 18, 3× = 27, …).
pub fn pixel_font(size: f32) -> FontId {
    let k = (size / 9.0).round().max(1.0);
    FontId::new(k * 9.0, pixel_family())
}

/// Pixel text with an optional hard drop shadow (one font pixel down-right).
#[allow(clippy::too_many_arguments)]
pub fn pixel_text(
    p: &Painter,
    pos: Pos2,
    align: Align2,
    text: &str,
    size: f32,
    color: Color32,
    shadow: Option<Color32>,
) -> Rect {
    let font = pixel_font(size);
    let k = font.size / 9.0;
    if let Some(sh) = shadow {
        p.text(pos + Vec2::splat(k), align, text, font.clone(), sh);
    }
    p.text(pos, align, text, font, color)
}

/// Draw a shared pixel bitmap centred on `center`, each bitmap pixel `px` points.
pub fn pixel_icon(p: &Painter, b: data::Bitmap, center: Pos2, px: f32, color: Color32) {
    let (w, h) = data::bitmap_size(b);
    let origin = center - Vec2::new(w as f32, h as f32) * px / 2.0;
    for (col, row) in data::bitmap_cells(b) {
        let r = Rect::from_min_size(
            origin + Vec2::new(col as f32, row as f32) * px,
            Vec2::splat(px),
        );
        p.rect_filled(r, 0.0, color);
    }
}

/// One of the six vowels as a bold pixel letter (`rondelek_core::arcade::
/// vowel_glyph`), centred on `cx`, sitting on `baseline`.
pub fn pixel_vowel(p: &Painter, label: &str, cx: f32, baseline: f32, px: f32, color: Color32) {
    let Some((b, base)) = data::vowel_glyph(label) else {
        return;
    };
    let (_, h) = data::bitmap_size(b);
    let top = baseline - base as f32 * px;
    pixel_icon(p, b, Pos2::new(cx, top + h as f32 * px / 2.0), px, color);
}

// ---- shapes ---------------------------------------------------------------------

/// A rectangle with its four corner pixels cut off — the 8-bit rounded corner.
pub fn notched(p: &Painter, r: Rect, px: f32, c: Color32) {
    p.rect_filled(r.shrink2(Vec2::new(px, 0.0)), 0.0, c);
    p.rect_filled(r.shrink2(Vec2::new(0.0, px)), 0.0, c);
}

/// An arcade panel: ink outline, then `face` with a light top-left bevel and a
/// dark bottom-right one.
pub fn panel(p: &Painter, r: Rect, px: f32, face: Color32) {
    notched(p, r, px, INK);
    let inner = r.shrink(px);
    notched(p, inner, px, shade(face, -0.35));
    notched(
        p,
        Rect::from_min_max(inner.min, inner.max - Vec2::splat(px)),
        px,
        shade(face, 0.3),
    );
    notched(p, inner.shrink(px), px, face);
}

/// The "this one" frame around `r`: a coloured line and an ink gap (like the
/// cabinet's screen well). Draw before the thing it frames.
pub fn highlight(p: &Painter, r: Rect, px: f32, c: Color32) {
    notched(p, r.expand(3.0 * px), px, c);
    notched(p, r.expand(px), px, INK);
}

// ---- fonts -----------------------------------------------------------------

/// Install the app's fonts: Space Grotesk (OFL) as the default proportional and
/// monospace family, with egui's defaults behind it as fallback (Cyrillic,
/// Greek, …), and Tiny5 (OFL) as the `pixel` family for kid-facing titles and
/// keys — it covers every shipped language, the regular fonts back it up.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "space_grotesk".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/SpaceGrotesk.ttf"
        ))),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "space_grotesk".to_owned());
    }
    fonts.font_data.insert(
        "tiny5".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../assets/fonts/Tiny5-Regular.ttf"
        ))),
    );
    let mut pixel = vec!["tiny5".to_owned()];
    pixel.extend(
        fonts
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default(),
    );
    fonts.families.insert(pixel_family(), pixel);
    ctx.set_fonts(fonts);
}

/// Whether [`install_fonts`] has taken effect (fonts apply from the next frame).
#[cfg(test)]
pub fn fonts_installed(ctx: &egui::Context) -> bool {
    ctx.fonts(|f| f.definitions().families.contains_key(&pixel_family()))
}

// ---- global style --------------------------------------------------------------

/// The dark arcade style for egui's own widgets (text fields, sliders, combo
/// boxes, scroll bars): navy surfaces, cream text, square corners.
pub fn apply_style(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        use egui::{FontFamily::Proportional, TextStyle};
        style.text_styles = [
            (TextStyle::Heading, pixel_font(27.0)),
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
        *v = egui::Visuals::dark();
        v.override_text_color = Some(TEXT);
        v.panel_fill = NIGHT;
        v.window_fill = SURFACE;
        v.extreme_bg_color = WELL; // text-edit background
        v.faint_bg_color = shade(NIGHT_HI, 0.05);
        v.code_bg_color = WELL;
        v.selection.bg_fill = shade(ORANGE, -0.2);
        v.selection.stroke = Stroke::new(2.0, BUTTER);
        v.hyperlink_color = CYAN;
        v.text_cursor.stroke = Stroke::new(2.0, BUTTER);
        v.window_corner_radius = CornerRadius::ZERO;
        v.menu_corner_radius = CornerRadius::ZERO;
        v.window_stroke = Stroke::new(3.0, INK);
        v.popup_shadow = egui::epaint::Shadow {
            offset: [6, 6],
            blur: 0,
            spread: 0,
            color: Color32::from_black_alpha(140),
        };
        for w in [
            &mut v.widgets.noninteractive,
            &mut v.widgets.inactive,
            &mut v.widgets.hovered,
            &mut v.widgets.active,
            &mut v.widgets.open,
        ] {
            w.corner_radius = CornerRadius::ZERO;
            w.fg_stroke.color = TEXT;
            w.expansion = 0.0;
        }
        // bg_fill doubles as the slider rail / checkbox well; weak_bg_fill is
        // the button face.
        v.widgets.noninteractive.bg_fill = SURFACE;
        v.widgets.noninteractive.bg_stroke = Stroke::new(2.0, INK);
        v.widgets.noninteractive.fg_stroke.color = TEXT;
        v.widgets.inactive.bg_fill = WELL;
        v.widgets.inactive.weak_bg_fill = SURFACE;
        v.widgets.inactive.bg_stroke = Stroke::new(2.0, INK);
        v.slider_trailing_fill = true;
        v.selection.bg_fill = CYAN;
        v.widgets.hovered.bg_fill = shade(NIGHT_HI, 0.12);
        v.widgets.hovered.weak_bg_fill = shade(NIGHT_HI, 0.12);
        v.widgets.hovered.bg_stroke = Stroke::new(2.0, CYAN);
        // (egui also takes *strong* text's colour from the active state, so
        // its ink must stay light.)
        v.widgets.active.bg_fill = BUTTER;
        v.widgets.active.weak_bg_fill = shade(NIGHT_HI, 0.25);
        v.widgets.active.fg_stroke.color = TEXT;
        v.widgets.active.bg_stroke = Stroke::new(2.0, BUTTER);
        v.widgets.open.weak_bg_fill = shade(NIGHT_HI, 0.12);
        v.widgets.open.bg_stroke = Stroke::new(2.0, CYAN);
    });
}

// ---- backgrounds & surfaces -------------------------------------------------------

/// Deterministic 0..1 hash of a star index.
fn hash01(i: u32, salt: u32) -> f32 {
    let mut h = i.wrapping_mul(0x9E37_79B9) ^ salt.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    (h & 0xFFFF) as f32 / 65535.0
}

/// The shell's backdrop: navy night, a starfield that twinkles slowly, and the
/// 80s stripe bands along the top and bottom. The twinkle rides on the app's
/// slow idle repaint (it asks for no extra frames).
pub fn background(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 0.0, NIGHT);
    let t = painter.ctx().input(|i| i.time) as f32;
    let p = PX;
    let cols = (rect.width() / p) as u32;
    let rows = (rect.height() / p) as u32;
    let n = (cols * rows / 420).clamp(40, 320);
    for i in 0..n {
        let x = rect.left() + (hash01(i, 1) * cols as f32).floor() * p;
        let y = rect.top() + (hash01(i, 2) * rows as f32).floor() * p;
        let col = match (hash01(i, 3) * 6.0) as u32 {
            0 => CYAN,
            1 => PINK,
            2 => BUTTER,
            _ => NIGHT_HI,
        };
        let phase = hash01(i, 4) * std::f32::consts::TAU;
        let speed = 0.5 + hash01(i, 5) * 0.7;
        let glow = 0.45 + 0.55 * (0.5 + 0.5 * (t * speed + phase).sin());
        let size = if hash01(i, 6) > 0.9 { 2.0 * p } else { p };
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(x, y), Vec2::splat(size)),
            0.0,
            col.gamma_multiply(glow),
        );
    }
    // The stripes go on a layer above the page (Middle, over the central
    // panel's Background) but below popups, so scrolling content slides
    // *under* them, framed, instead of over them.
    let over = painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("arcade_stripes"),
    ));
    stripes(&over, rect);
}

/// Height of the stripe bands along each edge.
pub const STRIPES_H: f32 = 8.0 * PX;

/// The part of a screen between the stripe bands, where content goes.
pub fn inside_stripes(r: Rect) -> Rect {
    r.shrink2(Vec2::new(0.0, STRIPES_H))
}

/// The 80s stripe bands: orange, butter, pink, cyan — from each edge inward.
pub fn stripes(painter: &Painter, rect: Rect) {
    for (i, c) in [ORANGE, BUTTER, PINK, CYAN].iter().enumerate() {
        let off = i as f32 * 2.0 * PX;
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.top() + off),
                Vec2::new(rect.width(), 2.0 * PX),
            ),
            0.0,
            *c,
        );
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.bottom() - off - 2.0 * PX),
                Vec2::new(rect.width(), 2.0 * PX),
            ),
            0.0,
            *c,
        );
    }
}

/// An arcade card around `add`'s contents (grown-ups page, forms): a notched,
/// bevelled navy panel with a hard ink outline and drop shadow.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let bg = ui.painter().add(Shape::Noop);
    let inner = egui::Frame::new()
        .inner_margin(egui::Margin::same(24))
        .show(ui, add);
    let r = inner.response.rect;
    let p = ui.painter();
    let mut shapes = Vec::new();
    // Collect the panel's rectangles into one shape behind the content.
    let collect = |rect: Rect, c: Color32, out: &mut Vec<Shape>| {
        out.push(Shape::rect_filled(rect.shrink2(Vec2::new(PX, 0.0)), 0.0, c));
        out.push(Shape::rect_filled(rect.shrink2(Vec2::new(0.0, PX)), 0.0, c));
    };
    collect(
        r.translate(Vec2::splat(2.0 * PX)),
        Color32::from_black_alpha(150),
        &mut shapes,
    );
    collect(r, INK, &mut shapes);
    let inner_r = r.shrink(PX);
    collect(inner_r, shade(SURFACE, -0.35), &mut shapes);
    collect(
        Rect::from_min_max(inner_r.min, inner_r.max - Vec2::splat(PX)),
        shade(SURFACE, 0.18),
        &mut shapes,
    );
    collect(inner_r.shrink(PX), SURFACE, &mut shapes);
    p.set(bg, Shape::Vec(shapes));
    inner
}

// ---- keycaps ------------------------------------------------------------------------

/// Draw a chunky arcade keycap into `rect`: a hard shadow, an ink outline, and
/// the face (with a light top/left bevel) standing on a dark lip. The face
/// sinks onto the lip while pressed and brightens a touch on hover. Returns the
/// face's content rect (where the label goes).
pub fn draw_keycap(p: &Painter, rect: Rect, face: Color32, hovered: bool, pressed: bool) -> Rect {
    let lip = 2.0 * PX;
    notched(
        p,
        rect.translate(Vec2::splat(PX)),
        PX,
        Color32::from_black_alpha(150),
    );
    notched(p, rect, PX, INK);
    let inner = rect.shrink(PX);
    notched(p, inner, PX, shade(face, -0.45));
    let sink = if pressed { lip } else { 0.0 };
    let face_rect = Rect::from_min_max(
        inner.min + Vec2::new(0.0, sink),
        Pos2::new(inner.max.x, inner.max.y - lip + sink),
    );
    let face_col = if hovered && !pressed {
        shade(face, 0.1)
    } else {
        face
    };
    notched(p, face_rect, PX, face_col);
    // Bevel: a light line along the top and the left.
    let light = shade(face_col, 0.32);
    p.rect_filled(
        Rect::from_min_size(
            face_rect.min + Vec2::new(PX, 0.0),
            Vec2::new(face_rect.width() - 2.0 * PX, PX),
        ),
        0.0,
        light,
    );
    p.rect_filled(
        Rect::from_min_size(
            face_rect.min + Vec2::new(0.0, PX),
            Vec2::new(PX, face_rect.height() - 2.0 * PX),
        ),
        0.0,
        light,
    );
    face_rect.shrink(PX)
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
    /// Draw the label as a bold pixel vowel (see [`pixel_vowel`]).
    vowel: bool,
}

impl<'a> KeyButton<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label: Some(label),
            icon: None,
            face: SURFACE,
            size: Vec2::new(200.0, 56.0),
            font: 18.0,
            a11y: None,
            selected: false,
            vowel: false,
        }
    }

    /// A key showing one of the six vowels as a bold pixel letter (the pixel
    /// font's own lowercase reads ambiguously at size). `label` is the vowel
    /// ("a" … "y"), also its accessible name.
    pub fn vowel(label: &'a str) -> Self {
        Self {
            vowel: true,
            ..Self::new(label)
        }
    }

    /// An icon-only square key; `name` is its accessible label.
    pub fn icon(icon: Icon, name: &'a str) -> Self {
        Self {
            label: None,
            icon: Some(icon),
            face: SURFACE,
            size: Vec2::splat(52.0),
            font: 18.0,
            a11y: Some(name),
            selected: false,
            vowel: false,
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
    /// Label size; snapped to the pixel font's crisp sizes (18, 27, …).
    pub fn font(mut self, font: f32) -> Self {
        self.font = font;
        self
    }
    /// Frame the key (the chosen option in a group).
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        // Grow wider when the label (in some language) doesn't fit.
        let mut size = self.size;
        if let Some(label) = self.label.filter(|_| !self.vowel) {
            let text_w = ui
                .painter()
                .layout_no_wrap(label.to_string(), pixel_font(self.font), Color32::WHITE)
                .size()
                .x;
            let icon_w = if self.icon.is_some() {
                ((size.y - 6.0 * PX) * 0.52).min(30.0) + 10.0
            } else {
                0.0
            };
            size.x = size
                .x
                .max((text_w + icon_w + 2.0 * (14.0 + 2.0 * PX)).ceil());
        }
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
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
            shade(self.face, -0.45)
        };
        if self.selected {
            highlight(p, rect, PX, CYAN);
        }
        let pressed = resp.is_pointer_button_down_on();
        let face_rect = draw_keycap(p, rect, face, resp.hovered() && enabled, pressed);
        let mut ink = ink_on(face);
        if !enabled {
            ink = ink.gamma_multiply(0.45);
        }
        let font = pixel_font(self.font);
        match (self.icon, self.label) {
            (Some(icon), None) => {
                let s = face_rect.height().min(face_rect.width()) * 0.62;
                draw_icon(
                    p,
                    icon,
                    Rect::from_center_size(face_rect.center(), Vec2::splat(s)),
                    ink,
                );
            }
            (Some(icon), Some(label)) => {
                let s = (face_rect.height() * 0.52).min(30.0);
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
                    Pos2::new(
                        (x0 + s + gap).round(),
                        (face_rect.center().y - galley.size().y / 2.0).round(),
                    ),
                    galley,
                    ink,
                );
            }
            (None, Some(label)) if self.vowel => {
                let px = (face_rect.height() * 0.5 / data::VOWEL_ROWS as f32)
                    .floor()
                    .max(1.0);
                let baseline = face_rect.center().y + px * data::VOWEL_ROWS as f32 / 2.0;
                pixel_vowel(p, label, face_rect.center().x, baseline, px, ink);
            }
            (None, Some(label)) => {
                p.text(face_rect.center(), Align2::CENTER_CENTER, label, font, ink);
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
        ui.spacing_mut().item_spacing.x = 16.0;
        for (value, label) in options {
            let on = *current == *value;
            let face = if on { CHOSEN } else { SURFACE };
            if KeyButton::new(label)
                .face(face)
                .selected(on)
                .size(Vec2::new(width, 46.0))
                .font(18.0)
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

// ---- avatar tiles -----------------------------------------------------------------

/// A child's picture in an ink-framed well: a photo/character texture
/// (centre-cropped) on cream or, without one, the drawn kid face.
pub fn paint_avatar(
    ui: &Ui,
    rect: Rect,
    tex: Option<(egui::TextureId, [usize; 2])>,
    bg: Color32,
    theme: &rondelek_core::config::Theme,
) {
    let p = ui.painter();
    notched(p, rect.expand(PX), PX, INK);
    match tex {
        Some((id, size)) => {
            p.rect_filled(rect, 0.0, CREAM);
            egui::Image::from_texture(egui::load::SizedTexture::new(id, rect.size()))
                .uv(cover_uv(size))
                .paint_at(ui, rect);
        }
        None => {
            p.rect_filled(rect, 0.0, shade(bg, 0.55));
            crate::ui::draw_kid_face(p, rect.shrink(rect.width() * 0.12), theme);
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

/// A small square status badge (e.g. voice calibration done) with an icon inside.
pub fn badge(p: &Painter, center: Pos2, radius: f32, fill: Color32, icon: Icon) {
    let r = Rect::from_center_size(center, Vec2::splat(radius * 2.0));
    notched(
        p,
        r.translate(Vec2::splat(PX)),
        PX,
        Color32::from_black_alpha(150),
    );
    panel(p, r, PX, fill);
    draw_icon(
        p,
        icon,
        Rect::from_center_size(center, Vec2::splat(radius * 1.15)),
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

// ---- text -------------------------------------------------------------------------

/// A big pixel heading: butter over a hard ink shadow, like the README
/// frames' captions. `size` snaps to the pixel font's crisp sizes.
pub fn title(ui: &mut Ui, text: &str, size: f32) {
    let font = pixel_font(size);
    let k = font.size / 9.0;
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font.clone(), BUTTER);
    let (rect, _) = ui.allocate_exact_size(galley.size() + Vec2::splat(k), Sense::hover());
    let pos = Pos2::new(rect.left().round(), rect.top().round());
    ui.painter().text(
        pos + Vec2::splat(k),
        Align2::LEFT_TOP,
        text,
        font.clone(),
        INK,
    );
    ui.painter().galley(pos, galley, BUTTER);
}

/// Secondary text (for the grown-ups): Space Grotesk, soft lilac-grey.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(egui::RichText::new(text).size(15.0).color(TEXT_DIM));
}

/// The 5×7 pixel wordmark (same glyphs as the skin's key labels and the
/// README pictures), drawn as hard square pixels with a drop shadow. ASCII
/// only; `cell` is one pixel's size.
pub fn pixel_wordmark(p: &Painter, left_center: Pos2, text: &str, cell: f32, ink: Color32) -> f32 {
    let draw = |origin: Pos2, color: Color32| {
        let mut x = origin.x;
        let top = origin.y - cell * 3.5;
        for ch in text.chars() {
            if ch != ' ' {
                let bits = crate::pixelart::char_bits(ch);
                for (row, bits_row) in bits.iter().enumerate() {
                    for col in 0..5 {
                        if (bits_row >> (4 - col)) & 1 == 1 {
                            let r = Rect::from_min_size(
                                Pos2::new(x + col as f32 * cell, top + row as f32 * cell),
                                Vec2::splat(cell),
                            );
                            p.rect_filled(r, 0.0, color);
                        }
                    }
                }
            }
            x += cell * 6.0;
        }
        x - cell
    };
    draw(left_center + Vec2::splat(cell), shade(ORANGE, -0.15));
    draw(left_center, ink)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_buttons_grow_to_fit_long_labels() {
        use egui_kittest::Harness;
        use egui_kittest::kittest::Queryable;
        let mut h = Harness::builder()
            .with_size(Vec2::new(900.0, 300.0))
            .build_ui(|ui| {
                if !fonts_installed(ui.ctx()) {
                    install_fonts(ui.ctx());
                    ui.ctx().request_repaint();
                    return;
                }
                KeyButton::new("Short")
                    .size(Vec2::new(200.0, 46.0))
                    .show(ui);
                KeyButton::new("A much longer label than the key was sized for")
                    .with_icon(Icon::Play)
                    .size(Vec2::new(200.0, 46.0))
                    .show(ui);
            });
        h.run();
        // Short labels keep the asked-for size; long ones get room.
        assert_eq!(h.get_by_label("Short").rect().width(), 200.0);
        let long = h
            .get_by_label("A much longer label than the key was sized for")
            .rect();
        assert!(long.width() > 300.0, "grew to {}", long.width());
    }

    #[test]
    fn tile_color_is_stable_and_from_the_shared_palette() {
        assert_eq!(tile_color("abc"), tile_color("abc"));
        let c = tile_color("anything");
        assert!(
            data::TILE_COLORS
                .iter()
                .any(|t| Color32::from_rgb(t[0], t[1], t[2]) == c)
        );
    }

    #[test]
    fn ink_contrasts_with_background() {
        assert_eq!(ink_on(CREAM), INK);
        assert_eq!(ink_on(NIGHT), CREAM);
        assert_eq!(ink_on(BUTTER), INK);
    }

    #[test]
    fn pixel_font_snaps_to_whole_font_pixels() {
        assert_eq!(pixel_font(19.0).size, 18.0);
        assert_eq!(pixel_font(28.0).size, 27.0);
        assert_eq!(pixel_font(3.0).size, 9.0);
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
