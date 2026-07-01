# Audio Device Management + Resilience — Design

**Date:** 2026-07-01
**Status:** Approved (pending implementation plan)

## Problem

Two related gaps in the audio layer:

1. **No device selection.** `Playback::new()` and `Capture::new()` each grab
   `cpal::default_host().default_output_device()` / `default_input_device()`
   once and never revisit it. A therapist can't route the sampler to a chosen
   input/output.
2. **No recovery from device changes.** `init_audio()` early-returns whenever
   `capture` and `playback` are both `Some`. When the active device disappears
   (e.g. bluetooth speaker disconnected, then sleep/wake), the `Playback` stays
   `Some` — a dead/stale stream — because cpal's error callback only
   `eprintln!`s and never nulls it out. The 2-second retry loop therefore never
   rebuilds, and the app goes silent until restart.

These are one feature: both need device enumeration, a notion of "the active
device", and a way to (re)build streams on demand.

## Desired behavior

- Input and output both default to **Auto** (follow the system default), which
  makes disconnect / default-change recovery automatic.
- The user can **pin** a specific input and/or output device.
- A pinned device that vanishes falls back to the default **temporarily**, and
  is **auto-reclaimed** when it reconnects ("seamless with reclaim").
- Selections **persist** across restarts.
- **F12** toggles a config panel from any screen; a discoverable entry button
  appears **only on the Profiles screen**.
- Recovery cadence: within ~1–2 seconds of a change.

## Architecture

Approach: a **dedicated device layer** that isolates the fiddly, testable
device logic from hardware and UI, matching the existing one-module-per-concern
layout under `src/audio/`.

### New module: `src/audio/device.rs`

Types and enumeration:

```rust
pub enum DevicePref { Auto, Pinned(String) }

pub fn list_input_devices() -> Vec<String>;
pub fn list_output_devices() -> Vec<String>;
```

Two **pure** decision functions — the heart of the feature, unit-tested with no
hardware:

```rust
pub struct Target { pub name: String, pub is_fallback: bool }

/// Given the user's preference, the currently-available device names, and the
/// system default name, decide which device to target.
/// - Auto            -> default (is_fallback = false)
/// - Pinned(present) -> that device (is_fallback = false)
/// - Pinned(absent)  -> default (is_fallback = true)
pub fn choose_target(
    pref: &DevicePref,
    available: &[String],
    default: Option<&str>,
) -> Option<Target>;

/// Rebuild when there is no stream, the stream is dead, or it is pointed at a
/// device other than the target (covers default-changed AND pinned-reappeared).
pub fn needs_rebuild(current: Option<&str>, alive: bool, target: &str) -> bool;
```

Thin hardware helpers resolve a chosen name to a real `cpal::Device` (enumerate
`host.output_devices()` / `input_devices()`, match by `name()`), plus
`default_output_name()` / `default_input_name()` for change detection.

### Stream liveness: `capture.rs` / `playback.rs`

- Constructors change to `open(device: &cpal::Device) -> Result<Self>`, keeping
  the existing sample-rate / channel logic.
- Each stream carries `alive: Arc<AtomicBool>` (initially `true`); the cpal
  error callback sets it `false` on error (this replaces today's bare
  `eprintln!`, which is the root of the silent-failure bug).
- Each stores the resolved `current_device: String` and exposes `is_alive()`
  and `current_device()`.

### Watchdog: rename `init_audio` -> `maintain_audio(dt)`

Runs each frame, throttled to ~1s via the existing retry timer so device
enumeration is not done every frame. Per direction (input, output):

1. Enumerate available device names + read the system default name.
2. `target = choose_target(pref, &available, default)`.
3. If `needs_rebuild(stream.current_device(), stream.is_alive(), &target.name)`,
   resolve the target name to a device and rebuild the stream.
4. On rebuild failure, set `audio_status` and leave the stream `None` to retry
   next tick.

Changing a selection in the panel only writes `Settings`; the watchdog rebuilds
on the next tick. This gives a **single** rebuild path.

### Settings additions (`src/config/settings.rs`)

```rust
#[serde(default)] pub input_device: Option<String>,   // None = Auto
#[serde(default)] pub output_device: Option<String>,  // None = Auto
```

`#[serde(default)]` keeps existing `settings.json` files compatible (missing
fields deserialize to `None`). Mapped to `DevicePref` at use; the panel writes
them and calls `save`.

### Config panel: `src/ui/config_panel.rs`

Mirrors `DevPanel`'s pattern:

```rust
pub struct ConfigPanel { pub visible: bool }
```

Rendered as an `egui::Window` titled **"Settings"** (a general configuration
surface expected to grow). First section heading: **"Audio devices"**, with:

- **Output** and **Input** each a radio/combo list of
  `"Auto (system default)"` + available device names; selecting writes the
  corresponding `Settings` field.
- The currently-active device shown, with a **"(fallback)"** marker when a
  pinned device is currently missing.
- A small live status line (`audio_status`) and a cheap **"Rescan"** button.

Code names stay purpose-neutral (`ConfigPanel` / `config_panel.rs`) so future
non-audio sections slot in as siblings.

### F12 toggle + Profiles entry button

- Input block (~`app.rs:934`): `if key_pressed(F12) { config_panel.toggle() }`,
  global to all screens.
- `draw_profiles` (~`app.rs:558`): an unobtrusive corner ⚙ **Settings** button
  that opens the panel — Profiles screen only.

### i18n

Add keys to existing locales (English as fallback for untranslated strings):
`config.title` ("Settings"), `config.audio` ("Audio devices"),
`config.output`, `config.input`, `config.auto`, `config.active`,
`config.fallback`, `config.rescan`, and `profiles.settings` (button label).
Reuse existing `audio.unavailable`.

## Testing

Unit tests on the pure logic (no hardware):

- `choose_target`: Auto → default; Pinned present → pinned, not fallback;
  Pinned absent → default, `is_fallback = true`; no default available → `None`.
- `needs_rebuild`: missing stream, dead stream, changed device, reclaim
  (pinned reappears), and the no-op case (alive + on target → false).
- `Settings` serde backward-compat: JSON without the new fields → `None`.

Hardware stream-building and enumeration stay untested (consistent with the
current `Capture` / `Playback`, which have no tests), covered instead by the
pure decision functions plus manual verification.

## Manual verification

Reproduce the original bug and confirm recovery:

- Run the app, switch/disconnect the output device mid-session, sleep/wake →
  audio recovers within ~1–2s.
- Pin a bluetooth device, disconnect it → falls back to default; reconnect →
  reclaims the pinned device.
- F12 toggles the panel on every screen; the ⚙ Settings button appears only on
  Profiles.

## Out of scope (YAGNI)

- Per-profile device settings (global only).
- Sample-rate / channel-count / latency selection UI (stays automatic).
- Real OS hotplug event APIs (polling every ~1s is sufficient).
