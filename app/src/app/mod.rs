use egui::{Align2, Color32, Key, Pos2, Rect, Sense, Ui, Vec2};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::i18n::{self, EUROPEAN_LANGS, I18n};
use crate::ui::{
    self, AudioFrame, ConfigPanel, OffVisualizer, Pad, PadMode, Renderer, Skin, SpectrumVisualizer,
    Visualizer, VowelVisualizer, compute_layout, draw_kid_face, gloss_overlay,
};
use rondelek_core::audio::vowel::{self, CalibrationCapture};
use rondelek_core::audio::{Capture, Playback, Sample, device};
use rondelek_core::config::atlas;
use rondelek_core::config::{
    NUM_SAMPLES, PadKind, REC_PAD, ROUNDING_PAD, SAMPLE_PADS, Settings, Theme,
};
use rondelek_core::profile::{self, Profile, SessionInfo};
use rondelek_core::session::Session;
use rondelek_core::util::now_secs;

mod calibrate;
mod games;
mod profile_form;
mod profiles;
mod sampler;
mod sessions;

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
    /// Voice mini-games menu (each game runs as its own child process).
    Games,
}

/// Voiced MFCC frames that count a vowel as "well recorded" (drives the progress
/// bar). At ~60 fps this is roughly a second of held sound. The child may hold
/// longer; only [`vowel::MIN_CAPTURE_SAMPLES`] are strictly required.
const CALIB_TARGET: usize = 45;

/// State of the calibration flow. The child records each of the six vowels
/// (`vowel::VOWELS` order) by **holding** a record button — no auto-advance — and
/// can re-record any vowel. Overview screen when `selected` is `None`; the
/// per-vowel record screen when it is `Some(i)`.
struct CalibrationState {
    /// Per-vowel captured MFCC frames, in `vowel::VOWELS` order. Empty = not
    /// yet recorded. Each new take APPENDS its frames — several takes (ideally
    /// at different pitch/loudness) give the templates honest variance, which
    /// is what separates close vowel pairs.
    captured: Vec<Vec<Vec<f32>>>,
    /// Completed takes per vowel (display only).
    takes: Vec<u32>,
    /// Per-vowel window RMS values, parallel to `captured` (adaptive gate).
    captured_rms: Vec<Vec<f32>>,
    /// The vowel currently open for recording; `None` = overview grid.
    selected: Option<usize>,
    /// True while the user holds the record button (push-to-talk).
    recording: bool,
    /// MFCC frames for the in-progress recording of `selected`.
    current: CalibrationCapture,
    /// Latest input level (peak, 0..=1), for the on-screen meter.
    live_level: f32,
    /// Whether the current window reads as voiced, for feedback.
    live_voiced: bool,
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
    skin: Skin,
    /// Installed skin folder names, refreshed when the config panel opens.
    available_skins: Vec<String>,
    visualizers: Vec<Box<dyn Visualizer>>,
    active_visualizer: usize,
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
    /// Running voice-game child process, if any (reaped each frame).
    game_child: Option<std::process::Child>,
    /// Dev runs only (`cargo run`): the `cargo build -p rondelek-game` started
    /// before a game launch, plus the id of the game to spawn once it succeeds.
    game_build: Option<(std::process::Child, String)>,
    /// Last game build/launch problem, shown on the Games screen.
    game_error: Option<String>,
    /// The current profile's loaded calibration (cached; drives the config
    /// panel's Calibration tab without re-reading disk each frame).
    current_calibration: Option<vowel::VowelCalibration>,

    // Texture caches.
    tex_cache: HashMap<PathBuf, egui::TextureHandle>,
    flag_cache: HashMap<String, egui::TextureHandle>,

