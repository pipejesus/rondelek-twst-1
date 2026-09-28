//! The grown-ups page: **one** scrolling page holding every setting, grouped
//! into cards. It replaces the old F12 settings window. Changes apply and save
//! immediately; each card says whether it affects just this child or everyone
//! on this computer.
//!
//! The page only edits `Settings` and reports *requests* (change language,
//! delete the child, play a test sound, …) in a [`SettingsOutcome`]; the app
//! owns audio, textures and profiles and carries them out. That split also lets
//! the UI tests drive the page headless with fake devices.

use std::collections::HashMap;
use std::path::PathBuf;

use egui::{Align, Color32, RichText, TextureHandle, TextureId, Ui, Vec2};

use crate::i18n::{EUROPEAN_LANGS, I18n};
use crate::ui::shell::{self, Icon, KeyButton, palette::*};
use crate::ui::{level_meter, when};
use rondelek_core::audio::device::{self, DevicePref};
use rondelek_core::config::{DetectionPreset, NUM_SAMPLES, Settings, Theme};
use rondelek_core::profile::SessionInfo;

/// The page's sections, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Child,
    Sessions,
    Sound,
    Voice,
    Screen,
    Games,
    Look,
    About,
}

/// Facts about the current child (only when a child is selected).
pub struct ChildInfo<'a> {
    pub name: &'a str,
    pub avatar: Option<(TextureId, [usize; 2])>,
    pub tile: Color32,
    /// `Some(created)` when calibrated.
    pub calibrated_at: Option<u64>,
    pub calibration_mic: Option<&'a str>,
    /// The child's sessions, newest first (`Profile::list_sessions`).
    pub sessions: &'a [SessionInfo],
    /// The one the Sounds tile carries on with (`profile::most_recently_used`).
    pub current_session: Option<usize>,
}

/// Everything the page reads, gathered by the app each frame.
pub struct SettingsCtx<'a> {
    pub i18n: &'a I18n,
    pub theme: &'a Theme,
    pub settings: &'a mut Settings,
    pub skins: &'a [String],
    pub outputs: &'a [String],
    pub inputs: &'a [String],
    pub output_default: Option<&'a str>,
    pub input_default: Option<&'a str>,
    /// The microphone actually in use (to warn about calibration mismatches).
    pub current_input: Option<&'a str>,
    pub input_peak: f32,
    pub child: Option<ChildInfo<'a>>,
    pub flags: &'a HashMap<String, TextureHandle>,
}

/// What the page asks the app to do after drawing.
#[derive(Default, Debug, PartialEq)]
pub struct SettingsOutcome {
    /// A setting changed (persist).
    pub changed: bool,
    pub chosen_language: Option<String>,
    /// `Some(None)` = built-in skin, `Some(Some(name))` = an installed one.
    pub chosen_skin: Option<Option<String>>,
    pub visualizer_changed: bool,
    pub recalibrate: bool,
    pub edit_child: bool,
    pub delete_child: bool,
    /// Open this session in the sampler.
    pub open_session: Option<PathBuf>,
    /// Start a new session with empty pads and open it.
    pub new_session: bool,
    pub test_sound: bool,
    pub open_data_folder: bool,
    pub open_skins_folder: bool,
}

/// Page state that outlives a frame.
#[derive(Default)]
pub struct SettingsPage {
    voice_advanced: bool,
    screen_advanced: bool,
    /// While asking "are you sure?": what the grown-up has typed so far.
    delete_confirm: Option<String>,
    /// List every session, not just the latest few.
    all_sessions: bool,
    /// Scroll this section into view on the next frame.
    pub scroll_to: Option<Section>,
}

const CARD_W: f32 = 700.0;
/// Sessions listed before "Show all".
const SESSIONS_SHOWN: usize = 5;

