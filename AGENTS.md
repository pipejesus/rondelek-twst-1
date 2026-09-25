# Rondelek TWST-1

Audio sampler for children with hearing implants: record short sounds onto pads
and play them back, turning speech/hearing practice into a game. Voice mini-games
(vowel-controlled) run as a separate raylib process. Built in Rust + egui + cpal,
targeting macOS, Linux and Windows (desktop only).

## Commands

```bash
cargo build                  # debug build of the whole workspace (app + game)
cargo build --release        # release build
cargo run                    # run the sampler app (default-run = rondelek)
cargo test                   # run tests (all crates)
cargo clippy --all-targets -- -D warnings  # lint
cargo fmt --all --check      # format check
cargo run --bin genskin      # regenerate the built-in base skin (skins/base/)
```

`cargo run` rebuilds only the app. The app launches games by running
`rondelek-game` from the folder its own exe is in, so run `cargo build` first,
or Games will start a stale (or missing) game binary.

## ⚑ Git workflow: one worktree per branch

- **`develop` is the main branch** (the GitHub default and the base for every PR).
  The main checkout (`rondelek-twst-1/`) always stays on `develop`. Never commit
  there directly. Only update it with `git pull --ff-only`, create worktrees from
  it, and run the current `develop` app.
- **Every feature, fix, refactor, docs change or experiment gets its own branch
  and its own worktree** under `.claude/worktrees/` (gitignored). The folder name
  is the branch name with `/` replaced by `-`:

  ```bash
  # from the main checkout
  git pull --ff-only
  git worktree add .claude/worktrees/feat-friendly-shell -b feat/friendly-shell develop
  ```

  Branch prefixes: `feat/`, `fix/`, `refactor/`, `docs/`, `experiment/`.
  Use kebab-case slugs.
- **Claude sessions:** after creating a worktree, switch into it with
  `EnterWorktree(path=…)` (or start `claude` inside that folder). Do all edits,
  builds and commits there. Don't `cd` back into the main checkout to edit.
- **Each worktree has its own `target/`.** The first build is a full compile,
  raylib included, and takes a few minutes. Run `cargo build` (whole workspace)
  before running the app; see Commands for why.
- **What worktrees share:** git objects, branches and the stash (never use a bare
  `git stash`/`pop`; make a WIP commit instead). They also share the
  **real user data**: `settings.json` in the OS config dir and the profile
  library in the OS data dir. An app running in any worktree reads and writes
  the same files, so be careful with experiments that change the on-disk format.
- **Finishing a branch:**
  1. `cargo fmt --all --check && cargo clippy --all-targets -- -D warnings && cargo test`
  2. Update docs/translations as the rules below require.
  3. `git push -u origin <branch>` → `gh pr create --base develop`. Agents
     **ask the user before pushing or opening a PR** unless they already asked
     for it in this task.
  4. After the PR is merged on GitHub (CI must be green): from the main checkout,
     `git pull --ff-only`, `git worktree remove .claude/worktrees/<dir>`,
     `git branch -d <branch>`.
- Follow-up work after a merge goes on a new branch and worktree. Don't reuse
  merged branches.
- `git worktree list` shows what's in flight. `git worktree prune` cleans up
  entries whose folders were deleted by hand.
- **CodeGraph** indexes only the main checkout (`develop`). Inside a worktree its
  answers describe `develop`, not your branch. Trust the files on disk for
  anything you've changed, or run `codegraph init -i` in the worktree to give it
  its own index.

## ⚑ Agent rules

- **Keep translations in sync.** When you add or change any user-facing string,
  update `assets/i18n/en.json` (the key source of truth) **and every seeded locale**
  (`pl, de, fr, es, it, uk`) so all files hold the same keys. `en.json` is the
  fallback for untranslated languages. A test
  (`i18n::tests::english_has_all_keys_others_match`) fails if keys drift, so run
  `cargo test`. Never hard-code user-facing English in UI code.

