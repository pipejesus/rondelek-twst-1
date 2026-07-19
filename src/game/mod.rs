//! Voice-controlled mini-games, rendered by raylib in their own
//! borderless-fullscreen window. The sampler app launches
//! `rondelek --game <id> --profile <dir>` as a child process (see main.rs);
//! this module owns everything past that point.
//!
//! To add a game: write a `VoiceGame` impl in a new file, register it in
//! [`GAMES`] and in the `match` inside [`run`], and add its i18n name key.

pub mod runner;
pub mod voice;

use raylib::prelude::*;
use std::path::PathBuf;

use crate::audio::vowel::VOWELS;
use crate::config::Settings;
use crate::profile::Profile;
use voice::VoiceBridge;

/// One frame of voice input, produced by [`VoiceBridge`].
pub struct VoiceInput {
    /// Stable currently-voiced vowel (index into [`VOWELS`]), gate applied.
    pub held: Option<usize>,
    /// Rising edge this frame (debounced) — for jump-like one-shot triggers.
    pub onset: Option<usize>,
    /// Smoothed per-vowel match scores 0..=1, for in-game feedback meters.
    pub scores: [f32; 6],
    /// Mic peak level 0..=1.
    pub level: f32,
}

/// A voice-driven mini-game. Implementations get voice input + dt each frame
/// and draw with plain raylib; the runner in [`run`] owns the window loop.
pub trait VoiceGame {
    fn update(&mut self, input: &VoiceInput, dt: f32);
    fn draw(&mut self, d: &mut RaylibDrawHandle, w: i32, h: i32);
}

/// (id, i18n name key) of every available game, in menu order.
pub const GAMES: &[(&str, &str)] = &[("runner", "games.runner.name")];

/// Run game `id` until its window closes (Esc). Loads the profile's vowel
/// calibration when a profile dir is given; without one the detector falls
/// back to the scaled reference set.
pub fn run(id: &str, profile_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let (settings, _) = Settings::load();
    let calibration = profile_dir
        .and_then(|dir| Profile::load(dir).ok())
        .and_then(|p| p.load_calibration());

    let mut game: Box<dyn VoiceGame> = match id {
        "runner" => Box::new(runner::Runner::new()),
        _ => anyhow::bail!("unknown game: {id}"),
    };

    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title("Rondelek")
        .resizable()
        .build();
    rl.toggle_borderless_windowed();
    rl.set_target_fps(60);

    let mut bridge = VoiceBridge::new(&settings, calibration);

    // Non-interactive smoke harness, mirroring the app's RONDELEK_SHOT:
    // auto-quit after N frames, optionally saving a screenshot near the end.
    let max_frames: Option<u64> = std::env::var("RONDELEK_GAME_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok());
    let shot: Option<String> = std::env::var("RONDELEK_GAME_SHOT").ok();

    let mut frame: u64 = 0;
    while !rl.window_should_close() {
        let dt = rl.get_frame_time().min(0.1);

        // Keyboard fallback: A/E/I/O/U/Y act as held vowels (testing, parents,
        // and playing without a mic).
        let keys = [
            KeyboardKey::KEY_A,
            KeyboardKey::KEY_E,
            KeyboardKey::KEY_I,
            KeyboardKey::KEY_O,
            KeyboardKey::KEY_U,
            KeyboardKey::KEY_Y,
        ];
        let kb_held = (0..VOWELS.len()).find(|&i| rl.is_key_down(keys[i]));

        let input = bridge.poll(dt, kb_held);
        game.update(&input, dt);

        let (w, h) = (rl.get_screen_width(), rl.get_screen_height());
        {
            let mut d = rl.begin_drawing(&thread);
            game.draw(&mut d, w, h);
        }

        frame += 1;
        if let Some(path) = &shot
            && Some(frame) == max_frames.map(|m| m.saturating_sub(1))
        {
            // raylib prepends the working directory to the filename, so chdir
            // to the target's parent and pass the bare file name.
            let p = std::path::Path::new(path);
            if let Some(parent) = p.parent().filter(|d| !d.as_os_str().is_empty()) {
                let _ = std::env::set_current_dir(parent);
            }
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            rl.take_screenshot(&thread, &name);
        }
        if max_frames.is_some_and(|m| frame >= m) {
            break;
        }
    }
    Ok(())
}