impl SettingsPage {
    pub fn show(&mut self, ui: &mut Ui, cx: SettingsCtx) -> SettingsOutcome {
        let mut out = SettingsOutcome::default();
        let SettingsCtx {
            i18n,
            theme,
            settings,
            skins,
            outputs,
            inputs,
            output_default,
            input_default,
            current_input,
            input_peak,
            child,
            flags,
        } = cx;
        let width = ui.available_width().min(CARD_W);

        let Self {
            voice_advanced,
            screen_advanced,
            delete_confirm,
            all_sessions,
            scroll_to,
        } = self;

        ui.vertical_centered(|ui| {
            ui.set_max_width(width);
            ui.spacing_mut().item_spacing.y = 18.0;

            if let Some(child) = &child {
                section(ui, scroll_to, Section::Child, |ui| {
                    child_card(
                        ui,
                        i18n,
                        theme,
                        child,
                        current_input,
                        delete_confirm,
                        &mut out,
                    )
                });
                section(ui, scroll_to, Section::Sessions, |ui| {
                    sessions_card(ui, i18n, child, all_sessions, &mut out)
                });
            }
            section(ui, scroll_to, Section::Sound, |ui| {
                sound_card(
                    ui,
                    i18n,
                    settings,
                    outputs,
                    inputs,
                    output_default,
                    input_default,
                    input_peak,
                    &mut out,
                )
            });
            section(ui, scroll_to, Section::Voice, |ui| {
                voice_card(ui, i18n, settings, voice_advanced, &mut out)
            });
            section(ui, scroll_to, Section::Screen, |ui| {
                screen_card(ui, i18n, settings, screen_advanced, &mut out)
            });
            section(ui, scroll_to, Section::Games, |ui| {
                games_card(ui, i18n, settings, &mut out)
            });
            section(ui, scroll_to, Section::Look, |ui| {
                look_card(ui, i18n, settings, skins, flags, &mut out)
            });
            section(ui, scroll_to, Section::About, |ui| {
                about_card(ui, i18n, &mut out)
            });
            ui.add_space(24.0);
        });
        out
    }
}

/// One card, scrolled into view when requested.
fn section(
    ui: &mut Ui,
    scroll_to: &mut Option<Section>,
    which: Section,
    body: impl FnOnce(&mut Ui),
) {
    let resp = shell::card(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.with_layout(egui::Layout::top_down(Align::Min), body);
    })
    .response;
    if *scroll_to == Some(which) {
        resp.scroll_to_me(Some(Align::TOP));
        *scroll_to = None;
    }
}

// ---- cards -------------------------------------------------------------------

fn card_header(ui: &mut Ui, icon: Icon, title: &str, scope: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), egui::Sense::hover());
        shell::draw_icon(ui.painter(), icon, rect, BUTTER);
        ui.label(
            RichText::new(title)
                .font(shell::pixel_font(27.0))
                .color(BUTTER),
        );
        ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(scope).size(13.0).color(TEXT_DIM));
        });
    });
    ui.add_space(4.0);
}

fn explain(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(14.0).color(TEXT_DIM));
}

#[allow(clippy::too_many_arguments)]
fn child_card(
    ui: &mut Ui,
    i18n: &I18n,
    theme: &Theme,
    child: &ChildInfo,
    current_input: Option<&str>,
    delete_confirm: &mut Option<String>,
    out: &mut SettingsOutcome,
) {
    card_header(
        ui,
        Icon::Pencil,
        i18n.t("settings.child.title"),
        i18n.t("settings.scope.child"),
    );
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(88.0), egui::Sense::hover());
        shell::paint_avatar(ui, rect, child.avatar, child.tile, theme);
        ui.vertical(|ui| {
            ui.label(RichText::new(child.name).size(24.0).strong());
            if KeyButton::new(i18n.t("settings.child.edit"))
                .with_icon(Icon::Pencil)
                .size(Vec2::new(240.0, 48.0))
                .font(16.0)
                .show(ui)
                .clicked()
            {
                out.edit_child = true;
            }
        });
    });

    ui.add_space(8.0);
    ui.label(
        RichText::new(i18n.t("settings.child.voice"))
            .size(17.0)
            .strong(),
    );
    match child.calibrated_at {
        Some(when) => {
            ui.label(
                RichText::new(format!(
                    "✔ {} · {}",
                    i18n.t("settings.child.calibrated"),
                    when::friendly(i18n, when)
                ))
                .color(OK),
            );
            if let (Some(cal_mic), Some(cur)) = (child.calibration_mic, current_input)
                && cal_mic != cur
            {
                ui.label(
                    RichText::new(format!(
                        "{} ({cal_mic} → {cur})",
                        i18n.t("settings.child.mic_changed")
                    ))
                    .color(DANGER),
                );
            }
        }
        None => explain(ui, i18n.t("settings.child.not_calibrated")),
    }
    if KeyButton::new(i18n.t("settings.child.recalibrate"))
        .with_icon(Icon::Mic)
        .face(BUTTER)
        .size(Vec2::new(280.0, 50.0))
        .font(16.0)
        .show(ui)
        .clicked()
    {
        out.recalibrate = true;
    }

    ui.add_space(10.0);
    ui.separator();
    match delete_confirm {
        None => {
            if KeyButton::new(i18n.t("settings.child.delete"))
                .with_icon(Icon::Trash)
                .size(Vec2::new(240.0, 46.0))
                .font(15.0)
                .show(ui)
                .clicked()
            {
                *delete_confirm = Some(String::new());
            }
        }
        Some(typed) => {
            explain(ui, i18n.t("settings.child.delete_explain"));
            ui.label(format!(
                "{} “{}”",
                i18n.t("settings.child.delete_type"),
                child.name
            ));
            ui.add(
                egui::TextEdit::singleline(typed)
                    .desired_width(280.0)
                    .hint_text(child.name),
            );
            let matches = typed.trim().to_lowercase() == child.name.trim().to_lowercase();
            let mut cancel = false;
            ui.horizontal(|ui| {
                if KeyButton::new(i18n.t("common.cancel"))
                    .size(Vec2::new(150.0, 46.0))
                    .font(15.0)
                    .show(ui)
                    .clicked()
                {
                    cancel = true;
                }
                if ui
                    .add_enabled_ui(matches, |ui| {
                        KeyButton::new(i18n.t("settings.child.delete_confirm"))
                            .with_icon(Icon::Trash)
                            .face(DANGER)
                            .size(Vec2::new(240.0, 46.0))
                            .font(15.0)
                            .show(ui)
                    })
                    .inner
                    .clicked()
                {
                    out.delete_child = true;
                    cancel = true;
                }
            });
            if cancel {
                *delete_confirm = None;
            }
        }
    }
}

