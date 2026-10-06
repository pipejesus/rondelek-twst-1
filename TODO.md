# Rondelek — TODO

## Avatars and camera
- **AVIF avatar input.** PNG, JPG and WebP only (pure Rust, portable). AVIF
  needs a native decoder (dav1d): worth it only if there's demand and the
  Windows build stays simple.
- Real artwork for the character avatars (`assets/avatars/` holds placeholders).

## Internationalization
- **The remaining European languages.** The picker lists about 36; en, pl, de,
  fr, es, it and uk are translated, the rest fall back to English. Add
  `assets/i18n/<code>.json` (its keys must match `en.json`).
- Local-time session-folder names, if grown-ups want them (needs a date/time
  dependency).

## Sessions and profiles
- **Session search** across a profile (the timestamped folders and manifest
  uids support it).
- **Notes on profiles and sessions**, keyed by their uids.
- Game profile picker: a "back" key from the control screen to the picker, and
  a friendly "add a child in Rondelek first" message when the library is empty
  (needs translated text in the game, which has none).
- UI tests beyond the settings page (Home and Hub navigation) with
  `egui_kittest`.

## Build and release
- CI: move `actions/checkout` and `softprops/action-gh-release` to versions that
  run on Node 24; recheck the apt package list when `ubuntu-latest` becomes
  Ubuntu 26.
- The macOS release targets in `release.yml` (commented out).

## Voice games
- If Lam::pula stops pleasing on the clouds and the sun, write a cloud shader of
  its own rather than editing `lampula.fs` (a verbatim copy); tune through
  `cloud_glass()` / `sun_glass()` or `RONDELEK_LAMPULA` first.
- Idea: a dev toggle that draws still-placeholder art (the hero, …) in
  greyscale, so the real art stands out.
- The kid avatars in the game's picker could be more vivid, to match the
  arcade cards.
- The game's entrance screens are text-free; words there ("PLAYER SELECT") need
  an i18n for the game first.
- An alternative tablet look: hand-drawn 3D vowel letters as flat-draw props,
  split from one GLB by mesh name. The letters must be exported side by side:
  the exporter culls faces between overlapping layers.
- Drawn art for the hero, the fish in the water and nearer planes (birds flying
  by); the meadow, the obstacles and the far planes stay code-built bricks.
- The sky may want tuning around the far planes (`MOUNTAIN_*`, `PALM_*`,
  `JUNGLE_*` in `runner.rs`).
- A bumped or shot obstacle flies up and then falls through the meadow
  (`fly_y`); it could fly off the screen or break into bricks instead.
- The pre-game obstacle toggles (`draw_obstacle_icon`) draw a block, a bar, a
  wall and a pillar, not the log, the fallen tree, the wall and the bamboo.
- Time the game on a laptop with integrated graphics (Intel or AMD, at 1080p)
  with `RONDELEK_GAME_STATS`, as in the book's Performance section.
- Upstream: report raylib-rs 6's `rl_mult_matrixf` passing the matrix
  transposed (it casts the row-ordered `Matrix` to the column-major `float[16]`
  rlgl expects); worked around with `transpose()`.

## App look
- A cartoon-toy sampler skin, a third look next to Arcade and Classic.
- An in-app browser for online skins (`skins/index.json` on GitHub).
- The shell's vector icons (`shell::Icon`) as pixel bitmaps.
- Per-pad labels ("ma", "pa") on the pads instead of the key letter.
