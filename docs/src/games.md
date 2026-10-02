# Voice games

This is the fun part. The voice games are where the vowel engine stops being
maths and starts being play: a child says "a" and the hero jumps.

The games live in their own crate, `game/` (`rondelek-game`), built on
[raylib](https://www.raylib.com/), and they run as a **separate process** next
to the app. On this page you'll see how a game is put together, how Vowel
Runner is drawn, where its art comes from, and how its shaders are tuned. If
you'd like to write a game of your own, there's a section on that too.

## The frame around every game

Every game sits inside the same frame. Here's what it gives you:

- **A process of its own.** The app releases the microphone, starts
  `rondelek-game <id> --profile <dir>` from the folder its own exe lives in, and
  stops repainting until the game exits ([Architecture](architecture.md),
  `docs/PERF.md`). raylib and eframe can't share one exe on Windows, which is
  why `game` must never depend on `app` (see `docs/WORKSPACE_SPLIT.md`).
- **It runs standalone too.** With no id, the game binary starts the first entry
  of `rondelek_core::games::GAMES`. With no `--profile`, it first shows the
  "who's playing?" picker (`game/src/profile_picker.rs`).
- **The loop** (`game::run` in `game/src/lib.rs`). First comes the text-free
  control screen (`select_controls`), where a grown-up assigns a vowel to each
  move, toggles obstacle kinds on and off, sets the turtle↔rabbit reaction
  slider and presses ▶. Then a 60 fps loop polls the voice and hands the game a
  `VoiceInput` every frame.
- **`VoiceGame`** is the trait a game implements: `init` (GPU resources, given
  the raylib handle), `update(&VoiceInput, dt)` (the gameplay, kept
  **raylib-free** so it can be unit-tested headless) and `draw`.
- **Voice** (`game/src/voice.rs`, `VoiceBridge`) is the capture stream plus the
  child's calibrated `VowelDetector`, gated the same way as the sampler's vowel
  visualizer. `VoiceInput` carries `held` (the vowel being voiced right now),
  `onset` (its first frame), per-vowel `scores` and the raw `level`. The keys
  **A E I O U Y** can stand in for a voice, which is handy when you're testing
  at a quiet desk, and it's also how headless recordings drive the game.

### The entrance screens: arcade style

Before play starts there are two screens: "who's playing?"
(`profile_picker.rs`) and the control screen (`select_controls` in `lib.rs`).
They share the 8-bit "chrome" of the README's pictures (and, these days, of the
app's shell too). It comes from `game/src/arcade.rs`, which draws using the
shared data in `rondelek_core::arcade`:

- **Palette**: `genarcade`'s colours (navy night, orange, butter, pink, cyan,
  cream) plus vivid card colours, `arcade::TILE_COLORS`. A child keeps the
  same colour *family* they have in the app (same order, via `stable_pick`),
  just a little brighter.
- **Backdrop**: `sky` (navy plus a starfield whose stars twinkle slowly, each on
  its own cycle) and `stripes` (the 80s bands along both edges, drawn last on
  the scrolling picker so they frame it).
- **Shapes**: everything sits on a grid of `p` real pixels per 8-bit pixel
  (`pixel_unit`), so it stays crisp at any window size. There are `notched`
  rectangles (corner pixels cut off), `panel` (an ink outline plus a light/dark
  bevel), and `highlight` (a coloured line and an ink gap, like the cabinet's
  screen well) to say "this one".
- **Pixel icons** are small bitmaps written as `#` rows: ▲ ▼ ★ ▶, a bold "?",
  check, mic, heart, cursor, turtle, rabbit. The **vowels** get their own bold
  lowercase bitmaps (`vowel_glyph`) on a shared baseline. The pixel font's
  lowercase reads ambiguously when blown up (its "a" looks like a "d"), and
  these are exactly the letters a child is learning, so they have to be clear.
- **Text** (only the children's names) uses **Tiny5** (OFL,
  `assets/fonts/Tiny5-Regular.ttf`). Its glyphs sit on a 9-row grid, so it's
  loaded at size 9 (one texel per font pixel) with nearest-neighbour filtering
  and drawn at whole multiples. It covers Latin with every shipped accent, plus
  Cyrillic, which fixed the picker's old missing-glyph Ukrainian names.

These screens stay **text-free**, so they need no translations: a pixel "?"
between two stars means "who's playing?", and ▶ means start. The only motion is
the slow twinkle and a one-pixel bob of the focus cursor.

## Adding a game

Got an idea for a new game? Here's the checklist:

1. Write a new file in `game/src/` with a type that implements `VoiceGame`.
2. Add an arm for its id to the `match id` in `game::run`.
3. Register it in `rondelek_core::games::GAMES` with a `GameInfo`: its `id`, plus
   the i18n keys for its name and its one-line tagline (`name_key`,
   `tagline_key`).
