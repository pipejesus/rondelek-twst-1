# Voice games

The voice games are where the vowel engine turns into play. They live in their
own crate, `game/` (`rondelek-game`), built on [raylib](https://www.raylib.com/),
and run as a **separate process** next to the app. This page covers how a game
is put together, how Vowel Runner is drawn, where its art comes from, and how
its shaders are tuned.

## The frame around every game

- **A process of its own.** The app starts `rondelek-game <id> --profile <dir>`
  from the folder its own exe is in, after releasing the microphone, and stops
  repainting until the game exits ([Architecture](architecture.md),
  `docs/PERF.md`). raylib and eframe can't share one exe on Windows, which is why
  `game` must never depend on `app` (see `docs/WORKSPACE_SPLIT.md`).
- **Standalone too.** With no id the game starts the first entry of
  `rondelek_core::games::GAMES`. With no `--profile` it first shows the "who's
  playing?" picker (`game/src/profile_picker.rs`).
- **The loop** (`game::run` in `game/src/lib.rs`): the text-free control screen
  (`select_controls`: the grown-up assigns a vowel to each move, toggles obstacle
  kinds, sets the turtle↔rabbit reaction slider, presses ▶), then a 60 fps loop
  that polls the voice and hands the game a `VoiceInput` each frame.
- **`VoiceGame`** is the trait a game implements: `init` (GPU resources, given
  the raylib handle), `update(&VoiceInput, dt)` (gameplay, **raylib-free** so it is
  unit-tested headless) and `draw`.
- **Voice** (`game/src/voice.rs`, `VoiceBridge`): the capture stream plus the
  child's calibrated `VowelDetector`, gated like the sampler's vowel visualizer.
  `VoiceInput` carries `held` (the vowel being voiced), `onset` (its first frame),
  per-vowel `scores` and the raw `level`. Keys **A E I O U Y** stand in for a voice
  (also how headless recordings drive the game).

### The entrance screens: arcade style

The two screens before play, "who's playing?" (`profile_picker.rs`) and the
control screen (`select_controls` in `lib.rs`), share the 8-bit "chrome" of the
README's pictures (and, since, the app's shell), from `game/src/arcade.rs`, which
draws with the shared data in `rondelek_core::arcade`:

- **Palette**: `genarcade`'s (navy night, orange, butter, pink, cyan, cream)
  plus vivid card colours, `arcade::TILE_COLORS`. A child keeps their colour
  *family* from the app (same order, via `stable_pick`), just brighter.
- **Backdrop**: `sky` (navy plus a starfield whose stars twinkle slowly, each
  on its own cycle) and `stripes` (the 80s bands along both edges, drawn last
  on the scrolling picker so they frame it).
- **Shapes**: everything sits on a grid of `p` real pixels per 8-bit pixel
  (`pixel_unit`), so it stays crisp at any window size. `notched` rectangles
  (corner pixels cut), `panel` (ink outline plus light/dark bevel), and
  `highlight` (a coloured line and an ink gap, the cabinet's screen well, for
  "this one").
- **Pixel icons** are small bitmaps (`#` rows): ▲ ▼ ★ ▶, a bold "?", check, mic,
  heart, cursor, turtle, rabbit. The **vowels** have their own bold lowercase
  bitmaps (`vowel_glyph`) on a shared baseline, because the pixel font's
  lowercase reads ambiguously when blown up ("a" looks like "d"), and these
  are the letters a child is learning.
- **Text** (only the children's names) uses **Tiny5** (OFL,
  `assets/fonts/Tiny5-Regular.ttf`). Its glyphs sit on a 9-row grid, so it is
  loaded at size 9 (one texel per font pixel) with nearest-neighbour filtering
  and drawn at whole multiples. It covers Latin with every shipped accent and
  Cyrillic, which fixed the picker's old missing-glyph Ukrainian names.

The screens stay **text-free** (so no translations are needed): a pixel "?"
between two stars means "who's playing?", ▶ means start. Motion is limited to
the slow twinkle and a one-pixel bob of the focus cursor.

Adding a game: a new file implementing `VoiceGame`, an arm in `game::run`'s
`match id`, an entry in `GAMES`, and its name key in all seven locales (the i18n
parity test enforces it).

## Vowel Runner

The hero runs on their own; the child only speaks. One vowel jumps (a second one
in the air is a "ninja" double jump with a salto), holding another ducks, a third
shoots a spinning star. Four obstacle kinds: a low block (jump), a bar (duck), a
wall (shoot it) and a tall pillar (double jump). **Nothing ever fails**: an
obstacle you hit just bounces away, and every obstacle you clear earns a point.
Gameplay runs in a 1280×720 logical space (`update`), and drawing maps it to world
units (100 logical px = 1 unit).

