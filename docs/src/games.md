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
  `VoiceInput` every frame. The control screen listens only once the press
  that opened it is let go (`InputGate`): a screen returns mid-frame, before
  raylib polls input again, so the picker's Enter or click would otherwise
  land on the control screen too, starting the game before anyone saw it.
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

The kinds are dealt from a shuffled bag (`next_kind`): each round holds every
kind the grown-up switched on once, in a fresh order, and a new round never
starts with the kind just dealt. So every vowel gets its turn often and evenly;
a plain random pick once left the first wall 18 obstacles in. Each game seeds
its generator from the clock, so no two games run the same course (the tests
keep a fixed seed).

A point needs the move the obstacle asks for, so no vowel can be skipped
(`Kind::earns_a_star`). Leaping over a bar instead of ducking, or
double-jumping over a wall instead of shooting it, gets the hero past
untouched but earns nothing. While an obstacle and the hero overlap along x,
the obstacle notes whether the hero was ever in the air or ever ducking, and
that decides it when it has gone by.

### Ledges and little suns

Now and then a **ledge pattern** takes an obstacle's turn (`LEDGE_CHANCE`,
never twice running, so a ledge and an obstacle never share a stretch): a long
low ledge, a high one, or a staircase from a low ledge up to a high one. Over
each ledge float **little suns**, the score sun small and turning slowly,
each one a point when the hero touches it. Unlike a cleared obstacle, a little
sun doesn't speed the game up.

Ledges are one-way, like Mario's: the hero jumps up through one from below and
lands on it only on the way down. `floor_below` is the whole trick. It's the
highest surface under the hero's feet that the feet were already above
(the ground, or a ledge the hero overlaps along x), and the hero lands on it
only while falling. A ledge that slides out from under the feet drops the
hero back down. The low ledge (`LEDGE_LOW`, 180 px) takes a single jump (apex
about 238 px) with room to spare and floats clear of the hero's head; the high
one (`LEDGE_HIGH`, 300 px) needs the double jump, or a hop from the low ledge
of a staircase. A test holds these numbers to each other.

Gameplay runs in a 1280×720 logical space (`update`), and drawing maps that to
world units (100 logical px = 1 unit).

### The scene, back to front

The camera rests in perspective at `(0.9, 2.2, 12.5)` (`CAMERA_AT`), looking
at `(0, 1, 0)`, with a fovy of 45°. It never moves sideways. Instead, each layer
scrolls by its own hand-tuned fraction of the distance travelled, so the world
seems to glide past while the camera stays put.

It does follow the hero up and down, and it aims ahead rather than chasing.
While the hero rises, it aims at where this jump will top out (the physics
knows at take-off); once they fall, or stand, at where they'll come to rest:
the ground, or a ledge. It rises by `CAM_FOLLOW` (about a third) of that
height past a small dead zone (`CAM_DEAD`, so a little hop leaves it still),
never more than `CAM_LIFT_MAX`, on a critically damped spring that never
overshoots: up in step with the jump (`CAM_RATE_UP`), and back down unhurried
(`CAM_RATE_DOWN`; a quicker return snapped back too hard). So it sets off with
the take-off, turns at the top, and is on its way down as the hero lands. The
first version chased the hero's height on one slow spring instead: it lagged a
quarter of a second behind every jump and was still rising as the hero fell,
so it was never in step, and the game felt like a ship at sea. Both where the camera stands and where it looks move together, so
nothing tilts. The scene is truly 3D, so rising shows it from a little higher,
and the near things slide down further than the far planes: vertical
parallax, for free. A test holds it in step with the jump (topping out within
0.15 s of the hero), smooth, and easing back after landing: not snapped down,
most of the way within a second, at rest within three.

### Coming and going out of sight

