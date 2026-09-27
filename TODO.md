# Rondelek — TODO / deferred work

Tracked items intentionally left for later batches.

## Avatars / camera
- **AVIF avatar input.** Currently PNG/JPG/WebP only (pure-Rust, portable). AVIF
  decoding needs a native library (dav1d) — revisit if there's demand and the
  Windows build cost is acceptable.

## Internationalization
- **Remaining European languages.** Framework + picker already list ~36 languages;
  fully translated: en, pl, de, fr, es, it, uk. The rest fall back to English.
  Add locale files under `assets/i18n/<code>.json` (keys must match `en.json`).
- Consider local-time (vs UTC) session-folder timestamps if therapists want it
  (would add a date/time dependency).

## Sessions / profiles
- **Session search** across a profile (the timestamped folders + manifest UIDs are
  already in place to support it).
- **Notes attached to profiles/sessions** via their UIDs (uids already stored in
  `profile.json` / `session.json`).
- Real artwork for the character avatars (`assets/avatars/`, placeholders now).
- **Game profile picker can't draw Cyrillic names (bug).** It draws names with
  Space Grotesk, which has no Cyrillic glyphs (U+0400–04FF), so a Ukrainian
  name shows as missing-glyph boxes. raylib has no per-glyph font fallback: load
  a second TTF that has Cyrillic (e.g. the Ubuntu/Noto font egui already ships
  in `epaint_default_fonts`) and use it for names Space Grotesk can't render.
  The app itself is fine (egui falls back to its default fonts).
  `font_codepoints` only checks what's *requested*, not what the TTF contains.
- Game profile picker: a "back" key from the control screen to the picker, and
  a friendly "add a child in Rondelek first" message when the library is empty
  (needs translated text in the game, which has none yet).
- CI: bump `actions/checkout` and `softprops/action-gh-release` to versions
  that run on Node 24 (GitHub warns Node 20 is deprecated); `ubuntu-latest`
  moves to Ubuntu 26 on 2026-10-19, so recheck the apt package list then.
- Re-enable the macOS/Linux release targets in `release.yml` when wanted.
- Prune old merged local/remote branches (`feat/*`, `fix/skinning-better`, …).
- UI tests beyond the settings page (Home/Hub navigation) with `egui_kittest`.

## Voice games: look & feel (after the 2026-09-26/27 look pass)
- **Lam::pula on the clouds and the sun is on trial.** If it stops pleasing,
  write a cloud-dedicated shader instead of editing `lampula.fs` (that file is a
  verbatim copy of flat-draw's). Tune through `cloud_glass()` / `sun_glass()` or
  `RONDELEK_LAMPULA` first.
- **Idea: grey out what's still placeholder.** Greg floated drawing every
  not-yet-replaced placeholder (mountains, obstacles, ground…) in greyscale so
  the real art stands out. Undecided; if done, as a dev toggle, and the brick
  hero stays in colour (it is permanent).
- The pre-game control screen and profile picker still use the pastel
  cream→peach look, while the game world is now a bright sunny day. Consider
  bringing them closer.
- **3D carved vowel letters on the signs — postponed, code parked on branch
  `feat/3d-vowel-letters` (pushed, not merged).** It already splits one GLB
  into per-letter props by mesh name (`FlatModel::load_parts`, one draw call
  each, one shared texture) and draws the signs in 3D: a tablet with the letter
  on its face. Blocked on art: the first `letters.glb` stacked all six letters
  in one spot, and the exporter culls faces between overlapping layers, so
  only the front "A" was whole. Greg will draw new letters (side by side in
  one file is fine) **and a wooden plate** for them to sit on (instead of the
  placeholder slab). Also decide upper vs lower case to match the vowel meter
  and pre-game screen.
- Obstacles are still procedural pastel blocks; replace with flat-draw art as
  it arrives in `greg/`.
- `cloud9_rain.glb` is unused (kept for a possible rain variant).
- **Re-measure the game's GPU/CPU cost on real hardware.** The water (Voronoi
  web, fish) and Lam::pula passes are new since `docs/PERF.md`'s ~10% figure;
  headless Xvfb runs use software GL, so their numbers don't count.
- Upstream: report raylib-rs 6's `rl_mult_matrixf` passing the matrix
  transposed (it casts the row-ordered `Matrix` struct to the column-major
  `float[16]` rlgl expects). We work around it with `transpose()`.

## App look
- **8-bit pixel-art revamp of the app** (the style of the README's arcade
  pictures) — planned by Greg, deliberately not started yet.

## Nice-to-have
- Per-pad labels (e.g. "ma", "pa") shown on the pads instead of the key letter.
