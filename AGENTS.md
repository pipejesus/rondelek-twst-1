# Rondelek TWST-1

Audio sampler for children with hearing implants: record short sounds onto pads
and play them back, turning speech/hearing practice into a game. Built in Rust +
egui + cpal, targeting macOS, Linux and Windows (desktop only).

## Commands

```bash
cargo build                  # debug build
cargo build --release        # release build
cargo run                    # build and run
cargo test                   # run tests
cargo clippy -- -D warnings  # lint
cargo fmt --check            # format check
```

## ⚑ Agent rules

- **Keep translations in sync.** When you add or change any user-facing string,
  update `assets/i18n/en.json` (the key source of truth) **and every seeded locale**
  (`pl, de, fr, es, it, uk`) so all files hold the same keys. `en.json` is the
  fallback for untranslated languages. There is a test (`i18n::tests`) that fails if
  keys drift — run `cargo test`.

- **Keep the developer handbook current.** The `docs/` mdBook (published to GitHub
  Pages) documents how the app is wired — audio routing, the visualizer, UI/layout,
  the data model, and dependencies. **After any change to behaviour, wiring, data
  layout, or dependencies, update the relevant page in `docs/src/` in the same
  commit.** Reference code by symbol name, not line number. Map from area of change
  to page: audio/streams → `audio.md`; visualizer/DSP → `visualizer.md`; screens,
  layout, rendering, i18n → `ui.md` (and `architecture.md` for the state machine);
  on-disk formats/settings → `data-model.md`; dependencies → `libraries.md`. New
  page → add it to `docs/src/SUMMARY.md`. See `docs/src/contributing.md` to build
  locally (`mdbook serve docs`).

## Project Structure

```
src/
  main.rs           entry point, window options, env overrides
  app.rs            App struct, screen state machine, main loop, all screens
  config/
    layout.rs       pad identities (key only) + window constants
    theme.rs        colour palette, typography, spacing
    settings.rs     user prefs incl. UI language (JSON in config dir)
  i18n/
    mod.rs          runtime translations, language list, flags, locale detection
  profile/
    mod.rs          Profile + profile.json, library root, avatars, session listing
  session/
    mod.rs          Session folder model + session.json manifest (incl. uid)
  camera/
    mod.rs          desktop webcam capture (nokhwa)
  audio/
    capture.rs      cpal mic stream; folds interleaved frames to mono
    playback.rs     cpal output stream; per-frame mixing + linear resampling
    sample.rs       sample buffer, WAV encode/decode via hound
  ui/
    layout.rs       fluid faceplate layout from the live window rect
    pad.rs          tactile "keycap" pad: state machine, drawing, input
    widgets.rs      shared glossy keycap / gloss overlay / kid-face placeholder
    renderer.rs     device case, screen bezel, wordmark + tick marks
    visualizer.rs   FFT + CPU dot-matrix display
    dev_panel.rs    hidden dev controls (Ctrl+Shift+D)
  util/mod.rs       lerp, UID + UTC timestamp helpers
assets/
  fonts/            bundled Space Grotesk (OFL) + licence
  i18n/             <lang>.json translation files (en is source of truth)
  flags/            <lang>.png picker flags (public domain, flagcdn)
```

## How it works

- **Profiles → sessions, in a managed library.** One install serves many children.
  The library lives under the OS data dir: `…/rondelek/profiles/<slug>-<uid>/` with a
  `profile.json` (uid, name, avatar) + optional square `avatar.png` +
  `sessions/<YYYY-MM-DD_HH-MM-SS>/`. Each `session.json` carries its own uid. Profiles
  and sessions both have stable UIDs for future cross-referencing (notes, search).
  Profile/session folder names never use the raw name — see `profile::slug` /
  `sanitize_name`.
- **Screens** (`app.rs`, `AppScreen`): `Profiles` (search + language picker + cards)
  → `NewProfile` (name + avatar upload / webcam) → `Sessions` (resume or new) →
  `Session` (the sampler). The sampler header is a glossy kid-face "back to profiles"
  button (left), the profile avatar flush to the top edge (centre), and REC (right).
- **i18n.** `i18n::I18n` resolves keys: active locale → English → key. System locale
  is detected on first run (`sys-locale`) and saved to settings. See the agent rule.
- **Layout is fully fluid.** No fixed grid; `ui/layout.rs` derives every rect from
  `ui.max_rect()` each frame. Pads are a 4×3 block of square keycaps.
- **Audio is mono end-to-end.** Capture averages channels to mono at the device rate;
  playback duplicates mono across output channels (one cursor per frame) and
  linear-resamples to the output rate so pitch is correct.
- **Visualizer is CPU dot-matrix** (amber, bottom-up). No GPU shader — portable.

## Controls & env

- Pads: keys `1-4 / Q-R / A-F`; `Space` toggles REC. In REC mode hold a pad to record,
  release (or `Esc`) to stop. In play mode, tap to play.
- The square button just left of REC cycles the active visualizer (spectrum ↔ vowel meter).
- Sessions screen: "Calibrate voice" runs per-child vowel calibration (a/i/u corners).
- `Ctrl+Shift+D` dev panel · `Ctrl+Shift+T` light/dark · `Ctrl+Shift+S` screenshot.
- Env (testing/kiosk/screenshots): `RONDELEK_LANG=<code>`, `RONDELEK_PROFILE=<dir>`
  (jump to a profile's Sessions), `RONDELEK_SESSION=<dir>` (jump into a session),
  `RONDELEK_SCREEN=newprofile|editprofile|calibrate`, `RONDELEK_SIZE=WxH`, `RONDELEK_VIZ=<n>` (select
  visualizer index), `RONDELEK_SHOT=<png>` (capture a few frames in and exit).

## Design notes

- Config (pads, theme, layout) lives in Rust source; user prefs (incl. language) are
  JSON via `dirs`. Fonts, translations and flags are embedded in the binary.
- No global state — everything is owned by the `App` struct. The sampler is drawn via
  a cloned `Painter`; the start/profile screens use ordinary egui widgets.
- Deferred work (AVIF, more languages, session search, notes) is in `TODO.md`.
