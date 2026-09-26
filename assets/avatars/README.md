# Character avatars

Built-in pictures a child can pick instead of a photo. Each file is embedded in
the app (see `app/src/ui/characters.rs`).

The current images are **generated placeholders** (`cargo run --bin genavatars`).

## Replacing the art

1. Draw a square image (256×256 or larger; the app scales it down).
2. Save it over the file with the same name, e.g. `fox.png`.
3. Rebuild. Every profile using that character shows the new picture, because
   profiles store the character's *name*, not a copy of the image.

Don't run `genavatars` again after replacing art: it would overwrite your files.

## Adding a character

Add `<name>.png` here and one line to `CHARACTERS` in
`app/src/ui/characters.rs`. Never rename or remove an existing name: profiles
on disk refer to it.
