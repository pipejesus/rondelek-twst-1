//! Per-child vowel calibration: the overview grid and the hold-to-record screen.

use super::*;

impl App {
    /// Enter the guided calibration flow for the current profile.
    pub(super) fn begin_calibration(&mut self) {
        if self.current_profile.is_none() {
            return;
        }
        self.calib = Some(CalibrationState {
            captured: vec![Vec::new(); vowel::VOWELS.len()],
            takes: vec![0; vowel::VOWELS.len()],
            captured_rms: vec![Vec::new(); vowel::VOWELS.len()],
            selected: None,
            recording: false,
            current: CalibrationCapture::new(),
            live_level: 0.0,
            live_voiced: false,
        });
        self.accumulated_samples.clear();
        self.screen = AppScreen::Calibrate;
    }

    /// Finish: build MFCC templates from the six captured vowels, save them to
    /// the profile, and return to the Sessions screen.
    pub(super) fn finish_calibration(&mut self) {
        if let Some(state) = self.calib.take()
            && state.captured.len() == vowel::VOWELS.len()
            && let Some(profile) = self.current_profile.as_ref()
        {
            let device = self
                .capture
                .as_ref()
                .map(|c| c.current_device().to_string());
            match vowel::build_calibration(
                &state.captured,
                &state.captured_rms,
                self.capture_rate,
                device,
                now_secs(),
            ) {
                Some(cal) => {
                    if let Err(e) = profile.save_calibration(&cal) {
                        eprintln!("Failed to save calibration: {e}");
                    }
                }
                None => eprintln!("Calibration incomplete — not enough voiced audio per vowel"),
            }
        }
        self.calib = None;
        self.apply_profile_calibration();
        self.screen = AppScreen::Sessions;
    }

    /// Pull the mic each frame. While the user is **holding** the record button
    /// (`state.recording`), feed voiced MFCC frames into the in-progress capture.
    /// There is no auto-advance — recording only happens while held.
    pub(super) fn pump_calibration(&mut self) {
        let dt = 1.0 / 60.0;
        self.maintain_audio(dt);
        let mic = self
            .capture
            .as_mut()
            .map(Capture::drain)
            .unwrap_or_default();
        if !mic.is_empty() {
            self.accumulated_samples.extend_from_slice(&mic);
            trim_rolling(&mut self.accumulated_samples, self.capture_rate);
        }

        let window =
            &self.accumulated_samples[self.accumulated_samples.len().saturating_sub(2048)..];
        let voiced = vowel::rms(window) >= self.settings.vowel_voicing_threshold;
        let level = window.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        self.input_peak = level;

        let recording = self
            .calib
            .as_ref()
            .is_some_and(|s| s.recording && s.selected.is_some());
        let frame = if recording && voiced {
            vowel::mfcc_frame(window, self.capture_rate)
        } else {
            None
        };
        if let Some(state) = self.calib.as_mut() {
            state.live_level = level;
            state.live_voiced = voiced;
            if recording {
                state.current.push(frame, vowel::rms(window));
            }
        }
    }

    pub(super) fn draw_calibrate(&mut self, ui: &mut Ui) {
        self.pump_calibration();
        if self.screen != AppScreen::Calibrate {
            return;
        }
        let selected = match self.calib.as_ref() {
            Some(s) => s.selected,
            None => {
                self.screen = AppScreen::Sessions;
                return;
            }
        };
        match selected {
            None => self.draw_calibrate_overview(ui),
            Some(i) => self.draw_calibrate_record(ui, i),
        }
    }

    /// Overview: a grid of the six vowels showing which are recorded. Tap one to
    /// record (or re-record) it; Save is enabled once all six are captured.
    pub(super) fn draw_calibrate_overview(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);

        let takes: Vec<u32> = self
            .calib
            .as_ref()
            .map(|s| s.takes.clone())
            .unwrap_or_default();
        let recorded: Vec<bool> = self
            .calib
            .as_ref()
            .map(|s| {
                s.captured
                    .iter()
                    .map(|f| f.len() >= vowel::MIN_CAPTURE_SAMPLES)
                    .collect()
            })
            .unwrap_or_default();
        let all_recorded = recorded.len() == vowel::VOWELS.len() && recorded.iter().all(|&b| b);

