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
| `serde_json` | Reads the optional shader tuning files (`RONDELEK_LAMPULA`, `RONDELEK_WATER`). Already a dependency of `core` and `app`. |

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
drawn in Lam::pula glass (its own `Lampula` instance, `sun_glass` in
`runner.rs`, so its look is tuned apart from the clouds'), falling back to its
own `sun.vs`/`sun.fs` (banded, front-lit, normals turned by `matNormal`) if
that shader won't compile. Shaders go in per draw through
`FlatModel::draw_shaded`, which copies the material rather than changing the
model's own. Setting the shader on the model would make raylib free it a
second time when the model unloads.

The count is printed *on* the sun's face, like a coin's value: raylib's pixel
font, light with a warm outline, drawn inside the 3D pass with the sun's own
matrix pushed on rlgl's stack, so it turns with the sun. It changes while the
face is turned away (`SunCoin::shown`), so the sun comes back round showing the
new number. Two raylib details are in that code. **raylib-rs 6's
`rl_mult_matrixf` passes the matrix transposed**: it casts the `Matrix` struct,
whose fields are laid out row by row, straight to the column-major `float[16]`
`rlMultMatrixf` reads, so the translation is lost. We pass `transpose()` to
cancel that. The text is also drawn with the depth test off (fenced by batch
flushes), because the glyph quads' see-through corners would otherwise hide the
light face behind its own outline.

### Shaders from flat-draw, and tuning tables

The clouds are drawn through **Lam::pula**, flat-draw's tinted-glass shader,
taken over **1:1**: `assets/shaders/lampula.fs` is flat-draw's `lampFS` and
`assets/shaders/flatdraw_model.vs` its shared `modelVS`, copied verbatim (each
file names the flat-draw commit). `game/src/lampula.rs` is the driver: it feeds
the uniforms flat-draw's `Mesh.lampula` feeds (eye, clock, the instance's
bounding box for the lamps to stand round, and `uBrick`, the world → pixel
lattice map flat-draw's `brickMatrix` builds). The lattice comes from
`FlatModel::lattice`, with pixels-per-unit recovered from the geometry itself
(the smallest step between vertex coordinates), because newer exports carry no
`meshes.json`. `Lampula::draw` takes any `FlatModel` and any transform, so
anything flat-draw drew can be put through the glass. The clouds wear it with
one change from flat-draw's defaults (`cloud_glass` in `runner.rs`): the "room
below" reflected at the silhouette is the horizon blue, not flat-draw's dark
floor, which turned the clouds muddy against a sunny sky. `RONDELEK_LAMPULA`
still applies on top. To keep them a backdrop rather than an attraction, the
cloud layer is then hazed: the sky's own gradient is drawn once more over it,
see-through (`CLOUD_HAZE`), which is invisible over bare sky and pulls the
clouds toward it (atmospheric perspective, without touching the shader). Their
motion is deliberately tiny and slow (a float of a few pixels, a faint breath,
no tilt), because sky motion is what makes children dizzy.

Tuning follows flat-draw's arrangement (`game/src/shader_params.rs`): a shader's
knobs are one table of rows (field, flat-draw key, default, range), from which
the uniform name (`vibrance` → `uVibrance`), the per-frame upload and a
`set(key, value)` that reads flat-draw's config format are all derived.
`LampulaParams`' defaults are flat-draw's `Def` column. Tests read the GLSL and
fail on a row without its uniform (or a uniform nothing sets), as flat-draw's
`TestEveryExposedParamHasItsUniform` does. Nothing is exposed in the game UI.
To experiment, edit a default, set `params` in code, or start the game with
`RONDELEK_LAMPULA=<json>` (flat-draw's own `~/.config/flatty/config.json` works,
so a look tuned in its Shaders pane carries straight over) or
`RONDELEK_WATER=<json>`.

The **water** (`game/src/water.rs`, `water.vs`/`water.fs`) is our own: a grid
from `GenMeshPlane` lying in front of the meadow's bank, lifted by a swell that
laps the bank, with toon colour steps, a moving Voronoi web of light-lines,
foam and bubbles at the shore, "+" twinkles and goldfish. It scrolls with the
ground; every x-frequency is a whole number of turns per `PERIOD`, the distance
the scroll wraps on, so the surface never jumps at the wrap (a test checks each
`kx(n)`). It takes the child's smoothed voice level: sound swells the waves and
lights more twinkles. Its defaults were calmed on 2026-09-27 (slower rolls, web
and foam, softer colours, fewer idle twinkles) so it keeps less of the child's
attention. Every changed row notes its old value, and the foam and twinkle
speeds, fixed in the shader until then, became rows (`foamSpeed`,
`twinkleSpeed`).

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
