# Character avatars

Built-in pictures a child can pick instead of a photo. Each file is embedded in
the app and the games (see `core/src/characters.rs`).

Two sets, both offered when picking a picture:

- **Pixel pals** (`pixel-*.png`): vivid 32×32 pixel art (saved at 8×, 256×256)
  in the app's arcade look, from `cargo run --bin genpixelpals`. The generator
  draws each animal like a pixel artist would: hard-edged shapes, shading from
  a top-left light, a selective outline tinted from each part's own colour,
  hand-placed faces, and a vivid background with sparkles.
- **Classic** (`fox.png`, `cat.png`, …): the original smooth faces, from
  `cargo run --bin genavatars`.

The two generators only ever write their own files.

## Replacing the art

1. Draw a square image (256×256 or larger; the app scales it down).
2. Save it over the file with the same name, e.g. `fox.png`.
3. Rebuild. Every profile using that character shows the new picture, because
   profiles store the character's *name*, not a copy of the image.

Don't run `genavatars` / `genpixelpals` again after replacing art: it would
overwrite your files. (A pixel pal drawn by hand should stay 256×256 with
8×8-pixel blocks, or any square size; the app scales it.)

## Adding a character

Add `<name>.png` here and one line to `CHARACTERS` in
`core/src/characters.rs`. Never rename or remove an existing name: profiles
on disk refer to it.
