# Skins

Hello, designer. This guide is for you.

The whole sampler faceplate (the background, the case, the screen bezel and
every single key) is drawn from one image. That means you can give the app a
completely new look without touching a line of code. A skin is a `skin.png`
spritesheet plus a `skin.json`, and you share it as a zip of the folder that
holds them.

## Installing a skin

Drop the `.zip` (or the unzipped folder) into:

- Linux: `~/.config/rondelek/skins/`
- Windows: `%APPDATA%\rondelek\skins\`
- macOS: `~/Library/Application Support/rondelek/skins/`

Then pick it in the app under **For grown-ups (F12) → Look & language → Sampler
skin**. You don't need to unzip anything yourself; zips are extracted
automatically on the next scan.

## Built-in skins

Two skins ship inside the app. They're always on offer, listed before any
installed ones:

- **Arcade** (`skins/arcade/`, `cargo run --bin genarcadeskin`) is the default.
  It turns the sampler into the app's arcade machine: a navy cabinet with the
  stripe trim, a cyan screen well, rainbow pixel keycaps and neon visualizer
  bars.
- **Classic** (`skins/base/`, `cargo run --bin genskin`) is the original flat,
  matte plastic look. It's also the **fallback**: if a skin leaves out
  `skin.png` or `skin.json`, it borrows Classic's.

If you name an installed skin's folder like a built-in (`arcade`, `base`), it
replaces that built-in.

## Files in a skin

A skin is just **two files**, and both are optional. Anything you leave out
falls back to the built-in Classic skin:

| File | Size | What it is |
|---|---|---|
| `skin.png` | 1536×1280 | the whole faceplate on one spritesheet (see the map below) |
| `skin.json` | — | metadata + colour overrides |

Everything the app draws from images lives in `skin.png`, at a **fixed set of
rectangles**. So you get to design the whole faceplate as one layered file, and
the app cuts each piece back out. The easiest way to start is to open
`skins/base/skin.png` or `skins/arcade/skin.png` and use it as your template.

### Spritesheet map (1536 × 1280)

| Region | Rect (x, y, w, h) | What it is |
|---|---|---|
| Case | 0, 0, 512, 512 | device body; nine-sliced (44 px corner), centre stretches |
| Bezel | 512, 0, 384, 384 | screen frame; nine-sliced (26 px corner). Centre is covered by the visualizer, so only the 26-px ring shows |
| Avatar frame | 896, 0, 256, 256 | frame over the child's photo; keep the centre transparent |
| Background | 896, 256, 512, 256 | window background; scaled to fill and centre-cropped |
| Key caps | 5 columns of 256², from y = 512 | 15 caps, index order below |

The 15 caps fill a grid five wide, starting at (0, 512) and going row by row:
first the **12 sample pads** (labelled 1 2 3 4 / Q W E R / A S D F), then
**REC**, **BACK** and **CYCLE**.

You only draw each cap's *idle* artwork. Pressing is handled by the app (the
cap sinks a few pixels and dims), so there are no `_pressed` images to make.
Leave each cap a small transparent margin, about 5% per side. The app draws a
soft raised shadow behind it, the sample LED (top-right) and the pulsing record
ring; their colours come from `skin.json`.

## Nine-slice

The case and the bezel stretch to fit any window size. To keep their corners
crisp while they do, they're drawn as a nine-slice: the corners of the region
are used exactly as drawn, the edges stretch along one direction, and the
centre stretches in both. The corner size is fixed by the app: 44 px for the
case and 26 px for the bezel, measured in `skin.png` pixels.

Because the centre gets stretched, keep it a flat colour and put any shading
near the edges. The bezel's 26-px inset is also the visualizer's opening, so the
screen fills the frame exactly and never overlaps it.

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

`colors` lets you override any of the app's palette entries, written as
`#RRGGBB` or `#RRGGBBAA`. The keys are: `panel_bg`, `panel_fg`, `pad_play_bg`,
`pad_play_fg`, `pad_play_hover`, `pad_play_pressed`, `pad_record_bg`,
`pad_record_fg`, `pad_function_bg`, `pad_function_fg`, `led_empty`, `led_full`,
`case_shadow`, `case_border`, `text_primary`, `text_secondary`,
`visualizer_bg`, `visualizer_dot_off`, `visualizer_bar_low`,
`visualizer_bar_mid`, `visualizer_bar_high`.

These colour everything that's still drawn in code rather than from the
spritesheet: the visualizer screen, the LEDs, the record ring and tint, status
text, and the profile/session screens.

## The base skin

`skins/base/` in this repo is the built-in Classic skin. It's generated in code
by `cargo run --bin genskin` and embedded into the app at compile time, and it's
the reference for the spritesheet layout and style. `skins/index.json` lists the
skins available in this repo (with screenshots); the planned in-app browser will
read it straight from GitHub.
