//! The single "Settings" window (F12). Vertical tabs, one per area — Audio,
//! Detection, Visualizer, Appearance, Calibration. This replaces the old
//! audio-only config window *and* the hidden Ctrl+Shift+D developer panel: there
//! is now exactly one place to configure everything.

use crate::audio::device::{self, DevicePref};
use crate::config::Settings;
use crate::i18n::{EUROPEAN_LANGS, I18n, endonym};
use crate::ui::level_meter;
use crate::util::format_timestamp;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Audio,
    Detection,
    Visualizer,
    Appearance,
    Calibration,
}

/// Read-only calibration facts the panel displays (owned by the app).
pub struct CalInfo {
    pub calibrated: bool,
    pub created: u64,
    pub mic: Option<String>,
    pub current_device: Option<String>,
}

/// What the panel wants the app to do after drawing.
#[derive(Default)]
pub struct ConfigOutcome {
    /// A setting changed — persist.
    pub changed: bool,
    /// The user asked to (re)calibrate the current profile.
    pub recalibrate: bool,
    /// The user picked a new UI language (app applies it — it owns the i18n).
    pub chosen_language: Option<String>,
    /// The user picked a skin: `Some(None)` = built-in base, `Some(Some(name))`
    /// = an installed skin folder. The app loads it — it owns the textures.
    pub chosen_skin: Option<Option<String>>,
}

pub struct ConfigPanel {
    pub visible: bool,
    tab: Tab,
}

impl ConfigPanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            tab: Tab::Audio,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn show(
        &mut self,
        ctx: &egui::Context,
        settings: &mut Settings,
        skins: &[String],
        i18n: &I18n,
        input_peak: f32,
        cal: &CalInfo,
    ) -> ConfigOutcome {
        let mut out = ConfigOutcome::default();
        if !self.visible {
            return out;
        }

        let mut open = self.visible;
        egui::Window::new(i18n.t("config.title"))
            .resizable(true)
            .default_width(460.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal_top(|ui| {
                    // Left: vertical tab list.
                    ui.vertical(|ui| {
                        ui.set_min_width(120.0);
                        for (tab, label) in [
                            (Tab::Audio, "Audio"),
                            (Tab::Detection, "Detection"),
                            (Tab::Visualizer, "Visualizer"),
                            (Tab::Appearance, "Appearance"),
                            (Tab::Calibration, "Calibration"),
                        ] {
                            ui.selectable_value(&mut self.tab, tab, label);
                        }
                    });
                    ui.separator();
                    // Right: the active tab's content.
                    ui.vertical(|ui| {
                        ui.set_min_width(300.0);
                        match self.tab {
                            Tab::Audio => audio_tab(ui, settings, i18n, input_peak, &mut out),
                            Tab::Detection => detection_tab(ui, settings, &mut out),
                            Tab::Visualizer => visualizer_tab(ui, settings, &mut out),
                            Tab::Appearance => appearance_tab(ui, settings, skins, i18n, &mut out),
                            Tab::Calibration => calibration_tab(ui, cal, &mut out),
                        }
                    });
                });
            });

        self.visible = open;
        out
    }
}

fn audio_tab(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    i18n: &I18n,
    input_peak: f32,
    out: &mut ConfigOutcome,
) {
    // Enumerate fresh each frame so hot-plugged devices appear without a refresh.
    let outputs = device::list_output_devices();
    let inputs = device::list_input_devices();
    let out_default = device::default_output_name();
    let in_default = device::default_input_name();

    ui.heading(i18n.t("config.audio"));
    ui.add_space(6.0);
    out.changed |= device_selector(
        ui,
        "cfg_output",
        i18n.t("config.output"),
        &outputs,
        out_default.as_deref(),
        &mut settings.output_device,
        i18n,
    );
    ui.add_space(12.0);
    out.changed |= device_selector(
        ui,
        "cfg_input",
        i18n.t("config.input"),
        &inputs,
        in_default.as_deref(),
        &mut settings.input_device,
        i18n,
    );
    ui.add_space(14.0);
    ui.label("Microphone level");
    level_meter(ui, input_peak, 300.0);
}

fn detection_tab(ui: &mut egui::Ui, settings: &mut Settings, out: &mut ConfigOutcome) {
    ui.heading("Vowel detection");
    ui.add_space(4.0);
    out.changed |= ui
        .add(egui::Slider::new(&mut settings.vowel_voicing_threshold, 0.0..=0.05).text("Voicing"))
        .on_hover_text("RMS below this reads as silence")
        .changed();
    out.changed |= ui
        .add(egui::Slider::new(&mut settings.vowel_smoothing, 0.0..=0.95).text("Smoothing"))
        .on_hover_text("Meter easing (0 = snappy, → 1 = sluggish)")
        .changed();
    out.changed |= ui
        .add(egui::Slider::new(&mut settings.vowel_show_threshold, 0.1..=0.9).text("Show at"))
        .on_hover_text("A vowel lights up only when its match clears this")
        .changed();
    out.changed |= ui
        .checkbox(&mut settings.vowel_steady, "Steady detection")
        .on_hover_text("Average ~0.25 s of voice before deciding - steadier on close vowel pairs, slightly slower to react")
        .changed();
    out.changed |= ui
        .add(egui::Slider::new(&mut settings.vowel_margin_threshold, 0.0..=0.6).text("Min margin"))
        .on_hover_text("…and only when it beats the runner-up by this much")
        .changed();
}