Nothing pops up or vanishes on screen. `game/src/view.rs` describes the camera
as plain geometry (`Eye`): where the picture's left and right edges fall at a
given depth and height (they lean a little, since the camera looks down and in
from the side), for whatever shape the window has and wherever the camera has
risen to. Everything the runner adds takes up a box in the world (`Bounds`),
with all it carries: an obstacle's tablet (hopping and bobbing at its highest),
the little suns over a ledge, a bullet's whirling cubes (`obstacle_bounds`,
`ledge_bounds`, …). A new thing is placed with all of its box just past the
right edge (`Eye::entry`, plus `view::MARGIN`); it's dropped only once all of
it is past the left edge (`Eye::gone_left`), and a bullet once it's past the
right (`Eye::gone_right`). The far planes' tiles and the cloud lanes are laid
across the same edges (`Eye::span`), so a wide window never sees their ends.
(Things used to come in at a fixed logical x, 1400, which a 16:9 picture
already shows: tests now hold every kind of thing to entering out of sight at
4:3 to 21:9.) A new game gets this by giving its things boxes and asking its
`Eye`.

| Layer | Where | How it's drawn |
|-------|-------|----------------|
| Sky | behind everything | 2D gradient `SKY_TOP` → `SKY_LOW` (a clear blue day) |
| Clouds | z −14, parallax 0.10 | `cloud.glb` (flat-draw) through **Lam::pula** glass, gently floating and breathing |
| Haze | — | the sky gradient again, see-through, in front of each far plane (see below) |
| **Mountains** | z −12, parallax 0.16 | code-built **bricks** (`props::mountains`): a blue-violet range with snowy peaks rising high over the jungle, some in front of the clouds |
| Valley mist | — | a white band rising from the mountains' feet |
| **Palms** | z −10.25, parallax 0.23 | code-built **bricks** (`props::GROVE`): a grove of palms, every one its own, their crowns in mist above the jungle |
| **Jungle** | z −5, parallax 0.36 | code-built **bricks** in the round (`props::jungle`): a rainforest of trees, palms, bushes, big leaves and ferns |
| Valley mist | — | a thin band at the jungle's feet, out of which the meadow comes |
| Meadow & bank | z 0, parallax 1.0 | code-built **bricks** (`props::ground`): a grass top over layered earth, tile after tile, through Lam::pula |
| **Water** | z 1.5 → 8.5 | little glass bricks rising and falling on a swell, through **Lam::pula** like the clouds; scrolls with the ground |
| Obstacles | z 0 | code-built **bricks** (`props.rs`): a toy brick, a bridge, a brick wall, a candy pillar, through Lam::pula |
| **Vowel tablets** | z −0.75 (floating), or on a tall obstacle's front | code-built **bricks** (`props::tablet`): white stone with the vowel carved in, through their own white-lit Lam::pula |
| Ledges, little suns | z 0 | floating strips of meadow bricks (`props::ledge`); `sun.glb`, small, through the sun's Lam::pula |
| Stars (bullets) | z 0 | procedural cubes |
| Hero | z 0 | the blocky brick hero under the toon shader (**permanent by design**) |
| Score sun | 3 units in front of the camera | `sun.glb` through its own Lam::pula, the count on its face |
| HUD | 2D | point sparks, the vowel meter (and flat vowel signs, only if the tablets fail to build) |

### Design rules for the scene

The child's attention belongs to the hero and the obstacles. Everything else is
there to be pleasant and then get out of the way. So:

- **Scenery stays calm.** Background motion is small and slow. The clouds float
  by a few pixels and breathe by 1.5%, and they don't tilt: a sway was tried,
  and it made heads spin and the glass glints flicker. The water was slowed and
  softened for the same reason, and its bricks only rise and fall: they slide
  along with the ground, and no brick ever tilts.
- **Scenery recedes.** The haze passes are atmospheric perspective: distant
  things take on the sky's colour. Each is simply the sky gradient drawn once
  more, see-through, so it's invisible over bare sky and softening over
  everything drawn before it.
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

