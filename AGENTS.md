# Rondelek TWST-1

A playful sound board and voice games for kids: record funny sounds onto pads
and play them back, then steer little games by saying vowels. A toy first,
with some sneaky practice with sounds along the way. Voice mini-games
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
cargo run --bin genskin      # regenerate the built-in Classic skin (skins/base/)
cargo run --bin genarcadeskin # regenerate the built-in Arcade skin (skins/arcade/), the default
cargo run --bin genavatars   # regenerate the classic character avatars (assets/avatars/)
cargo run --bin genpixelpals # regenerate the pixel-art "pixel pals" (assets/avatars/pixel-*.png)
cargo run --bin genicon      # regenerate the app icon (assets/icon/rondelek.png)
cargo run --bin genarcade    # regenerate the README's banner, marquee + framed shots (docs/images/arcade/)
docs/images/arcade/record-runner.sh  # re-record the README's gameplay GIF (Linux: Xvfb, ffmpeg; demo mode; 10 s offline take at 1080p/30 fps; TAKE= re-cuts a take, KEEP= copies it out)
packaging/appimage/build.sh target/release v0.0.0-local dist  # Linux AppImage from a release build
```

`cargo run` only rebuilds the app, but the app launches games by running
`rondelek-game` from its own folder. So when it runs under cargo, pressing a
game first runs `cargo build -p rondelek-game` (same profile) and launches the
game once that finishes (`dev_game_build`). Shipped builds skip that step.

## Git workflow

- **`develop` is the main branch** (the GitHub default). It only moves by
  fast-forward: work happens on a branch off it, one topic per branch, each
  change its own commit. Branch prefixes: `feat/`, `fix/`, `refactor/`,
  `docs/`, `experiment/`, with kebab-case slugs.
- **Before you push:** `cargo fmt --all --check && cargo clippy --all-targets
  -- -D warnings && cargo test`, and update the docs and translations as the
  rules below require. CI (`.github/workflows/ci.yml`) runs the same on every
  pull request.
- **Contributions** come as pull requests against `develop` (from a fork,
  unless you have write access).
- **Your data is real.** Any build reads and writes the same `settings.json`
  (OS config dir) and profile library (OS data dir), so be careful with
  experiments that change the on-disk format, and take screenshots from a
  throwaway library (see Controls & env).
- **Releases** are cut by the maintainer from `v*` tags; how a tag becomes
  downloads is in `docs/src/releases.md`.

## ⚑ Agent rules

- **Keep translations in sync.** When you add or change any user-facing string,
  update `assets/i18n/en.json` (the key source of truth) **and every seeded locale**
  (`pl, de, fr, es, it, uk`) so all files hold the same keys. `en.json` is the
  fallback for untranslated languages. A test
  (`i18n::tests::english_has_all_keys_others_match`) fails if keys drift, so run
  `cargo test`. Never hard-code user-facing English in UI code.

- **Never break users' saved data.** `settings.json` and every profile's
  `profile.json` / `calibration.json` / `session.json` already exist on
  families' machines. New `Settings` fields need `#[serde(default…)]`; renamed
  fields need `#[serde(alias = "old")]`. Never remove or retype a field without a
  migration and a test that parses the old shape.

- **Keep the book current.** The `docs/` mdBook, *The Rondelek Book* (published
  to GitHub Pages), has two parts. **Playing with Rondelek** is the guide for
  parents: `getting-started.md` (download, first child), `voice-calibration.md`,
  `sound-board.md`, `playing-games.md`, `grown-ups.md` (the settings page),
  `controls.md`. **Under the hood** documents how the app is wired. **After any
  change to behaviour, wiring, data layout, or dependencies, update the relevant
  page in `docs/src/` in the same commit**, including the parents' page when
  something they see or do changes. Reference code by symbol name, not line
  number. Which page covers what: building, env vars → `building.md`;
  audio/streams → `audio.md`; visualizer/DSP → `visualizer.md`; screens, layout,
  rendering, i18n → `ui.md` (and `architecture.md` for the state machine);
  on-disk formats/settings → `data-model.md`; voice games (scene, art, shaders,
  tuning, adding a game) → `games.md`; dependencies → `libraries.md`. For a new
  page, add it to `docs/src/SUMMARY.md`. See `docs/src/contributing.md` to build
  locally (`mdbook serve docs`).
