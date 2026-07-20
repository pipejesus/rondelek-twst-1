# Skins

The whole sampler faceplate — background, case, screen bezel, every key —
is drawn from one image, so a designer can reskin the app without touching
code. A skin is a `skin.png` spritesheet plus a `skin.json`, distributed as
a zip of the folder holding them.

## Installing a skin

Drop the `.zip` (or the unzipped folder) into:

- Linux: `~/.config/rondelek/skins/`
- Windows: `%APPDATA%\rondelek\skins\`
- macOS: `~/Library/Application Support/rondelek/skins/`

then pick it in the app: **F12 → Appearance → Skin**. Zips are extracted
automatically on the next scan.

## Files in a skin

A skin is just **two files**, both optional — anything missing falls back to
the built-in Base skin:

| File | Size | What it is |
|---|---|---|
| `skin.png` | 1536×1280 | the whole faceplate on one spritesheet (see the map below) |
| `skin.json` | — | metadata + colour overrides |

Everything the app draws from images lives in `skin.png` at a **fixed set of
rectangles**, so you design the whole faceplate as one layered file and the
app slices each element back out. Open `skins/base/skin.png` as your template.

### Spritesheet map (1536 × 1280)

| Region | Rect (x, y, w, h) | What it is |
|---|---|---|
| Case | 0, 0, 512, 512 | device body; nine-sliced (44 px corner), centre stretches |
| Bezel | 512, 0, 384, 384 | screen frame; nine-sliced (26 px corner). Centre is covered by the visualizer, so only the 26-px ring shows |
| Avatar frame | 896, 0, 256, 256 | frame over the child's photo; keep the centre transparent |
| Background | 896, 256, 512, 256 | window background; scaled to fill and centre-cropped |
| Key caps | 5 columns of 256², from y = 512 | 15 caps, index order below |

The 15 caps fill a 5-wide grid starting at (0, 512), row-major: the **12
sample pads** (labelled 1 2 3 4 / Q W E R / A S D F), then **REC**, **BACK**,
**CYCLE**.

Caps carry only their *idle* artwork — pressing is engine-driven (the cap
sinks a few pixels and dims), so there are no `_pressed` images. Give each cap
a small transparent margin (≈5% per side); the engine draws a soft raised
shadow behind it, the sample LED (top-right), and the pulsing record ring —
those colours come from `skin.json`.

## Nine-slice

The case and bezel stretch to any window size. To keep corners crisp they are
drawn as a nine-slice: the corners of the region are used as-is, edges stretch
along one axis, and the centre stretches in both. The corner size (44 px for
the case, 26 px for the bezel, in `skin.png` pixels) is fixed by the app.

Because the centre gets stretched, keep it flat colour — put shading only near
the edges. The bezel's 26-px inset is also the visualizer's opening, so the
screen fills exactly the frame and never overlaps it.

## skin.json

```json
{
  "name": "My Skin",
  "author": "You",
  "colors": {
    "visualizer_bg": "#1A1814",
    "pad_record_bg": "#FF6A1A"
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

`skins/base/` in this repo is the built-in skin, generated procedurally by
`cargo run --bin genskin` and embedded into the app at compile time. It is the
reference for the spritesheet layout and style. `skins/index.json`
lists the skins available in this repo (with screenshots) — the planned
in-app browser reads it straight from GitHub.
