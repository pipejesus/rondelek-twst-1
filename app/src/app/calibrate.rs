//! Voice calibration (the vowel detector's per-child templates): the overview of six vowel
//! keys and the hold-to-record screen.

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
    /// the profile, and return to the hub.
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
        if let Some(p) = self.current_profile.as_ref() {
            self.calibrated
                .insert(p.dir.clone(), self.current_calibration.is_some());
        }
        self.screen = AppScreen::Hub;
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
                self.screen = AppScreen::Hub;
                return;
            }
        };
        match selected {
            None => self.draw_calibrate_overview(ui),
            Some(i) => self.draw_calibrate_record(ui, i),
        }
    }

    /// Overview: the six vowels as big keys (green with a tick once recorded).
    /// Tap one to record (or re-record) it; Save is enabled once all six are in.
    pub(super) fn draw_calibrate_overview(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        shell::background(ui.painter(), full);
        let full = shell::inside_stripes(full);

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
        let done_count = recorded.iter().filter(|&&b| b).count();
        let all_recorded = recorded.len() == vowel::VOWELS.len() && recorded.iter().all(|&b| b);

        let (back, _) = self.top_bar(ui, full, true, false);
        let mut open: Option<usize> = None;
        let mut cancel = back;
        let mut save = false;

        let content = Rect::from_min_max(Pos2::new(full.left(), full.top() + 20.0), full.max);
        let mut content_ui = ui.new_child(egui::UiBuilder::new().max_rect(content));
        egui::ScrollArea::vertical().show(&mut content_ui, |ui| {
            ui.vertical_centered(|ui| {
                shell::title(ui, self.i18n.t("calibrate.title"), 34.0);
                shell::hint(ui, self.i18n.t("calibrate.overview_hint"));
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!("{done_count} / {}", vowel::VOWELS.len()))
                        .size(20.0)
                        .strong(),
                );
                ui.add_space(14.0);

                let key = 124.0;
                let gap = 20.0;
                for row in 0..2 {
                    let (row_rect, _) = ui
                        .allocate_exact_size(Vec2::new(3.0 * key + 2.0 * gap, key), Sense::hover());
                    for col in 0..3 {
                        let i = row * 3 + col;
                        let is_done = recorded.get(i).copied().unwrap_or(false);
                        let r = Rect::from_min_size(
                            row_rect.min + Vec2::new(col as f32 * (key + gap), 0.0),
                            Vec2::splat(key),
                        );
                        let letter = vowel::VOWELS[i].label();
                        let face = if is_done {
                            palette::OK
                        } else {
                            palette::SURFACE
                        };
                        if ui
                            .put(r, KeyButton::vowel(letter).face(face).size(r.size()))
                            .clicked()
                        {
                            open = Some(i);
                        }
                        if is_done {
                            shell::badge(
                                ui.painter(),
                                r.right_top() + Vec2::new(-10.0, 10.0),
                                14.0,
                                palette::OK,
                                Icon::Check,
                            );
                            let n = takes.get(i).copied().unwrap_or(0);
                            if n > 1 {
                                ui.painter().text(
                                    r.left_top() + Vec2::new(14.0, 16.0),
                                    Align2::LEFT_CENTER,
                                    format!("×{n}"),
                                    egui::FontId::proportional(15.0),
                                    palette::INK,
                                );
                            }
                        }
                    }
                    ui.add_space(gap);
                }

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    let row_w = 180.0 + 12.0 + 240.0;
                    ui.add_space(((ui.available_width() - row_w) / 2.0).max(0.0));
                    if ui
                        .add(
                            KeyButton::new(self.i18n.t("calibrate.cancel"))
                                .size(Vec2::new(180.0, 60.0)),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                    if ui
                        .add_enabled_ui(all_recorded, |ui| {
                            ui.add(
                                KeyButton::new(self.i18n.t("calibrate.save"))
                                    .with_icon(Icon::Check)
                                    .face(palette::ORANGE)
                                    .size(Vec2::new(240.0, 60.0))
                                    .font(21.0),
                            )
                        })
                        .inner
                        .clicked()
                    {
                        save = true;
                    }
                });
            });
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
            self.screen = AppScreen::Hub;
        }
    }

    /// Record one vowel by **holding** the big key (or Space). No auto-advance:
    /// capture runs only while held; releasing commits the take. After a good
    /// take, "Next sound" jumps to the next vowel still missing.
    pub(super) fn draw_calibrate_record(&mut self, ui: &mut Ui, i: usize) {
        let full = ui.max_rect();
        shell::background(ui.painter(), full);
        let full = shell::inside_stripes(full);

        let (was_recording, count, level, voiced, already, takes_done, next_missing) = self
            .calib
            .as_ref()
            .map(|s| {
                let rec = |k: usize| {
                    s.captured
                        .get(k)
                        .map(|f| f.len() >= vowel::MIN_CAPTURE_SAMPLES)
                        .unwrap_or(false)
                };
                let next = (1..vowel::VOWELS.len())
                    .map(|d| (i + d) % vowel::VOWELS.len())
                    .find(|&k| !rec(k));
                (
                    s.recording,
                    s.current.count(),
                    s.live_level,
                    s.live_voiced,
                    rec(i),
                    s.takes.get(i).copied().unwrap_or(0),
                    next,
                )
            })
            .unwrap_or((false, 0, 0.0, false, false, 0, None));

        let target = vowel::VOWELS[i];
        let progress = (count as f32 / CALIB_TARGET as f32).clamp(0.0, 1.0);

        // Holding either the big key or Space records.
        let mut holding = ui.input(|inp| inp.key_down(Key::Space));
        let (mut back, _) = self.top_bar(ui, full, true, false);
        let mut reset = false;
        let mut next: Option<usize> = None;

        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            shell::hint(ui, self.i18n.t("calibrate.say"));
            // The vowel, big, on a butter plate.
            let (plate, _) = ui.allocate_exact_size(Vec2::splat(170.0), Sense::hover());
            shell::panel(ui.painter(), plate, shell::PX, palette::BUTTER);
            let px = 12.0;
            shell::pixel_vowel(
                ui.painter(),
                target.label(),
                plate.center().x,
                plate.center().y + px * 3.0,
                px,
                palette::INK,
            );
            ui.add_space(16.0);

            let rec_label = if was_recording {
                self.i18n.t("calibrate.recording")
            } else {
                self.i18n.t("calibrate.record")
            };
            let resp = ui.add(
                KeyButton::new(rec_label)
                    .with_icon(Icon::Mic)
                    .face(if was_recording {
                        palette::DANGER
                    } else {
                        palette::ORANGE
                    })
                    .size(Vec2::new(340.0, 96.0))
                    .font(24.0),
            );
            if resp.is_pointer_button_down_on() {
                holding = true;
            }
            shell::hint(ui, self.i18n.t("calibrate.record_hint"));

            ui.add_space(8.0);
            if was_recording {
                ui.add_sized(
                    [340.0, 18.0],
                    egui::ProgressBar::new(progress).fill(palette::ORANGE),
                );
                crate::ui::level_meter(ui, level, 340.0, &self.i18n);
                if !voiced {
                    shell::hint(ui, self.i18n.t("calibrate.keep_going"));
                }
            } else if already {
                ui.label(
                    egui::RichText::new(format!(
                        "✔ {} ×{}",
                        self.i18n.t("calibrate.recorded"),
                        takes_done
                    ))
                    .color(palette::OK)
                    .size(18.0),
                );
                shell::hint(ui, self.i18n.t("calibrate.more_takes"));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let row_w = 200.0 + 12.0 + 240.0;
                    ui.add_space(((ui.available_width() - row_w) / 2.0).max(0.0));
                    if ui
                        .add(
                            KeyButton::new(self.i18n.t("calibrate.reset"))
                                .with_icon(Icon::Refresh)
                                .size(Vec2::new(200.0, 52.0))
                                .font(16.0),
                        )
                        .clicked()
                    {
                        reset = true;
                    }
                    let (label, face) = match next_missing {
                        Some(_) => (self.i18n.t("calibrate.next"), palette::OK),
                        None => (self.i18n.t("calibrate.all_done"), palette::OK),
                    };
                    if ui
                        .add(
                            KeyButton::new(label)
                                .with_icon(Icon::Play)
                                .face(face)
                                .size(Vec2::new(240.0, 52.0))
                                .font(17.0),
                        )
                        .clicked()
                    {
                        match next_missing {
                            Some(k) => next = Some(k),
                            None => back = true,
                        }
                    }
                });
            }
        });

        // A click on Back or Next shouldn't also count as a hold this frame.
        if back || next.is_some() {
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
        if let Some(k) = next {
            if let Some(s) = self.calib.as_mut() {
                s.selected = Some(k);
                s.recording = false;
                s.current.clear();
            }
            self.accumulated_samples.clear();
        } else if back && let Some(s) = self.calib.as_mut() {
            s.selected = None;
            s.recording = false;
            s.current.clear();
        }
    }
}