/// The child's sessions: each with its date and how many pads hold a sound,
/// the one Sounds carries on with marked, and a key to open any of them.
fn sessions_card(
    ui: &mut Ui,
    i18n: &I18n,
    child: &ChildInfo,
    all: &mut bool,
    out: &mut SettingsOutcome,
) {
    card_header(
        ui,
        Icon::Pads,
        i18n.t("settings.sessions.title"),
        i18n.t("settings.scope.child"),
    );
    let sessions = child.sessions;
    if sessions.is_empty() {
        explain(ui, i18n.t("settings.sessions.none"));
    } else {
        explain(ui, i18n.t("settings.sessions.explain"));
        for (i, s) in sessions.iter().enumerate() {
            // The latest few, plus the current one even when it's older.
            let current = child.current_session == Some(i);
            if (*all || i < SESSIONS_SHOWN || current) && session_row(ui, i18n, s, current) {
                out.open_session = Some(s.dir.clone());
            }
        }
        if sessions.len() > SESSIONS_SHOWN {
            let label = if *all {
                i18n.t("settings.sessions.show_fewer").to_string()
            } else {
                i18n.t("settings.sessions.show_all")
                    .replace("{n}", &sessions.len().to_string())
            };
            if KeyButton::new(&label)
                .size(Vec2::new(240.0, 42.0))
                .font(15.0)
                .show(ui)
                .clicked()
            {
                *all = !*all;
            }
        }
    }
    ui.add_space(4.0);
    if KeyButton::new(i18n.t("settings.sessions.new"))
        .with_icon(Icon::Plus)
        .size(Vec2::new(360.0, 48.0))
        .font(15.0)
        .show(ui)
        .clicked()
    {
        out.new_session = true;
    }
}

/// One session in the list. True when its open key was pressed.
fn session_row(ui: &mut Ui, i18n: &I18n, s: &SessionInfo, current: bool) -> bool {
    let date = if s.created > 0 {
        when::friendly(i18n, s.created)
    } else {
        s.folder.clone()
    };
    let recorded = if s.recorded == 0 {
        i18n.t("settings.sessions.nothing_recorded").to_string()
    } else {
        i18n.t("settings.sessions.recorded")
            .replace("{n}", &s.recorded.to_string())
            .replace("{total}", &NUM_SAMPLES.to_string())
    };
    let mut open = false;
    egui::Frame::new()
        .fill(WELL)
        .stroke(egui::Stroke::new(
            2.0,
            if current { OK } else { shade_line() },
        ))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.label(RichText::new(date).size(18.0).strong().color(TEXT));
                    ui.label(RichText::new(recorded).size(14.0).color(TEXT_DIM));
                    if current {
                        ui.label(
                            RichText::new(format!("✔ {}", i18n.t("settings.sessions.current")))
                                .size(14.0)
                                .color(OK),
                        );
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(Align::Center), |ui| {
                    let (label, face) = if current {
                        (i18n.t("settings.sessions.continue"), OK)
                    } else {
                        (i18n.t("settings.sessions.open"), SURFACE)
                    };
                    open = KeyButton::new(label)
                        .with_icon(Icon::Play)
                        .face(face)
                        .size(Vec2::new(170.0, 46.0))
                        .font(15.0)
                        .show(ui)
                        .clicked();
                });
            });
        });
    open
}

