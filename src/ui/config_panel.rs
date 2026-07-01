use crate::audio::device::{self, DevicePref};
use crate::config::Settings;
use crate::i18n::I18n;

/// User-facing "Settings" window (F12). Currently hosts audio device
/// selection; designed to grow additional sections over time.
pub struct ConfigPanel {
    pub visible: bool,
}

impl ConfigPanel {
    pub fn new() -> Self {
        Self { visible: false }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Draw the panel when visible. Returns `true` if the user changed a
    /// device selection, so the caller can persist settings.
    pub fn show(&mut self, ctx: &egui::Context, settings: &mut Settings, i18n: &I18n) -> bool {
        if !self.visible {
            return false;
        }

        // Enumerate fresh each frame the window is open, so hot-plugged devices
        // appear without any manual refresh.
        let outputs = device::list_output_devices();
        let inputs = device::list_input_devices();
        let out_default = device::default_output_name();
        let in_default = device::default_input_name();

        let mut changed = false;
        let mut open = self.visible;

        egui::Window::new(i18n.t("config.title"))
            .resizable(true)
            .default_width(340.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.heading(i18n.t("config.audio"));
                ui.add_space(6.0);
                changed |= device_selector(
                    ui,
                    "cfg_output",
                    i18n.t("config.output"),
                    &outputs,
                    out_default.as_deref(),
                    &mut settings.output_device,
                    i18n,
                );
                ui.add_space(12.0);
                changed |= device_selector(
                    ui,
                    "cfg_input",
                    i18n.t("config.input"),
                    &inputs,
                    in_default.as_deref(),
                    &mut settings.input_device,
                    i18n,
                );
            });

        self.visible = open;
        changed
    }
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