        let done = Color32::from_rgb(0x3C, 0xB0, 0x4B);
        let mut open: Option<usize> = None;
        let mut cancel = false;
        let mut save = false;

        ui.vertical_centered(|ui| {
            ui.add_space((full.height() * 0.08).min(48.0));
            ui.label(
                egui::RichText::new(self.i18n.t("calibrate.title"))
                    .color(self.theme.text_primary)
                    .size(24.0)
                    .strong(),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(self.i18n.t("calibrate.overview_hint"))
                    .color(self.theme.text_secondary)
                    .size(15.0),
            );
            ui.add_space(24.0);

            for row in 0..2 {
                ui.allocate_ui_with_layout(
                    egui::vec2(3.0 * 104.0, 104.0),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        for col in 0..3 {
                            let i = row * 3 + col;
                            let is_done = recorded.get(i).copied().unwrap_or(false);
                            let letter = vowel::VOWELS[i].label();
                            let n = takes.get(i).copied().unwrap_or(0);
                            let text = if is_done && n > 1 {
                                format!("{letter}\n✓×{n}")
                            } else if is_done {
                                format!("{letter}\n✓")
                            } else {
                                letter.to_string()
                            };
                            let fill = if is_done {
                                done
                            } else {
                                self.theme.pad_play_bg
                            };
                            let txt_color = if is_done {
                                Color32::WHITE
                            } else {
                                self.theme.text_primary
                            };
                            let btn = egui::Button::new(
                                egui::RichText::new(text)
                                    .color(txt_color)
                                    .size(34.0)
                                    .strong(),
                            )
                            .fill(fill)
                            .min_size(egui::vec2(96.0, 96.0));
                            if ui.add(btn).clicked() {
                                open = Some(i);
                            }
                        }
                    },
                );
                ui.add_space(10.0);
            }

            ui.add_space(18.0);
            ui.allocate_ui_with_layout(
                egui::vec2(320.0, 44.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if ui
                        .add_sized(
                            [150.0, 44.0],
                            egui::Button::new(self.i18n.t("calibrate.cancel")),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                    let save_btn = egui::Button::new(
                        egui::RichText::new(self.i18n.t("calibrate.save")).color(Color32::WHITE),
                    )
                    .fill(ORANGE);
                    if ui
                        .add_enabled(
                            all_recorded,
                            egui::Button::min_size(save_btn, [150.0, 44.0].into()),
                        )
                        .clicked()
                    {
                        save = true;
                    }
                },
            );
        });