/// The quiet outline of a session row that isn't the current one.
fn shade_line() -> Color32 {
    shell::shade(SURFACE, 0.25)
}

#[allow(clippy::too_many_arguments)]
fn sound_card(
    ui: &mut Ui,
    i18n: &I18n,
    settings: &mut Settings,
    outputs: &[String],
    inputs: &[String],
    output_default: Option<&str>,
    input_default: Option<&str>,
    input_peak: f32,
    out: &mut SettingsOutcome,
) {
    card_header(
        ui,
        Icon::Speaker,
        i18n.t("settings.sound.title"),
        i18n.t("settings.scope.computer"),
    );
    out.changed |= device_selector(
        ui,
        "cfg_output",
        i18n.t("settings.sound.speaker"),
        outputs,
        output_default,
        &mut settings.output_device,
        i18n,
    );
    ui.horizontal(|ui| {
        ui.label(i18n.t("settings.sound.volume"));
        out.changed |= ui
            .add(egui::Slider::new(&mut settings.volume, 0.0..=1.0).show_value(false))
            .changed();
        if KeyButton::new(i18n.t("settings.sound.test"))
            .with_icon(Icon::Play)
            .face(OK)
            .size(Vec2::new(200.0, 44.0))
            .font(15.0)
            .show(ui)
            .clicked()
        {
            out.test_sound = true;
        }
    });
    ui.add_space(8.0);
    out.changed |= device_selector(
        ui,
        "cfg_input",
        i18n.t("settings.sound.microphone"),
        inputs,
        input_default,
        &mut settings.input_device,
        i18n,
    );
    ui.label(i18n.t("settings.sound.level"));
    level_meter(ui, input_peak, 320.0, i18n);
    explain(ui, i18n.t("settings.sound.level_hint"));
}

fn voice_card(
    ui: &mut Ui,
    i18n: &I18n,
    settings: &mut Settings,
    advanced: &mut bool,
    out: &mut SettingsOutcome,
) {
    card_header(
        ui,
        Icon::Mic,
        i18n.t("settings.voice.title"),
        i18n.t("settings.scope.computer"),
    );
    explain(ui, i18n.t("settings.voice.explain"));
    let mut choice = DetectionPreset::matching(settings);
    let options = [
        (
            Some(DetectionPreset::Relaxed),
            i18n.t("settings.voice.relaxed"),
        ),
        (
            Some(DetectionPreset::Normal),
            i18n.t("settings.voice.normal"),
        ),
        (
            Some(DetectionPreset::Strict),
            i18n.t("settings.voice.strict"),
        ),
    ];
    if shell::segmented(ui, &mut choice, &options, 180.0)
        && let Some(preset) = choice
    {
        preset.apply(settings);
        out.changed = true;
    }
    if choice.is_none() {
        explain(ui, i18n.t("settings.voice.custom"));
    }
    ui.checkbox(advanced, i18n.t("settings.advanced"));
    if *advanced {
        slider(
            ui,
            &mut settings.vowel_voicing_threshold,
            0.0..=0.05,
            i18n.t("settings.voice.voicing"),
            out,
        );
        slider(
            ui,
            &mut settings.vowel_show_threshold,
            0.1..=0.9,
            i18n.t("settings.voice.show"),
            out,
        );
        slider(
            ui,
            &mut settings.vowel_margin_threshold,
            0.0..=0.6,
            i18n.t("settings.voice.margin"),
            out,
        );
        slider(
            ui,
            &mut settings.vowel_smoothing,
            0.0..=0.95,
            i18n.t("settings.voice.smoothing"),
            out,
        );
        out.changed |= ui
            .checkbox(&mut settings.vowel_steady, i18n.t("settings.voice.steady"))
            .changed();
    }
}

