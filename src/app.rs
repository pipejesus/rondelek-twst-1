use egui::{Align2, Color32, Key, Pos2, Rect, Sense, Ui, Vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::audio::vowel::{
    self, CalibrationCapture, VowelCalibration, VowelConfig, practice_targets,
};
use crate::audio::{Capture, Playback, Sample, device};
use crate::config::{
    self, NUM_SAMPLES, PadKind, REC_PAD, ROUNDING_PAD, SAMPLE_PADS, Settings, Theme,
};
use crate::i18n::{self, EUROPEAN_LANGS, I18n};
use crate::profile::{self, Profile, SessionInfo};
use crate::session::Session;
use crate::ui::{
    AudioFrame, ConfigPanel, DevPanel, Pad, PadMode, Renderer, SpectrumVisualizer, Visualizer,
    VowelVisualizer, compute_layout, draw_keycap, draw_kid_face, gloss_overlay,
};
use crate::util::now_secs;

/// Index of the REC control pad within `self.pads` (after the sample pads).
const REC_PAD_IDX: usize = NUM_SAMPLES;

/// The signature orange, used for primary actions.
const ORANGE: Color32 = Color32::from_rgb(0xFF, 0x6A, 0x1A);

/// Top-level screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppScreen {
    /// Pick or create a child profile (+ language picker).
    Profiles,
    /// Create or edit a child profile (name + avatar).
    ProfileForm,
    /// A profile's sessions: resume a past one or start fresh.
    Sessions,
    /// Per-child voice calibration for the vowel detector.
    Calibrate,
    /// The sampler.
    Session,
}

/// Voiced frames to collect per vowel before it is accepted and we advance.
const CALIB_TARGET: usize = 24;

/// Whether we're waiting for the child to begin the current vowel, or actively
/// capturing it. Capture only runs in `Listening`, so each vowel has a clear
/// start the user controls (no auto-jumping mid-sound).
#[derive(Clone, Copy, PartialEq)]
enum CalibPhase {
    Ready,
    Listening,
}

/// State of the guided calibration flow. All six vowels are captured (in
/// `vowel::VOWELS` order) so both detection modes can be derived.
struct CalibrationState {
    /// Which vowel we're on (index into `vowel::VOWELS`).
    step: usize,
    phase: CalibPhase,
    capture: CalibrationCapture,
    /// Vowels measured so far, in `vowel::VOWELS` order.
    measured: Vec<(f32, f32)>,
    /// Latest live formant estimate, for on-screen feedback.
    live_formants: Option<(f32, f32)>,
}

/// Whether the profile form is creating a new profile or editing an existing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FormMode {
    Create,
    Edit,
}

/// The avatar decision captured by the profile form.
#[derive(Clone, Debug)]
enum AvatarChoice {
    /// Create: no avatar yet. Edit: keep the profile's existing avatar.
    Keep,
    /// A freshly picked or captured image to import on submit.
    New(PathBuf),
    /// Edit: clear the avatar back to the default face on submit.
    Remove,
}

pub struct App {
    capture: Option<Capture>,
    playback: Option<Playback>,
    samples: Vec<Sample>,
    pads: Vec<Pad>,
    theme: Theme,
    visualizers: Vec<Box<dyn Visualizer>>,
    active_visualizer: usize,
    dev_panel: DevPanel,
    config_panel: ConfigPanel,
    settings: Settings,
    settings_path: PathBuf,
    i18n: I18n,

    screen: AppScreen,
    profiles: Vec<Profile>,
    profile_search: String,
    current_profile: Option<Profile>,
    profile_sessions: Vec<SessionInfo>,
    session: Option<Session>,

    // Profile form state (shared by create + edit).
    form_mode: FormMode,
    form_name: String,
    form_avatar: AvatarChoice,
    form_error: Option<String>,
    camera: Option<crate::camera::CameraSession>,

    // Vowel calibration flow (per profile).
    calib: Option<CalibrationState>,

    // Texture caches.
    tex_cache: HashMap<PathBuf, egui::TextureHandle>,
    flag_cache: HashMap<String, egui::TextureHandle>,

    record_mode: bool,
    recording_active: bool,
    recording_sample_idx: usize,
    accumulated_samples: Vec<f32>,
    playback_monitor: Vec<f32>,
    capture_rate: u32,
    audio_status: String,
    audio_retry_timer: f32,
    frame_count: u64,
    awaiting_shot: Option<PathBuf>,
    auto_shot: Option<(PathBuf, u64)>,
}

/// Install the bundled Space Grotesk (OFL) font as the default proportional and
/// monospace family. egui's default fonts remain as fallback so Cyrillic/Greek
/// (Ukrainian, Russian, …) still render.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "space_grotesk".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/SpaceGrotesk.ttf"
        ))),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "space_grotesk".to_owned());
    }
    ctx.set_fonts(fonts);
}

