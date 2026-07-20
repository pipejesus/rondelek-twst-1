mod app;
mod camera;
mod i18n;
mod ui;

use app::App;
use egui::ViewportBuilder;
use rondelek_core::config::{WINDOW_HEIGHT, WINDOW_WIDTH};

fn main() -> eframe::Result {
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
