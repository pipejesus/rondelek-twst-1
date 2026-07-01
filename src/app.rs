use egui::{Align2, Color32, Key, Pos2, Rect, Sense, Ui, Vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::audio::{Capture, Playback, Sample};
use crate::config::{
    self, NUM_SAMPLES, PadKind, REC_PAD, ROUNDING_PAD, SAMPLE_PADS, Settings, Theme,
};
use crate::i18n::{self, EUROPEAN_LANGS, I18n};
use crate::profile::{self, Profile, SessionInfo};
use crate::session::Session;
use crate::ui::{
    DevPanel, Pad, PadMode, Renderer, Visualizer, compute_layout, draw_keycap, draw_kid_face,
    gloss_overlay,
};

/// Index of the REC control pad within `self.pads` (after the sample pads).
const REC_PAD_IDX: usize = NUM_SAMPLES;

/// The signature orange, used for primary actions.
const ORANGE: Color32 = Color32::from_rgb(0xFF, 0x6A, 0x1A);

/// Top-level screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppScreen {
    /// Pick or create a child profile (+ language picker).
    Profiles,
    /// Create a new profile (name + avatar).
    NewProfile,
    /// A profile's sessions: resume a past one or start fresh.
    Sessions,
    /// The sampler.
    Session,
}

pub struct App {
    capture: Option<Capture>,
    playback: Option<Playback>,
    samples: Vec<Sample>,
    pads: Vec<Pad>,
    theme: Theme,
    visualizer: Visualizer,
    dev_panel: DevPanel,
    settings: Settings,
    settings_path: PathBuf,
    i18n: I18n,

    screen: AppScreen,
    profiles: Vec<Profile>,
    profile_search: String,
    current_profile: Option<Profile>,
    profile_sessions: Vec<SessionInfo>,
    session: Option<Session>,

    // New-profile form state.
    form_name: String,
    form_avatar_src: Option<PathBuf>,
    form_error: Option<String>,
    camera: Option<crate::camera::CameraSession>,

    // Texture caches.
    tex_cache: HashMap<PathBuf, egui::TextureHandle>,
    flag_cache: HashMap<String, egui::TextureHandle>,

    record_mode: bool,
    recording_active: bool,
    recording_sample_idx: usize,
    accumulated_samples: Vec<f32>,
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
            visualizer: Visualizer::new(capture_rate),
            dev_panel: DevPanel::new(),
            settings,
            settings_path,
            i18n,
            screen: AppScreen::Profiles,
            profiles: Vec::new(),
            profile_search: String::new(),
            current_profile: None,
            profile_sessions: Vec::new(),
            session: None,
            form_name: String::new(),
            form_avatar_src: None,
            form_error: None,
            camera: None,
            tex_cache: HashMap::new(),
            flag_cache: HashMap::new(),
            record_mode: false,
            recording_active: false,
            recording_sample_idx: 0,
            accumulated_samples: Vec::new(),
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
        if let Ok(path) = std::env::var("RONDELEK_SESSION") {
            app.open_session_dir(PathBuf::from(path));
        }