### The scene, back to front

A fixed perspective camera (at `(0.9, 2.2, 12.5)`, looking at `(0, 1, 0)`, fovy
45°). Layers scroll by hand-tuned factors of the distance travelled, so the world
reads as moving while the camera stays put.

| Layer | Where | How it's drawn |
|-------|-------|----------------|
| Sky | behind everything | 2D gradient `SKY_TOP` → `SKY_LOW` (a clear blue day) |
| Clouds | z −14, parallax 0.10 | `cloud.glb` (flat-draw) through **Lam::pula** glass, gently floating and breathing |
| Haze | — | the sky gradient again at `CLOUD_HAZE` opacity, so the clouds sink into the sky |
| Hills | z −9, parallax 0.25 | stepped green cubes under the fog shader (fog colour = horizon blue) |
| Bushes | z −4, parallax 0.55 | `bush.glb` (flat-draw), plain |
| Meadow & bank | z 0, parallax 1.0 | grass caps over a strip of earth |
| **Water** | z 1.5 → 12 | a grid mesh through `water.vs`/`water.fs`, scrolling with the ground |
| Obstacles, stars | z 0 | procedural cubes |
| Hero | z 0 | the blocky brick hero under the toon shader (**permanent by design**) |
| Score sun | 3 units in front of the camera | `sun.glb` through its own Lam::pula, the count on its face |
| HUD | 2D | vowel signs over obstacles, point sparks, the vowel meter |

### Design rules for the scene

The child's attention belongs to the hero and the obstacles. So:

- **Scenery stays calm.** Background motion is small and slow: the clouds float by
  a few pixels, breathe by 1.5%, and do not tilt; a tested sway made heads spin
  and made the glass glints flicker. The water was slowed and softened for the
  same reason.
- **Scenery recedes.** The haze pass is atmospheric perspective: distant things
  take on the sky's colour. It is the sky gradient drawn a second time,
  see-through: invisible over bare sky, softening over a cloud.
- **Reward, don't punish.** Points spin the sun; bumps just bounce the obstacle.
- **Voice is welcome in any form.** The hero's mouth opens with any sound, and the
  water swells and twinkles more while the child makes sound (a smoothed level,
  `voice_glow`: quick up, slow down). Babbling counts.

### The score sun and its count

The score is Greg's pixel sun, a real 3D prop floating `SUN_DIST` in front of the
camera along the ray through its HUD spot, turned to face the camera. Each point
adds a full turn to a spring's target (`SunCoin`). The sun whirls round, swings a
little past and settles, and quick points add turns instead of restarting. A
jelly scale pop and a ring of "+" sparks ride along.

The count is printed **on the sun's face**, like a coin's value: raylib's pixel
font, cream with a warm outline, drawn in the 3D pass with the sun's matrix on
rlgl's stack, so it turns with the sun. `SunCoin::shown` swaps the number while
the face is turned away, so the sun comes back round showing the new count (one
reveal per turn, tested). Two raylib details:

- **raylib-rs 6's `rl_mult_matrixf` passes the matrix transposed.** It casts the
  `Matrix` struct (fields laid out m0, m4, m8, m12, … row by row) straight to the
  column-major `float[16]` that `rlMultMatrixf` reads, which loses the translation.
  Pass `m.transpose()`.
- **Text in 3D is drawn with the depth test off**, fenced by
  `rlDrawRenderBatchActive` on both sides. Otherwise the glyph quads' see-through
  corners hide the light fill behind its own outline.

## Where the art comes from

- **flat-draw / Flatty** (`~/zed-projects/flat-draw`, our Go + raylib pixel
  editor; treat that repo as read-only from here) draws 2D pixel art and exports
  extruded 3D layers as `.glb`, with the texture atlas inside.
- **`greg/`** at the repo root is Greg's local drop folder for new artwork. It is
  gitignored. When a file is used, it is **moved** (not copied) into its place,
  e.g. `assets/models/<english-name>.glb`, so `greg/` only holds what's waiting.
- **`game/src/models.rs`** (`FlatModel`) embeds each GLB in the binary, loads it
  through a temp file (raylib has no load-from-memory for models), merges its
  per-layer meshes into one mesh (one draw call per prop), and records its
  bounding box and **pixels-per-unit**. Newer exports carry no `meshes.json`, so
  pixels-per-unit is recovered from the geometry: the smallest step between
  vertex coordinates is one drawn pixel. `FlatModel::lattice` rebuilds the
  model → pixel-lattice map flat-draw's `brickMatrix` gives its brick shaders.
- **Shaders go in per draw** (`FlatModel::draw_shaded`, `models::draw_mesh_with`):
  the material is *copied* with the shader swapped in, never changed, because a
  shader set on a model's material is freed again by raylib when the model
  unloads (a double free with our own `Shader`).

