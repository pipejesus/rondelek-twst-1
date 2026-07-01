mod app;
mod audio;
mod camera;
mod config;
mod i18n;
mod profile;
mod session;
mod ui;
mod util;

use app::App;
use config::{WINDOW_HEIGHT, WINDOW_WIDTH};
use egui::ViewportBuilder;

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

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([init_w, init_h])
            .with_min_inner_size([520.0, 560.0])
            .with_title("Rondelek TWST-1")
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "Rondelek TWST-1",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
