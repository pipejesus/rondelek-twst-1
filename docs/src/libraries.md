# Libraries

Here's every third-party crate in the house, and why it's here. Versions live in
`Cargo.toml`. `Cargo.lock` is committed too (this is an application, not a
library), so builds are reproducible.

## GUI

| Crate | Role |
|-------|------|
| `eframe` | Application shell + windowing around `egui`. |
| `egui` | Immediate-mode GUI toolkit — all widgets and the painter. |
| `winit` | Low-level windowing under `eframe`; used directly only to force XWayland via `--x11` on Linux (`main.rs`). |
| `egui_kittest` *(dev only)* | Headless UI tests: renders the settings page, clicks controls by AccessKit label (`settings_page::tests`). Never linked into the app. |

## Audio

| Crate | Role |
|-------|------|
| `cpal` | Cross-platform audio I/O: the input (capture) and output (playback) streams and their callbacks. |
| `hound` | WAV encode/decode for recorded samples (`audio/sample.rs`). |
| `rustfft` | FFT for the spectrum visualizer (`ui/visualizer.rs`). |
| `spectrograms` | MFCC feature extraction for vowel detection (`audio/vowel.rs`); pure-Rust FFT backend. |
| `non-empty-slice` | Non-empty slice/`nzu!` types required by the `spectrograms` API. |

## Voice games (`rondelek-game` only)

| Crate | Role |
|-------|------|
| `raylib` | Window, 3D renderer and input for the voice mini-games. Confined to the `game` crate on purpose — see `docs/WORKSPACE_SPLIT.md`. |
| `serde_json` | Reads the optional shader tuning files (`RONDELEK_LAMPULA`, `RONDELEK_WATER`). Already a dependency of `core` and `app`. |

The game art comes from our own **flat-draw** pixel editor, as `.glb` files. The
game embeds them and draws them itself, and two of its shaders (Lam::pula, plus
the model vertex shader it needs) are copied from flat-draw verbatim. The full
tour of models, shaders, tuning tables and the water is in
[Voice games](games.md).

## Camera & images

| Crate | Role |
|-------|------|
| `nokhwa` | Desktop webcam capture for avatars (`input-native`). |
| `image` | Decode / resize / encode images — avatar processing and camera frames (`jpeg`, `png`, `webp`). |
| `rfd` | Native file-open dialog for uploading an avatar. |

## Persistence & utilities

| Crate | Role |
|-------|------|
| `serde` / `serde_json` | (De)serialize `profile.json`, `session.json`, `settings.json`. |
| `zip` | Extract uploaded skin `.zip` archives into the skins folder (`ui/skin.rs`). |
| `dirs` | Locate the OS data and config directories. |
| `uuid` (v4) | Stable IDs for profiles and sessions. |
| `sys-locale` | Detect the system language on first run. |
| `chrono` (`clock`) | Local time for the dates grown-ups see (sessions, voice calibration), in `ui/when.rs`. Stored times stay UTC Unix seconds. |
| `anyhow` | Ergonomic error handling (`Result` + `Context`) throughout. |

## Declared but currently unused

One crate is sitting on the shelf for now:

| Crate | Note |
|-------|------|
| `ringbuf` | Declared in `Cargo.toml` but **not currently used** — the audio taps use `std::collections::VecDeque` behind a `Mutex`. It is a candidate either for adoption (a lock-free tap) or removal. |

> If you add, remove or repurpose a dependency, please update this page in the
> same change (see [Contributing](contributing.md)). Future you will be grateful.
