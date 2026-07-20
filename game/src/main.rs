//! `rondelek-game <id> [--profile <dir>]` — runs one voice mini-game in its
//! own raylib window. Spawned as a child process by the sampler app (see
//! `app/src/app.rs::launch_game`); never invoked directly by a user.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(id) = args.first() else {
        eprintln!("usage: rondelek-game <id> [--profile <dir>]");
        std::process::exit(2);
    };
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
