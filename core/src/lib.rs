//! Shared logic between the sampler app (`rondelek`) and the voice-game
//! runner (`rondelek-game`): audio capture/vowel detection, settings,
//! profiles and sessions. Deliberately GUI-toolkit-free — no `eframe`,
//! `winit`, or `raylib` — so the two binaries can each pull in their own
//! (mutually incompatible on Windows, see `game/src/lib.rs`) windowing stack
//! without both ending up in the same link.
pub mod audio;
pub mod config;
pub mod games;
pub mod profile;
pub mod session;
pub mod util;