- **Never break users' saved data.** `settings.json` and every profile's
  `profile.json` / `calibration.json` / `session.json` already exist on
  therapists' machines. New `Settings` fields need `#[serde(default…)]`; renamed
  fields need `#[serde(alias = "old")]`. Never remove or retype a field without a
  migration and a test that parses the old shape.

- **Keep the developer handbook current.** The `docs/` mdBook (published to GitHub
  Pages) documents how the app is wired: audio routing, the visualizer, UI/layout,
  the data model, and dependencies. **After any change to behaviour, wiring, data
  layout, or dependencies, update the relevant page in `docs/src/` in the same
  commit.** Reference code by symbol name, not line number. Which page covers
  what: audio/streams → `audio.md`; visualizer/DSP → `visualizer.md`; screens,
  layout, rendering, i18n → `ui.md` (and `architecture.md` for the state machine);
  on-disk formats/settings → `data-model.md`; dependencies → `libraries.md`. For a
  new page, add it to `docs/src/SUMMARY.md`. See `docs/src/contributing.md` to build
  locally (`mdbook serve docs`).

## Project Structure

A three-crate Cargo workspace. `core` must stay GUI-free, so the `game` binary
can link it without pulling in eframe/winit (the two can't share an exe on
Windows; see `game/src/lib.rs`).

```
core/src/                 rondelek-core: shared, GUI-toolkit-free
  config/
    settings.rs           user prefs (JSON in the OS config dir) incl. language, devices
    layout.rs             pad identities (key only) + window constants
    theme.rs              Theme colour struct + defaults (skins override via skin.json)
    atlas.rs              fixed sprite rectangles of the skin spritesheet
  audio/
    capture.rs            cpal mic stream; folds interleaved frames to mono
    playback.rs           cpal output stream; mixing + linear resampling
    device.rs             device enumeration, Auto/pinned choice, fallback
    sample.rs             sample buffer, WAV encode/decode via hound
    vowel.rs              MFCC template-matching vowel detector + calibration
  profile/mod.rs          Profile + profile.json, library root, avatars, calibration, sessions
  session/mod.rs          Session folder model + session.json manifest (incl. uid)
  games.rs                registry of voice games (id + i18n name key)
  util/mod.rs             lerp, UID + UTC timestamp helpers
app/src/                  rondelek: the egui sampler app
  main.rs                 entry point, window options, --x11, RONDELEK_SIZE
  app.rs                  App struct, AppScreen state machine, all shell screens
  i18n/mod.rs             runtime translations, language list, flags
  camera/mod.rs           desktop webcam capture (nokhwa) for avatars
  pixelart.rs             procedural pixel-art engine for the bin/ generators only
  bin/genskin.rs          generates skins/base/ (the embedded base skin)
  bin/genbanner.rs        generates docs/images/banner.png
  ui/
    config_panel.rs       the F12 Settings window (tabs: audio, detection, …)
    skin.rs               skin loading (skin.png spritesheet + skin.json colours)
    layout.rs             fluid faceplate layout from the live window rect
    pad.rs                sampler pad: state machine, input, drawing (skin caps)
    renderer.rs           device case + screen bezel from the skin
    visualizer.rs         Visualizer trait + FFT dot-matrix spectrum
    vowel_visualizer.rs   big detected vowel + per-vowel match meters
    level_meter.rs        input-level bar (calibration, settings)
    widgets.rs            kid-face placeholder avatar, gloss overlay
game/src/                 rondelek-game: raylib voice games (child process)
  lib.rs                  run loop, text-free pre-game control selection
  runner.rs               "Vowel Runner" 2.5D game
  voice.rs                mic + vowel detector → per-frame game input
  models.rs               embedded flat-draw GLB props (clouds, bushes)
assets/
  fonts/                  bundled Space Grotesk (OFL) + licence
  i18n/                   <lang>.json translation files (en is source of truth)
  flags/                  <lang>.png picker flags (public domain, flagcdn)
  models/                 flat-draw scenery: <name>.glb (+ .meshes.json metadata)
  shaders/                GLSL for the voice games (toon, fog)
skins/base/               generated base skin (skin.png + skin.json), embedded
```

