# Skins

The whole sampler faceplate — background, case, screen bezel, every key —
is drawn from images, so a designer can reskin the app without touching
code. A skin is a folder of PNGs plus a `skin.json`, distributed as a zip
of that folder.

## Installing a skin

Drop the `.zip` (or the unzipped folder) into:

- Linux: `~/.config/rondelek/skins/`
- Windows: `%APPDATA%\rondelek\skins\`
- macOS: `~/Library/Application Support/rondelek/skins/`

then pick it in the app: **F12 → Appearance → Skin**. Zips are extracted
automatically on the next scan.

## Files in a skin

All file names are fixed. **Every file is optional** — anything missing
falls back to the built-in Base Pastel skin, so a skin can be as small as
a `skin.json` that only changes colours.

| File | Suggested size | What it is |
|---|---|---|
| `skin.json` | — | metadata, nine-slice insets, colour overrides |
| `background.png` | 1280×800 | window background; uniformly scaled to fill and centre-cropped |
| `case.png` | 512×512 | the device body; drawn as a nine-slice (see below) |
| `grain.png` | 128×128 | tileable matte-grain overlay for the case centre (optional texture detail) |
| `bezel.png` | 384×384 | frame around the visualizer screen; nine-slice |
| `pad_1.png` … `pad_12.png` | 256×256 | the 12 sample keys (row-major, 4×3 grid) — bake your icon into each |
| `pad_N_pressed.png` | 256×256 | pressed artwork per key (drawn while held) |
| `rec.png`, `rec_pressed.png` | 256×256 | the record key (top right) |
| `back.png`, `back_pressed.png` | 256×256 | back-to-profiles key (top left) |
| `cycle.png`, `cycle_pressed.png` | 256×256 | visualizer-cycle key |
| `avatar_frame.png` | 256×256 | frame drawn over the child's photo; keep the centre transparent |

Key images should carry a small transparent margin (≈4% per side) — the
engine nudges the artwork down while pressed and draws a soft shadow
behind it. The engine also draws the sample LED (top-right of each key)
and the pulsing record ring; their colours come from `skin.json`.

## Nine-slice

The case and bezel stretch to any window size. To keep corners crisp, they
are drawn as a nine-slice: the four corners of the source image are used
as-is, edges stretch along one axis, and the centre stretches in both.
`slice` in `skin.json` is the corner size in source-image pixels.

Because the centre gets stretched, keep it flat colour — put texture and
shading only near the edges. For matte grain across the whole case,
provide the tileable `grain.png` instead; the engine tiles it 1:1 over the
case centre so it never smears.

## skin.json

```json
{
  "format": 1,
  "name": "My Skin",
  "author": "You",
  "slice": { "case": 96, "bezel": 72 },
  "colors": {
    "visualizer_bg": "#33303A",
    "pad_record_bg": "#E9655A"
  }
}
```

`colors` overrides any of the app's palette entries with `#RRGGBB` or
`#RRGGBBAA`. Keys: `panel_bg`, `panel_fg`, `pad_play_bg`, `pad_play_fg`,
`pad_play_hover`, `pad_play_pressed`, `pad_record_bg`, `pad_record_fg`,
`pad_function_bg`, `pad_function_fg`, `led_empty`, `led_full`,
`case_shadow`, `case_border`, `text_primary`, `text_secondary`,
`visualizer_bg`, `visualizer_dot_off`, `visualizer_bar_low`,
`visualizer_bar_mid`, `visualizer_bar_high`.

They colour everything still drawn procedurally: the visualizer screen,
LEDs, the record ring and tint, status text, and the profile/session
screens.

## The base skin

`skins/base-pastel/` in this repo is the built-in skin, generated
procedurally by `cargo run --bin genskin` and embedded into the app at
compile time. It is the reference for sizes and style. `skins/index.json`
lists the skins available in this repo (with screenshots) — the planned
in-app browser reads it straight from GitHub.
