//! Voice-controlled mini-games, rendered by raylib in their own
//! borderless-fullscreen window. The sampler app spawns the sibling
//! `rondelek-game <id> [--profile <dir>]` binary as a child process (see
//! `app/src/app/games.rs::spawn_game`); this crate owns everything past that
//! point.
//!
//! This is a separate crate (not a module of the `rondelek` app) on purpose:
//! raylib and the `windows` crate (pulled in by eframe/winit/accesskit on the
//! app side) both define a symbol named `ShowCursor`, which is a hard MSVC
//! linker error — `LNK2005` / `LNK1169` — the moment both end up in the same
//! Windows binary. Keeping raylib's dependents in their own executable is
//! what keeps that from happening.
//!
//! Every game starts on a control-selection screen where a grown-up picks
//! which vowel triggers which move (three slots: jump ▲, duck ▼, shoot ★) and
//! which obstacle kinds appear. The detector is then *focused* on the chosen
//! vowels, so close sound-alikes among the unchosen ones can't cause misfires.
//! The screen is deliberately text-free (letters, arrows, a play button) and
//! drawn in the arcade style of [`arcade`], like the "who's playing?" screen.
//!
//! Started **without** a profile (double-clicked, or run with no arguments),
//! a game first shows the "who's playing?" screen ([`profile_picker`]) so the
//! right child's voice calibration is used. The app always passes
//! `--profile`, so launched from its Games menu the picker is skipped.
//!
//! To add a game: write a `VoiceGame` impl in a new file, register it in the
//! `match` inside [`run`], and add its id + i18n name key to
//! `rondelek_core::games::GAMES`. It gets the profile picker and the control
//! screen for free.

pub mod arcade;
pub mod brick_water;
pub mod bricks;
pub mod lampula;
pub mod models;
pub mod profile_picker;
pub mod props;
pub mod runner;
mod shader_params;
pub mod twinkle;
pub mod view;
pub mod voice;
pub mod water;

use raylib::prelude::*;
use std::path::PathBuf;

use rondelek_core::audio::vowel::VOWELS;
use rondelek_core::config::Settings;
use rondelek_core::profile::Profile;
use voice::VoiceBridge;

// The pastel colours Vowel Runner's hero and HUD still use. (The entrance
// screens moved to the vivid arcade palette in `arcade`, the obstacles to
// bricks in `props`.)
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
    /// Demo mode (`RONDELEK_GAME_AUTOPLAY`): this frame's input as a child
    /// who knows every vowel would give it, for recording the README's
    /// gameplay. `None` (the default) for a game without a demo.
    fn autoplay(&mut self) -> Option<VoiceInput> {
        None
    }
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

/// The game time between recorded frames (`RONDELEK_GAME_RECORD`): 30 a second.
const RECORD_DT: f32 = 1.0 / 30.0;

/// raylib saves screenshots relative to the directory it was *initialized*
/// in, so shoot to a bare name there and move the file to the requested
/// destination (copy + remove — rename can't cross filesystems into /tmp).
pub(crate) fn snap(rl: &mut RaylibHandle, thread: &RaylibThread, path: &str) {
    const NAME: &str = "rondelek_shot.png";
    rl.take_screenshot(thread, NAME);
    if path != NAME && std::fs::copy(NAME, path).is_ok() {
        let _ = std::fs::remove_file(NAME);
    }
}