    record_mode: bool,
    recording_active: bool,
    recording_sample_idx: usize,
    accumulated_samples: Vec<f32>,
    playback_monitor: Vec<f32>,
    /// Latest input peak (0..=1) for the config/calibration level meter.
    input_peak: f32,
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
            "../../../assets/fonts/SpaceGrotesk.ttf"
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

pub(crate) fn color_image_from_bytes(bytes: &[u8]) -> Option<egui::ColorImage> {
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

        // Discover first: it extracts any freshly dropped skin zips, which the
        // selected skin may be about to load from.
        let available_skins = ui::skin::discover();
        let skin_dir = settings
            .skin
            .as_ref()
            .map(|name| ui::skin::skins_dir().join(name));
        let skin = Skin::load(&cc.egui_ctx, skin_dir.as_deref());
        let theme = skin.theme.clone();

        let capture_rate = 44100;
        let samples = (0..NUM_SAMPLES)
            .map(|_| Sample::new(capture_rate))
            .collect();
        let pads = SAMPLE_PADS
            .iter()
            .chain(std::iter::once(&REC_PAD))
            .map(|def| Pad::from_def(def, egui::Rect::ZERO))
            .collect();

        let visualizers: Vec<Box<dyn Visualizer>> = vec![
            Box::new(SpectrumVisualizer::new()),
            Box::new(VowelVisualizer::new()),
            Box::new(OffVisualizer),
        ];
        // Clamp in case a newer config selected a visualizer we no longer have.
        let active_visualizer = settings.active_visualizer.min(visualizers.len() - 1);

        let mut app = Self {
            capture: None,
            playback: None,
            samples,
            pads,
            theme,
            skin,
            available_skins,
            visualizers,
            active_visualizer,
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
            game_child: None,
            game_build: None,
            game_error: None,
            current_calibration: None,
            tex_cache: HashMap::new(),
            flag_cache: HashMap::new(),
            record_mode: false,
            recording_active: false,
            recording_sample_idx: 0,
            accumulated_samples: Vec::new(),
            playback_monitor: Vec::new(),
            input_peak: 0.0,
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
        if std::env::var("RONDELEK_SCREEN").as_deref() == Ok("games") {
            app.screen = AppScreen::Games;
        }
        if std::env::var("RONDELEK_SCREEN").as_deref() == Ok("calibrate") {
            app.begin_calibration();
            // Harness: jump straight into a vowel's record screen.
            if let Ok(v) = std::env::var("RONDELEK_CALIB_VOWEL")
                && let Ok(i) = v.parse::<usize>()
                && let Some(s) = app.calib.as_mut()
            {
                s.selected = Some(i.min(vowel::VOWELS.len() - 1));
            }
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
                        match device::resolve_output(&pref, &available) {
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
                        match device::resolve_input(&pref, &available) {
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
            self.input_peak = mic.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
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
    /// the profile isn't calibrated; detection then stays idle, no fallback).
    fn apply_profile_calibration(&mut self) {
        let cal = self
            .current_profile
            .as_ref()
            .and_then(|p| p.load_calibration());
        for viz in &mut self.visualizers {
            viz.set_calibration(cal.clone());
        }
        self.current_calibration = cal;
    }

    fn set_language(&mut self, code: &str) {
        self.settings.language = code.to_string();
        self.i18n.set_lang(code);
        self.save_settings();
    }

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
}

/// When running under `cargo run` (Cargo sets `CARGO` and `CARGO_MANIFEST_DIR`
/// for the process it runs), start `cargo build -p rondelek-game` with this
/// binary's profile. `None` outside cargo, e.g. a shipped build.
fn dev_game_build() -> Option<std::io::Result<std::process::Child>> {
    let cargo = std::env::var_os("CARGO")?;
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")?;
    let mut cmd = std::process::Command::new(cargo);
    cmd.current_dir(manifest_dir)
        .args(["build", "-p", "rondelek-game"]);
    if !cfg!(debug_assertions) {
        cmd.arg("--release");
    }
    Some(cmd.spawn())
}

fn uv_full() -> Rect {
    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0))
}

/// Draw a skinned header key cap, sinking it while held.
fn draw_cap_button(
    skin: &Skin,
    painter: &egui::Painter,
    cap_idx: usize,
    rect: Rect,
    resp: &egui::Response,
) {
    let pressed = resp.is_pointer_button_down_on();
    skin.cap(painter, cap_idx, rect, pressed, Color32::WHITE);
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
        // Repaint policy: animated screens tick at ~30 FPS — plenty for
        // meters and the recording pulse, and half the tessellation work of
        // 60 (this is not a game). Static screens idle at a slow heartbeat.
        // Input events wake egui immediately either way.
        self.frame_count += 1;

        // Reap a finished game process so the Games screen unlocks.
        if let Some(child) = &mut self.game_child
            && matches!(child.try_wait(), Ok(Some(_)) | Err(_))
        {
            self.game_child = None;
            // The game owns `game_reaction` and saves it itself. Adopt its value
            // so our next save doesn't write back the stale in-memory copy.
            self.settings.game_reaction = Settings::load_from(&self.settings_path).game_reaction;
        }

        self.poll_game_build();

        // Repaint policy. While a game child owns the fullscreen window this
        // window is fully occluded; on Wayland an occluded window gets no
        // frame callbacks, so *any* pending repaint makes winit busy-wait at
        // ~100% on one core (the same spin docs/PERF.md chased). Request
        // nothing then — the focus/occlusion event winit delivers when the
        // game window closes wakes us to reap the child and resume painting.
        // ponytail: reaping now relies on the compositor refocusing us when the
        // game closes; add a timer-based reap only if a compositor is found
        // that doesn't (no repaint while hidden is the whole point — see PERF.md).
        if self.game_child.is_none() {
            let animating = matches!(self.screen, AppScreen::Session | AppScreen::Calibrate)
                || self.config_panel.visible
                || self.camera.is_some()
                || self.auto_shot.is_some();
            let delay = if animating { 33 } else { 100 };
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(delay));
        }

        if ui.input(|i| i.key_pressed(Key::F12)) {
            self.config_panel.toggle();
            if self.config_panel.visible {
                // Rescan on open so freshly dropped skin zips show up.
                self.available_skins = ui::skin::discover();
            }
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
            AppScreen::Games => self.draw_games(ui),
        }

        if self.config_panel.visible {
            // Keep the level meter live even on screens that don't drain the mic.
            if !matches!(self.screen, AppScreen::Session | AppScreen::Calibrate) {
                let mic = self
                    .capture
                    .as_mut()
                    .map(Capture::drain)
                    .unwrap_or_default();
                if !mic.is_empty() {
                    self.input_peak = mic.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
                }
            }
            let cal_info = crate::ui::CalInfo {
                calibrated: self.current_calibration.is_some(),
                created: self.current_calibration.as_ref().map_or(0, |c| c.created),
                mic: self
                    .current_calibration
                    .as_ref()
                    .and_then(|c| c.input_device.clone()),
                current_device: self
                    .capture
                    .as_ref()
                    .map(|c| c.current_device().to_string()),
            };
            let outcome = self.config_panel.show(
                ui.ctx(),
                &mut self.settings,
                &self.available_skins,
                &self.i18n,
                self.input_peak,
                &cal_info,
            );
            if outcome.changed {
                self.save_settings();
            }
            if let Some(code) = outcome.chosen_language {
                self.set_language(&code);
                self.save_settings();
            }
            if let Some(choice) = outcome.chosen_skin {
                self.settings.skin = choice;
                let dir = self
                    .settings
                    .skin
                    .as_ref()
                    .map(|name| ui::skin::skins_dir().join(name));
                self.skin = Skin::load(ui.ctx(), dir.as_deref());
                self.theme = self.skin.theme.clone();
                self.save_settings();
            }
            if outcome.recalibrate {
                self.config_panel.visible = false;
                self.begin_calibration();
            }
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