## How it works

- **Profiles → sessions, in a managed library.** One install serves many children.
  The library lives under the OS data dir: `…/rondelek/profiles/<slug>-<uid>/` with a
  `profile.json` (uid, name, avatar), an optional square `avatar.png`, an optional
  `calibration.json`, and `sessions/<YYYY-MM-DD_HH-MM-SS>/`. Each `session.json`
  carries its own uid. Folder names never use the raw name; see `profile::slug` /
  `sanitize_name`.
- **Screens** (`app.rs`, `AppScreen`): `Profiles` (search + language + cards) →
  `Sessions` (the child's hub: edit, calibrate, games, resume/new session), plus
  `ProfileForm` (create/edit, upload or webcam avatar), `Calibrate`, `Games`, and
  `Session` (the skinned sampler). Settings is the F12 `ConfigPanel` window.
- **Vowel detection** (`core::audio::vowel`): MFCC template matching against the
  child's own six calibrated vowels (a e i o u y). Calibration is required; there
  is no uncalibrated fallback. The sampler's vowel visualizer and the games share
  the same gate settings (`vowel_*` in `Settings`).
- **Games** run as a separate `rondelek-game <id> --profile <dir>` process. The
  app releases the mic first and stops repainting while the game is up (see
  `docs/PERF.md`).
- **Skins.** One `skin.png` spritesheet (regions fixed by `config::atlas`) plus
  `skin.json` colours. The base skin is embedded; user skins are folders or zips
  in the skins dir.
- **i18n.** `i18n::I18n` resolves keys: active locale → English → key. The system
  locale is detected on first run (`sys-locale`) and saved to settings.
- **Layout is fully fluid.** No fixed grid; `ui/layout.rs` derives every rect from
  `ui.max_rect()` each frame. Pads are a 4×3 block of square keycaps.
- **Audio is mono end-to-end.** Capture averages channels to mono at the device rate;
  playback duplicates mono across output channels and linear-resamples to the
  output rate so pitch is correct.

## Controls & env

- Pads: keys `1-4 / Q-R / A-F`; `Space` toggles REC. In REC mode hold a pad to record,
  release (or `Esc`) to stop. In play mode, tap to play.
- The square button just left of REC cycles the visualizer (spectrum → vowels → off).
- `F12` Settings · `Ctrl+Shift+S` screenshot.
- Env (testing/kiosk/screenshots): `RONDELEK_LANG=<code>`, `RONDELEK_PROFILE=<dir>`
  (jump to a profile's Sessions), `RONDELEK_SESSION=<dir>` (jump into a session),
  `RONDELEK_SCREEN=newprofile|editprofile|calibrate|games`,
  `RONDELEK_CALIB_VOWEL=<n>` (with `calibrate`: open vowel n), `RONDELEK_SIZE=WxH`,
  `RONDELEK_VIZ=<n>` (visualizer index), `RONDELEK_SHOT=<png>` (capture a few frames
  in and exit). Game: `RONDELEK_GAME_SCREEN=select`, `RONDELEK_GAME_FRAMES=<n>`,
  `RONDELEK_GAME_SHOT=<png>`.

## Design notes

- Pads and layout live in Rust source. Colours come from the skin (`skin.json`
  over `theme_light()`). User prefs are JSON via `dirs`. Fonts, translations,
  flags, the base skin, shaders and models are embedded in the binaries.
- No global state: everything is owned by the `App` struct. The sampler is drawn
  via a cloned `Painter`; the other screens currently use ordinary egui widgets.
- Keep idle CPU low: static screens repaint slowly, nothing repaints while a game
  child is running (see `docs/PERF.md`).
- Deferred work is in `TODO.md`. `docs/archive/` holds early design notes that
  are **historical only**. Never implement from them.
