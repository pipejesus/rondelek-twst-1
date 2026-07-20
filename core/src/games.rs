//! Registry of available voice mini-games. Lives in `core` (not in the
//! `rondelek-game` crate) because it's plain data the sampler app's menu
//! needs too, and the app can't depend on `rondelek-game` — that would drag
//! raylib into the app binary, defeating the point of splitting them.

/// (id, i18n name key) of every available game, in menu order.
pub const GAMES: &[(&str, &str)] = &[("runner", "games.runner.name")];
