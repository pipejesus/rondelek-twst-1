# Data model & storage

Rondelek stores everything as **plain files** the user owns — no database, no
network. There are two roots, both resolved via the `dirs` crate.

## The profile library

Under the OS **data** directory (`dirs::data_dir()`), e.g. on macOS
`~/Library/Application Support/rondelek/profiles/`:

```
profiles/
  <slug>-<short-uid>/            one child
    profile.json                 { uid, name, avatar, created }
    avatar.png                   optional, square, 256px
    calibration.json             optional, per-child vowel calibration
    sessions/
      2026-07-01_22-11-06/       one practice run (timestamped folder)
        session.json             { uid, name, created, modified, pads[] }
        pad_01.wav               a recorded sample (mono, device rate)
        pad_05.wav
        ...
```

- **Folder names never use the raw child name.** The profile folder is
  `<slug>-<short-uid>` (`profile::slug` + a short UID); sessions are timestamped.
  The display name lives only in the manifest, so renaming a profile does **not**
  rename its folder.
- Both profiles and sessions carry a stable **`uid`** for future cross-referencing
  (notes, search).
- Avatars are always stored square: any uploaded or captured image is centre-cropped
  and resized to 256px (`profile::square_avatar`).

```mermaid
flowchart TD
    lib[profiles library] --> prof[Profile: profile.json + avatar.png]
    prof --> sess[Session: session.json]
    sess --> pad[PadEntry x NUM_SAMPLES]
    pad --> wav[pad_NN.wav if recorded]
```

### Calibration

`calibration.json` (`vowel::VowelCalibration`, written after a child runs voice
calibration) holds two `(F1, F2)` target sets, indexed like `vowel::VOWELS`, plus
`created`: **`measured`** — the child's own six vowels (used by *Play* mode) — and
**`practice`** — the correct references mapped into the child's voice-space (used by
*Practice* mode). Absent = the child uses the scaled reference set. See
[The visualizer → Calibration](visualizer.md#calibration).

### Manifests

- **`profile.json`** (`profile::ProfileManifest`): `uid`, `name` (sanitised
  display name), `avatar` (filename or none), `created`.
- **`session.json`** (`session::SessionManifest`): `uid`, `name`, `created`,
  `modified`, and a `pads` vector of `PadEntry { label, file, has_sample }`, always
  normalised to exactly `NUM_SAMPLES` entries.

Both are written with `serde_json` pretty-printing. Missing/legacy fields fall back
to `serde` defaults so older manifests keep loading.

## Settings

Under the OS **config** directory (`dirs::config_dir()`), e.g.
`~/Library/Application Support/rondelek/settings.json` — a single
`config::Settings` document: volume, dark mode, the visualizer knobs
(`visualizer_smoothing` / `_decay` / `_num_bars` / `_floor_db`), dev-panel
visibility, UI `language`, and pinned `input_device` / `output_device` names.

New fields use `#[serde(default = "...")]` so upgrading never invalidates an
existing settings file.

## What is embedded vs on disk

| Embedded in the binary | On disk (user-owned) |
|------------------------|----------------------|
| Fonts, translations, picker flags | Profiles, sessions, recordings (WAV) |
| Pad identities, theme, layout constants (Rust source) | User settings (JSON) |