fn color_image_from_path(path: &Path) -> Option<egui::ColorImage> {
    let img = image::open(path).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

fn color_image_from_bytes(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);

        let (mut settings, settings_path) = Settings::load();
        if let Ok(lang) = std::env::var("RONDELEK_LANG") {
            settings.language = lang;
        }
        let i18n = I18n::new(&settings.language);

        let theme = if settings.dark_mode {
            config::theme_dark()
        } else {
            config::theme_light()
        };

        let capture_rate = 44100;
        let samples = (0..NUM_SAMPLES)
            .map(|_| Sample::new(capture_rate))
            .collect();
        let pads = SAMPLE_PADS
            .iter()
            .chain(std::iter::once(&REC_PAD))
            .map(|def| Pad::from_def(def, egui::Rect::ZERO))
            .collect();

        let mut app = Self {
            capture: None,
            playback: None,
            samples,
            pads,
            theme,
            visualizers: vec![
                Box::new(SpectrumVisualizer::new()),
                Box::new(VowelVisualizer::new()),
            ],
            // Clamp in case a newer config selected a visualizer we no longer have.
            active_visualizer: settings.active_visualizer.min(1),
            dev_panel: DevPanel::new(),
            config_panel: ConfigPanel::new(),
            settings,
            settings_path,
            i18n,
            screen: AppScreen::Profiles,
            profiles: Vec::new(),
            profile_search: String::new(),
            current_profile: None,
            profile_sessions: Vec::new(),
            session: None,
            form_mode: FormMode::Create,
            form_name: String::new(),
            form_avatar: AvatarChoice::Keep,
            form_error: None,
            camera: None,
            calib: None,
            tex_cache: HashMap::new(),
            flag_cache: HashMap::new(),
            record_mode: false,
            recording_active: false,
            recording_sample_idx: 0,
            accumulated_samples: Vec::new(),
            playback_monitor: Vec::new(),
            capture_rate,
            audio_status: String::new(),
            audio_retry_timer: 0.0,
            frame_count: 0,
            awaiting_shot: None,
            auto_shot: std::env::var("RONDELEK_SHOT")
                .ok()
                .map(|p| (PathBuf::from(p), 50)),
        };

        app.refresh_profiles();

        // Screenshot/kiosk harness shortcuts.
        if let Ok(path) = std::env::var("RONDELEK_PROFILE")
            && let Ok(profile) = Profile::load(PathBuf::from(path))
        {
            app.select_profile(profile);
        }
        if std::env::var("RONDELEK_SCREEN").as_deref() == Ok("newprofile") {
            app.begin_new_profile();
        }
        // Edit form for the profile selected via RONDELEK_PROFILE above.
        if std::env::var("RONDELEK_SCREEN").as_deref() == Ok("editprofile") {
            app.begin_edit_profile();
        }
        if std::env::var("RONDELEK_SCREEN").as_deref() == Ok("calibrate") {
            app.begin_calibration();
        }
        if let Ok(path) = std::env::var("RONDELEK_SESSION") {
            app.open_session_dir(PathBuf::from(path));
        }
        if let Ok(v) = std::env::var("RONDELEK_VIZ")
            && let Ok(i) = v.parse::<usize>()
        {
            app.active_visualizer = i.min(app.visualizers.len().saturating_sub(1));
        }

        app
    }

    // ---- audio ----------------------------------------------------------

    /// Per-frame audio watchdog (throttled ~1s). For each direction, work out
    /// the target device (Auto → system default; Pinned → that device, or the
    /// default as a fallback) and rebuild the stream when it is missing, dead,
    /// or pointed at the wrong device. This recovers from disconnects and
    /// system-default changes, and reclaims a pinned device when it returns.
    fn maintain_audio(&mut self, dt: f32) {
        if self.audio_retry_timer > 0.0 {
            self.audio_retry_timer -= dt;
            return;
        }
        self.audio_retry_timer = 1.0;

        let mut errors = Vec::new();

        // ---- output ----
        {
            let pref = device::DevicePref::from_setting(&self.settings.output_device);
            let available = device::list_output_devices();
            let default = device::default_output_name();
            match device::choose_target(&pref, &available, default.as_deref()) {
                Some(target) => {
                    let current = self.playback.as_ref().map(|p| p.current_device());
                    let alive = self
                        .playback
                        .as_ref()
                        .map(|p| p.is_alive())
                        .unwrap_or(false);
                    if device::needs_rebuild(current, alive, &target.name) {
                        match device::output_device_by_name(&target.name) {
                            Some(dev) => match Playback::open(&dev) {
                                Ok(pb) => self.playback = Some(pb),
                                Err(e) => {
                                    self.playback = None;
                                    errors.push(format!("speaker: {e}"));
                                }
                            },
                            None => {
                                self.playback = None;
                                errors.push(format!("speaker: {} unavailable", target.name));
                            }
                        }
                    }
                }
                None => {
                    // Transient enumeration failure: keep a healthy stream
                    // rather than dropping audio for ~1s. Only tear down if
                    // the stream is already dead (or absent).
                    let alive = self
                        .playback
                        .as_ref()
                        .map(|p| p.is_alive())
                        .unwrap_or(false);
                    if !alive {
                        self.playback = None;
                        errors.push("speaker: none".to_string());
                    }
                }
            }
        }

        // ---- input ----
        {
            let pref = device::DevicePref::from_setting(&self.settings.input_device);
            let available = device::list_input_devices();
            let default = device::default_input_name();
            match device::choose_target(&pref, &available, default.as_deref()) {
                Some(target) => {
                    let current = self.capture.as_ref().map(|c| c.current_device());
                    let alive = self.capture.as_ref().map(|c| c.is_alive()).unwrap_or(false);
                    if device::needs_rebuild(current, alive, &target.name) {
                        match device::input_device_by_name(&target.name) {
                            Some(dev) => match Capture::open(&dev) {
                                Ok(cap) => {
                                    self.capture_rate = cap.sample_rate();
                                    self.capture = Some(cap);
                                }
                                Err(e) => {
                                    self.capture = None;
                                    errors.push(format!("microphone: {e}"));
                                }
                            },
                            None => {
                                self.capture = None;
                                errors.push(format!("microphone: {} unavailable", target.name));
                            }
                        }
                    }
                }
                None => {
                    // Transient enumeration failure: keep a healthy stream
                    // rather than dropping audio for ~1s. Only tear down if
                    // the stream is already dead (or absent).
                    let alive = self.capture.as_ref().map(|c| c.is_alive()).unwrap_or(false);
                    if !alive {
                        self.capture = None;
                        errors.push("microphone: none".to_string());
                    }
                }
            }
        }

        self.audio_status = if errors.is_empty() {
            String::new()
        } else {
            format!(
                "{} ({})",
                self.i18n.t("audio.unavailable"),
                errors.join("; ")
            )
        };
    }

    fn toggle_record_mode(&mut self) {
        self.record_mode = !self.record_mode;
        self.recording_active = false;
        for pad in &mut self.pads {
            pad.set_mode(if self.record_mode {
                PadMode::Record
            } else {
                PadMode::Play
            });
            pad.is_recording = false;
        }
    }

    fn start_recording(&mut self, sample_idx: usize) {
        self.samples[sample_idx].clear();
        self.samples[sample_idx].set_sample_rate(self.capture_rate);
        self.recording_sample_idx = sample_idx;
        self.recording_active = true;
        self.pads[sample_idx].has_sample = false;
        self.pads[sample_idx].is_recording = true;
    }

    fn stop_recording(&mut self) {
        if !self.recording_active {
            return;
        }
        let idx = self.recording_sample_idx;
        self.pads[idx].is_recording = false;
        self.recording_active = false;

        if !self.samples[idx].buf.is_empty() {
            self.samples[idx].has_data = true;
            self.pads[idx].has_sample = true;

            if let Some(session) = self.session.as_mut()
                && let Err(e) = session.save_sample(idx, &self.samples[idx])
            {
                self.audio_status = format!("Could not save recording: {e}");
                eprintln!("Failed to save sample {idx}: {e}");
            }
        }
    }

    fn play_sample(&mut self, sample_idx: usize) {
        if !self.samples[sample_idx].has_data {
            return;
        }
        if let Some(ref mut pb) = self.playback {
            let rate = self.samples[sample_idx].sample_rate();
            pb.play(self.samples[sample_idx].buf.clone(), rate);
        }
    }

    /// Pump both audio taps into the active visualizer each frame: the mic
    /// (`accumulated_samples`) and the playback monitor (`playback_monitor`).
    /// Recording still writes the raw mic chunk to the sample buffer.
    fn drain_capture(&mut self) {
        // --- microphone tap ---
        let mic = self
            .capture
            .as_mut()
            .map(Capture::drain)
            .unwrap_or_default();
        if !mic.is_empty() {
            self.accumulated_samples.extend_from_slice(&mic);
            trim_rolling(&mut self.accumulated_samples, self.capture_rate);
            if self.recording_active && self.recording_sample_idx < self.samples.len() {
                self.samples[self.recording_sample_idx]
                    .buf
                    .extend_from_slice(&mic);
            }
        }

        // --- playback monitor tap ---
        let playing = self.playback.as_ref().is_some_and(Playback::is_playing);
        let (monitor, playback_rate) = match self.playback.as_mut() {
            Some(pb) => (pb.drain_monitor(), pb.output_rate()),
            None => (Vec::new(), self.capture_rate),
        };
        if playing {
            self.playback_monitor.extend_from_slice(&monitor);
            trim_rolling(&mut self.playback_monitor, playback_rate);
        } else if !self.playback_monitor.is_empty() {
            // Playback has finished: drop the tap so the last clip's tail can't
            // stay frozen on the display or out-shout a quiet microphone. The
            // visualizer then releases the shape smoothly toward the live mic.
            self.playback_monitor.clear();
        }

        // Refresh only on frames that brought new audio (or a just-cleared tap),
        // so the decay/smoothing cadence follows the sound, not the frame rate.
        if mic.is_empty() && self.playback_monitor.is_empty() {
            return;
        }
        let frame = AudioFrame {
            input: &self.accumulated_samples,
            input_rate: self.capture_rate,
            playback: &self.playback_monitor,
            playback_rate,
        };
        self.visualizers[self.active_visualizer].update(&frame, &self.settings);
    }

    /// Advance to the next visualizer, wrapping around, and remember the choice.
    fn cycle_visualizer(&mut self) {
        if self.visualizers.is_empty() {
            return;
        }
        self.active_visualizer = (self.active_visualizer + 1) % self.visualizers.len();
        self.settings.active_visualizer = self.active_visualizer;
        self.save_settings();
    }

    fn save_settings(&self) {
        self.settings.save(&self.settings_path);
    }

    // ---- textures -------------------------------------------------------

    fn texture_from_path(&mut self, ctx: &egui::Context, path: &Path) -> Option<egui::TextureId> {
        if let Some(t) = self.tex_cache.get(path) {
            return Some(t.id());
        }
        let ci = color_image_from_path(path)?;
        let t = ctx.load_texture(path.to_string_lossy(), ci, egui::TextureOptions::LINEAR);
        let id = t.id();
        self.tex_cache.insert(path.to_path_buf(), t);
        Some(id)
    }

    fn flag_texture(&mut self, ctx: &egui::Context, code: &str) -> Option<egui::TextureId> {
        if let Some(t) = self.flag_cache.get(code) {
            return Some(t.id());
        }
        let bytes = i18n::flag_png(code)?;
        let ci = color_image_from_bytes(bytes)?;
        let t = ctx.load_texture(format!("flag_{code}"), ci, egui::TextureOptions::LINEAR);
        let id = t.id();
        self.flag_cache.insert(code.to_string(), t);
        Some(id)
    }

    // ---- navigation -----------------------------------------------------

    fn refresh_profiles(&mut self) {
        self.profiles = profile::list_profiles();
    }

    fn go_to_profiles(&mut self) {
        self.stop_recording();
        if let Some(session) = self.session.as_ref() {
            let _ = session.save_manifest();
        }
        self.session = None;
        self.current_profile = None;
        self.refresh_profiles();
        self.screen = AppScreen::Profiles;
    }

    fn select_profile(&mut self, profile: Profile) {
        self.profile_sessions = profile.list_sessions();
        self.current_profile = Some(profile);
        self.apply_profile_calibration();
        self.screen = AppScreen::Sessions;
    }

    /// Push the current profile's calibration to the visualizers (or `None` when
    /// the profile isn't calibrated, falling back to the scaled reference set).
    fn apply_profile_calibration(&mut self) {
        let cal = self
            .current_profile
            .as_ref()
            .and_then(|p| p.load_calibration());
        for viz in &mut self.visualizers {
            viz.set_calibration(cal.clone());
        }
    }

    // ---- vowel calibration ---------------------------------------------

    /// Enter the guided calibration flow for the current profile.
    fn begin_calibration(&mut self) {
        if self.current_profile.is_none() {
            return;
        }
        self.calib = Some(CalibrationState {
            step: 0,
            phase: CalibPhase::Ready,
            capture: CalibrationCapture::new(),
            measured: Vec::new(),
            live_formants: None,
        });
        self.accumulated_samples.clear();
        self.screen = AppScreen::Calibrate;
    }

    /// Finish: derive personalised targets from the captured corners, save them
    /// to the profile, and return to the Sessions screen.
    fn finish_calibration(&mut self) {
        if let Some(state) = self.calib.take()
            && state.measured.len() == vowel::VOWELS.len()
            && let Some(profile) = self.current_profile.as_ref()
        {
            let mut measured = [(0.0f32, 0.0f32); 6];
            for (slot, m) in measured.iter_mut().zip(state.measured.iter()) {
                *slot = *m;
            }
            let cal = VowelCalibration {
                practice: practice_targets(&measured),
                measured,
                created: now_secs(),
            };
            if let Err(e) = profile.save_calibration(&cal) {
                eprintln!("Failed to save calibration: {e}");
            }
        }
        self.calib = None;
        self.apply_profile_calibration();
        self.screen = AppScreen::Sessions;
    }

    /// Begin capturing the current vowel: flush stale audio (so the previous
    /// vowel can't bleed in) and switch to the listening phase.
    fn start_listening(&mut self) {
        self.accumulated_samples.clear();
        if let Some(state) = self.calib.as_mut() {
            state.capture.clear();
            state.phase = CalibPhase::Listening;
        }
    }

    /// Pull the mic and, **only while listening**, feed the current corner's
    /// capture. Returns the live formant estimate for display. When a corner is
    /// captured it stops at the next `Ready` phase so the user starts the next
    /// vowel deliberately (rather than auto-jumping mid-sound).
    fn pump_calibration(&mut self) -> Option<(f32, f32)> {
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

        let listening = matches!(
            self.calib.as_ref().map(|s| s.phase),
            Some(CalibPhase::Listening)
        );
        if !listening {
            return None;
        }

        let window =
            &self.accumulated_samples[self.accumulated_samples.len().saturating_sub(2048)..];
        let cfg = VowelConfig {
            voicing_threshold: self.settings.vowel_voicing_threshold,
            prototypes: vowel::default_prototypes(self.settings.vowel_speaker_scale),
        };
        let formants = vowel::analyze(window, self.capture_rate, &cfg).formants;

        let mut finished = false;
        if let Some(state) = self.calib.as_mut() {
            state.live_formants = formants;
            state.capture.push(formants);
            if state.capture.count() >= CALIB_TARGET
                && let Some(measured) = state.capture.result()
            {
                state.measured.push(measured);
                state.capture.clear();
                state.step += 1;
                if state.measured.len() == vowel::VOWELS.len() {
                    finished = true;
                } else {
                    // Wait for the user to start the next vowel.
                    state.phase = CalibPhase::Ready;
                }
            }
        }
        if finished {
            self.finish_calibration();
        }
        formants
    }

    fn draw_calibrate(&mut self, ui: &mut Ui) {
        ui.ctx().request_repaint();
        let live = self.pump_calibration();
        // pump_calibration may have finished and switched screens.
        if self.screen != AppScreen::Calibrate {
            return;
        }
        let (step, count, phase) = match self.calib.as_ref() {
            Some(s) => (
                s.step.min(vowel::VOWELS.len() - 1),
                s.capture.count(),
                s.phase,
            ),
            None => {
                self.screen = AppScreen::Sessions;
                return;
            }
        };

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);
        let target = vowel::VOWELS[step];
        let progress = (count as f32 / CALIB_TARGET as f32).clamp(0.0, 1.0);

        let mut start = false;
        let mut cancel = false;
        let mut retry = false;

        // Space starts the current sound when ready (handy for an adult helper).
        if phase == CalibPhase::Ready && ui.input(|i| i.key_pressed(Key::Space)) {
            start = true;
        }

        ui.vertical_centered(|ui| {
            ui.add_space((full.height() * 0.10).min(64.0));
            ui.label(
                egui::RichText::new(self.i18n.t("calibrate.title"))
                    .color(self.theme.text_primary)
                    .size(24.0)
                    .strong(),
            );
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("{} / {}", step + 1, vowel::VOWELS.len()))
                    .color(self.theme.text_secondary)
                    .size(14.0),
            );
            ui.add_space(20.0);
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
            ui.add_space(16.0);

            match phase {
                CalibPhase::Ready => {
                    let start_btn = egui::Button::new(
                        egui::RichText::new(self.i18n.t("calibrate.start")).color(Color32::WHITE),
                    )
                    .fill(ORANGE);
                    if ui.add_sized([220.0, 44.0], start_btn).clicked() {
                        start = true;
                    }
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(self.i18n.t("calibrate.ready_hint"))
                            .color(self.theme.text_secondary)
                            .size(13.0),
                    );
                }
                CalibPhase::Listening => {
                    ui.add_sized([300.0, 18.0], egui::ProgressBar::new(progress).fill(ORANGE));
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(self.i18n.t("calibrate.hold"))
                            .color(self.theme.text_secondary)
                            .size(13.0),
                    );
                    if let Some((f1, f2)) = live {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(format!("F1 {f1:.0}  F2 {f2:.0} Hz"))
                                .color(self.theme.text_secondary)
                                .monospace(),
                        );
                    }
                }
            }

            ui.add_space(24.0);
            ui.allocate_ui_with_layout(
                egui::vec2(300.0, 40.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if ui
                        .add_sized(
                            [145.0, 40.0],
                            egui::Button::new(self.i18n.t("calibrate.cancel")),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                    if phase == CalibPhase::Listening
                        && ui
                            .add_sized(
                                [145.0, 40.0],
                                egui::Button::new(self.i18n.t("calibrate.retry")),
                            )
                            .clicked()
                    {
                        retry = true;
                    }
                },
            );
        });

        if start {
            self.start_listening();
        } else if cancel {
            self.calib = None;
            self.screen = AppScreen::Sessions;
        } else if retry {
            // Discard this attempt and wait for the user to start it again.
            self.accumulated_samples.clear();
            if let Some(s) = self.calib.as_mut() {
                s.capture.clear();
                s.phase = CalibPhase::Ready;
            }
        }
    }

    fn begin_new_profile(&mut self) {
        self.form_mode = FormMode::Create;
        self.form_name.clear();
        self.form_avatar = AvatarChoice::Keep;
        self.form_error = None;
        self.screen = AppScreen::ProfileForm;
    }

    /// Enter the form pre-filled with the current profile's data for editing.
    fn begin_edit_profile(&mut self) {
        let Some(profile) = self.current_profile.as_ref() else {
            return;
        };
        self.form_mode = FormMode::Edit;
        self.form_name = profile.name().to_string();
        self.form_avatar = AvatarChoice::Keep;
        self.form_error = None;
        self.screen = AppScreen::ProfileForm;
    }

    fn upload_avatar_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        {
            self.form_avatar = AvatarChoice::New(path);
        }
    }

    /// Submit the profile form: create a new profile or apply edits to the
    /// current one, depending on `form_mode`.
    fn submit_profile_form(&mut self) {
        let name = profile::sanitize_name(&self.form_name);
        if name.is_empty() {
            self.form_error = Some(self.i18n.t("form.name_required").to_string());
            return;
        }
        match self.form_mode {
            FormMode::Create => {
                let avatar = match &self.form_avatar {
                    AvatarChoice::New(p) => Some(p.clone()),
                    _ => None,
                };
                match Profile::create(&self.form_name, avatar.as_deref()) {
                    Ok(profile) => {
                        self.refresh_profiles();
                        self.select_profile(profile);
                    }
                    Err(e) => self.form_error = Some(format!("{e}")),
                }
            }
            FormMode::Edit => {
                let Some(mut profile) = self.current_profile.take() else {
                    self.screen = AppScreen::Profiles;
                    return;
                };
                // Evict any cached texture for this avatar path before rewriting
                // it, so the new image (or default face) renders after save.
                if let Some(path) = profile.avatar_path() {
                    self.tex_cache.remove(&path);
                }
                let result =
                    profile
                        .set_name(&self.form_name)
                        .and_then(|()| match &self.form_avatar {
                            AvatarChoice::New(p) => profile.set_avatar(p),
                            AvatarChoice::Remove => profile.clear_avatar(),
                            AvatarChoice::Keep => Ok(()),
                        });
                match result {
                    Ok(()) => {
                        self.refresh_profiles();
                        self.select_profile(profile);
                    }
                    Err(e) => {
                        self.form_error = Some(format!("{e}"));
                        self.current_profile = Some(profile);
                    }
                }
            }
        }
    }

    fn start_new_session(&mut self) {
        let result = self.current_profile.as_ref().map(|p| p.new_session());
        match result {
            Some(Ok(session)) => self.enter_session(session),
            Some(Err(e)) => self.audio_status = format!("Could not create session: {e}"),
            None => {}
        }
    }

    fn open_session_info(&mut self, idx: usize) {
        let Some(dir) = self.profile_sessions.get(idx).map(|s| s.dir.clone()) else {
            return;
        };
        match Session::open(dir) {
            Ok(session) => self.enter_session(session),
            Err(e) => self.audio_status = format!("Could not open session: {e}"),
        }
    }

    /// Open a session directly from a path (screenshot/kiosk harness).
    fn open_session_dir(&mut self, path: PathBuf) {
        match Session::open(path) {
            Ok(session) => self.enter_session(session),
            Err(e) => self.audio_status = format!("Could not open session: {e}"),
        }
    }

    fn enter_session(&mut self, session: Session) {
        self.samples = session.load_samples(self.capture_rate);
        for (idx, sample) in self.samples.iter().enumerate() {
            if let Some(pad) = self.pads.get_mut(idx) {
                pad.has_sample = sample.has_data;
            }
        }
        self.record_mode = false;
        self.recording_active = false;
        for pad in &mut self.pads {
            pad.set_mode(PadMode::Play);
            pad.is_recording = false;
        }
        self.session = Some(session);
        self.screen = AppScreen::Session;
    }

    /// Open the webcam for avatar capture, or report gracefully if unavailable.
    fn open_camera(&mut self) {
        match crate::camera::CameraSession::open() {
            Ok(cam) => {
                self.camera = Some(cam);
                self.form_error = None;
            }
            Err(_) => self.form_error = Some(self.i18n.t("camera.unavailable").to_string()),
        }
    }

    /// Live camera modal shown over the new-profile form. Returns nothing; on
    /// capture it stores a temp PNG as the pending avatar and closes the camera.
    fn camera_modal(&mut self, ui: &mut Ui) {
        if self.camera.is_none() {
            return;
        }
        let ctx = ui.ctx().clone();
        let frame = self.camera.as_mut().and_then(|c| c.grab().ok());
        let tex = frame.as_ref().map(|(rgba, w, h)| {
            let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
            ctx.load_texture("camera_preview", ci, egui::TextureOptions::LINEAR)
        });

        let mut capture = false;
        let mut cancel = false;
        egui::Window::new(self.i18n.t("camera.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(&ctx, |ui| {
                match (&tex, &frame) {
                    (Some(t), Some((_, w, h))) => {
                        // Fit the native frame into the preview box without
                        // stretching, then overlay the centred square crop guide.
                        let (fw, fh) = (*w as f32, *h as f32);
                        let scale = (400.0 / fw).min(300.0 / fh);
                        let disp = egui::vec2(fw * scale, fh * scale);
                        let (rect, _) = ui.allocate_exact_size(disp, Sense::hover());
                        let p = ui.painter();
                        p.image(t.id(), rect, uv_full(), Color32::WHITE);
                        let side = rect.width().min(rect.height());
                        let sq = Rect::from_center_size(rect.center(), egui::vec2(side, side));
                        draw_crop_guide(p, sq);
                    }
                    _ => {
                        ui.label(self.i18n.t("camera.unavailable"));
                    }
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let cap = egui::Button::new(
                        egui::RichText::new(self.i18n.t("camera.capture")).color(Color32::WHITE),
                    )
                    .fill(ORANGE);
                    if ui.add_sized([150.0, 36.0], cap).clicked() {
                        capture = true;
                    }
                    if ui
                        .add_sized(
                            [120.0, 36.0],
                            egui::Button::new(self.i18n.t("camera.cancel")),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                });
            });

        if capture {
            if let Some((rgba, w, h)) = &frame {
                match crate::camera::save_frame_png(rgba, *w, *h) {
                    Ok(path) => self.form_avatar = AvatarChoice::New(path),
                    Err(e) => self.form_error = Some(format!("{e}")),
                }
            }
            self.camera = None;
        } else if cancel {
            self.camera = None;
        }
    }

    fn set_language(&mut self, code: &str) {
        self.settings.language = code.to_string();
        self.i18n.set_lang(code);
        self.save_settings();
    }

    // ---- screenshots ----------------------------------------------------

    fn request_screenshot(&mut self, ctx: &egui::Context, path: PathBuf) {
        self.awaiting_shot = Some(path);
        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
    }

    fn screenshot_path(&self) -> PathBuf {
        let name = format!("rondelek-{}.png", self.frame_count);
        match self.session.as_ref() {
            Some(s) => s.dir.join(name),
            None => PathBuf::from(name),
        }
    }

    fn save_pending_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.awaiting_shot.clone() else {
            return;
        };
        let image = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        if let Some(image) = image {
            self.awaiting_shot = None;
            let [w, h] = image.size;
            let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
            match image::save_buffer(&path, &rgba, w as u32, h as u32, image::ColorType::Rgba8) {
                Ok(()) => self.audio_status = format!("Saved screenshot → {}", path.display()),
                Err(e) => self.audio_status = format!("Screenshot failed: {e}"),
            }
            if self.auto_shot.is_some() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    // ---- screens: profiles / new profile / sessions ---------------------

    fn draw_profiles(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);

        // Language picker, anchored top-right.
        let ctx = ui.ctx().clone();
        egui::Area::new(egui::Id::new("lang_picker"))
            .anchor(Align2::RIGHT_TOP, [-16.0, 16.0])
            .show(&ctx, |ui| {
                self.language_picker(ui);
            });

        // Settings entry (opens the F12 panel), anchored top-left.
        let settings_label = format!("⚙ {}", self.i18n.t("profiles.settings"));
        let mut open_settings = false;
        egui::Area::new(egui::Id::new("settings_entry"))
            .anchor(Align2::LEFT_TOP, [16.0, 16.0])
            .show(&ctx, |ui| {
                if ui.button(settings_label).clicked() {
                    open_settings = true;
                }
            });
        if open_settings {
            self.config_panel.visible = true;
        }

        // Preload avatar textures for the current filter so cards can read them.
        let query = self.profile_search.to_lowercase();
        let visible: Vec<usize> = self
            .profiles
            .iter()
            .enumerate()
            .filter(|(_, p)| query.is_empty() || p.name().to_lowercase().contains(&query))
            .map(|(i, _)| i)
            .collect();
        for &i in &visible {
            if let Some(path) = self.profiles[i].avatar_path() {
                let _ = self.texture_from_path(&ctx, &path);
            }
        }

        let mut clicked: Option<usize> = None;
        let mut new_profile = false;

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((full.height() * 0.08).min(64.0));
                ui.label(
                    egui::RichText::new("RONDELEK · TWST-1")
                        .color(self.theme.text_primary)
                        .size(30.0)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(self.i18n.t("app.tagline"))
                        .color(self.theme.text_secondary)
                        .size(14.0),
                );
                ui.add_space(22.0);

                ui.add(
                    egui::TextEdit::singleline(&mut self.profile_search)
                        .hint_text(self.i18n.t("profiles.search"))
                        .desired_width(300.0),
                );
                ui.add_space(8.0);

                let new_btn = egui::Button::new(
                    egui::RichText::new(self.i18n.t("profiles.new"))
                        .size(16.0)
                        .color(Color32::WHITE),
                )
                .fill(ORANGE)
                .corner_radius(8.0);
                if ui.add_sized([300.0, 44.0], new_btn).clicked() {
                    new_profile = true;
                }

                ui.add_space(18.0);

                if visible.is_empty() {
                    ui.label(
                        egui::RichText::new(self.i18n.t("profiles.none"))
                            .color(self.theme.text_secondary),
                    );
                }

                for &i in &visible {
                    let name = self.profiles[i].name().to_string();
                    let tex = self.profiles[i]
                        .avatar_path()
                        .and_then(|p| self.tex_cache.get(&p).map(|t| t.id()));
                    if self.profile_card(ui, &name, tex) {
                        clicked = Some(i);
                    }
                    ui.add_space(8.0);
                }
            });
        });

        if new_profile {
            self.begin_new_profile();
        } else if let Some(i) = clicked {
            let profile = self.profiles[i].clone();
            self.select_profile(profile);
        }
    }

    /// One clickable profile row (avatar + name). Returns true when clicked.
    fn profile_card(&self, ui: &mut Ui, name: &str, tex: Option<egui::TextureId>) -> bool {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(300.0, 60.0), Sense::click());
        let p = ui.painter();
        let bg = if resp.hovered() {
            self.theme.pad_play_hover
        } else {
            self.theme.pad_play_bg
        };
        p.rect_filled(rect, 10.0, bg);

        let pad = 8.0;
        let av = Rect::from_min_size(
            Pos2::new(rect.left() + pad, rect.top() + pad),
            Vec2::splat(rect.height() - pad * 2.0),
        );
        if let Some(id) = tex {
            p.image(id, av, uv_full(), Color32::WHITE);
            gloss_overlay(p, av, 8.0);
        } else {
            draw_kid_face(p, av, &self.theme);
        }
        p.text(
            Pos2::new(av.right() + 14.0, rect.center().y),
            Align2::LEFT_CENTER,
            name,
            egui::FontId::proportional(18.0),
            self.theme.text_primary,
        );
        resp.clicked()
    }

    fn language_picker(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let current = self.i18n.lang().to_string();
        let mut chosen: Option<String> = None;

        egui::ComboBox::from_id_salt("lang_combo")
            .selected_text(i18n::endonym(&current))
            .width(150.0)
            .show_ui(ui, |ui| {
                for lang in EUROPEAN_LANGS {
                    let flag = self.flag_texture(&ctx, lang.code);
                    ui.horizontal(|ui| {
                        if let Some(id) = flag {
                            ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(
                                id,
                                egui::vec2(22.0, 15.0),
                            )));
                        }
                        if ui
                            .selectable_label(current == lang.code, lang.endonym)
                            .clicked()
                        {
                            chosen = Some(lang.code.to_string());
                        }
                    });
                }
            });

        if let Some(code) = chosen {
            self.set_language(&code);
        }
    }

    fn draw_profile_form(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);
        let ctx = ui.ctx().clone();

        // Resolve which image the preview should show:
        //  - New(p)  → the freshly chosen image
        //  - Remove  → none (default face)
        //  - Keep    → Edit: the profile's existing avatar; Create: none
        let preview_path = match &self.form_avatar {
            AvatarChoice::New(p) => Some(p.clone()),
            AvatarChoice::Remove => None,
            AvatarChoice::Keep => self
                .current_profile
                .as_ref()
                .filter(|_| self.form_mode == FormMode::Edit)
                .and_then(|p| p.avatar_path()),
        };
        // Load the preview texture and remember its pixel size so the preview can
        // centre-crop (cover) instead of stretching a non-square source.
        let avatar_tex = preview_path.as_ref().and_then(|p| {
            self.texture_from_path(&ctx, p)
                .map(|id| (id, self.tex_cache.get(p).map_or([1, 1], |t| t.size())))
        });

        let mut do_upload = false;
        let mut do_camera = false;
        let mut do_remove_photo = false;
        let mut do_submit = false;
        let mut do_cancel = false;

        ui.vertical_centered(|ui| {
            ui.add_space((full.height() * 0.10).min(72.0));
            let title_key = match self.form_mode {
                FormMode::Create => "form.title",
                FormMode::Edit => "form.edit_title",
            };
            ui.label(
                egui::RichText::new(self.i18n.t(title_key))
                    .color(self.theme.text_primary)
                    .size(24.0)
                    .strong(),
            );
            ui.add_space(16.0);

            // Avatar preview.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(140.0, 140.0), Sense::hover());
            let p = ui.painter();
            if let Some((id, size)) = avatar_tex {
                p.rect_filled(rect, 14.0, self.theme.panel_fg);
                p.image(id, rect.shrink(4.0), cover_uv(size), Color32::WHITE);
                gloss_overlay(p, rect.shrink(4.0), 12.0);
            } else {
                p.rect_filled(rect, 14.0, self.theme.pad_play_bg);
                draw_kid_face(p, rect.shrink(10.0), &self.theme);
            }

            ui.add_space(12.0);
            ui.allocate_ui_with_layout(
                egui::vec2(298.0, 36.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let w = 145.0;
                    if ui
                        .add_sized([w, 36.0], egui::Button::new(self.i18n.t("form.upload")))
                        .clicked()
                    {
                        do_upload = true;
                    }
                    if ui
                        .add_sized([w, 36.0], egui::Button::new(self.i18n.t("form.take_photo")))
                        .clicked()
                    {
                        do_camera = true;
                    }
                },
            );

            // Edit mode: allow clearing the photo back to the default face,
            // shown only when there is actually a photo to remove.
            let has_photo = matches!(self.form_avatar, AvatarChoice::New(_))
                || (self.form_mode == FormMode::Edit
                    && matches!(self.form_avatar, AvatarChoice::Keep)
                    && self
                        .current_profile
                        .as_ref()
                        .is_some_and(|p| p.avatar_path().is_some()));
            if self.form_mode == FormMode::Edit && has_photo {
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [298.0, 32.0],
                        egui::Button::new(self.i18n.t("form.remove_photo")),
                    )
                    .clicked()
                {
                    do_remove_photo = true;
                }
            }

            ui.add_space(16.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.form_name)
                    .hint_text(self.i18n.t("form.name_hint"))
                    .desired_width(300.0),
            );

            if let Some(err) = &self.form_error {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(err).color(self.theme.led_full));
            }

            ui.add_space(18.0);
            ui.allocate_ui_with_layout(
                egui::vec2(298.0, 40.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if ui
                        .add_sized([145.0, 40.0], egui::Button::new(self.i18n.t("form.cancel")))
                        .clicked()
                    {
                        do_cancel = true;
                    }
                    let submit_key = match self.form_mode {
                        FormMode::Create => "form.create",
                        FormMode::Edit => "form.save",
                    };
                    let submit = egui::Button::new(
                        egui::RichText::new(self.i18n.t(submit_key)).color(Color32::WHITE),
                    )
                    .fill(ORANGE);
                    if ui.add_sized([145.0, 40.0], submit).clicked() {
                        do_submit = true;
                    }
                },
            );
        });

        // Camera capture modal (over the form) when active.
        self.camera_modal(ui);

        if do_upload {
            self.upload_avatar_dialog();
        }
        if do_camera {
            self.open_camera();
        }
        if do_remove_photo {
            self.form_avatar = AvatarChoice::Remove;
        }
        if do_cancel {
            // Create returns to the profile list; edit returns to the profile's
            // sessions screen (the profile is still selected).
            self.screen = match self.form_mode {
                FormMode::Create => AppScreen::Profiles,
                FormMode::Edit => AppScreen::Sessions,
            };
        }
        if do_submit {
            self.submit_profile_form();
        }
    }

    fn draw_sessions(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);
        let ctx = ui.ctx().clone();

        let avatar_tex = self
            .current_profile
            .as_ref()
            .and_then(|p| p.avatar_path())
            .and_then(|p| self.texture_from_path(&ctx, &p));
        let profile_name = self
            .current_profile
            .as_ref()
            .map(|p| p.name().to_string())
            .unwrap_or_default();

        let mut back = false;
        let mut edit_profile = false;
        let mut calibrate = false;
        let mut new_session = false;
        let mut open_idx: Option<usize> = None;

        // Back button, top-left.
        if ui
            .put(
                Rect::from_min_size(
                    Pos2::new(full.left() + 12.0, full.top() + 12.0),
                    egui::vec2(120.0, 34.0),
                ),
                egui::Button::new(format!("‹ {}", self.i18n.t("sessions.back"))),
            )
            .clicked()
        {
            back = true;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space((full.height() * 0.08).min(56.0));

                // Avatar + name heading.
                let (rect, _) = ui.allocate_exact_size(egui::vec2(96.0, 96.0), Sense::hover());
                let p = ui.painter();
                if let Some(id) = avatar_tex {
                    p.rect_filled(rect, 12.0, self.theme.panel_fg);
                    p.image(id, rect.shrink(3.0), uv_full(), Color32::WHITE);
                    gloss_overlay(p, rect.shrink(3.0), 10.0);
                } else {
                    p.rect_filled(rect, 12.0, self.theme.pad_play_bg);
                    draw_kid_face(p, rect.shrink(8.0), &self.theme);
                }
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(&profile_name)
                        .color(self.theme.text_primary)
                        .size(22.0)
                        .strong(),
                );
                ui.add_space(10.0);
                if ui
                    .add_sized(
                        [200.0, 32.0],
                        egui::Button::new(self.i18n.t("sessions.edit")),
                    )
                    .clicked()
                {
                    edit_profile = true;
                }
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [200.0, 32.0],
                        egui::Button::new(self.i18n.t("sessions.calibrate")),
                    )
                    .clicked()
                {
                    calibrate = true;
                }
                ui.add_space(16.0);

                let new_btn = egui::Button::new(
                    egui::RichText::new(self.i18n.t("sessions.new"))
                        .size(16.0)
                        .color(Color32::WHITE),
                )
                .fill(ORANGE)
                .corner_radius(8.0);
                if ui.add_sized([300.0, 44.0], new_btn).clicked() {
                    new_session = true;
                }
                ui.add_space(16.0);

                if self.profile_sessions.is_empty() {
                    ui.label(
                        egui::RichText::new(self.i18n.t("sessions.none"))
                            .color(self.theme.text_secondary),
                    );
                }
                for (i, info) in self.profile_sessions.iter().enumerate() {
                    let label = info.folder.replace('_', "  ").replace('-', ".");
                    if ui
                        .add_sized([300.0, 36.0], egui::Button::new(label))
                        .clicked()
                    {
                        open_idx = Some(i);
                    }
                    ui.add_space(6.0);
                }
            });
        });

        if back {
            self.go_to_profiles();
        } else if edit_profile {
            self.begin_edit_profile();
        } else if calibrate {
            self.begin_calibration();
        } else if new_session {
            self.start_new_session();
        } else if let Some(i) = open_idx {
            self.open_session_info(i);
        }
    }

    // ---- screen: sampler ------------------------------------------------

    fn draw_session(&mut self, ui: &mut Ui) {
        let dt = ui.input(|i| i.unstable_dt);
        self.maintain_audio(dt);
        self.drain_capture();

        let pointer_pos = ui.input(|i| i.pointer.hover_pos());
        let primary_down = ui.input(|i| i.pointer.primary_down());

        if ui.input(|i| i.key_pressed(Key::Escape)) && self.recording_active {
            self.stop_recording();
        }
        if ui.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::D)) {
            self.dev_panel.toggle();
            self.settings.show_dev_panel = self.dev_panel.visible;
            self.save_settings();
        }
        if ui.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::T)) {
            self.settings.dark_mode = !self.settings.dark_mode;
            self.theme = if self.settings.dark_mode {
                config::theme_dark()
            } else {
                config::theme_light()
            };
            self.save_settings();
        }

        let full_bounds = ui.max_rect();
        let layout = compute_layout(full_bounds);
        let ctx = ui.ctx().clone();

        // Avatar texture for the header (current profile).
        let avatar_path = self.current_profile.as_ref().and_then(|p| p.avatar_path());
        let avatar_id = avatar_path
            .as_ref()
            .and_then(|p| self.texture_from_path(&ctx, p));

        let painter = ui.painter().clone();
        Renderer::draw_case(&painter, &layout, &self.theme);

        let active = self.active_visualizer;
        if self.auto_shot.is_some() {
            self.visualizers[active].demo_fill(self.settings.visualizer_num_bars);
        }
        self.visualizers[active].draw(&painter, layout.screen, &self.theme, &self.settings);

        // Pads (sample pads use the grid; REC uses the header-right rect).
        for (i, rect) in layout.pads.iter().enumerate() {
            self.pads[i].set_rect(*rect);
        }
        self.pads[REC_PAD_IDX].set_rect(layout.rec);

        let pointer = pointer_pos.unwrap_or(Pos2::ZERO);
        for idx in 0..self.pads.len() {
            let kind = self.pads[idx].kind;
            let key = self.pads[idx].key;
            let sample_idx = self.pads[idx].sample_idx;
            let rect = self.pads[idx].rect;

            let key_held = ui.input(|i| i.key_down(key));
            let mouse_over = rect.contains(pointer);
            let allowed = match kind {
                PadKind::Function => true,
                PadKind::Sample => {
                    !(self.record_mode
                        && self.recording_active
                        && sample_idx != self.recording_sample_idx)
                }
            };
            let activated = allowed && (key_held || (mouse_over && primary_down));
            let (just_activated, just_released) = self.pads[idx].update(activated, dt);

            if just_activated {
                match kind {
                    PadKind::Function => {
                        self.stop_recording();
                        self.toggle_record_mode();
                    }
                    PadKind::Sample => {
                        if self.record_mode && !self.recording_active {
                            self.start_recording(sample_idx);
                        } else if !self.record_mode {
                            self.play_sample(sample_idx);
                        }
                    }
                }
            }
            if just_released
                && kind == PadKind::Sample
                && self.recording_active
                && sample_idx == self.recording_sample_idx
            {
                self.stop_recording();
            }
            self.pads[idx].draw(&painter, &self.theme);
        }

        // Header: glossy kid-face "back to profiles" button (left).
        let back_resp = ui.interact(
            layout.back,
            egui::Id::new("back_to_profiles"),
            Sense::click(),
        );
        let cap = draw_keycap(
            &painter,
            layout.back,
            self.theme.pad_function_bg,
            ROUNDING_PAD,
        );
        draw_kid_face(&painter, cap.shrink(cap.width() * 0.12), &self.theme);
        if back_resp.clicked() {
            self.go_to_profiles();
            return;
        }

        // Header: cycle-visualizer button (square, just left of REC).
        let cycle_resp = ui.interact(layout.cycle, egui::Id::new("cycle_viz"), Sense::click());
        let cap = draw_keycap(
            &painter,
            layout.cycle,
            self.theme.pad_function_bg,
            ROUNDING_PAD,
        );
        draw_viz_icon(&painter, cap, &self.theme);
        if cycle_resp.clicked() {
            self.cycle_visualizer();
        }

        // Header: profile avatar, flush to the top edge, under gloss.
        let cap = draw_keycap(&painter, layout.avatar, self.theme.panel_fg, ROUNDING_PAD);
        if let Some(id) = avatar_id {
            painter.image(id, cap, uv_full(), Color32::WHITE);
            gloss_overlay(&painter, cap, ROUNDING_PAD);
        } else {
            draw_kid_face(&painter, cap.shrink(cap.width() * 0.12), &self.theme);
        }

        // Bottom status line: record state, else any audio error.
        let status = if self.record_mode {
            if self.recording_active {
                format!(
                    "{} {}",
                    self.i18n.t("session.recording"),
                    self.recording_sample_idx + 1
                )
            } else {
                self.i18n.t("session.rec_hold").to_string()
            }
        } else {
            self.audio_status.clone()
        };
        if !status.is_empty() {
            let color = if self.record_mode {
                self.theme.pad_record_bg
            } else {
                self.theme.led_full
            };
            painter.text(
                Pos2::new(full_bounds.center().x, full_bounds.bottom() - 8.0),
                Align2::CENTER_BOTTOM,
                status,
                egui::FontId::proportional(13.0),
                color,
            );
        }

        self.dev_panel
            .show(ui.ctx(), &mut self.settings, &mut self.theme);
    }
}

