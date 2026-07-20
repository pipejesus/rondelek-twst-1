//! Voice-controlled mini-games, rendered by raylib in their own
//! borderless-fullscreen window. The sampler app spawns the sibling
//! `rondelek-game <id> [--profile <dir>]` binary as a child process (see
//! `app/src/app.rs::launch_game`); this crate owns everything past that
//! point.
//!
//! This is a separate crate (not a module of the `rondelek` app) on purpose:
//! raylib and the `windows` crate (pulled in by eframe/winit/accesskit on the
//! app side) both define a symbol named `ShowCursor`, which is a hard MSVC
//! linker error — `LNK2005` / `LNK1169` — the moment both end up in the same
//! Windows binary. Keeping raylib's dependents in their own executable is
//! what keeps that from happening.
//!
//! Every game starts on a control-selection screen where the therapist picks
//! which vowel triggers which move (two big slots: jump ▲ and duck ▼). The
//! detector is then *focused* on just that pair, so close sound-alikes among
//! the unchosen vowels can't cause misfires. The screen is deliberately
//! text-free (letters, arrows, a play button) — no font or locale issues.
//!
//! To add a game: write a `VoiceGame` impl in a new file, register it in the
//! `match` inside [`run`], and add its id + i18n name key to
//! `rondelek_core::games::GAMES`.

pub mod runner;
pub mod voice;

use raylib::prelude::*;
use std::path::PathBuf;

use rondelek_core::audio::vowel::VOWELS;
use rondelek_core::config::Settings;
use rondelek_core::profile::Profile;
use voice::VoiceBridge;

// Base Pastel palette, shared by the selection screen and the games.
pub(crate) const CREAM: Color = Color::new(251, 242, 228, 255);
pub(crate) const PEACH: Color = Color::new(246, 220, 198, 255);
pub(crate) const MINT: Color = Color::new(198, 229, 211, 255);
pub(crate) const MINT_DARK: Color = Color::new(154, 197, 172, 255);
pub(crate) const ROSE: Color = Color::new(245, 169, 188, 255);
pub(crate) const LILAC: Color = Color::new(201, 184, 232, 255);
pub(crate) const SKY: Color = Color::new(169, 212, 239, 255);
pub(crate) const BUTTER: Color = Color::new(245, 226, 158, 255);
pub(crate) const CHARCOAL: Color = Color::new(74, 68, 60, 255);

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
    /// Load GPU resources (shaders, models). Called once, after the window
    /// exists; unit tests skip it, so games must tolerate running without it.
    fn init(&mut self, _rl: &mut RaylibHandle, _thread: &RaylibThread) {}
    fn update(&mut self, input: &VoiceInput, dt: f32);
    fn draw(&mut self, d: &mut RaylibDrawHandle, w: i32, h: i32);
}


/// The keyboard fallback: A/E/I/O/U/Y act as held vowels.
fn keyboard_vowel(rl: &RaylibHandle) -> Option<usize> {
    const KEYS: [KeyboardKey; 6] = [
        KeyboardKey::KEY_A,
        KeyboardKey::KEY_E,
        KeyboardKey::KEY_I,
        KeyboardKey::KEY_O,
        KeyboardKey::KEY_U,
        KeyboardKey::KEY_Y,
    ];
    (0..VOWELS.len()).find(|&i| rl.is_key_down(KEYS[i]))
}

/// raylib saves screenshots relative to the directory it was *initialized*
/// in, so shoot to a bare name there and move the file to the requested
/// destination (copy + remove — rename can't cross filesystems into /tmp).
fn snap(rl: &mut RaylibHandle, thread: &RaylibThread, path: &str) {
    const NAME: &str = "rondelek_shot.png";
    rl.take_screenshot(thread, NAME);
    if path != NAME && std::fs::copy(NAME, path).is_ok() {
        let _ = std::fs::remove_file(NAME);
    }
}

