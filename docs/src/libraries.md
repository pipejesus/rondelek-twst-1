# Libraries

Every third-party crate and why it's here. Versions are in `Cargo.toml`;
`Cargo.lock` is committed (this is an application, not a library) for reproducible
builds.

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

Game scenery is drawn by hand in our **flat-draw** tool (2D pixel drawing →
extruded 3D layers) and exported as `.glb` into `assets/models/`, alongside the
`*.meshes.json` flat-draw writes next to it (authoring metadata: canvas size,
pivot, per-layer bounds — read by people, not by the game). `game/src/models.rs`
embeds the GLB in the binary, loads it through raylib, merges the drawing's
per-layer meshes into one (they share a single texture atlas) so each prop costs
one draw call, and draws it many times with per-instance transforms.

The same path draws Vowel Runner's score icon, `sun.glb` (exported by the newer
build of the tool, which calls itself "Flatty" and writes no `*.meshes.json`).
It floats a few units in front of the camera, turned to face it, so it sits
over the HUD but is a real 3D prop: each point adds one full turn to where a
spring is heading (`SunCoin` in `runner.rs`), so the sun whirls round, swings a
little past and settles, and points in quick succession simply add turns. It is
lit by its own `sun.vs`/`sun.fs` (banded like the hero's toon shader, but lit
from the front, with normals turned by `matNormal`), passed per draw through
`FlatModel::draw_shaded`, which copies the material rather than changing the
model's own. Setting the shader on the model would make raylib free it a
second time when the model unloads.

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
| `anyhow` | Ergonomic error handling (`Result` + `Context`) throughout. |

## Declared but currently unused

| Crate | Note |
|-------|------|
| `ringbuf` | Declared in `Cargo.toml` but **not currently used** — the audio taps use `std::collections::VecDeque` behind a `Mutex`. It is a candidate either for adoption (a lock-free tap) or removal. |

> If you add, remove, or repurpose a dependency, update this page in the same
> change (see [Contributing](contributing.md)).