fn screen_card(
    ui: &mut Ui,
    i18n: &I18n,
    settings: &mut Settings,
    advanced: &mut bool,
    out: &mut SettingsOutcome,
) {
    card_header(
        ui,
        Icon::Pads,
        i18n.t("settings.screen.title"),
        i18n.t("settings.scope.computer"),
    );
    explain(ui, i18n.t("settings.screen.explain"));
    let options = [
        (0usize, i18n.t("settings.screen.spectrum")),
        (1, i18n.t("settings.screen.vowels")),
        (2, i18n.t("settings.screen.off")),
    ];
    if shell::segmented(ui, &mut settings.active_visualizer, &options, 180.0) {
        out.changed = true;
        out.visualizer_changed = true;
    }
    ui.checkbox(advanced, i18n.t("settings.advanced"));
    if *advanced {
        slider(
            ui,
            &mut settings.visualizer_smoothing,
            0.0..=0.99,
            i18n.t("settings.screen.smoothing"),
            out,
        );
        slider(
            ui,
            &mut settings.visualizer_decay,
            0.01..=1.0,
            i18n.t("settings.screen.decay"),
            out,
        );
        slider(
            ui,
            &mut settings.visualizer_floor_db,
            -80.0..=-24.0,
            i18n.t("settings.screen.floor"),
            out,
        );
        let mut bars = settings.visualizer_num_bars as f32;
        let before = settings.visualizer_num_bars;
        slider(
            ui,
            &mut bars,
            16.0..=256.0,
            i18n.t("settings.screen.bars"),
            out,
        );
        settings.visualizer_num_bars = bars.round() as usize;
        out.changed |= settings.visualizer_num_bars != before;
    }
}

fn games_card(ui: &mut Ui, i18n: &I18n, settings: &mut Settings, out: &mut SettingsOutcome) {
    card_header(
        ui,
        Icon::Star,
        i18n.t("settings.games.title"),
        i18n.t("settings.scope.computer"),
    );
    explain(ui, i18n.t("settings.games.explain"));
    ui.horizontal(|ui| {
        ui.label(i18n.t("settings.games.steady"));
        out.changed |= ui
            .add(egui::Slider::new(&mut settings.game_reaction, 0.0..=1.0).show_value(false))
            .changed();
        ui.label(i18n.t("settings.games.snappy"));
    });
}

fn look_card(
    ui: &mut Ui,
    i18n: &I18n,
    settings: &mut Settings,
    skins: &[String],
    flags: &HashMap<String, TextureHandle>,
    out: &mut SettingsOutcome,
) {
    card_header(
        ui,
        Icon::Star,
        i18n.t("settings.look.title"),
        i18n.t("settings.scope.computer"),
    );
    ui.label(
        RichText::new(i18n.t("settings.look.language"))
            .size(17.0)
            .strong(),
    );
    ui.horizontal_wrapped(|ui| {
        for lang in EUROPEAN_LANGS {
            let on = i18n.lang() == lang.code;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(150.0, 44.0), egui::Sense::click());
            resp.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, lang.endonym)
            });
            let face = if on { CHOSEN } else { SURFACE };
            let pressed = resp.is_pointer_button_down_on();
            let face_rect = shell::draw_keycap(ui.painter(), rect, face, resp.hovered(), pressed);
            if on {
                ui.painter().rect_stroke(
                    rect.expand(3.0),
                    egui::CornerRadius::same(14),
                    egui::Stroke::new(2.5, ORANGE),
                    egui::StrokeKind::Outside,
                );
            }
            let mut x = face_rect.left() + 10.0;
            if let Some(tex) = flags.get(lang.code) {
                let fr = egui::Rect::from_min_size(
                    egui::pos2(x, face_rect.center().y - 9.0),
                    Vec2::new(26.0, 18.0),
                );
                ui.painter().image(
                    tex.id(),
                    fr,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
                x = fr.right() + 8.0;
            }
            ui.painter().text(
                egui::pos2(x, face_rect.center().y),
                egui::Align2::LEFT_CENTER,
                lang.endonym,
                egui::FontId::proportional(15.0),
                shell::ink_on(face),
            );
            if resp.clicked() && !on {
                out.chosen_language = Some(lang.code.to_string());
            }
        }
    });

    ui.add_space(8.0);
    ui.label(
        RichText::new(i18n.t("settings.look.skin"))
            .size(17.0)
            .strong(),
    );
    ui.horizontal_wrapped(|ui| {
        // The built-ins first: Arcade (the default, `None`) and Classic; then
        // any installed skins, except one that shadows a built-in's name
        // (it takes that built-in's place instead).
        let mut options: Vec<(Option<&str>, &str)> = vec![
            (None, i18n.t("settings.look.skin_arcade")),
            (Some("base"), i18n.t("settings.look.skin_base")),
        ];
        options.extend(
            skins
                .iter()
                .filter(|s| !crate::ui::skin::is_builtin(s))
                .map(|s| (Some(s.as_str()), s.as_str())),
        );
        let chosen = match settings.skin.as_deref() {
            Some(crate::ui::skin::DEFAULT_SKIN) => None,
            other => other,
        };
        for (value, label) in options {
            let on = chosen == value;
            if KeyButton::new(label)
                .face(if on { CHOSEN } else { SURFACE })
                .selected(on)
                .size(Vec2::new(180.0, 46.0))
                .font(15.0)
                .show(ui)
                .clicked()
                && !on
            {
                out.chosen_skin = Some(value.map(str::to_string));
            }
        }
    });
    if KeyButton::new(i18n.t("settings.look.skins_folder"))
        .with_icon(Icon::Folder)
        .size(Vec2::new(260.0, 44.0))
        .font(15.0)
        .show(ui)
        .clicked()
    {
        out.open_skins_folder = true;
    }
}