fn visualizer_tab(ui: &mut egui::Ui, settings: &mut Settings, out: &mut ConfigOutcome) {
    ui.heading("Visualizer");
    ui.add_space(4.0);
    out.changed |= ui
        .add(egui::Slider::new(&mut settings.visualizer_smoothing, 0.0..=0.99).text("Smoothing"))
        .changed();
    out.changed |= ui
        .add(egui::Slider::new(&mut settings.visualizer_decay, 0.01..=1.0).text("Decay"))
        .changed();
    out.changed |= ui
        .add(
            egui::Slider::new(&mut settings.visualizer_floor_db, -80.0..=-24.0)
                .text("Floor (dB)")
                .suffix(" dB"),
        )
        .on_hover_text("Lower = more sensitive = more movement")
        .changed();
    let mut bars = settings.visualizer_num_bars as f64;
    if ui
        .add(egui::Slider::new(&mut bars, 16.0..=256.0).text("Bars"))
        .changed()
    {
        settings.visualizer_num_bars = bars as usize;
        out.changed = true;
    }
}

fn appearance_tab(
    ui: &mut egui::Ui,
    settings: &mut Settings,
    skins: &[String],
    i18n: &I18n,
    out: &mut ConfigOutcome,
) {
    ui.heading("Appearance");
    ui.add_space(4.0);

    ui.label("Skin");
    let current_label = settings.skin.as_deref().unwrap_or("Base Pastel (built-in)");
    egui::ComboBox::from_id_salt("cfg_skin")
        .selected_text(current_label)
        .width(200.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(settings.skin.is_none(), "Base Pastel (built-in)")
                .clicked()
                && settings.skin.is_some()
            {
                out.chosen_skin = Some(None);
            }
            for name in skins {
                if ui
                    .selectable_label(settings.skin.as_deref() == Some(name), name)
                    .clicked()
                    && settings.skin.as_deref() != Some(name)
                {
                    out.chosen_skin = Some(Some(name.clone()));
                }
            }
        });
    if skins.is_empty() {
        ui.small(format!(
            "Drop skin zips into {}",
            crate::ui::skin::skins_dir().display()
        ));
    }
    ui.add_space(4.0);
    out.changed |= ui
        .add(egui::Slider::new(&mut settings.volume, 0.0..=1.0).text("Volume"))
        .changed();

    ui.add_space(12.0);
    ui.label("Language");
    let current = i18n.lang().to_string();
    egui::ComboBox::from_id_salt("cfg_lang")
        .selected_text(endonym(&current))
        .width(200.0)
        .show_ui(ui, |ui| {
            for lang in EUROPEAN_LANGS {
                if ui
                    .selectable_label(current == lang.code, lang.endonym)
                    .clicked()
                    && current != lang.code
                {
                    out.chosen_language = Some(lang.code.to_string());
                }
            }
        });
}

fn calibration_tab(ui: &mut egui::Ui, cal: &CalInfo, out: &mut ConfigOutcome) {
    ui.heading("Calibration");
    ui.add_space(6.0);
    if cal.calibrated {
        ui.label(egui::RichText::new("Calibrated ✓").strong());
        ui.label(format!("When: {}", format_timestamp(cal.created)));
        if let Some(mic) = &cal.mic {
            ui.label(format!("Microphone: {mic}"));
        }
        // Warn if the current input device differs from the calibrated one.
        if let (Some(cur), Some(cal_mic)) = (&cal.current_device, &cal.mic)
            && cur != cal_mic
        {
            ui.add_space(4.0);
            ui.colored_label(
                egui::Color32::from_rgb(0xD6, 0x8A, 0x2E),
                format!("Now using \"{cur}\" — recalibrate if recognition seems off."),
            );
        }
    } else {
        ui.label("This profile isn't calibrated yet.");
    }
    ui.add_space(10.0);
    if ui.button("Recalibrate…").clicked() {
        out.recalibrate = true;
    }
    ui.label(
        egui::RichText::new(
            "Opens the vowel grid to record all six (re-record any before saving).",
        )
        .small()
        .weak(),
    );
}

/// One labelled device dropdown ("Auto" + each device) plus a line showing the
/// currently-active device and a "(fallback)" marker when a pinned device is
/// missing. Returns true if the selection changed.
fn device_selector(
    ui: &mut egui::Ui,
    id_salt: &str,
    label: &str,
    devices: &[String],
    default: Option<&str>,
    setting: &mut Option<String>,
    i18n: &I18n,
) -> bool {
    let mut changed = false;

    ui.label(label);

    let selected_text = match setting.as_deref() {
        None => i18n.t("config.auto").to_string(),
        Some(name) => name.to_string(),
    };

    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected_text)
        .width(300.0)
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

    // Show what will actually be used (and flag a fallback).
    let pref = DevicePref::from_setting(setting);
    if let Some(target) = device::choose_target(&pref, devices, default) {
        let mut text = format!("{} {}", i18n.t("config.active"), target.name);
        if target.is_fallback {
            text.push_str(&format!(" ({})", i18n.t("config.fallback")));
        }
        ui.label(egui::RichText::new(text).small().weak());
    }

    changed
}