4. Add those keys to every locale, all seven of them. The i18n parity test will
   let you know if you miss one.

Some things come for free. Every game gets the "who's playing?" picker
(`profile_picker::choose_child`) when it's launched without a profile, the voice
plumbing, and the 60 fps loop, all through `game::run`.

One thing doesn't come for free yet: the pre-game selection screen is still
Runner-specific. `select_controls` returns the Runner's moves and its obstacle
toggles, so it will need generalising before a second game can use it.

## Vowel Runner

The hero runs all by themselves; the child only has to speak. One vowel jumps
(a second one in mid-air is a "ninja" double jump with a salto), holding another
ducks, and a third shoots a spinning star. There are four obstacle kinds: a low
block (jump it), a bar (duck under it), a wall (shoot it) and a tall pillar
(double jump it). **Nothing ever fails**: an obstacle you bump into just bounces
away, and every obstacle you clear earns a point.

Gameplay runs in a 1280×720 logical space (`update`), and drawing maps that to
world units (100 logical px = 1 unit).

### The scene, back to front

The camera is fixed in perspective at `(0.9, 2.2, 12.5)`, looking at
`(0, 1, 0)`, with a fovy of 45°. It never moves. Instead, each layer scrolls by
its own hand-tuned fraction of the distance travelled, so the world seems to
glide past while the camera stays put.

| Layer | Where | How it's drawn |
|-------|-------|----------------|
| Sky | behind everything | 2D gradient `SKY_TOP` → `SKY_LOW` (a clear blue day) |
| Clouds | z −14, parallax 0.10 | `cloud.glb` (flat-draw) through **Lam::pula** glass, gently floating and breathing |
| Haze | — | the sky gradient again at `CLOUD_HAZE` opacity, so the clouds sink into the sky |
| Hills | z −9, parallax 0.25 | stepped green cubes under the fog shader (fog colour = horizon blue) |
| Bushes | z −4, parallax 0.55 | `bush.glb` (flat-draw), plain |
| Meadow & bank | z 0, parallax 1.0 | grass caps over a strip of earth |
| **Water** | z 1.5 → 8.5 | little glass bricks rising and falling on a swell, through **Lam::pula** like the clouds; scrolls with the ground |
| Obstacles, stars | z 0 | procedural cubes |
| Hero | z 0 | the blocky brick hero under the toon shader (**permanent by design**) |
| Score sun | 3 units in front of the camera | `sun.glb` through its own Lam::pula, the count on its face |
| HUD | 2D | vowel signs over obstacles, point sparks, the vowel meter |

### Design rules for the scene

The child's attention belongs to the hero and the obstacles. Everything else is
there to be pleasant and then get out of the way. So:

- **Scenery stays calm.** Background motion is small and slow. The clouds float
  by a few pixels and breathe by 1.5%, and they don't tilt: a sway was tried,
  and it made heads spin and the glass glints flicker. The water was slowed and
  softened for the same reason, and its bricks only rise and fall: they slide
  along with the ground, and no brick ever tilts.
- **Scenery recedes.** The haze pass is atmospheric perspective: distant things
  take on the sky's colour. It's simply the sky gradient drawn a second time,
  see-through, so it's invisible over bare sky and softening over a cloud.
- **Reward, don't punish.** Points spin the sun; bumps just bounce the obstacle
  away.
- **Voice is welcome in any form.** The hero's mouth opens with any sound, and
  the water swells and sparkles more while the child is making sound (a
  smoothed level, `voice_glow`: quick up, slow down). Babbling counts.

### The score sun and its count

The score is Greg's pixel sun. It's a real 3D prop, floating `SUN_DIST` in front
of the camera along the ray through its HUD spot, and turned to face the camera.
Each point adds a full turn to a spring's target (`SunCoin`), so the sun whirls
round, swings a little past and settles. Quick points add more turns rather than
restarting the spin. A jelly scale pop and a ring of "+" sparks come along for
the ride.

The count is printed **on the sun's face**, like the value on a coin. It uses
raylib's pixel font, cream with a warm outline, drawn in the 3D pass with the
sun's matrix on rlgl's stack, so the number turns with the sun.
`SunCoin::shown` swaps the number while the face is turned away, so the sun
comes back round already showing the new count (one reveal per turn, tested).

Two raylib details that are easy to trip over:

- **raylib-rs 6's `rl_mult_matrixf` passes the matrix transposed.** It casts the
  `Matrix` struct (fields laid out m0, m4, m8, m12, … row by row) straight to
  the column-major `float[16]` that `rlMultMatrixf` reads, which loses the
  translation. Pass `m.transpose()`.