        if let Some(i) = open {
            if let Some(s) = self.calib.as_mut() {
                s.selected = Some(i);
                s.recording = false;
                s.current.clear();
            }
            self.accumulated_samples.clear();
        } else if save && all_recorded {
            self.finish_calibration();
        } else if cancel {
            self.calib = None;
            self.screen = AppScreen::Sessions;
        }
    }

    /// Record one vowel by **holding** the record button (or Space). No
    /// auto-advance: capture runs only while held; releasing commits the take.
    pub(super) fn draw_calibrate_record(&mut self, ui: &mut Ui, i: usize) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);

        let (was_recording, count, level, voiced, already, takes_done) = self
            .calib
            .as_ref()
            .map(|s| {
                (
                    s.recording,
                    s.current.count(),
                    s.live_level,
                    s.live_voiced,
                    s.captured
                        .get(i)
                        .map(|f| f.len() >= vowel::MIN_CAPTURE_SAMPLES)
                        .unwrap_or(false),
                    s.takes.get(i).copied().unwrap_or(0),
                )
            })
            .unwrap_or((false, 0, 0.0, false, false, 0));

        let target = vowel::VOWELS[i];
        let progress = (count as f32 / CALIB_TARGET as f32).clamp(0.0, 1.0);
        let recording_red = Color32::from_rgb(0xD6, 0x3A, 0x2E);

        // Holding either the button or Space records.
        let mut holding = ui.input(|inp| inp.key_down(Key::Space));
        let mut back = false;
        let mut cancel = false;
        let mut reset = false;

        ui.vertical_centered(|ui| {
            ui.add_space((full.height() * 0.08).min(48.0));
            ui.label(
                egui::RichText::new(self.i18n.t("calibrate.say"))
                    .color(self.theme.text_secondary)
                    .size(16.0),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(target.label())
                    .color(ORANGE)
                    .size(96.0)
                    .strong(),
            );
            ui.add_space(14.0);

            let rec_label = if was_recording {
                self.i18n.t("calibrate.recording")
            } else {
                self.i18n.t("calibrate.record")
            };
            let rec_btn = egui::Button::new(
                egui::RichText::new(rec_label)
                    .color(Color32::WHITE)
                    .size(18.0),
            )
            .fill(if was_recording { recording_red } else { ORANGE })
            .min_size(egui::vec2(240.0, 56.0));
            let resp = ui.add(rec_btn);
            if resp.is_pointer_button_down_on() {
                holding = true;
            }
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(self.i18n.t("calibrate.record_hint"))
                    .color(self.theme.text_secondary)
                    .size(13.0),
            );

            ui.add_space(12.0);
            if was_recording {
                ui.add_sized([300.0, 16.0], egui::ProgressBar::new(progress).fill(ORANGE));
                ui.add_space(6.0);
                crate::ui::level_meter(ui, level, 300.0);
                if !voiced {
                    ui.label(
                        egui::RichText::new("(keep the sound going…)")
                            .color(self.theme.text_secondary)
                            .size(12.0),
                    );
                }
            } else if already {
                ui.label(
                    egui::RichText::new(format!(
                        "{} ✓×{}",
                        self.i18n.t("calibrate.recorded"),
                        takes_done
                    ))
                    .color(Color32::from_rgb(0x3C, 0xB0, 0x4B))
                    .size(15.0),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(self.i18n.t("calibrate.more_takes"))
                        .color(self.theme.text_secondary)
                        .size(13.0),
                );
                ui.add_space(4.0);
                if ui
                    .add_sized(
                        [160.0, 28.0],
                        egui::Button::new(self.i18n.t("calibrate.reset")),
                    )
                    .clicked()
                {
                    reset = true;
                }
            }

            ui.add_space(20.0);
            ui.allocate_ui_with_layout(
                egui::vec2(320.0, 44.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if ui
                        .add_sized(
                            [150.0, 44.0],
                            egui::Button::new(self.i18n.t("calibrate.back")),
                        )
                        .clicked()
                    {
                        back = true;
                    }
                    if ui
                        .add_sized(
                            [150.0, 44.0],
                            egui::Button::new(self.i18n.t("calibrate.cancel")),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                },
            );
        });

        // A click on Back/Cancel shouldn't also count as a hold this frame.
        if back || cancel {
            holding = false;
        }

        // Apply hold-state edges: press = fresh take; release = commit if enough.
        let press_edge = holding && !was_recording;
        let release_edge = !holding && was_recording;
        let enough = count >= vowel::MIN_CAPTURE_SAMPLES;
        if let Some(s) = self.calib.as_mut() {
            s.recording = holding;
            if press_edge {
                s.current.clear();
            }
            if release_edge {
                if enough {
                    let (frames, rms) = s.current.take();
                    s.captured[i].extend(frames);
                    s.captured_rms[i].extend(rms);
                    s.takes[i] += 1;
                } else {
                    s.current.clear();
                }
            }
        }
        if press_edge {
            self.accumulated_samples.clear();
        }

        if reset && let Some(s) = self.calib.as_mut() {
            s.captured[i].clear();
            s.captured_rms[i].clear();
            s.takes[i] = 0;
        }
        if back {
            if let Some(s) = self.calib.as_mut() {
                s.selected = None;
                s.recording = false;
                s.current.clear();
            }
        } else if cancel {
            self.calib = None;
            self.screen = AppScreen::Sessions;
        }
    }
}