/// Run game `id` until its window closes (Esc). With a profile dir, uses that
/// child's vowel calibration; without one, first asks who's playing
/// ([`profile_picker::choose_child`]). Without a calibration the detector
/// recognises nothing (there is no fallback); the A/E/I/O/U/Y keys still
/// simulate vowels.
pub fn run(id: &str, profile_dir: Option<PathBuf>) -> anyhow::Result<()> {
    let (settings, _) = Settings::load();

    let (mut rl, thread) = raylib::init()
        .size(1280, 720)
        .title("Rondelek")
        .resizable()
        .build();
    rl.toggle_borderless_windowed();
    rl.set_target_fps(60);
    // Same icon as the sampler app (`cargo run --bin genicon`).
    if let Ok(icon) =
        Image::load_image_from_mem(".png", include_bytes!("../../assets/icon/rondelek.png"))
    {
        rl.set_window_icon(&icon);
    }

    // Non-interactive smoke harness, mirroring the app's RONDELEK_SHOT:
    // auto-quit after N frames, optionally saving a screenshot near the end.
    // RONDELEK_GAME_SCREEN=profiles / select runs the harness on the profile
    // picker / control-selection screen; otherwise the harness skips both
    // (no profile, default a/e/i controls).
    let max_frames: Option<u64> = std::env::var("RONDELEK_GAME_FRAMES")
        .ok()
        .and_then(|v| v.parse().ok());
    let shot: Option<String> = std::env::var("RONDELEK_GAME_SHOT").ok();
    let harness_screen = std::env::var("RONDELEK_GAME_SCREEN").ok();
    let harness_select = harness_screen.as_deref() == Some("select");
    let harness_profiles = harness_screen.as_deref() == Some("profiles");

    // Phase 0: whose voice? Given by the app via --profile, else asked here.
    // (The harness skips the picker unless RONDELEK_GAME_SCREEN=profiles.)
    let profile = match profile_dir {
        Some(dir) => Profile::load(dir).ok(),
        None if max_frames.is_some() && !harness_profiles => None,
        None => {
            let opts = profile_picker::PickerOptions {
                harness_frames: max_frames.filter(|_| harness_profiles),
                shot: shot.as_deref(),
            };
            match profile_picker::choose_child(&mut rl, &thread, &opts) {
                profile_picker::Pick::Child(p) => Some(p),
                profile_picker::Pick::NoChildren => None,
                profile_picker::Pick::Closed => return Ok(()),
            }
        }
    };
    let calibration = profile.as_ref().and_then(|p| p.load_calibration());

    // Phase 1: a grown-up picks which vowel drives which move, plus the
    // Reaction slider (turtle = steady, rabbit = snappy).
    let picked = if max_frames.is_some() && !harness_select {
        // harness default: a jumps, e ducks, i shoots; all obstacles on.
        Some((0, 1, 2, settings.game_reaction, [true, true, true, true]))
    } else {
        select_controls(
            &mut rl,
            &thread,
            settings.game_reaction,
            if harness_select { max_frames } else { None },
            shot.as_deref(),
        )
    };
    let Some((jump_vowel, duck_vowel, shoot_vowel, reaction, enabled)) = picked else {
        return Ok(()); // window closed on the selection screen
    };
    if harness_select {
        return Ok(());
    }
    if (reaction - settings.game_reaction).abs() > 0.001 {
        // Remember the grown-up's choice for next time.
        let (mut fresh, path) = Settings::load();
        fresh.game_reaction = reaction;
        fresh.save(&path);
    }

    let mut bridge = VoiceBridge::new(&settings, calibration);
    bridge.set_focus(&[jump_vowel, duck_vowel, shoot_vowel]);
    bridge.set_reaction(reaction);

    // The grown-up's obstacle pick → the kinds allowed to spawn.
    let kinds: Vec<runner::Kind> = [
        runner::Kind::Jump,
        runner::Kind::Duck,
        runner::Kind::Wall,
        runner::Kind::High,
    ]
    .into_iter()
    .zip(enabled)
    .filter_map(|(k, on)| on.then_some(k))
    .collect();

    let mut game: Box<dyn VoiceGame> = match id {
        "runner" => Box::new(runner::Runner::new(
            jump_vowel,
            duck_vowel,
            shoot_vowel,
            kinds,
        )),
        _ => anyhow::bail!("unknown game: {id}"),
    };
    game.init(&mut rl, &thread);

    // Phase 2: play (or, in demo mode, let the game play itself).
    let autoplay = std::env::var_os("RONDELEK_GAME_AUTOPLAY").is_some();
    // Recording (`RONDELEK_GAME_RECORD=<dir>`): every frame is saved as
    // <dir>/00000.qoi, 00001.qoi, … (QOI: lossless and quick to write, where
    // PNG took seconds a frame), the game stepping a fixed RECORD_DT each
    // however slowly it draws, so a machine that can't draw it in real time
    // (a virtual display) still makes smooth, real-speed video of it.
    // `RONDELEK_GAME_RECORD_FROM=<s>` plays the first s seconds unsaved, so
    // a short take starts where the action does.
    let record = std::env::var_os("RONDELEK_GAME_RECORD").map(PathBuf::from);
    if let Some(dir) = &record
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        anyhow::bail!("can't record into {}: {e}", dir.display());
    }
    let record_from = std::env::var("RONDELEK_GAME_RECORD_FROM")
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .map_or(0, |s| (s / RECORD_DT).round().max(0.0) as u64);
    let mut frame: u64 = 0;
    while !rl.window_should_close() {
        let dt = match record {
            Some(_) => RECORD_DT,
            None => rl.get_frame_time().min(0.1),
        };
        let kb_held = keyboard_vowel(&rl);
        let input = match autoplay.then(|| game.autoplay()).flatten() {
            Some(demo) => demo,
            None => bridge.poll(dt, kb_held),
        };
        game.update(&input, dt);

        let (w, h) = (rl.get_screen_width(), rl.get_screen_height());
        {
            let mut d = rl.begin_drawing(&thread);
            game.draw(&mut d, w, h);
            if let Some(dir) = record.as_ref().filter(|_| frame >= record_from) {
                // Read the frame back before it is shown: flush what raylib
                // still has batched, then take the back buffer.
                // SAFETY: a plain rlgl flush, between our own draw calls.
                unsafe { raylib::ffi::rlDrawRenderBatchActive() };
                let path = dir.join(format!("{:05}.qoi", frame - record_from));
                d.load_image_from_screen(&thread)
                    .export_image(&path.to_string_lossy());
            }
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

/// Mini obstacle icon inside a toggle: 0 = low block, 1 = high bar, 2 = tall
/// wall, 3 = tall pillar with an up-chevron. Plain rectangles, snapped to the
/// arcade pixel `p`, so it matches the rest of the screen.
fn draw_obstacle_icon(
    d: &mut RaylibDrawHandle,
    rect: Rectangle,
    kind: usize,
    color: Color,
    p: f32,
) {
    let snap = |v: f32| (v / p).round() * p;
    let cx = rect.x + rect.width / 2.0;
    let bottom = snap(rect.y + rect.height * 0.80);
    let block = |d: &mut RaylibDrawHandle, x: f32, y: f32, w: f32, h: f32| {
        d.draw_rectangle_rec(
            Rectangle {
                x: snap(x),
                y: snap(y),
                width: snap(w).max(p),
                height: snap(h).max(p),
            },
            color,
        );
    };
    match kind {
        0 => {
            let (bw, bh) = (rect.width * 0.34, rect.height * 0.34);
            block(d, cx - bw / 2.0, bottom - bh, bw, bh);
        }
        1 => {
            let bw = rect.width * 0.5;
            let top = rect.y + rect.height * 0.30;
            block(d, cx - bw / 2.0, top, bw, 2.0 * p);
            for px in [cx - bw / 2.0 + p, cx + bw / 2.0 - 2.0 * p] {
                block(d, px, top + 2.0 * p, p, bottom - top - 2.0 * p);
            }
        }
        2 => {
            let bw = rect.width * 0.32;
            let top = rect.y + rect.height * 0.22;
            block(d, cx - bw / 2.0, top, bw, bottom - top);
            // Mortar courses.
            for k in 1..3 {
                let y = top + (bottom - top) * k as f32 / 3.0;
                d.draw_rectangle_rec(
                    Rectangle {
                        x: snap(cx - bw / 2.0),
                        y: snap(y),
                        width: snap(bw),
                        height: (p / 2.0).max(1.0),
                    },
                    arcade::NIGHT_LO,
                );
            }
        }
        _ => {
            let bw = rect.width * 0.18;
            let top = rect.y + rect.height * 0.28;
            block(d, cx - bw / 2.0, top, bw, bottom - top);
            arcade::icon(d, arcade::ICON_UP, cx, top - 2.0 * p, p, color);
        }
    }
}

/// Holds a screen's input back until the press that opened it is let go.
///
/// A screen returns the moment it sees a press, mid-frame, before raylib
/// polls input again, so the next screen's first frame sees that very press
/// (`is_key_pressed` is still true). Without this, Enter on the "who's
/// playing?" picker would also start the game from the options screen before
/// anyone saw it, and a click there would land on whatever sits under the
/// cursor on the next screen. So a new screen listens only once Enter, Space
/// and the mouse buttons are all up.
#[derive(Default)]
struct InputGate {
    open: bool,
}

impl InputGate {
    /// Whether this frame's input counts, given whether any of the gate's
    /// keys and buttons is down. Once open, it stays open.
    fn update(&mut self, held: bool) -> bool {
        self.open |= !held;
        self.open
    }

    /// [`InputGate::update`] from raylib's current state.
    fn check(&mut self, rl: &RaylibHandle) -> bool {
        let held = [
            KeyboardKey::KEY_ENTER,
            KeyboardKey::KEY_KP_ENTER,
            KeyboardKey::KEY_SPACE,
        ]
        .iter()
        .any(|&k| rl.is_key_down(k))
            || [
                MouseButton::MOUSE_BUTTON_LEFT,
                MouseButton::MOUSE_BUTTON_RIGHT,
            ]
            .iter()
            .any(|&b| rl.is_mouse_button_down(b));
        self.update(held)
    }
}

/// Let a grown-up assign a vowel to each of the three moves (jump ▲, duck ▼,
/// shoot ★), toggle which obstacle kinds appear (difficulty), and set the
/// Reaction slider. Returns `(jump, duck, shoot, reaction, [jump, duck, wall,
/// high])`, or `None` if the window was closed. Text-free by design.
fn select_controls(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    initial_reaction: f32,
    harness_frames: Option<u64>,
    shot: Option<&str>,
) -> Option<(usize, usize, usize, f32, [bool; 4])> {
    // sel[0]=jump, sel[1]=duck, sel[2]=shoot; kept distinct by swapping.
    let mut sel = [0usize, 1, 2];
    let mut active = 0usize; // slot the next vowel click fills
    let mut enabled = [true; 4]; // jump / duck / wall / high obstacles
    let mut reaction = initial_reaction.clamp(0.0, 1.0);
    let mut frame: u64 = 0;
    // The picker's Enter or click must not carry over (see InputGate).
    let mut gate = InputGate::default();

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

        // Layout (1280x720 logical): three slots, six vowel cards, three
        // obstacle toggles, reaction slider, play button.
        let slot = |i: usize| r(240.0 + i as f32 * 280.0, 70.0, 240.0, 160.0);
        let card = |i: usize| {
            r(
                1280.0 / 2.0 - 6.0 * 130.0 / 2.0 + i as f32 * 130.0 + 15.0,
                268.0,
                100.0,
                100.0,
            )
        };
        let toggle = |i: usize| r(295.0 + i as f32 * 180.0, 402.0, 150.0, 96.0);
        let slider = r(1280.0 / 2.0 - 240.0, 548.0, 480.0, 26.0);
        let play = r(1280.0 / 2.0 - 110.0, 616.0, 220.0, 80.0);

        // --- input ---
        let live = gate.check(rl);
        let mouse = rl.get_mouse_position();
        let clicked = live && rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
        // Reaction slider: drag anywhere on (or near) the track.
        if live && rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT) {
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
            if play.check_collision_point_rec(mouse) {
                return Some((sel[0], sel[1], sel[2], reaction, enabled));
            }
            // Slot click: pick which move the next vowel fills.
            for sn in 0..3 {
                if slot(sn).check_collision_point_rec(mouse) {
                    active = sn;
                }
            }
            // Vowel card click: assign to the active slot, keeping all distinct.
            for i in 0..6 {
                if card(i).check_collision_point_rec(mouse) {
                    if let Some(other) = (0..3).find(|&sn| sn != active && sel[sn] == i) {
                        sel[other] = sel[active]; // swap, never duplicate
                    }
                    sel[active] = i;
                    active = (active + 1) % 3; // step to the next move
                }
            }
            // Obstacle toggle click: flip on/off, but never leave all four off.
            for i in 0..4 {
                if toggle(i).check_collision_point_rec(mouse)
                    && !(enabled[i] && enabled.iter().filter(|&&e| e).count() == 1)
                {
                    enabled[i] = !enabled[i];
                }
            }
        }
        if live
            && (rl.is_key_pressed(KeyboardKey::KEY_ENTER)
                || rl.is_key_pressed(KeyboardKey::KEY_SPACE))
        {
            return Some((sel[0], sel[1], sel[2], reaction, enabled));
        }

        // --- draw ---
        {
            use arcade::{BUTTER, CREAM, CYAN, GREEN, INK, NIGHT_HI, NIGHT_LO, ORANGE, PINK};
            let t = rl.get_time() as f32;
            let p = arcade::pixel_unit(s);
            let mut d = rl.begin_drawing(thread);
            arcade::backdrop(&mut d, w, h, p, t);

            // The three moves share the first three card colours: jump blue,
            // duck violet, shoot sunflower.
            let slot_color = [
                arcade::TILE_COLORS[0],
                arcade::TILE_COLORS[1],
                arcade::TILE_COLORS[2],
            ];
            // Vowel size: `frac` of the box's height for the letter's body.
            let letter_px = |rect: Rectangle, frac: f32| {
                (rect.height * frac / arcade::VOWEL_ROWS).round().max(2.0)
            };

            // Slots: ▲ / ▼ / ★ over the assigned vowel. The active one (the
            // slot the next vowel click fills) is framed, with a cursor.
            for i in 0..3 {
                let rect = slot(i);
                if i == active {
                    arcade::highlight(&mut d, rect, p, CREAM);
                    let bob = ((t * 2.4).sin() * p).round();
                    arcade::icon_shadowed(
                        &mut d,
                        arcade::ICON_CURSOR,
                        rect.x + rect.width / 2.0,
                        rect.y - 6.0 * p + bob,
                        p,
                        BUTTER,
                        INK,
                    );
                }
                arcade::panel(&mut d, rect, p, slot_color[i]);
                let glyph = [arcade::ICON_UP, arcade::ICON_DOWN, arcade::ICON_STAR][i];
                let (_, gh) = arcade::bitmap_size(glyph);
                let px = (rect.height * 0.18 / gh).round().max(p / 2.0);
                // No drop shadows on the big glyphs: they must read at a glance.
                arcade::icon(
                    &mut d,
                    glyph,
                    rect.x + rect.width / 2.0,
                    rect.y + rect.height * 0.24,
                    px,
                    INK,
                );
                let px = letter_px(rect, 0.30);
                arcade::vowel(
                    &mut d,
                    VOWELS[sel[i]].label(),
                    rect.x + rect.width / 2.0,
                    rect.y + rect.height * 0.775, // baseline: room for the i dot and the y tail
                    px,
                    INK,
                );
            }

            // Vowel keys: dark keycaps; the three assigned ones wear their
            // move's colour.
            for (i, vowel) in VOWELS.iter().enumerate() {
                let rect = card(i);
                let assigned = (0..3).find(|&sn| sel[sn] == i);
                let (face, ink) = match assigned {
                    Some(sn) => (slot_color[sn], INK),
                    None => (NIGHT_HI, CREAM),
                };
                arcade::panel(&mut d, rect, p, face);
                let px = letter_px(rect, 0.40);
                arcade::vowel(
                    &mut d,
                    vowel.label(),
                    rect.x + rect.width / 2.0,
                    rect.y + rect.height * 0.46 + px * arcade::VOWEL_ROWS / 2.0,
                    px,
                    ink,
                );
            }

            // Obstacle toggles (difficulty): a lit toggle shows its obstacle
            // in colour and a green light; an unlit one is dim with a dark
            // light.
            let toggle_fill = [
                PINK,
                arcade::TILE_COLORS[1],
                arcade::shade(CREAM, -0.3),
                ORANGE,
            ];
            for i in 0..4 {
                let rect = toggle(i);
                let on = enabled[i];
                arcade::panel(&mut d, rect, p, if on { NIGHT_HI } else { NIGHT_LO });
                let icon = if on {
                    toggle_fill[i]
                } else {
                    arcade::with_alpha(toggle_fill[i], 70)
                };
                draw_obstacle_icon(&mut d, rect, i, icon, p);
                let led = Rectangle {
                    x: rect.x + rect.width - 7.0 * p,
                    y: rect.y + 3.0 * p,
                    width: 4.0 * p,
                    height: 4.0 * p,
                };
                arcade::notched(&mut d, led, p, INK);
                let lamp = Rectangle {
                    x: led.x + p,
                    y: led.y + p,
                    width: 2.0 * p,
                    height: 2.0 * p,
                };
                d.draw_rectangle_rec(lamp, if on { GREEN } else { NIGHT_LO });
            }

            // Reaction slider: turtle (steady) ↔ rabbit (snappy), wordless. A
            // pixel track that fills cyan up to the knob.
            arcade::notched(&mut d, slider, p, INK);
            let inner = Rectangle {
                x: slider.x + p,
                y: slider.y + p,
                width: slider.width - 2.0 * p,
                height: slider.height - 2.0 * p,
            };
            d.draw_rectangle_rec(inner, NIGHT_LO);
            d.draw_rectangle_rec(
                Rectangle {
                    width: (inner.width * reaction).round(),
                    ..inner
                },
                CYAN,
            );
            let knob = Rectangle {
                x: slider.x + reaction * slider.width - 4.0 * p,
                y: slider.y - 3.0 * p,
                width: 8.0 * p,
                height: slider.height + 6.0 * p,
            };
            arcade::panel(&mut d, knob, p, BUTTER);
            let mid = slider.y + slider.height / 2.0;
            arcade::icon_shadowed(
                &mut d,
                arcade::ICON_TURTLE,
                slider.x - 60.0 * s,
                mid,
                p,
                GREEN,
                INK,
            );
            arcade::icon_shadowed(
                &mut d,
                arcade::ICON_RABBIT,
                slider.x + slider.width + 52.0 * s,
                mid - p,
                p,
                PINK,
                INK,
            );

            // Play: the "PRESS START" button — orange, with a butter ▶.
            arcade::panel(&mut d, play, p, ORANGE);
            let (_, ph) = arcade::bitmap_size(arcade::ICON_PLAY);
            let px = (play.height * 0.5 / ph).round().max(p / 2.0);
            arcade::icon(
                &mut d,
                arcade::ICON_PLAY,
                play.x + play.width / 2.0 + px / 2.0,
                play.y + play.height / 2.0,
                px,
                BUTTER,
            );
        }

        frame += 1;
        if let Some(path) = shot
            && Some(frame) == harness_frames.map(|m| m.saturating_sub(1))
        {
            snap(rl, thread, path);
        }
        if harness_frames.is_some_and(|m| frame >= m) {
            return Some((sel[0], sel[1], sel[2], reaction, enabled));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::InputGate;

    #[test]
    fn the_gate_waits_for_the_press_that_opened_the_screen_to_end() {
        let mut gate = InputGate::default();
        // Enter is still down from the previous screen: nothing counts…
        assert!(!gate.update(true));
        assert!(!gate.update(true));
        // …until it's let go; from then on, every press counts.
        assert!(gate.update(false));
        assert!(gate.update(true), "a new press, on this screen");
    }

    #[test]
    fn a_screen_opened_with_nothing_held_listens_at_once() {
        assert!(InputGate::default().update(false));
    }
}
