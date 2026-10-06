//! Registry of available voice mini-games. Lives in `core` (not in the
//! `rondelek-game` crate) because it's plain data the sampler app's menu
//! needs too, and the app can't depend on `rondelek-game` — that would drag
//! raylib into the app binary, defeating the point of splitting them.

/// One entry in the games menu.
pub struct GameInfo {
    /// Id passed to `rondelek-game <id>`.
    pub id: &'static str,
    /// i18n key of the game's name.
    pub name_key: &'static str,
    /// i18n key of the one-line description under the name.
    pub tagline_key: &'static str,
}

/// Every available game, in menu order.
pub const GAMES: &[GameInfo] = &[GameInfo {
    id: "runner",
    name_key: "games.runner.name",
    tagline_key: "games.runner.tagline",
}];