Current models: `cloud.glb` (faceless cloud), `bush.glb`, `sun.glb`, and
`cloud9_rain.glb` (unused, kept).

## Shaders

| File | Used for |
|------|----------|
| `base.vs` + `toon.fs` | the hero: banded comic shading |
| `fog.fs` | the far hills, fogged toward the horizon blue |
| `flatdraw_model.vs` + `lampula.fs` | **Lam::pula**, copied 1:1 from flat-draw: clouds and the sun |
| `sun.vs` + `sun.fs` | the sun's fallback if Lam::pula won't compile |
| `water.vs` + `water.fs` | the sea |

### Lam::pula, 1:1 from flat-draw

flat-draw's tinted-glass shader: the model as one piece of glass in the artwork's
own colours, lit by three warm lamps standing round it, with highlights, a
reflected "room" at the silhouette, and star-shaped glints at brick corners.
`lampula.fs` is flat-draw's `lampFS` and `flatdraw_model.vs` its `modelVS`,
**verbatim**; each file names the flat-draw commit, and a test guards the copy.
Don't edit them to change the look; tune the parameters instead.

`game/src/lampula.rs` feeds the uniforms flat-draw's `Mesh.lampula` feeds: the
eye, the clock, the instance's bounding box (the lamps stand round it) and
`uBrick`, the world → pixel-lattice map. `Lampula::draw` takes any `FlatModel`
under any transform, so anything drawn in flat-draw can go through the glass.
Each use gets its own instance and look:

- `cloud_glass()`: flat-draw's defaults, but the "room below" is the horizon blue
  (flat-draw's dark floor turned the clouds muddy).
- `sun_glass()`: the same for now, kept separate so the sun can be tuned alone.

### The water

Our own shader. A `GenMeshPlane` grid lies in front of the bank; `water.vs` lifts
it with a swell rolling toward the bank (so the water laps the earth), and
`water.fs` draws toon depth steps (turquoise → blue), a slowly shifting Voronoi
web of light-lines, crest bands, wobbly foam and a row of bubbles at the shore,
"+" twinkles, and goldfish gliding underneath. It scrolls with the ground, and
**every x-frequency is a whole number of turns per `PERIOD`**, the distance the
scroll wraps on, so the surface never jumps at the wrap (a test parses every
`kx(n)`). The defaults were calmed on 2026-09-27; each changed row in
`WaterParams` notes its previous value.

### Tuning: tables, not constants

`game/src/shader_params.rs` carries flat-draw's arrangement over: a shader's knobs
are **one table** (`shader_params!`), one row per knob giving the Rust field,
flat-draw's key, the default and the range. From that row come the uniform name
(`vibrance` → `uVibrance`), the per-frame upload, and `set(key, value)` in
flat-draw's config format (floats, toggles as 0/1, colours as `key.r/.g/.b`).
Adding a knob is one row plus one `uniform`. Each shader has a test that reads its
GLSL and fails on a row without a uniform, or a uniform nothing sets (flat-draw's
`TestEveryExposedParamHasItsUniform`, both ways round). `LampulaParams`' defaults
are flat-draw's `Def` column.

Nothing is exposed in the game's UI. To experiment:

- edit a default in the table (or a preset such as `cloud_glass()`) and rebuild;
- or, without rebuilding, point an env var at a JSON file and restart the game:
  - `RONDELEK_LAMPULA=<file.json>`: flat-draw's own `~/.config/flatty/config.json`
    works as-is (its `shaderParams.lampula` is used), so a look tuned live in
    flat-draw's Shaders pane carries straight over. So does a
    `saved-ideas/lampula-*.json` snapshot, or a plain
    `{ "exposure": 2.4, "lamp1On": 0, "ground": "#9FD4FF" }`.
  - `RONDELEK_WATER=<file.json>`: e.g. `{ "cellSpeed": 0.9, "fishOn": 0 }`.

Unknown keys and bad values are reported on stderr and skipped. A typo costs one
value, never the game.

## Testing and recording

- `cargo test -p rondelek-game`: gameplay (jumps, ducks, shooting, the double
  jump's reach), the sun's spring and reveal, the voice glow, the tuning tables
  and their shaders, the pixel-lattice recovery.
- Headless runs: `RONDELEK_GAME_FRAMES=<n>` skips the pre-game screens and quits
  after n frames; `RONDELEK_GAME_SHOT=<png>` saves a screenshot;
  `RONDELEK_GAME_SCREEN=profiles|select` shows those screens instead.
- `docs/images/arcade/record-runner.sh` records the README's gameplay GIF on a
  virtual display (Xvfb + xdotool pressing vowel keys + ffmpeg).