/// Run game `id` until its window closes (Esc). Loads the profile's vowel
/// calibration when a profile dir is given; without one the detector falls
/// back to the scaled reference set.
pub fn run(id: &str, profile_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let (settings, _) = Settings::load();
    let calibration = profile_dir
        .and_then(|dir| Profile::load(dir).ok())
        .and_then(|p| p.load_calibration());

    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title("Rondelek")
        .resizable()
        .build();
    rl.toggle_borderless_windowed();
    rl.set_target_fps(60);

    // Non-interactive smoke harness, mirroring the app's RONDELEK_SHOT:
    // auto-quit after N frames, optionally saving a screenshot near the end.
    // RONDELEK_GAME_SCREEN=select runs the harness on the selection screen;
    // otherwise the harness skips selection with the default a/e pair.
    let max_frames: Option<u64> = std::env::var("RONDELEK_GAME_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok());
    let shot: Option<String> = std::env::var("RONDELEK_GAME_SHOT").ok();
    let harness_select = std::env::var("RONDELEK_GAME_SCREEN").as_deref() == Ok("select");

    // Phase 1: the therapist picks which vowel drives which move, plus the
    // Reaction slider (turtle = steady, rabbit = snappy).
    let picked = if max_frames.is_some() && !harness_select {
        Some((0, 1, settings.game_reaction)) // harness default: a jumps, e ducks
    } else {
        select_controls(
            &mut rl,
            &thread,
            settings.game_reaction,
            if harness_select { max_frames } else { None },
            shot.as_deref(),
        )
    };
    let Some((jump_vowel, duck_vowel, reaction)) = picked else {
        return Ok(()); // window closed on the selection screen
    };
    if harness_select {
        return Ok(());
    }
    if (reaction - settings.game_reaction).abs() > 0.001 {
        // Remember the therapist's choice for next time.
        let (mut fresh, path) = Settings::load();
        fresh.game_reaction = reaction;
        fresh.save(&path);
    }

    let mut bridge = VoiceBridge::new(&settings, calibration);
    bridge.set_focus([jump_vowel, duck_vowel]);
    bridge.set_reaction(reaction);

    let mut game: Box<dyn VoiceGame> = match id {
        "runner" => Box::new(runner::Runner::new(jump_vowel, duck_vowel)),
        _ => anyhow::bail!("unknown game: {id}"),
    };
    game.init(&mut rl, &thread);

    // Phase 2: play.
    let mut frame: u64 = 0;
    while !rl.window_should_close() {
        let dt = rl.get_frame_time().min(0.1);
        let kb_held = keyboard_vowel(&rl);
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
            snap(&mut rl, &thread, path);
        }
        if max_frames.is_some_and(|m| frame >= m) {
            break;
        }
    }
    Ok(())
}

// ---- control selection screen --------------------------------------------