- **The README is the front page for parents.** Keep it short and friendly:
  what Rondelek is, the two ways to play, the Quick start (with the voice
  calibration), downloads, and links into the book. Details belong in the book.
- **Tone:** warm, relaxed and plain, in the docs and in the app's grown-up text.
  Rondelek is a playful toy for kids (with some sneaky practice), never pitched
  as therapy or as being for any particular condition.

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
  characters.rs           built-in character avatars (shared by app + games)
  session/mod.rs          Session folder model + session.json manifest (incl. uid)
  games.rs                registry of voice games (id + i18n name key)
  util/mod.rs             lerp, UID + UTC timestamp helpers
  arcade.rs               the arcade look's shared data: palette, child colours,
                          pixel icon + vowel bitmaps (app shell and games)
app/src/                  rondelek: the egui sampler app
  main.rs                 entry point, window options, --x11, RONDELEK_SIZE
  app/
    mod.rs                App struct, AppScreen state machine, per-frame loop, audio
    home.rs               "Who's playing?" (child tiles + new child)
    hub.rs                the child's hub (sounds / games / voice calibration) + sessions
    profile_form.rs       create/edit a child: name, character/photo/upload
    calibrate.rs          voice calibration (the vowel detector's per-child templates)
    games.rs              games menu + launching the game process
    settings.rs           glue for the grown-ups settings page
    sampler.rs            the skinned sampler
  i18n/mod.rs             runtime translations, language list, flags
  camera/mod.rs           desktop webcam capture (nokhwa) for avatars
  pixelart.rs             procedural pixel-art engine (bin/ generators; the app
                          uses only its 5×7 font for the wordmark)
  bin/genskin.rs          generates skins/base/ (the embedded Classic skin, the fallback)
  bin/genarcadeskin.rs    generates skins/arcade/ (the embedded Arcade skin, the default)
  bin/genavatars.rs       generates the classic (smooth) character avatars
  bin/genpixelpals.rs     generates the vivid 32×32 pixel-art avatars (pixel-*)
  bin/genicon.rs          generates assets/icon/rondelek.png (window + AppImage icon)
  bin/genarcade.rs        generates docs/images/arcade/ (README banner, marquee +
                          framed shots of docs/images/*.png)
  ui/
    shell.rs              the arcade shell kit: palette, fonts, egui style,
                          starfield + stripes, keycaps, panels, pixel text/icons
    settings_page.rs      the one-page "For grown-ups" settings, incl. the sessions
                          list (+ kittest UI tests)
    when.rs               friendly local dates ("Today, 14:05"), translated
    characters.rs         re-exports rondelek_core::characters
    skin.rs               skin loading (skin.png spritesheet + skin.json colours)
    layout.rs             fluid faceplate layout from the live window rect
    pad.rs                sampler pad: state machine, input, drawing (skin caps)
    renderer.rs           device case + screen bezel from the skin
    visualizer.rs         Visualizer trait + FFT dot-matrix spectrum
    vowel_visualizer.rs   big detected vowel + per-vowel match meters
    level_meter.rs        input-level bar (calibration, settings)
    widgets.rs            kid-face placeholder avatar
game/src/                 rondelek-game: raylib voice games (child process)
  lib.rs                  run loop, text-free pre-game control selection
  arcade.rs               the entrance screens' 8-bit look: palette, starfield,
                          notched panels, pixel icons + vowels, Tiny5 text
  profile_picker.rs       reusable "who's playing?" screen (standalone launches)
  runner.rs               "Vowel Runner" 2.5D game
  voice.rs                mic + vowel detector → per-frame game input
  view.rs                 what the camera sees, as geometry: where the picture's
                          edges fall, so things come and go out of sight
  models.rs               embedded flat-draw GLB props (clouds, score sun)
  shader_params.rs        shader tuning tables (flat-draw's Param rows) + JSON overrides
  lampula.rs              flat-draw's Lam::pula glass shader, 1:1 (clouds, sun, water)
  bricks.rs               brick models built in code (grid of colours → one mesh,
                          lit by Lam::pula; bricks may be see-through)
  obstacles.rs            Vowel Runner's obstacles in little bricks: a log and a
                          sleeping tortoise (taking turns), a fallen tree on
                          boulders, the brick wall, bamboo
  props.rs                Vowel Runner's meadow, ledges, vowel tablets and far planes
                          (mountains, palms, jungle with its glassy front),
                          built in bricks
  twinkle.rs              now and then a glint of sun on the jungle's bricks
  sway.rs                 the wind in the jungle's palms (jungle_sway.vs in
                          front of Lam::pula; WindParams, RONDELEK_WIND)
  brick_water.rs          the voice-reactive water along the front: little glass
                          bricks through Lam::pula (brick_water.vs + lampula.fs)
assets/
  fonts/                  bundled Space Grotesk (app) and Tiny5 (game's pixel
                          text; covers every shipped language), OFL + licences
  i18n/                   <lang>.json translation files (en is source of truth)
  flags/                  <lang>.png picker flags (public domain, flagcdn)
  avatars/                <name>.png character avatars (placeholders, replaceable)
  models/                 flat-draw scenery: <name>.glb
  shaders/                GLSL for the voice games (toon, sun, brick water,
                          the jungle's wind; lampula.fs +
                          flatdraw_model.vs copied verbatim from flat-draw)
  icon/                   rondelek.png app icon (generated by genicon, embedded)
skins/base/               generated Classic skin (skin.png + skin.json), embedded
skins/arcade/             generated Arcade skin (the default), embedded
packaging/appimage/       Linux AppImage: AppRun (app / --game), desktop entry,
                          build.sh + smoke.sh used by the release workflow
```

## How it works

- **Profiles → sessions, in a managed library.** One install serves many children.
  The library lives under the OS data dir: `…/rondelek/profiles/<slug>-<uid>/` with a
  `profile.json` (uid, name, avatar photo or character), an optional square
  `avatar.png`, an optional
  `calibration.json`, and `sessions/<YYYY-MM-DD_HH-MM-SS>/`. Each `session.json`
  carries its own uid. Folder names never use the raw name; see `profile::slug` /
  `sanitize_name`.
- **Screens** (`app/`, `AppScreen`): `Home` ("Who's playing?") → `Hub` (Sounds /
  Games / Voice calibration) → `Session` (the skinned sampler), plus `ProfileForm`,
  `Calibrate`, `Games` and `Settings` (the one-page "For grown-ups" settings,
  opened by the gear keys or F12). Kids never browse session lists: Sounds
  continues the session used last. Grown-ups see and open every session on the
  settings page (Sessions card); opening one makes it the one Sounds continues.
- **Wording:** it's **"voice calibration"** (it builds the vowel
  detector's calibration; never "voice check"), and grown-up text says
  **"profile"**, not "child", for the thing you edit or delete.
- **Two looks.** The sampler is drawn from the skin. Every other screen uses the
  shell kit (`ui/shell.rs`), in the **arcade style** shared with the games'
  entrance screens (palette + pixel bitmaps in `core::arcade`): navy night,
  notched keycaps, pixel titles (Tiny5) for kid-facing words, Space Grotesk for
  grown-up text, bold pixel vowels. **Vibrant, never pastel**, but
  calm: bright accents on the deep navy. Use `KeyButton` and friends for new
  shell UI, not stock egui buttons, and give every clickable an AccessKit label
  (tests click by label).
- **Vowel detection** (`core::audio::vowel`): MFCC template matching against the
  child's own six calibrated vowels (a e i o u y). Calibration is required; there
  is no uncalibrated fallback. The sampler's vowel visualizer and the games share
  the same gate settings (`vowel_*` in `Settings`).
- **Games** run as a separate `rondelek-game <id> --profile <dir>` process. The
  app releases the mic first and stops repainting while the game is up (see
  `docs/src/architecture.md`). The game binary also runs **on its own**: with no id it starts
  the first game in `GAMES`, and with no `--profile` it shows the reusable
  "who's playing?" picker (`game/src/profile_picker.rs`) first. New games get
  this for free through `game::run`. Everything about the games' look (scene
  layers, flat-draw models, Lam::pula, the water, shader tuning tables and their
  `RONDELEK_LAMPULA`/`RONDELEK_WATER` overrides) is in `docs/src/games.md`.
- **Game art** drawn by hand comes from the flat-draw pixel editor as `.glb`
  (`assets/models/<english-name>.glb`, embedded). Its glass shader is copied
  verbatim (`lampula.fs`, `flatdraw_model.vs`): tune it through its parameters,
  never edit the files.
- **Game scenery must stay calm.** Background motion is tiny and slow (kids get
  dizzy), and scenery must not compete with the hero for attention (hence the
  cloud haze and the slow water).
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
- `F12` opens/closes the settings page · `Ctrl+Shift+S` screenshot.
- Env (testing/kiosk/screenshots): `RONDELEK_LANG=<code>`, `RONDELEK_PROFILE=<dir>`
  (jump to a child's hub), `RONDELEK_SESSION=<dir>` (jump into a session),
  `RONDELEK_SCREEN=newprofile|editprofile|calibrate|games|settings`,
  `RONDELEK_CALIB_VOWEL=<n>` (with `calibrate`: open vowel n), `RONDELEK_SIZE=WxH`,
  `RONDELEK_VIZ=<n>` (visualizer index), `RONDELEK_SHOT=<png>` (capture a few frames
  in and exit). Game: `RONDELEK_GAME_SCREEN=profiles|select`, `RONDELEK_GAME_FRAMES=<n>`,
  `RONDELEK_GAME_AUTOPLAY=1` (demo mode: the game plays itself),
  `RONDELEK_GAME_SEED=<n>` (the same course every run),
  `RONDELEK_GAME_STATS=1` (load and frame timing on exit), `RONDELEK_GAME_FPS=<n>`
  (cap the frame rate; 0 = uncapped, for timing),
  `RONDELEK_GAME_RECORD=<dir>` (save every frame, stepping 1/30 s each, for video;
  `RONDELEK_GAME_RECORD_FROM=<s>` skips the first s seconds),
  `RONDELEK_GAME_SHOT=<png>`, `RONDELEK_LAMPULA=<json>` / `RONDELEK_WATER=<json>` /
  `RONDELEK_PROPS=<json>` (shader tuning overrides for the clouds and sun / the water /
  the meadow and obstacles, read at launch; `RONDELEK_LAMPULA` also takes flat-draw's
  own `~/.config/flatty/config.json`), `RONDELEK_WIND=<json>` (the wind in the
  jungle's palms, `WindParams`).
- **Screenshots without real data (Linux):** the profile library and settings
  live under `dirs::data_dir()` / `dirs::config_dir()`, which follow
  `XDG_DATA_HOME` / `XDG_CONFIG_HOME`. Point both at a temp folder and write a
  few fake `profiles/<slug>-<id>/profile.json` files (e.g.
  `{"uid":"demo-1","name":"Maya","avatar":null,"character":"fox","created":0}`)
  to get clean, publishable screenshots. Never publish screenshots of a real
  library: it holds children's names and photos.

## Design notes

- Pads and layout live in Rust source. Colours come from the skin (`skin.json`
  over `theme_light()`). User prefs are JSON via `dirs`. Fonts, translations,
  flags, the base skin, shaders and models are embedded in the binaries.
- No global state: everything is owned by the `App` struct. The sampler is drawn
  via a cloned `Painter`; the other screens use the shell kit (`ui/shell.rs`).
- Keep idle CPU low: static screens repaint slowly, nothing repaints while a game
  child is running (see `docs/src/architecture.md`).
- Deferred work is in `TODO.md`.