fn about_card(ui: &mut Ui, i18n: &I18n, out: &mut SettingsOutcome) {
    card_header(ui, Icon::Folder, i18n.t("settings.about.title"), "");
    ui.label(format!("Rondelek TWST-1 · v{}", env!("CARGO_PKG_VERSION")));
    explain(ui, i18n.t("settings.about.data"));
    if KeyButton::new(i18n.t("settings.about.open_data"))
        .with_icon(Icon::Folder)
        .size(Vec2::new(260.0, 44.0))
        .font(15.0)
        .show(ui)
        .clicked()
    {
        out.open_data_folder = true;
    }
}

// ---- controls ----------------------------------------------------------------

fn slider(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
    out: &mut SettingsOutcome,
) {
    ui.label(label);
    out.changed |= ui
        .add(egui::Slider::new(value, range).max_decimals(3))
        .changed();
}

/// A labelled device dropdown ("Auto" + each device) plus what is actually in
/// use, flagging a fallback when a pinned device is missing. True if changed.
fn device_selector(
    ui: &mut Ui,
    id_salt: &str,
    label: &str,
    devices: &[String],
    default: Option<&str>,
    setting: &mut Option<String>,
    i18n: &I18n,
) -> bool {
    let mut changed = false;
    ui.label(RichText::new(label).size(17.0).strong());
    let selected_text = match setting.as_deref() {
        None => i18n.t("config.auto").to_string(),
        Some(name) => name.to_string(),
    };
    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected_text)
        .width(ui.available_width().min(420.0))
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(setting.is_none(), i18n.t("config.auto"))
                .clicked()
                && setting.is_some()
            {
                *setting = None;
                changed = true;
            }
            for dev in devices {
                let is_sel = setting.as_deref() == Some(dev.as_str());
                if ui.selectable_label(is_sel, dev).clicked() && !is_sel {
                    *setting = Some(dev.clone());
                    changed = true;
                }
            }
        });
    let pref = DevicePref::from_setting(setting);
    if let Some(target) = device::choose_target(&pref, devices, default) {
        let mut text = format!("{} {}", i18n.t("config.active"), target.name);
        if target.is_fallback {
            text.push_str(&format!(" ({})", i18n.t("config.fallback")));
        }
        explain(ui, &text);
    }
    changed
}