/// Let the therapist assign a vowel to each of the two moves and set the
/// Reaction slider. Returns `(jump, duck, reaction)`, or `None` if the window
/// was closed. Text-free: two slot cards (▲ jump / ▼ duck), a row of vowel
/// letter cards, a turtle↔rabbit slider, a ▶ button.
fn select_controls(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    initial_reaction: f32,
    harness_frames: Option<u64>,
    shot: Option<&str>,
) -> Option<(usize, usize, f32)> {
    let mut jump = 0usize;
    let mut duck = 1usize;
    let mut reaction = initial_reaction.clamp(0.0, 1.0);
    let mut picking_jump = true;
    let mut frame: u64 = 0;

    while !rl.window_should_close() {
        let (w, h) = (rl.get_screen_width() as f32, rl.get_screen_height() as f32);
        let s = (w / 1280.0).min(h / 720.0);
        let (ox, oy) = ((w - 1280.0 * s) / 2.0, (h - 720.0 * s) / 2.0);
        let r = |x: f32, y: f32, rw: f32, rh: f32| Rectangle {
            x: ox + x * s,
            y: oy + y * s,
            width: rw * s,
            height: rh * s,
        };

        // Layout (1280x720 logical): two slots, six vowel cards, play button.
        let slot_jump = r(1280.0 / 2.0 - 330.0, 120.0, 260.0, 220.0);
        let slot_duck = r(1280.0 / 2.0 + 70.0, 120.0, 260.0, 220.0);
        let slider = r(1280.0 / 2.0 - 240.0, 556.0, 480.0, 26.0);
        let card = |i: usize| {
            r(
                1280.0 / 2.0 - 6.0 * 130.0 / 2.0 + i as f32 * 130.0 + 15.0,
                420.0,
                100.0,
                100.0,
            )
        };
        let play = r(1280.0 / 2.0 - 110.0, 612.0, 220.0, 86.0);

        // --- input ---
        let mouse = rl.get_mouse_position();
        let clicked = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
        // Reaction slider: drag anywhere on (or near) the track.
        if rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
            let grab = Rectangle {
                x: slider.x - 20.0,
                y: slider.y - 24.0,
                width: slider.width + 40.0,
                height: slider.height + 48.0,
            };
            if grab.check_collision_point_rec(mouse) {
                reaction = ((mouse.x - slider.x) / slider.width).clamp(0.0, 1.0);
            }
        }
        if clicked {
            if slot_jump.check_collision_point_rec(mouse) {
                picking_jump = true;
            } else if slot_duck.check_collision_point_rec(mouse) {
                picking_jump = false;
            } else if play.check_collision_point_rec(mouse) {
                return Some((jump, duck, reaction));
            } else {
                for i in 0..6 {
                    if card(i).check_collision_point_rec(mouse) {
                        if picking_jump {
                            if i == duck {
                                duck = jump; // swap instead of duplicating
                            }
                            jump = i;
                        } else {
                            if i == jump {
                                jump = duck;
                            }
                            duck = i;
                        }
                        picking_jump = !picking_jump;
                    }
                }
            }
        }
        if rl.is_key_pressed(KeyboardKey::KEY_ENTER) || rl.is_key_pressed(KeyboardKey::KEY_SPACE) {
            return Some((jump, duck, reaction));
        }

        // --- draw ---
        {
            let mut d = rl.begin_drawing(thread);
            d.clear_background(CREAM);
            d.draw_rectangle_gradient_v(0, 0, w as i32, h as i32, CREAM, PEACH);

            let arrow = |d: &mut RaylibDrawHandle, rect: Rectangle, up: bool| {
                let cx = rect.x + rect.width / 2.0;
                let top = rect.y + rect.height * 0.14;
                let bot = rect.y + rect.height * 0.40;
                let half = rect.width * 0.16;
                // Counter-clockwise winding so raylib fills the triangle.
                let (a, b, c) = if up {
                    (
                        Vector2::new(cx, top),
                        Vector2::new(cx - half, bot),
                        Vector2::new(cx + half, bot),
                    )
                } else {
                    (
                        Vector2::new(cx - half, top),
                        Vector2::new(cx, bot),
                        Vector2::new(cx + half, top),
                    )
                };
                d.draw_triangle(a, b, c, CHARCOAL);
            };
            let letter = |d: &mut RaylibDrawHandle, rect: Rectangle, text: &str, frac: f32| {
                let fs = (rect.height * frac) as i32;
                let tw = d.measure_text(text, fs);
                d.draw_text(
                    text,
                    (rect.x + rect.width / 2.0) as i32 - tw / 2,
                    (rect.y + rect.height * 0.44) as i32,
                    fs,
                    CHARCOAL,
                );
            };

            // Slots: fill in the "assignment" colour, outline the active one.
            for (slot, up, vowel, active) in [
                (slot_jump, true, jump, picking_jump),
                (slot_duck, false, duck, !picking_jump),
            ] {
                d.draw_rectangle_rounded(slot, 0.25, 8, if up { SKY } else { LILAC });
                arrow(&mut d, slot, up);
                letter(&mut d, slot, VOWELS[vowel].label(), 0.46);
                if active {
                    let grow = Rectangle {
                        x: slot.x - 5.0,
                        y: slot.y - 5.0,
                        width: slot.width + 10.0,
                        height: slot.height + 10.0,
                    };
                    d.draw_rectangle_rounded_lines(grow, 0.25, 8, CHARCOAL);
                }
            }

            // Vowel cards; the two assigned ones wear their slot colour.
            for i in 0..6 {
                let rect = card(i);
                let fill = if i == jump {
                    SKY
                } else if i == duck {
                    LILAC
                } else {
                    Color::new(255, 255, 255, 220)
                };
                d.draw_rectangle_rounded(rect, 0.3, 6, fill);
                d.draw_rectangle_rounded_lines(rect, 0.3, 6, MINT_DARK);
                let fs = (rect.height * 0.5) as i32;
                let tw = d.measure_text(VOWELS[i].label(), fs);
                d.draw_text(
                    VOWELS[i].label(),
                    (rect.x + rect.width / 2.0) as i32 - tw / 2,
                    (rect.y + rect.height * 0.26) as i32,
                    fs,
                    CHARCOAL,
                );
            }

            // Reaction slider: turtle (steady) ↔ rabbit (snappy), wordless.
            d.draw_rectangle_rounded(slider, 1.0, 6, Color::new(255, 255, 255, 200));
            d.draw_rectangle_rounded_lines(slider, 1.0, 6, MINT_DARK);
            let knob = Rectangle {
                x: slider.x + reaction * slider.width - 12.0 * s,
                y: slider.y - 8.0 * s,
                width: 24.0 * s,
                height: slider.height + 16.0 * s,
            };
            d.draw_rectangle_rounded(knob, 0.6, 4, BUTTER);
            d.draw_rectangle_rounded_lines(knob, 0.6, 4, CHARCOAL);
            // Turtle glyph (left): low shell + head, blocky.
            {
                let gx = slider.x - 74.0 * s;
                let gy = slider.y + slider.height / 2.0;
                let px = |x: f32, y: f32, w: f32, h: f32| Rectangle {
                    x: gx + x * s,
                    y: gy + y * s,
                    width: w * s,
                    height: h * s,
                };
                d.draw_rectangle_rounded(px(0.0, -12.0, 40.0, 20.0), 0.8, 4, MINT_DARK);
                d.draw_rectangle_rounded(px(36.0, -4.0, 14.0, 10.0), 0.6, 4, MINT_DARK);
                d.draw_rectangle_rec(px(6.0, 8.0, 8.0, 6.0), MINT_DARK);
                d.draw_rectangle_rec(px(26.0, 8.0, 8.0, 6.0), MINT_DARK);
            }
            // Rabbit glyph (right): body + two tall ears, blocky.
            {
                let gx = slider.x + slider.width + 28.0 * s;
                let gy = slider.y + slider.height / 2.0;
                let px = |x: f32, y: f32, w: f32, h: f32| Rectangle {
                    x: gx + x * s,
                    y: gy + y * s,
                    width: w * s,
                    height: h * s,
                };
                d.draw_rectangle_rounded(px(0.0, -8.0, 30.0, 22.0), 0.8, 4, CHARCOAL);
                d.draw_rectangle_rounded(px(4.0, -30.0, 8.0, 24.0), 0.8, 4, CHARCOAL);
                d.draw_rectangle_rounded(px(16.0, -30.0, 8.0, 24.0), 0.8, 4, CHARCOAL);
                d.draw_rectangle_rec(px(30.0, -2.0, 8.0, 8.0), CHARCOAL);
            }

            // Play button: mint pill with a ▶ triangle.
            d.draw_rectangle_rounded(play, 0.5, 8, MINT);
            d.draw_rectangle_rounded_lines(play, 0.5, 8, MINT_DARK);
            let cx = play.x + play.width / 2.0;
            let cy = play.y + play.height / 2.0;
            let ph = play.height * 0.28;
            d.draw_triangle(
                Vector2::new(cx - ph * 0.6, cy - ph),
                Vector2::new(cx - ph * 0.6, cy + ph),
                Vector2::new(cx + ph, cy),
                CHARCOAL,
            );
        }

        frame += 1;
        if let Some(path) = shot
            && Some(frame) == harness_frames.map(|m| m.saturating_sub(1))
        {
            snap(rl, thread, path);
        }
        if harness_frames.is_some_and(|m| frame >= m) {
            return Some((jump, duck, reaction));
        }
    }
    None
}