- **Text in 3D is drawn with the depth test off**, fenced by
  `rlDrawRenderBatchActive` on both sides. Otherwise the see-through corners of
  the glyph quads hide the light fill behind its own outline.

## Where the art comes from

- **flat-draw / Flatty** (`~/zed-projects/flat-draw`, our Go + raylib pixel
  editor; please treat that repo as read-only from here) draws 2D pixel art and
  exports extruded 3D layers as `.glb`, with the texture atlas inside.
- **`greg/`** at the repo root is Greg's local drop folder for new artwork. It's
  gitignored. When a file gets used, it's **moved** (not copied) into its place,
  e.g. `assets/models/<english-name>.glb`, so `greg/` only ever holds what's
  still waiting its turn.
- **`game/src/models.rs`** (`FlatModel`) embeds each GLB in the binary and loads
  it through a temp file (raylib has no load-from-memory for models). It merges
  the per-layer meshes into one mesh (one draw call per prop), and records the
  bounding box and the **pixels-per-unit**. Newer exports carry no
  `meshes.json`, so pixels-per-unit is recovered from the geometry: the smallest
  step between vertex coordinates is one drawn pixel. `FlatModel::lattice`
  rebuilds the model → pixel-lattice map that flat-draw's `brickMatrix` gives
  its brick shaders.
- **Shaders go in per draw** (`FlatModel::draw_shaded`,
  `models::draw_mesh_with`). The material is *copied* with the shader swapped
  in, never changed in place. A shader set on a model's material gets freed by
  raylib when the model unloads, which would be a double free with our own
  `Shader`.

The current models are `cloud.glb` (a faceless cloud), `bush.glb`, `sun.glb`,
and `cloud9_rain.glb` (unused, but kept).

## Shaders

| File | Used for |
|------|----------|
| `base.vs` + `toon.fs` | the hero: banded comic shading |
| `fog.fs` | the far hills, fogged toward the horizon blue |
| `flatdraw_model.vs` + `lampula.fs` | **Lam::pula**, copied 1:1 from flat-draw: clouds and the sun |
| `brick_water.vs` + `lampula.fs` | the brick water: our vertex stage in front of Lam::pula, unchanged |
| `sun.vs` + `sun.fs` | the sun's fallback if Lam::pula won't compile |
| `water.vs` + `water.fs` | the toon water (kept, see below) |

### Lam::pula, 1:1 from flat-draw

Lam::pula is flat-draw's tinted-glass shader. It renders the model as one piece
of glass in the artwork's own colours, lit by three warm lamps standing round
it, with highlights, a reflected "room" at the silhouette, and star-shaped
glints at the brick corners.

`lampula.fs` is flat-draw's `lampFS` and `flatdraw_model.vs` is its `modelVS`,
copied **verbatim**. Each file names the flat-draw commit it came from, and a
test guards the copy. So please don't edit them to change the look; tune the
parameters instead.

`game/src/lampula.rs` feeds in the same uniforms flat-draw's `Mesh.lampula`
does: the eye, the clock, the instance's bounding box (the lamps stand round
it) and `uBrick`, the world → pixel-lattice map. `Lampula::draw` takes any
`FlatModel` under any transform, so anything drawn in flat-draw can go through
the glass. `Lampula::draw_mesh` takes any other mesh, given its brick lattice
and box, and `Lampula::load_with_vs` puts a vertex stage of your own in front
of the unchanged `lampula.fs` (the brick water does both). Each use gets its
own instance and look:

- `cloud_glass()`: flat-draw's defaults, except the "room below" is the horizon
  blue (flat-draw's dark floor made the clouds muddy).
- `sun_glass()`: the same for now, kept separate so the sun can be tuned on its
  own.
- `water_glass()`: the clouds' glass with only the light changed. Gold lamps
  turned the blue water murky green and white ones washed it pale, so its
  lamps are a clear sky blue; the room it reflects is the sky above and a deep
  sea below.

### The water

The water is made of little glass bricks, like the clouds above it
(`game/src/brick_water.rs`). One mesh holds a column of bricks for every
`WATER_CELL` (0.25 units, about a cloud brick on screen) from the bank's face
to just past the bottom of the screen: about 3,600 columns, one draw call.
Only the faces that can ever be seen are built (tops, fronts, sides), and each
column runs four bricks deep, so a swell never lifts its bottom into view.

`brick_water.vs` lifts every column on a gentle swell rolling toward the bank
(the toon water's swell, unchanged), so a column a little higher than the one
in front shows its side, and the waves read as steps of bricks. It also picks
each brick's colour: toon steps from shallow at the bank to deep further out,
a little jitter per brick so the steps come out dithered like pixel art, and
white foam bricks lapping at the bank and riding the crests. The colours sit
in a 2×2 palette texture (shallow, deep / foam, foam) that the vertex stage
points into, since the texture is the only colour `lampula.fs` reads.

The light is **Lam::pula's**, through the very same `lampula.fs`. Two tricks
make a shader written for still drawings work on moving water:

- `fragWorld` is each brick's *resting* place. Lam::pula finds the brick edges
  and corners by mapping `fragWorld` onto the lattice, so a brick that has
  bobbed up keeps its own edges and corner glints. (The light hardly notices:
  the swell moves a brick by a few hundredths of a unit.)
- The bricks slide along with the ground by less than one brick, then hop back
  by exactly one as the pattern moves on by one. The hop can't be seen, and
  the lattice (`uBrick`) moves with the mesh, so it never leaves the bricks.

While the child makes sound, the swell grows (`voiceSwell`) and the corner
glints get brighter (`voiceSpark` scales Lam::pula's `sparkGain`).

It scrolls with the ground, and **every x-frequency is a whole number of turns
per `PERIOD`**, the distance the scroll wraps on. That way the surface never
jumps at the wrap (a test parses every `kx(n)` to make sure), and `WATER_CELL`
must fit `PERIOD` a whole number of times (tested too).

#### The toon water, kept

The first water, a smooth cartoon sea, is kept whole and tested
(`game/src/water.rs`, `water.vs` + `water.fs`). Launch the game with
`RONDELEK_WATER_STYLE=toon` to bring it back, or set `WATER_STYLE` in
`runner.rs` to make it the default again.

It's a `GenMeshPlane` grid with the same swell, and `water.fs` draws toon
depth steps (turquoise → blue), a slowly shifting Voronoi web of light-lines,
crest bands, wobbly foam and a row of bubbles at the shore, "+" twinkles, and
goldfish gliding underneath. Its defaults were calmed down on 2026-09-27, and
each changed row in `WaterParams` notes its previous value.

### Tuning: tables, not constants

`game/src/shader_params.rs` borrows flat-draw's way of doing things: a shader's
knobs are **one table** (`shader_params!`), with one row per knob giving the
Rust field, flat-draw's key, the default and the range. Everything else comes
from that row: the uniform name (`vibrance` → `uVibrance`), the per-frame
upload, and `set(key, value)` in flat-draw's config format (floats, toggles as
0/1, colours as `key.r/.g/.b`).

Adding a knob is one row plus one `uniform`. Each shader has a test that reads
its GLSL and fails on a row without a uniform, or on a uniform nothing sets
(flat-draw's `TestEveryExposedParamHasItsUniform`, checked both ways round).
`LampulaParams`' defaults are flat-draw's `Def` column.

None of this is exposed in the game's UI. When you want to experiment, you have
two options:

- edit a default in the table (or a preset such as `cloud_glass()`) and rebuild;
- or, without rebuilding, point an env var at a JSON file and restart the game:
  - `RONDELEK_LAMPULA=<file.json>`: flat-draw's own `~/.config/flatty/config.json`
    works as-is (its `shaderParams.lampula` is used), so a look you tuned live in
    flat-draw's Shaders pane carries straight over. So does a
    `saved-ideas/lampula-*.json` snapshot, or a plain
    `{ "exposure": 2.4, "lamp1On": 0, "ground": "#9FD4FF" }`.
  - `RONDELEK_WATER=<file.json>` tunes whichever water is showing. The brick
    water takes keys of `BrickWaterParams` and of its glass (any Lam::pula
    key), e.g. `{ "swellAmp": 0.1, "deep": "#1E5AC8", "lamp0": "#FFFFFF" }`;
    the toon water takes `WaterParams`, e.g. `{ "cellSpeed": 0.9, "fishOn": 0 }`.
    The water ignores `RONDELEK_LAMPULA`, so retuning the clouds' glass leaves
    its lamps alone.
  - `RONDELEK_WATER_STYLE=toon` (or `bricks`) picks the water.

Unknown keys and bad values are reported on stderr and skipped. A typo costs you
one value, never the whole game.

## Testing and recording

- `cargo test -p rondelek-game` covers the gameplay (jumps, ducks, shooting,
  the double jump's reach), the sun's spring and reveal, the voice glow, the
  tuning tables and their shaders, and the pixel-lattice recovery.
- For headless runs, `RONDELEK_GAME_FRAMES=<n>` skips the pre-game screens and
  quits after n frames, `RONDELEK_GAME_SHOT=<png>` saves a screenshot, and
  `RONDELEK_GAME_SCREEN=profiles|select` shows those screens instead.
- `docs/images/arcade/record-runner.sh` records the README's gameplay GIF on a
  virtual display (Xvfb, plus xdotool pressing vowel keys, plus ffmpeg).
