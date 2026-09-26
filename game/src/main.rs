//! `rondelek-game [<id>] [--profile <dir>]` — runs one voice mini-game in its
//! own raylib window. The sampler app spawns it with both arguments (see
//! `app/src/app/games.rs::spawn_game`). It also works on its own: with no
//! game id it starts the first registered game, and with no `--profile` it
//! asks who's playing first. So double-clicking `rondelek-game.exe` works.

// Release builds on Windows are GUI apps: without this, every launch also
// opens a black console window. Debug builds keep the console for logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The game id is the first argument that isn't an option (or its value);
    // default to the first registered game.
    let id = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .map(String::as_str)
        .unwrap_or(rondelek_core::games::GAMES[0].id);
    let profile = args
        .iter()
        .position(|a| a == "--profile")
        .and_then(|j| args.get(j + 1))
        .map(std::path::PathBuf::from);
    if let Err(e) = rondelek_game::run(id, profile) {
        eprintln!("game error: {e}");
        std::process::exit(1);
    }
}
