use crate::config::{Settings, Theme};

pub struct DevPanel {
    pub visible: bool,
}

impl DevPanel {
    pub fn new() -> Self {
        Self { visible: false }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn show(&mut self, ctx: &egui::Context, settings: &mut Settings, theme: &mut Theme) {
        if !self.visible {
            return;
        }

        egui::Window::new("Dev Panel")
            .resizable(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Visualizer");
                ui.add(
                    egui::Slider::new(&mut settings.visualizer_smoothing, 0.0..=0.99)
                        .text("Smoothing"),
                );
                ui.add(egui::Slider::new(&mut settings.visualizer_decay, 0.01..=1.0).text("Decay"));
                ui.add(
                    egui::Slider::new(&mut settings.visualizer_floor_db, -80.0..=-24.0)
                        .text("Floor (dB)")
                        .suffix(" dB"),
                )
                .on_hover_text("Lower = more sensitive = more movement");
                let mut bars = settings.visualizer_num_bars as f64;
                if ui
                    .add(egui::Slider::new(&mut bars, 16.0..=256.0).text("Bars"))
                    .changed()
                {
                    settings.visualizer_num_bars = bars as usize;
                }
                ui.separator();
                ui.heading("Vowel detector");
                ui.add(
                    egui::Slider::new(&mut settings.vowel_speaker_scale, 0.9..=1.8)
                        .text("Speaker scale"),
                )
                .on_hover_text("Higher for children (shorter vocal tract = higher formants)");
                ui.add(
                    egui::Slider::new(&mut settings.vowel_voicing_threshold, 0.0..=0.05)
                        .text("Voicing"),
                )
                .on_hover_text("RMS below this reads as silence");
                ui.add(
                    egui::Slider::new(&mut settings.vowel_smoothing, 0.0..=0.95)
                        .text("Vowel smoothing"),
                );

                ui.separator();
                ui.heading("Theme");
                if ui
                    .button(if settings.dark_mode {
                        "☀ Light Mode"
                    } else {
                        "🌙 Dark Mode"
                    })
                    .clicked()
                {
                    settings.dark_mode = !settings.dark_mode;
                    *theme = if settings.dark_mode {
                        crate::config::theme_dark()
                    } else {
                        crate::config::theme_light()
                    };
                }
                ui.separator();
                ui.heading("Audio");
                ui.add(egui::Slider::new(&mut settings.volume, 0.0..=1.0).text("Volume"));
            });
    }
}