        app
    }

    // ---- audio ----------------------------------------------------------

    fn init_audio(&mut self, dt: f32) {
        if self.capture.is_some() && self.playback.is_some() {
            return;
        }
        if self.audio_retry_timer > 0.0 {
            self.audio_retry_timer -= dt;
            return;
        }
        self.audio_retry_timer = 2.0;

        let mut errors = Vec::new();
        if self.capture.is_none() {
            match Capture::new() {
                Ok(cap) => {
                    self.capture_rate = cap.sample_rate();
                    if self.visualizer.sample_rate() != cap.sample_rate() {
                        self.visualizer = Visualizer::new(cap.sample_rate());
                    }
                    self.capture = Some(cap);
                }
                Err(e) => errors.push(format!("microphone: {e}")),
            }
        }
        if self.playback.is_none() {
            match Playback::new() {
                Ok(pb) => self.playback = Some(pb),
                Err(e) => errors.push(format!("speaker: {e}")),
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

    fn drain_capture(&mut self) {
        let chunk = match self.capture {
            Some(ref mut cap) => cap.drain(),
            None => return,
        };
        if chunk.is_empty() {
            return;
        }
        self.accumulated_samples.extend_from_slice(&chunk);
        let max_buf = (self.visualizer.sample_rate() as usize * 3).min(16384);
        if self.accumulated_samples.len() > max_buf {
            let excess = self.accumulated_samples.len() - max_buf;
            self.accumulated_samples.drain(0..excess);
        }
        if self.recording_active && self.recording_sample_idx < self.samples.len() {
            self.samples[self.recording_sample_idx]
                .buf
                .extend_from_slice(&chunk);
        }
        self.visualizer
            .process_samples(&self.accumulated_samples, &self.settings);
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
        self.screen = AppScreen::Sessions;
    }

    fn begin_new_profile(&mut self) {
        self.form_name.clear();
        self.form_avatar_src = None;
        self.form_error = None;
        self.screen = AppScreen::NewProfile;
    }

    fn upload_avatar_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        {
            self.form_avatar_src = Some(path);
        }
    }

    fn create_profile_from_form(&mut self) {
        let name = profile::sanitize_name(&self.form_name);
        if name.is_empty() {
            self.form_error = Some(self.i18n.t("form.name_required").to_string());
            return;
        }
        let avatar = self.form_avatar_src.clone();
        match Profile::create(&self.form_name, avatar.as_deref()) {
            Ok(profile) => {
                self.refresh_profiles();
                self.select_profile(profile);
            }
            Err(e) => self.form_error = Some(format!("{e}")),
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
                if let Some(t) = &tex {
                    ui.add(egui::Image::from_texture(egui::load::SizedTexture::new(
                        t.id(),
                        egui::vec2(320.0, 240.0),
                    )));
                } else {
                    ui.label(self.i18n.t("camera.unavailable"));
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
                    Ok(path) => self.form_avatar_src = Some(path),
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

    fn draw_new_profile(&mut self, ui: &mut Ui) {
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.theme.panel_bg);
        let ctx = ui.ctx().clone();

        let avatar_tex = self
            .form_avatar_src
            .clone()
            .and_then(|p| self.texture_from_path(&ctx, &p));

        let mut do_upload = false;
        let mut do_camera = false;
        let mut do_create = false;
        let mut do_cancel = false;

        ui.vertical_centered(|ui| {
            ui.add_space((full.height() * 0.10).min(72.0));
            ui.label(
                egui::RichText::new(self.i18n.t("form.title"))
                    .color(self.theme.text_primary)
                    .size(24.0)
                    .strong(),
            );
            ui.add_space(16.0);

            // Avatar preview.
            let (rect, _) = ui.allocate_exact_size(egui::vec2(140.0, 140.0), Sense::hover());
            let p = ui.painter();
            if let Some(id) = avatar_tex {
                p.rect_filled(rect, 14.0, self.theme.panel_fg);
                p.image(id, rect.shrink(4.0), uv_full(), Color32::WHITE);
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
                    let create = egui::Button::new(
                        egui::RichText::new(self.i18n.t("form.create")).color(Color32::WHITE),
                    )
                    .fill(ORANGE);
                    if ui.add_sized([145.0, 40.0], create).clicked() {
                        do_create = true;
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
        if do_cancel {
            self.screen = AppScreen::Profiles;
        }
        if do_create {
            self.create_profile_from_form();
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
        } else if new_session {
            self.start_new_session();
        } else if let Some(i) = open_idx {
            self.open_session_info(i);
        }
    }

    // ---- screen: sampler ------------------------------------------------

    fn draw_session(&mut self, ui: &mut Ui) {
        let dt = ui.input(|i| i.unstable_dt);
        self.init_audio(dt);
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

        if self.auto_shot.is_some() {
            self.visualizer.demo_fill(self.settings.visualizer_num_bars);
        }
        self.visualizer
            .draw(&painter, layout.screen, &self.theme, &self.settings);

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

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        ui.ctx().request_repaint();
        self.frame_count += 1;

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
            AppScreen::NewProfile => self.draw_new_profile(ui),
            AppScreen::Sessions => self.draw_sessions(ui),
            AppScreen::Session => self.draw_session(ui),
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