The current models are `cloud.glb` (a faceless cloud) and `sun.glb`.
`bush.glb` (unused since the jungle plane took the bushes' place) and
`cloud9_rain.glb` (unused, kept for a possible rain variant) stay in
`assets/models/`.

## Shaders

| File | Used for |
|------|----------|
| `base.vs` + `toon.fs` | the hero: banded comic shading |
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
and where its lamps stand (a box fixed in the world, so something that scrolls
slides under still lamps), and `Lampula::load_with_vs` puts a vertex stage of
your own in front of the unchanged `lampula.fs` (the brick water does both).
Each use gets its own instance and look:

- `cloud_glass()`: flat-draw's defaults, except the "room below" is the horizon
  blue (flat-draw's dark floor made the clouds muddy).
- `sun_glass()`: the same for now, kept separate so the sun can be tuned on its
  own.
- `water_glass()`: the clouds' glass with only the light changed. Gold lamps
  turned the blue water murky green and white ones washed it pale, so its
  lamps are a clear sky blue; the room it reflects is the sky above and a deep
  sea below.
- `world_glass()`: the meadow and the obstacles, solid and toned down (see
  below).

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

### The far planes: mountains, palms and jungle

Behind the meadow lie three far planes, 90s style: continuous bands, each at
its own depth and scrolled at its own rate, each hazed toward the sky by how
far away it is, so the farther a plane lies, the more it looks like the sky.
All are code-built bricks (`props::mountains`, `props::palm_grove`,
`props::jungle`), laid tile after tile like the meadow (`lay_tiles`), and lit
by `backdrop_glass()`: the meadow's glass without glints or highlights, so
nothing out there pulls the eye. The mountains and the palms are pixel-art
silhouettes extruded a brick or few deep, the way flat-draw turns a drawing
into a prop; the jungle, the nearest, is built in the round.

- **The mountains** (0.4-unit bricks, z −12): a paler back range with four big
  snowy peaks and smaller shoulders, behind a darker ridge of rolling foothills,
  each slope shaded on the side away from the sun. The slopes step two bricks
  along for one up, never steeper: sheer columns of bricks read as a city. The
  big peaks rise high into the clouds' band, well over the jungle's canopy, and
  the depth test puts them in front of a cloud now and then.
- **The palms** (`GROVE`, 0.25-unit bricks, z −10.25), a plane of their own
  sliding by at its own pace behind the jungle, which hides all but their
  crowns. `props::palms` makes every palm its own: short to tall, straight or
  leaning — the leaners by turns one way and the other, their trunks bending
  upright toward the crown like a coconut palm's — with 6 to 9 long fronds
  fanned out to both sides, rising and then drooping past level, leaflets
  along their outer halves, ringed trunks, coconuts under most crowns. The
  crowns rise above the camera, so the grove is built with every face (their
  undersides show). Its distance comes from the mist it stands in, not its
  colours: recoloured toward the mountains' blue, it stopped reading as palms.
  (A second, farther grove was retired when the jungle grew tall: it crowded
  the scene.)
- **The jungle** (0.25-unit bricks, z −5, 10 deep), the way a film's
  rainforest is, in rows from the back: a dark heart of leaf masses in deep
  shade (so between the trunks there's jungle, not sky); tall trees with pale
  trunks flaring into buttress roots, round canopies spilling into each other
  and lianas hanging in front; palms rising through the canopy, their long
  fronds hung with leaflets breaking the skyline; darker bushes between; and
  big-leaved plants and ferns crowding the floor at the front. The leaf masses
  are ellipsoids of bricks (`jungle_plants`, `shade_lobes`), each brick shaded
  by where it sits on its own mass (sunlit up and to the left, shaded below,
  darker toward the ground and the back) and darker still in the creases
  where masses meet, so each reads as a ball of leaves rather than a cut-out.
  Leaves and fronds are drawn brick by brick along arcs (`Jungle::leaf`,
  `Jungle::frond`). A tile is too leafy for one mesh, so it builds into
  several (see below); it stays light, though, about 58,000 vertices.

All start below the meadow's sightline (`MOUNTAIN_BASE`, the grove's `base`,
`JUNGLE_BASE`), so no floor ever shows under them. The draw order does the
rest:

1. the sky, then the clouds;
2. a haze pass, then the mountains;
3. a haze pass, then **valley mist** at the mountains' feet (`MOUNTAIN_MIST`: a
   white band, clear a little way up and thick at the foot);
4. the palms, then a haze pass and their mist (`PALM_MIST`), which stands
   high, up in their crowns: the tall jungle in front hides everything lower,
   so a mist at their feet would never be seen;
5. the jungle, then a last haze pass and a low, thin mist at the jungle's feet
   (`JUNGLE_MIST`), out of which the meadow comes.

The hazes are set as how far each plane ends up pulled toward the sky
(`CLOUD_HAZE`, `MOUNTAIN_HAZE`, `PALM_HAZE`, `JUNGLE_HAZE`),
and `haze_step` works out the
pass in front of each plane from them, since a plane behind also gets every
pass in front of it. The clouds' figure is the one they had before the planes
came, so they look just as they did.

### The meadow and the obstacles: bricks built in code

The ground and the obstacles aren't drawings: they're generated, in the same
brick style as the clouds and the water. `game/src/bricks.rs` is a small kit
for it. A `Grid` holds a palette colour per brick (0 = empty), and
`BrickModel::build` walls it in: one quad per brick face that borders an empty
cell, its texcoords pointing at its colour in a one-row palette texture (a
`PaletteMaterial`, shared with the brick water). That's the shape of a
flat-draw export, so Lam::pula lights it the same way, finding the brick edges
and corners through `BrickModel::lattice`. raylib's indices are 16-bit, so a
grid with more faces than one mesh holds (16383, four vertices each) builds
into several, drawn one after another (`Faces::meshes`). `Build` can leave out
the faces the camera never sees: the bottoms (`open_below`, for the ground)
and the backs (`open_behind`).

`game/src/props.rs` builds the runner's pieces from a hash of each brick's
place, never a random generator, so they look the same every game:

- **The meadow** (`ground`, 0.25-unit bricks, the water's size): a flat grass
  top in three greens, flecked with tufts, buttercups and daisies off the
  hero's lane, over layered earth with the odd stone, and grass hanging over
  the bank's front edge. One tile is 32 units long. Its last column meets its
  first (`Build::wrap_x`), so the runner lays it tile after tile without a
  seam, and leaves out the bottoms and backs, which are never seen. The grass
  is flat on purpose: bricks standing up out of it read as toys left lying
  about and pulled the eye off the obstacles.
- **The ledges** (`ledge`, meadow bricks): a floating strip of meadow, 12 or
  16 bricks long and two thick, its earth row a brick short at each end, with
  grass hanging over the front.
- **The obstacles** (0.125-unit bricks, half the meadow's, for a little detail
  at the hitboxes' size): a pink toy brick with studs (jump), a purple bridge
  on two posts (duck), a red brick wall with its mortar sunk in, so each brick
  catches the light (shoot), and a candy-striped pillar with a gold knob
  (double jump). The colours keep the vowel families: pink for the jump
  vowel's two, purple for the duck vowel, brick red for the shoot vowel. Tests
  hold each one to its hitbox in `runner.rs`, and the bridge to leaving room
  for a ducking hero.
- **The vowel tablets** (`tablet`, 0.1-unit bricks, 13 × 15 and 4 thick): a
  slab of white limestone, arched on top, pillowed front and back (the
  outermost ring of each set back a brick, so the edges read as worn round),
  a nick out of the rim's front here and there (the outline itself stays
  smooth: notches right through it made the sides look jagged), with
  the arcade's bold pixel vowel (`rondelek_core::arcade::vowel_glyph`, the one
  on the entrance screens) cut a brick deep into the face and its floor
  painted. The paint is the obstacle family's colour, deep (`TABLET_PAINT`).
  **The white is kept for the stone** (Greg's call): it must stand apart from
  the colourful world, so its shades are cool greys, never cream. Tests hold
  every letter to its carving, clear of the rim, on an otherwise plain face.

They all share one Lam::pula, `world_glass()`: the clouds' glass made solid,
under warm daylight lamps, and toned down. At the clouds' full strength the
coloured bricks went pastel: the grass is seen nearly edge-on, where the
reflected room and the highlights are strongest, and the exposure curve
flattened what was left. So it has less exposure, a faint room, softer
highlights and more vibrance. The meadow's lamps stand round a fixed stretch
of the world (`ground_lamps`), so it slides under still lamps; each obstacle
is lit by lamps round itself, so every one is lit alike.

The tablets have a glass of their own, `stone_glass()`: the same, made chalk.
Under the meadow's warm lamps the white came out cream, and with the shading
and the reflected room it went silver, so it has white lamps, light all round
(a high ambient, a soft wrap), little sheen and almost no room. Lit that
brightly, a paint comes out lighter than it goes in, hence the deep paints.

The runner builds each move's tablet once the grown-up has picked the vowels
(`Tablets::build`): one with the jump vowel, one with the duck vowel, one with
the shoot vowel. Over a low obstacle (block, bridge) the tablet floats a
little behind the hero's lane, so a jumping hero passes in front of it, not
through it, and bobs slow and small, each out of step. A tall one (wall,
pillar) wears it on its front, over its upper part. A bumped obstacle carries
its tablet off. **The tablets answer the voice**: say a tablet's vowel and it
hops (`tablet_hop`, one quick arc), and while the vowel is held its letter
lights up in the family colour at its most vivid (`TABLET_LIT_PAINT`, a second
build of the same grid). The stone stays white.

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
  - `RONDELEK_PROPS=<file.json>` tunes the meadow's and the obstacles' glass
    (any Lam::pula key), e.g. `{ "exposure": 1.4, "lamp0": "#FFFFFF" }`. Like
    the water, it ignores `RONDELEK_LAMPULA`.

Unknown keys and bad values are reported on stderr and skipped. A typo costs you
one value, never the whole game.

## Testing and recording

- `cargo test -p rondelek-game` covers the gameplay (jumps, ducks, shooting,
  the double jump's reach, which moves earn a star, landing on ledges and
  jumping up through them, the little suns), the sun's spring and
  reveal, the voice glow, the tuning tables and their shaders, the
  pixel-lattice recovery, and the brick props (faces turned outward, corners
  on the lattice, sizes against the hitboxes).
- For headless runs, `RONDELEK_GAME_FRAMES=<n>` skips the pre-game screens and
  quits after n frames, `RONDELEK_GAME_SHOT=<png>` saves a screenshot, and
  `RONDELEK_GAME_SCREEN=profiles|select` shows those screens instead.
- **Demo mode**: `RONDELEK_GAME_AUTOPLAY=1` lets the game play itself
  (`VoiceGame::autoplay`; Vowel Runner's is `Runner::pilot`), the way a child
  who knows every vowel would: it jumps blocks, ducks under bars, shoots walls,
  double-jumps pillars and hops up ledges for their suns, saying each vowel so
  the hero's mouth moves and the water dances. Its triggers are lead *times*,
  so it keeps up as the game speeds up. A test lets it play four minutes
  without a single bump, which also proves every obstacle can be cleared at
  every speed the game reaches.
- `docs/images/arcade/record-runner.sh [start] [length] [still]` records the
  README's gameplay GIF and its still on a virtual display (Xvfb, demo mode,
  ffmpeg). Every game deals a fresh course, so the full take is kept: look
  through it, then re-cut that same take with `TAKE=<runner-take.mp4>`
  instead of recording a different game. Earlier GIFs stay beside the current
  one for history (`runner-v0.3.gif`, `runner-v0.3.png`).
