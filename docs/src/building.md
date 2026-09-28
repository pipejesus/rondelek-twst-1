# Building from source

Welcome to the workshop. This page gets Rondelek running from its source code,
on Windows, macOS or Linux, even if you've never touched Rust before. Then it
shows you the everyday commands, and a few handy switches for testing.

## 1. Install Rust

Rust is installed with **`rustup`**, the official toolchain manager. Grab it from
[rustup.rs](https://rustup.rs). On macOS and Linux it's one command:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then restart your terminal and check that it worked:

```bash
rustc --version   # should print 1.88 or newer (edition 2024 + let-chains)
```

## 2. A few system bits

**macOS** needs nothing extra: audio (CoreAudio) and the camera (AVFoundation)
are built in. The first time you use *Take photo*, macOS asks for camera
permission. If you say no, the app just shows "Camera unavailable" and
everything else keeps working. (You can change your mind later under
*System Settings → Privacy & Security → Camera*.)

**Linux** needs a few development packages for audio, file dialogs, the window
and the webcam. On Debian or Ubuntu:

```bash
sudo apt install build-essential pkg-config cmake clang \
  libasound2-dev libgtk-3-dev libudev-dev libv4l-dev \
  libx11-dev libxcb1-dev libxrandr-dev libxinerama-dev libxcursor-dev \
  libxi-dev libgl1-mesa-dev
```

`cmake`, `clang` and the X11/GL headers are for raylib, which the game binary
compiles from source. Package names differ a little between distributions. If
a build fails, the error message usually names the missing library.

**Windows** needs the **MSVC C++ Build Tools** (the *Desktop development with
C++* workload in the Visual Studio Installer). After that, `rustup` and
`cargo` work just as described here. Audio and the camera use the built-in
Windows APIs.

## 3. Get the code and run it

```bash
git clone https://github.com/pipejesus/rondelek-twst-1.git rondelek
cd rondelek
cargo run --release
```

The first build compiles every dependency and takes a few minutes. That's
normal, and it's cached afterwards. The app opens on *Who's playing?*.

> ### Please use `--release`, especially for the webcam
>
> Plain `cargo run` makes a **debug** build: quick to compile, but the image
> and audio code runs unoptimised. The webcam preview shows it most, because
> decoding and scaling each frame is real work:
>
> | Build | Per frame | Live preview |
> |-------|-----------|--------------|
> | `cargo run` (debug) | ~1300 ms | **~1 fps** (a slideshow) |
> | `cargo run --release` | ~38 ms | **~26 fps** (smooth) |
>
> That's about 34 times faster. If the camera feels like a slideshow, you're
> almost certainly on a debug build.

## Building the programs to share

To make optimised programs you can copy anywhere:

```bash
cargo build --release
```

You'll find **two** programs in `target/release/`: the app, **`rondelek`**, and
the game runner, **`rondelek-game`** (with `.exe` on Windows). Fonts,
translations, flags, the built-in skins, shaders and 3D models are all baked
in. The app starts games by running `rondelek-game` from its own folder, though,
so **always keep and copy the two together**.

Under plain `cargo run`, pressing a game first builds `rondelek-game` (with the
same profile) and then launches it, so you never play a stale game by
accident. The ready-made downloads are built by CI; [Releases & packaging](releases.md)
tells that story.

## Everyday commands

```bash
cargo run --release          # build and run (games are rebuilt on launch)
cargo test                   # run the test suite
cargo fmt --all              # tidy the formatting
cargo clippy --all-targets   # catch common mistakes
```

**Branches and worktrees.** `develop` is the main branch. Work happens on a
branch in its own worktree under `.claude/worktrees/` (usually one per working
session) and lands on `develop` as a fast-forward. The whole workflow is in
`AGENTS.md`.

**Git hooks** (optional, but they save round trips). `pre-commit` checks the
formatting, and `pre-push` runs clippy and the tests, the same checks as CI.
Turn them on once per clone:

```bash
git config core.hooksPath .githooks
```

**Translations stay in sync.** `assets/i18n/en.json` is the source of truth,
and every other language file (`pl, de, fr, es, it, uk`) must carry the same
keys. `cargo test` fails if they drift. `AGENTS.md` has the full rule.

## Handy switches for testing

These environment variables jump straight to a screen, and can grab a
screenshot on the way. They're great for testing, demos and pictures:

| Variable | What it does |
|----------|--------------|
| `RONDELEK_LANG=de` | start in a specific language |
| `RONDELEK_SIZE=640x760` | set the starting window size |
| `RONDELEK_PROFILE=<dir>` | open that child's screen |
| `RONDELEK_SESSION=<dir>` | jump straight into a session (the sound board) |
| `RONDELEK_SCREEN=newprofile` \| `editprofile` \| `calibrate` \| `games` \| `settings` | open that screen (`editprofile`, `calibrate` and `games` need `RONDELEK_PROFILE`) |
| `RONDELEK_CALIB_VOWEL=<n>` | with `calibrate`: open vowel *n*'s record screen |
| `RONDELEK_VIZ=<n>` | pick visualizer *n* (0 sound bars, 1 vowels, 2 off) |
| `RONDELEK_SHOT=<file.png>` | draw a few frames, save a screenshot, and quit |
| `RONDELEK_GAME_SCREEN=profiles` \| `select` | *(game)* open the who's-playing picker or the setup screen |
| `RONDELEK_GAME_FRAMES=<n>` / `RONDELEK_GAME_SHOT=<png>` | *(game)* quit after *n* frames / save a screenshot |

For example, to capture the sound board and quit:

```bash
RONDELEK_SESSION="$HOME/Library/Application Support/rondelek/profiles/<id>/sessions/<ts>" \
RONDELEK_SHOT=out.png cargo run --release
```

**Pictures without real children.** The library and settings live under the OS
data and config folders, which on Linux follow `XDG_DATA_HOME` and
`XDG_CONFIG_HOME`. Point both at a temporary folder and add a few made-up
profiles, and you get clean screenshots without anyone's real photos in them.
`AGENTS.md` shows the recipe.

## If the fans spin up on Linux (Wayland)

winit's Wayland backend can busy-wait between the compositor's frame
callbacks, which may keep one CPU core busy while the sound board animates. If
your laptop's fans start humming, run under XWayland instead, which waits
properly for vsync:

```sh
cargo run --release -- --x11
```

Still screens idle at a slow, gentle repaint on either backend, so this mostly
matters for the sound board and the voice calibration.
