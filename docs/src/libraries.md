# Libraries

Every third-party crate and why it's here. Versions are in `Cargo.toml`;
`Cargo.lock` is committed (this is an application, not a library) for reproducible
builds.

## GUI

| Crate | Role |
|-------|------|
| `eframe` | Application shell + windowing around `egui`. |
| `egui` | Immediate-mode GUI toolkit — all widgets and the painter. |

## Audio

| Crate | Role |
|-------|------|
| `cpal` | Cross-platform audio I/O: the input (capture) and output (playback) streams and their callbacks. |
| `hound` | WAV encode/decode for recorded samples (`audio/sample.rs`). |
| `rustfft` | FFT for the spectrum visualizer (`ui/visualizer.rs`). |

## Camera & images

| Crate | Role |
|-------|------|
| `nokhwa` | Desktop webcam capture for avatars (`input-native`; not compiled on Android). |
| `image` | Decode / resize / encode images — avatar processing and camera frames (`jpeg`, `png`, `webp`). |
| `rfd` | Native file-open dialog for uploading an avatar. |

## Persistence & utilities

| Crate | Role |
|-------|------|
| `serde` / `serde_json` | (De)serialize `profile.json`, `session.json`, `settings.json`. |
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
