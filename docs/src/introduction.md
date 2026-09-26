# Rondelek TWST-1 — Developer Handbook

Rondelek TWST-1 is a desktop **audio sampler for children with hearing implants**:
record short sounds onto pads and play them back, turning speech and hearing
practice into a game. One installation serves many children through a library of
**profiles**, each holding **sessions** of recordings on disk.

<p align="center">
  <img src="images/sampler.png" alt="The sampler faceplate: amber dot-matrix display over a 4x3 grid of keycap pads" width="380">
</p>

This handbook is for people **working on** Rondelek — how the pieces fit together,
especially the audio routing and the rendering. If you just want to build and run
the app, start with the repository [`README`](https://github.com/pipejesus/rondelek-twst-1#readme).

## Tech stack at a glance

- **Language:** Rust (edition 2024), a workspace of three crates: `core` (GUI-free
  shared logic), `app` (the sampler) and `game` (voice games, a separate process).
- **GUI:** [`egui`](https://github.com/emilk/egui) / `eframe`, immediate-mode.
- **Games:** [`raylib`](https://www.raylib.com/) in its own window and process.
- **Audio:** [`cpal`](https://github.com/RustAudio/cpal) for capture + playback, `hound` for WAV, `rustfft` for the spectrum, `spectrograms` for the MFCC vowel detector.
- **Camera:** [`nokhwa`](https://github.com/l1npengtul/nokhwa) (desktop) for avatar capture.
- **Persistence:** plain JSON files (`serde`) under the OS data/config directories — no database, no network.

## How to read this

| Chapter | What it covers |
|---------|----------------|
| [Architecture](architecture.md) | The `App` struct, the per-frame loop, the screen state machine, threading. |
| [Audio routing](audio.md) | **The essential one.** Capture, playback, the visualizer taps, device management. |
| [The visualizer](visualizer.md) | The pluggable `Visualizer` trait, the spectrum DSP pipeline, vowel detection and calibration. |
| [UI, layout & rendering](ui.md) | The fluid faceplate layout, the renderer, theming, fonts, i18n. |
| [Data model & storage](data-model.md) | What lives on disk and how it's laid out. |
| [Voice games](games.md) | The game process, Vowel Runner's scene, art from flat-draw, shaders and their tuning. |
| [Libraries](libraries.md) | Every dependency and why it's here. |

> **Keeping this current:** these docs are treated as part of the code. See
> [Contributing to these docs](contributing.md) — the rule is that any change to
> behaviour or wiring updates the relevant page in the same commit.