fn uv_full() -> Rect {
    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0))
}

/// Paint a small ascending-bars glyph on the cycle-visualizer keycap.
fn draw_viz_icon(painter: &egui::Painter, rect: Rect, theme: &Theme) {
    let area = rect.shrink(rect.width() * 0.30);
    let n = 4;
    let gap = area.width() * 0.14;
    let bar_w = (area.width() - gap * (n as f32 - 1.0)) / n as f32;
    for i in 0..n {
        let h = area.height() * (0.35 + 0.21 * i as f32);
        let x = area.left() + i as f32 * (bar_w + gap);
        let bar = Rect::from_min_max(
            Pos2::new(x, area.bottom() - h),
            Pos2::new(x + bar_w, area.bottom()),
        );
        painter.rect_filled(bar, egui::CornerRadius::same(1), theme.pad_function_fg);
    }
}

/// Keep a visualizer feed buffer to at most ~3 seconds (capped) so it only ever
/// holds recent audio.
fn trim_rolling(buf: &mut Vec<f32>, rate: u32) {
    let max = (rate as usize * 3).min(16384);
    if buf.len() > max {
        let excess = buf.len() - max;
        buf.drain(0..excess);
    }
}

/// UV rect that samples the centred square of a `size` texture — the equivalent
/// of CSS `object-fit: cover` into a square target. Drawing a non-square image
/// into a square rect with this UV centre-crops instead of stretching, matching
/// what `Profile::set_avatar` stores. Square textures yield the full [0,1] UV.
fn cover_uv(size: [usize; 2]) -> Rect {
    let (w, h) = (size[0] as f32, size[1] as f32);
    if w <= 0.0 || h <= 0.0 {
        return uv_full();
    }
    let side = w.min(h);
    let ux = (w - side) / 2.0 / w;
    let uy = (h - side) / 2.0 / h;
    Rect::from_min_max(Pos2::new(ux, uy), Pos2::new(1.0 - ux, 1.0 - uy))
}

