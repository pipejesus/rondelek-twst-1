mod app;
mod audio;
mod camera;
mod config;
mod game;
mod i18n;
mod profile;
mod session;
mod ui;
mod util;

use app::App;
use config::{WINDOW_HEIGHT, WINDOW_WIDTH};
use egui::ViewportBuilder;

fn main() -> eframe::Result {
    // `--game <id> [--profile <dir>]`: run a voice mini-game (own raylib
    // window) instead of the sampler app. The app spawns itself with these
    // args when a kid picks a game.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--game") {
        let id = args.get(i + 1).cloned().unwrap_or_default();
        let profile = args
            .iter()
            .position(|a| a == "--profile")
            .and_then(|j| args.get(j + 1))
            .map(std::path::PathBuf::from);
        if let Err(e) = game::run(&id, profile) {
            eprintln!("game error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    // Optional initial-size override ("WIDTHxHEIGHT"), useful for testing the
    // fluid layout at different window shapes.
    let (init_w, init_h) = std::env::var("RONDELEK_SIZE")
        .ok()
        .and_then(|s| {
            let (w, h) = s.split_once('x')?;
            Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
        })
        .unwrap_or((WINDOW_WIDTH as f32, WINDOW_HEIGHT as f32));

    // `--x11` (Linux): run under XWayland instead of native Wayland. winit's
    // Wayland backend busy-spins between compositor frame callbacks, burning a
    // full core; the X11 path blocks properly on vsync.
    #[cfg(target_os = "linux")]
    let x11_hook: Option<eframe::EventLoopBuilderHook> = std::env::args()
        .any(|a| a == "--x11")
        .then(|| -> eframe::EventLoopBuilderHook {
            Box::new(|builder| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                builder.with_x11();
            })
        });

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([init_w, init_h])
            .with_min_inner_size([520.0, 560.0])
            .with_title("Rondelek TWST-1")
            .with_resizable(true),
        #[cfg(target_os = "linux")]
        event_loop_builder: x11_hook,
        ..Default::default()
    };

    eframe::run_native(
        "Rondelek TWST-1",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