/// Headless UI tests: render the real page, click controls by their visible
/// (AccessKit) labels, and check what actually changed in `Settings`.
#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    struct State {
        settings: Settings,
        page: SettingsPage,
        i18n: I18n,
        theme: Theme,
        flags: HashMap<String, TextureHandle>,
        skins: Vec<String>,
        with_child: bool,
        sessions: Vec<SessionInfo>,
        current_session: Option<usize>,
        /// Every non-empty outcome, in frame order.
        outcomes: Vec<SettingsOutcome>,
    }

    impl State {
        fn new(with_child: bool) -> Self {
            Self {
                settings: Settings::default(),
                page: SettingsPage::default(),
                i18n: I18n::new("en"),
                theme: rondelek_core::config::theme_light(),
                flags: HashMap::new(),
                skins: vec!["sunny".to_string()],
                with_child,
                sessions: Vec::new(),
                current_session: None,
                outcomes: Vec::new(),
            }
        }

        fn any(&self, f: impl Fn(&SettingsOutcome) -> bool) -> bool {
            self.outcomes.iter().any(f)
        }
    }

    fn harness(with_child: bool) -> Harness<'static, State> {
        let outputs = vec!["Speakers".to_string(), "Headphones".to_string()];
        let inputs = vec!["USB Mic".to_string()];
        Harness::builder()
            .with_size(Vec2::new(900.0, 4000.0))
            .build_ui_state(
                move |ui, s: &mut State| {
                    // The page uses the app's fonts (the pixel family); they
                    // take effect a frame after installing, so the first
                    // frame only sets them up.
                    if !shell::fonts_installed(ui.ctx()) {
                        shell::install_fonts(ui.ctx());
                        shell::apply_style(ui.ctx());
                        ui.ctx().request_repaint();
                        return;
                    }
                    let child = s.with_child.then_some(ChildInfo {
                        name: "Maya",
                        avatar: None,
                        tile: BLUE,
                        calibrated_at: None,
                        calibration_mic: None,
                        sessions: &s.sessions,
                        current_session: s.current_session,
                    });
                    let out = s.page.show(
                        ui,
                        SettingsCtx {
                            i18n: &s.i18n,
                            theme: &s.theme,
                            settings: &mut s.settings,
                            skins: &s.skins,
                            outputs: &outputs,
                            inputs: &inputs,
                            output_default: Some("Speakers"),
                            input_default: Some("USB Mic"),
                            current_input: Some("USB Mic"),
                            input_peak: 0.2,
                            child,
                            flags: &s.flags,
                        },
                    );
                    if out != SettingsOutcome::default() {
                        s.outcomes.push(out);
                    }
                },
                State::new(with_child),
            )
    }

    /// A child with `n` sessions, newest first, one a day back from 2026-09-26;
    /// session `i` has `i % 4` pads recorded.
    fn harness_with_sessions(n: usize, current: Option<usize>) -> Harness<'static, State> {
        let mut h = harness(true);
        let day = 86_400;
        h.state_mut().sessions = (0..n)
            .map(|i| SessionInfo {
                dir: PathBuf::from(format!("/library/maya/sessions/s{i}")),
                folder: format!("s{i}"),
                created: 1_790_424_000 - i as u64 * day,
                modified: 0,
                last_opened: 0,
                recorded: i % 4,
                uid: format!("uid-{i}"),
            })
            .collect();
        h.state_mut().current_session = current;
        h.run();
        h
    }

    /// How many open keys ("Open" plus the current one's "Continue") show.
    fn open_keys(h: &Harness<'static, State>) -> usize {
        h.query_all_by_label("Open").count() + h.query_all_by_label("Continue").count()
    }

    #[test]
    fn detection_preset_buttons_change_all_detection_knobs() {
        let mut h = harness(false);
        h.get_by_label("Strict").click();
        h.run();
        let s = &h.state().settings;
        assert_eq!(DetectionPreset::matching(s), Some(DetectionPreset::Strict));
        assert!(h.state().any(|o| o.changed), "a change must be saved");
        // Nothing unrelated moved.
        assert_eq!(s.volume, Settings::default().volume);
        assert_eq!(s.language, Settings::default().language);
    }

    #[test]
    fn visualizer_choice_is_saved_and_reported() {
        let mut h = harness(false);
        h.get_by_label("Vowels").click();
        h.run();
        assert_eq!(h.state().settings.active_visualizer, 1);
        assert!(h.state().any(|o| o.changed && o.visualizer_changed));
    }

    #[test]
    fn picking_a_language_asks_the_app_to_switch() {
        let mut h = harness(false);
        h.get_by_label("Polski").click();
        h.run();
        assert!(
            h.state()
                .any(|o| o.chosen_language.as_deref() == Some("pl"))
        );
    }

    #[test]
    fn picking_a_skin_asks_the_app_to_load_it() {
        let mut h = harness(false);
        h.get_by_label("sunny").click();
        h.run();
        assert!(
            h.state()
                .any(|o| o.chosen_skin == Some(Some("sunny".to_string())))
        );
    }

    #[test]
    fn test_sound_button_requests_a_chime() {
        let mut h = harness(false);
        h.get_by_label("Play a test sound").click();
        h.run();
        assert!(h.state().any(|o| o.test_sound));
    }

    #[test]
    fn advanced_voice_controls_appear_on_request() {
        let mut h = harness(false);
        assert!(h.query_by_label("Quietest sound that counts").is_none());
        h.get_all_by_label("Show advanced settings")
            .next()
            .unwrap()
            .click();
        h.run();
        assert!(h.query_by_label("Quietest sound that counts").is_some());
    }

    #[test]
    fn child_card_only_shows_with_a_child() {
        let h = harness(false);
        assert!(h.query_by_label("Delete this profile…").is_none());
        assert!(h.query_by_label("New session with empty pads").is_none());
        let h = harness(true);
        assert!(h.query_by_label("Delete this profile…").is_some());
        assert!(h.query_by_label("New session with empty pads").is_some());
    }

    #[test]
    fn a_child_without_sessions_is_told_how_the_first_starts() {
        let mut h = harness(true);
        assert!(
            h.query_by_label("No sessions yet. The first one starts when Sounds is opened.")
                .is_some()
        );
        assert_eq!(open_keys(&h), 0);
        h.get_by_label("New session with empty pads").click();
        h.run();
        assert!(h.state().any(|o| o.new_session));
        assert!(!h.state().any(|o| o.open_session.is_some()));
    }

    #[test]
    fn sessions_list_with_counts_and_the_current_one_marked() {
        let h = harness_with_sessions(3, Some(1));
        assert_eq!(h.get_all_by_label("Open").count(), 2);
        assert_eq!(h.get_all_by_label("Continue").count(), 1);
        assert_eq!(h.get_all_by_label("✔ Sounds opens this one").count(), 1);
        // Session 0 has nothing yet; 1 and 2 have 1 and 2 pads.
        assert!(h.query_by_label("No recordings yet").is_some());
        assert!(h.query_by_label("1 of 12 pads recorded").is_some());
        assert!(h.query_by_label("2 of 12 pads recorded").is_some());
        // Dates read as dates, not folder names.
        assert!(h.query_by_label("s0").is_none());
    }

    #[test]
    fn open_and_continue_request_that_session() {
        let mut h = harness_with_sessions(3, Some(1));
        // The second "Open" is session 2 (session 1 shows "Continue").
        h.get_all_by_label("Open").nth(1).unwrap().click();
        h.run();
        let want = PathBuf::from("/library/maya/sessions/s2");
        assert!(h.state().any(|o| o.open_session.as_ref() == Some(&want)));

        h.get_by_label("Continue").click();
        h.run();
        let want = PathBuf::from("/library/maya/sessions/s1");
        assert!(h.state().any(|o| o.open_session.as_ref() == Some(&want)));
        assert!(!h.state().any(|o| o.new_session || o.changed));
    }

    #[test]
    fn long_lists_fold_but_keep_the_current_session_visible() {
        // Current is the oldest of 8: it shows next to the latest five.
        let mut h = harness_with_sessions(8, Some(7));
        assert_eq!(open_keys(&h), SESSIONS_SHOWN + 1);
        assert_eq!(h.get_all_by_label("Continue").count(), 1);

        h.get_by_label("Show all (8)").click();
        h.run();
        assert_eq!(open_keys(&h), 8);

        h.get_by_label("Show fewer").click();
        h.run();
        assert_eq!(open_keys(&h), SESSIONS_SHOWN + 1);
    }

    #[test]
    fn short_lists_have_no_show_all() {
        let h = harness_with_sessions(SESSIONS_SHOWN, Some(0));
        assert_eq!(open_keys(&h), SESSIONS_SHOWN);
        assert!(h.query_by_label_contains("Show all").is_none());
    }

    #[test]
    fn deleting_a_child_needs_the_name_typed() {
        let mut h = harness(true);
        h.get_by_label("Delete this profile…").click();
        h.run();
        // Wrong name: the delete key stays disabled.
        h.state_mut().page.delete_confirm = Some("Max".to_string());
        h.run();
        h.get_by_label("Delete for good").click();
        h.run();
        assert!(!h.state().any(|o| o.delete_child));

        // Right name (any case): now it goes through.
        h.state_mut().page.delete_confirm = Some("maya".to_string());
        h.run();
        h.get_by_label("Delete for good").click();
        h.run();
        assert!(h.state().any(|o| o.delete_child));
    }

    #[test]
    fn recalibrate_and_edit_are_requests_not_changes() {
        let mut h = harness(true);
        h.get_by_label("Calibrate the voice").click();
        h.run();
        h.get_by_label("Change name or picture").click();
        h.run();
        let st = h.state();
        assert!(st.any(|o| o.recalibrate));
        assert!(st.any(|o| o.edit_child));
        assert_eq!(
            st.settings.vowel_voicing_threshold,
            Settings::default().vowel_voicing_threshold
        );
    }
}