/// Camera crop guide: a faint full-square outline plus four rounded corner
/// brackets, marking the centred region that becomes the avatar (matching the
/// centre-crop applied on save).
fn draw_crop_guide(p: &egui::Painter, sq: Rect) {
    let faint = Color32::from_rgba_unmultiplied(255, 255, 255, 70);
    p.rect_stroke(
        sq,
        egui::CornerRadius::same(8),
        egui::Stroke::new(1.0, faint),
        egui::StrokeKind::Inside,
    );

    let bracket = Color32::WHITE;
    let len = (sq.width() * 0.16).clamp(14.0, 40.0);
    let t = 3.0;
    let stroke = egui::Stroke::new(t, bracket);
    let corners = [
        (sq.left_top(), egui::vec2(1.0, 0.0), egui::vec2(0.0, 1.0)),
        (sq.right_top(), egui::vec2(-1.0, 0.0), egui::vec2(0.0, 1.0)),
        (
            sq.left_bottom(),
            egui::vec2(1.0, 0.0),
            egui::vec2(0.0, -1.0),
        ),
        (
            sq.right_bottom(),
            egui::vec2(-1.0, 0.0),
            egui::vec2(0.0, -1.0),
        ),
    ];
    for (c, dx, dy) in corners {
        p.line_segment([c, c + dx * len], stroke);
        p.line_segment([c, c + dy * len], stroke);
        // Round the joint by capping it with a small filled dot.
        p.circle_filled(c, t * 0.5, bracket);
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        ui.ctx().request_repaint();
        self.frame_count += 1;

        if ui.input(|i| i.key_pressed(Key::F12)) {
            self.config_panel.toggle();
        }

        if ui.input(|i| i.modifiers.ctrl && i.modifiers.shift && i.key_pressed(Key::S)) {
            let path = self.screenshot_path();
            self.request_screenshot(ui.ctx(), path);
        }
        if let Some((path, frame)) = self.auto_shot.clone()
            && self.frame_count == frame
        {
            self.request_screenshot(ui.ctx(), path);
        }

        match self.screen {
            AppScreen::Profiles => self.draw_profiles(ui),
            AppScreen::ProfileForm => self.draw_profile_form(ui),
            AppScreen::Sessions => self.draw_sessions(ui),
            AppScreen::Calibrate => self.draw_calibrate(ui),
            AppScreen::Session => self.draw_session(ui),
        }

        if self
            .config_panel
            .show(ui.ctx(), &mut self.settings, &self.i18n)
        {
            self.save_settings();
        }

        self.save_pending_screenshot(ui.ctx());
    }

    fn on_exit(&mut self) {
        self.save_settings();
        if let Some(session) = self.session.as_ref() {
            let _ = session.save_manifest();
        }
        if let Some(ref mut cap) = self.capture {
            cap.stop();
        }
        if let Some(ref mut pb) = self.playback {
            pb.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cover_uv_square_is_full() {
        let uv = cover_uv([256, 256]);
        assert_eq!(uv.min, Pos2::new(0.0, 0.0));
        assert_eq!(uv.max, Pos2::new(1.0, 1.0));
    }

    #[test]
    fn cover_uv_landscape_crops_horizontally() {
        // 600x200: keep the centred 200-wide square → u in [1/3, 2/3], full v.
        let uv = cover_uv([600, 200]);
        assert!((uv.min.x - 1.0 / 3.0).abs() < 1e-6);
        assert!((uv.max.x - 2.0 / 3.0).abs() < 1e-6);
        assert_eq!(uv.min.y, 0.0);
        assert_eq!(uv.max.y, 1.0);
    }

    #[test]
    fn cover_uv_portrait_crops_vertically() {
        let uv = cover_uv([200, 600]);
        assert_eq!(uv.min.x, 0.0);
        assert_eq!(uv.max.x, 1.0);
        assert!((uv.min.y - 1.0 / 3.0).abs() < 1e-6);
        assert!((uv.max.y - 2.0 / 3.0).abs() < 1e-6);
    }

    #[test]
    fn cover_uv_degenerate_is_full() {
        assert_eq!(cover_uv([0, 0]), uv_full());
    }
}
