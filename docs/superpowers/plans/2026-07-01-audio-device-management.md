# Audio Device Management + Resilience Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the user pick audio input/output devices from an F12 "Settings" panel (Auto by default, pinnable), and make the sampler recover automatically when the active device disconnects or the system default changes.

**Architecture:** A new `src/audio/device.rs` isolates device enumeration and two pure decision functions (`choose_target`, `needs_rebuild`). `Capture`/`Playback` gain an `open(device)` constructor plus an `alive` flag flipped by cpal's error callback. A per-frame watchdog (`maintain_audio`, replacing `init_audio`) rebuilds streams when they are missing, dead, or pointed at the wrong device. A `ConfigPanel` egui window edits the persisted device selection.

**Tech Stack:** Rust (edition 2024), eframe/egui 0.35, cpal 0.18, serde/serde_json.

## Global Constraints

- Keep `Cargo.lock` committed; build with the existing toolchain (edition 2024).
- Device selection persists in `Settings` (`settings.json`), backward-compatible via `#[serde(default)]`.
- Recovery cadence ~1s (watchdog throttle).
- i18n rule (from `src/i18n/mod.rs`): any new user-facing string MUST be added to `en.json` AND every seeded locale (`pl, de, fr, es, it, uk`) so the `english_has_all_keys_others_match` test stays green.
- Follow existing patterns: panels mirror `DevPanel` (a `struct { visible: bool }` rendered as an `egui::Window`); keyboard read via `ui.input(|i| i.key_pressed(Key::…))`.
- Out of scope (do NOT build): per-profile device settings, sample-rate/channel/latency UI, OS hotplug event APIs.

---

### Task 1: Device layer — pure logic + enumeration

**Files:**
- Create: `src/audio/device.rs`
- Modify: `src/audio/mod.rs`
- Test: unit tests inside `src/audio/device.rs`

**Interfaces:**
- Produces:
  - `enum DevicePref { Auto, Pinned(String) }` with `fn from_setting(&Option<String>) -> DevicePref`
  - `struct Target { pub name: String, pub is_fallback: bool }`
  - `fn choose_target(pref: &DevicePref, available: &[String], default: Option<&str>) -> Option<Target>`
  - `fn needs_rebuild(current: Option<&str>, alive: bool, target: &str) -> bool`
  - `fn list_input_devices() -> Vec<String>` / `fn list_output_devices() -> Vec<String>`
  - `fn default_input_name() -> Option<String>` / `fn default_output_name() -> Option<String>`
  - `fn input_device_by_name(&str) -> Option<cpal::Device>` / `fn output_device_by_name(&str) -> Option<cpal::Device>`

- [ ] **Step 1: Write the failing tests**

Create `src/audio/device.rs` with the pure logic and its tests:

```rust
use cpal::traits::{DeviceTrait, HostTrait};

/// What device the user wants for one direction (input or output).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DevicePref {
    /// Follow the system default device.
    Auto,
    /// Use a specific device by name, falling back to default if it is absent.
    Pinned(String),
}

impl DevicePref {
    /// Map a persisted `Option<String>` setting (None = Auto) to a preference.
    pub fn from_setting(setting: &Option<String>) -> Self {
        match setting {
            Some(name) => DevicePref::Pinned(name.clone()),
            None => DevicePref::Auto,
        }
    }
}

/// The device the watchdog should target, plus whether it is a fallback for a
/// pinned-but-currently-missing device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub name: String,
    pub is_fallback: bool,
}

/// Decide which device to target given the preference, the currently-available
/// device names, and the system default name.
pub fn choose_target(
    pref: &DevicePref,
    available: &[String],
    default: Option<&str>,
) -> Option<Target> {
    match pref {
        DevicePref::Auto => default.map(|d| Target {
            name: d.to_string(),
            is_fallback: false,
        }),
        DevicePref::Pinned(name) => {
            if available.iter().any(|d| d == name) {
                Some(Target {
                    name: name.clone(),
                    is_fallback: false,
                })
            } else {
                default.map(|d| Target {
                    name: d.to_string(),
                    is_fallback: true,
                })
            }
        }
    }
}

/// Rebuild the stream when there is none, it is dead, or it is pointed at a
/// device other than the target (covers default-changed AND pinned-reappeared).
pub fn needs_rebuild(current: Option<&str>, alive: bool, target: &str) -> bool {
    match current {
        None => true,
        Some(_) if !alive => true,
        Some(c) => c != target,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn devs(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn auto_targets_default() {
        let t = choose_target(&DevicePref::Auto, &devs(&["A", "B"]), Some("A")).unwrap();
        assert_eq!(t.name, "A");
        assert!(!t.is_fallback);
    }

    #[test]
    fn auto_with_no_default_is_none() {
        assert!(choose_target(&DevicePref::Auto, &devs(&["A"]), None).is_none());
    }

    #[test]
    fn pinned_present_targets_pinned() {
        let pref = DevicePref::Pinned("B".to_string());
        let t = choose_target(&pref, &devs(&["A", "B"]), Some("A")).unwrap();
        assert_eq!(t.name, "B");
        assert!(!t.is_fallback);
    }

    #[test]
    fn pinned_absent_falls_back_to_default() {
        let pref = DevicePref::Pinned("Bluetooth".to_string());
        let t = choose_target(&pref, &devs(&["A", "B"]), Some("A")).unwrap();
        assert_eq!(t.name, "A");
        assert!(t.is_fallback);
    }

    #[test]
    fn needs_rebuild_when_missing() {
        assert!(needs_rebuild(None, false, "A"));
    }

    #[test]
    fn needs_rebuild_when_dead() {
        assert!(needs_rebuild(Some("A"), false, "A"));
    }

    #[test]
    fn needs_rebuild_when_device_changed() {
        assert!(needs_rebuild(Some("A"), true, "B"));
    }

    #[test]
    fn no_rebuild_when_alive_and_on_target() {
        assert!(!needs_rebuild(Some("A"), true, "A"));
    }

    #[test]
    fn from_setting_maps_none_to_auto() {
        assert_eq!(DevicePref::from_setting(&None), DevicePref::Auto);
        assert_eq!(
            DevicePref::from_setting(&Some("X".to_string())),
            DevicePref::Pinned("X".to_string())
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib choose_target`
Expected: FAIL — `device.rs` is not yet a module, so it won't compile / the tests aren't found.

- [ ] **Step 3: Add the hardware enumeration helpers + register the module**

Append the hardware helpers to `src/audio/device.rs` (below the pure logic, above `#[cfg(test)]`):

```rust
/// Names of all available output devices (best-effort; empty on error).
pub fn list_output_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.output_devices() {
        Ok(devs) => devs.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

/// Names of all available input devices (best-effort; empty on error).
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devs) => devs.filter_map(|d| d.name().ok()).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn default_output_name() -> Option<String> {
    cpal::default_host()
        .default_output_device()
        .and_then(|d| d.name().ok())
}

pub fn default_input_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.name().ok())
}

pub fn output_device_by_name(name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    host.output_devices()
        .ok()?
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
}

pub fn input_device_by_name(name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    host.input_devices()
        .ok()?
        .find(|d| d.name().map(|n| n == name).unwrap_or(false))
}
```

Register the module in `src/audio/mod.rs` — change it to:

```rust
pub mod capture;
pub mod device;
pub mod playback;
pub mod sample;

pub use capture::Capture;
pub use playback::Playback;
pub use sample::Sample;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib device::`
Expected: PASS — all 9 tests in Step 1 pass.

- [ ] **Step 5: Commit**

```bash
git add src/audio/device.rs src/audio/mod.rs
git commit -m "feat(audio): device layer — enumeration + pure target/rebuild logic"
```

---

### Task 2: Settings — persisted device selection

**Files:**
- Modify: `src/config/settings.rs`
- Test: unit test inside `src/config/settings.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `Settings.input_device: Option<String>`, `Settings.output_device: Option<String>` (both `None` = Auto).

- [ ] **Step 1: Write the failing test**

Add this test module at the bottom of `src/config/settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_device_fields_default_to_none() {
        // A settings JSON written before this feature existed.
        let json = r#"{
            "volume": 0.8,
            "dark_mode": false,
            "visualizer_smoothing": 0.7,
            "visualizer_decay": 0.4,
            "visualizer_num_bars": 36,
            "show_dev_panel": false,
            "language": "en"
        }"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.input_device, None);
        assert_eq!(s.output_device, None);
    }

    #[test]
    fn device_fields_round_trip() {
        let mut s = Settings::default();
        s.output_device = Some("Speakers".to_string());
        let json = serde_json::to_string(&s).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(back.output_device, Some("Speakers".to_string()));
        assert_eq!(back.input_device, None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib missing_device_fields_default_to_none`
Expected: FAIL — `Settings` has no `input_device` / `output_device` fields (compile error).

- [ ] **Step 3: Add the fields with serde defaults**

In `src/config/settings.rs`, add the two fields to the `Settings` struct (after the `language` field, before the closing brace at line 20):

```rust
    /// Pinned input device name; None = follow system default.
    #[serde(default)]
    pub input_device: Option<String>,
    /// Pinned output device name; None = follow system default.
    #[serde(default)]
    pub output_device: Option<String>,
```

And in `impl Default for Settings`, add to the returned struct (after `language: default_language(),`):

```rust
            input_device: None,
            output_device: None,
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib settings::`
Expected: PASS — both new tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/config/settings.rs
git commit -m "feat(settings): persist input/output device selection (serde default = Auto)"
```

---

### Task 3: Capture/Playback — `open(device)` + liveness flag

**Files:**
- Modify: `src/audio/playback.rs`
- Modify: `src/audio/capture.rs`

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `Playback::open(device: &cpal::Device) -> Result<Playback>`, `Playback::is_alive() -> bool`, `Playback::current_device() -> &str`
  - `Capture::open(device: &cpal::Device) -> Result<Capture>`, `Capture::is_alive() -> bool`, `Capture::current_device() -> &str`
  - `Playback::new()` / `Capture::new()` retained as thin default-device wrappers (removed in Task 4).

- [ ] **Step 1: Rewrite `Playback` to `open(device)` with an alive flag**

In `src/audio/playback.rs`:

Change the imports at the top (add `AtomicBool`/`Ordering`):

```rust
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
```

Add two fields to the `Playback` struct (after `sample_rate: u32,`):

```rust
    alive: Arc<AtomicBool>,
    current_device: String,
```

Replace the `pub fn new() -> Result<Self> { … }` method (lines 16–84) with an `open` method plus a thin `new` wrapper:

```rust
    /// Build an output stream on `device`. The stream's error callback flips an
    /// `alive` flag so the watchdog can detect a disconnected device.
    pub fn open(device: &cpal::Device) -> Result<Self> {
        let current_device = device.name().unwrap_or_default();

        let supported = device
            .default_output_config()
            .context("Failed to get default output config")?;

        let channels = supported.channels().max(1) as usize;
        let sample_rate: u32 = supported.sample_rate();

        let sources: Sources = Arc::new(Mutex::new(Vec::new()));
        let src_clone = Arc::clone(&sources);

        let alive = Arc::new(AtomicBool::new(true));
        let alive_cb = Arc::clone(&alive);

        let config = cpal::StreamConfig {
            channels: channels as u16,
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if let Ok(mut src) = src_clone.lock() {
                        for frame in data.chunks_mut(channels) {
                            let mut mixed: f32 = 0.0;
                            let mut active = 0;
                            src.retain_mut(|(buf, pos)| {
                                if *pos < buf.len() {
                                    mixed += buf[*pos];
                                    *pos += 1;
                                    active += 1;
                                    true
                                } else {
                                    false
                                }
                            });
                            if active > 1 {
                                mixed /= active as f32;
                            }
                            let value = mixed.clamp(-1.0, 1.0);
                            for sample in frame.iter_mut() {
                                *sample = value;
                            }
                        }
                    } else {
                        data.fill(0.0);
                    }
                },
                move |err| {
                    alive_cb.store(false, Ordering::Relaxed);
                    eprintln!("Playback error: {err}");
                },
                None,
            )
            .context("Failed to build output stream")?;

        stream.play().context("Failed to start output stream")?;

        Ok(Self {
            stream: Some(stream),
            sources,
            sample_rate,
            alive,
            current_device,
        })
    }

    /// Convenience: open the current system default output device.
    pub fn new() -> Result<Self> {
        let device = cpal::default_host()
            .default_output_device()
            .context("No output audio device found")?;
        Self::open(&device)
    }

    /// False once cpal has reported a stream error (e.g. device disconnected).
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Name of the device this stream was built on.
    pub fn current_device(&self) -> &str {
        &self.current_device
    }
```

(Leave `play`, `stop`, `Drop`, `resample_linear`, and the existing tests unchanged.)

- [ ] **Step 2: Rewrite `Capture` to `open(device)` with an alive flag**

In `src/audio/capture.rs`:

Change the imports at the top:

```rust
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
```

Add two fields to the `Capture` struct (after `sample_rate: u32,`):

```rust
    alive: Arc<AtomicBool>,
    current_device: String,
```

Replace `pub fn new() -> Result<Self> { … }` (lines 13–65) with:

```rust
    /// Build an input stream on `device`, folding multi-channel frames to mono.
    pub fn open(device: &cpal::Device) -> Result<Self> {
        let current_device = device.name().unwrap_or_default();

        let supported = device
            .default_input_config()
            .context("Failed to get default input config")?;

        let sample_rate: u32 = supported.sample_rate();
        let channels = supported.channels();

        let buffer = Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
        let buf_clone = Arc::clone(&buffer);

        let alive = Arc::new(AtomicBool::new(true));
        let alive_cb = Arc::clone(&alive);

        let config = cpal::StreamConfig {
            channels,
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        let channels = channels.max(1) as usize;

        let stream = device
            .build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if let Ok(mut buf) = buf_clone.lock() {
                        for frame in data.chunks(channels) {
                            if buf.len() > 32768 {
                                let _ = buf.pop_front();
                            }
                            buf.push_back(fold_to_mono(frame));
                        }
                    }
                },
                move |err| {
                    alive_cb.store(false, Ordering::Relaxed);
                    eprintln!("Capture error: {err}");
                },
                None,
            )
            .context("Failed to build input stream")?;

        stream.play().context("Failed to start input stream")?;

        Ok(Self {
            stream: Some(stream),
            buffer,
            sample_rate,
            alive,
            current_device,
        })
    }

    /// Convenience: open the current system default input device.
    pub fn new() -> Result<Self> {
        let device = cpal::default_host()
            .default_input_device()
            .context("No input audio device found")?;
        Self::open(&device)
    }

    /// False once cpal has reported a stream error (e.g. device disconnected).
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Name of the device this stream was built on.
    pub fn current_device(&self) -> &str {
        &self.current_device
    }
```

(Leave `drain`, `sample_rate`, `stop`, `Drop`, `fold_to_mono`, and the existing tests unchanged.)

- [ ] **Step 3: Build and run existing tests**

Run: `cargo build`
Expected: compiles cleanly (the existing `init_audio` still calls `Capture::new()` / `Playback::new()`, which now delegate to `open`).

Run: `cargo test --lib`
Expected: PASS — existing `fold_to_mono` and `resample_linear` tests still pass, plus Task 1/2 tests.

> Note: `open()` builds real audio streams, so it is not unit-tested (consistent with the pre-existing untested `Capture`/`Playback`). Its logic is covered by the Task 1 pure functions and the Task 8 manual verification.

- [ ] **Step 4: Commit**

```bash
git add src/audio/playback.rs src/audio/capture.rs
git commit -m "feat(audio): open(device) constructors + liveness flag on Capture/Playback"
```

---

### Task 4: Watchdog — `maintain_audio` replaces `init_audio`

**Files:**
- Modify: `src/app.rs` (imports ~line 5; `init_audio` method 193–232; call site line 928)
- Modify: `src/audio/playback.rs`, `src/audio/capture.rs` (remove the temporary `new()` wrappers)

**Interfaces:**
- Consumes: `device::{DevicePref, choose_target, needs_rebuild, list_*_devices, default_*_name, *_device_by_name}` (Task 1); `Settings.{input_device,output_device}` (Task 2); `Playback/Capture::{open,is_alive,current_device}` (Task 3).
- Produces: `App::maintain_audio(&mut self, dt: f32)`.

- [ ] **Step 1: Import the device module in `app.rs`**

Change the audio import (line 5) from:

```rust
use crate::audio::{Capture, Playback, Sample};
```

to:

```rust
use crate::audio::{Capture, Playback, Sample, device};
```

- [ ] **Step 2: Replace `init_audio` with `maintain_audio`**

Replace the whole `fn init_audio(&mut self, dt: f32) { … }` method (lines 193–232) with:

```rust
    /// Per-frame audio watchdog (throttled ~1s). For each direction, work out
    /// the target device (Auto → system default; Pinned → that device, or the
    /// default as a fallback) and rebuild the stream when it is missing, dead,
    /// or pointed at the wrong device. This recovers from disconnects and
    /// system-default changes, and reclaims a pinned device when it returns.
    fn maintain_audio(&mut self, dt: f32) {
        if self.audio_retry_timer > 0.0 {
            self.audio_retry_timer -= dt;
            return;
        }
        self.audio_retry_timer = 1.0;

        let mut errors = Vec::new();

        // ---- output ----
        {
            let pref = device::DevicePref::from_setting(&self.settings.output_device);
            let available = device::list_output_devices();
            let default = device::default_output_name();
            match device::choose_target(&pref, &available, default.as_deref()) {
                Some(target) => {
                    let current = self.playback.as_ref().map(|p| p.current_device());
                    let alive = self.playback.as_ref().map(|p| p.is_alive()).unwrap_or(false);
                    if device::needs_rebuild(current, alive, &target.name) {
                        match device::output_device_by_name(&target.name) {
                            Some(dev) => match Playback::open(&dev) {
                                Ok(pb) => self.playback = Some(pb),
                                Err(e) => {
                                    self.playback = None;
                                    errors.push(format!("speaker: {e}"));
                                }
                            },
                            None => {
                                self.playback = None;
                                errors.push(format!("speaker: {} unavailable", target.name));
                            }
                        }
                    }
                }
                None => {
                    self.playback = None;
                    errors.push("speaker: none".to_string());
                }
            }
        }

        // ---- input ----
        {
            let pref = device::DevicePref::from_setting(&self.settings.input_device);
            let available = device::list_input_devices();
            let default = device::default_input_name();
            match device::choose_target(&pref, &available, default.as_deref()) {
                Some(target) => {
                    let current = self.capture.as_ref().map(|c| c.current_device());
                    let alive = self.capture.as_ref().map(|c| c.is_alive()).unwrap_or(false);
                    if device::needs_rebuild(current, alive, &target.name) {
                        match device::input_device_by_name(&target.name) {
                            Some(dev) => match Capture::open(&dev) {
                                Ok(cap) => {
                                    self.capture_rate = cap.sample_rate();
                                    if self.visualizer.sample_rate() != cap.sample_rate() {
                                        self.visualizer = Visualizer::new(cap.sample_rate());
                                    }
                                    self.capture = Some(cap);
                                }
                                Err(e) => {
                                    self.capture = None;
                                    errors.push(format!("microphone: {e}"));
                                }
                            },
                            None => {
                                self.capture = None;
                                errors.push(format!("microphone: {} unavailable", target.name));
                            }
                        }
                    }
                }
                None => {
                    self.capture = None;
                    errors.push("microphone: none".to_string());
                }
            }
        }

        self.audio_status = if errors.is_empty() {
            String::new()
        } else {
            format!("{} ({})", self.i18n.t("audio.unavailable"), errors.join("; "))
        };
    }
```

- [ ] **Step 3: Update the call site**

In `draw_session`, change line 928 from `self.init_audio(dt);` to:

```rust
        self.maintain_audio(dt);
```

- [ ] **Step 4: Remove the temporary `new()` wrappers**

Now that nothing calls them, delete the `pub fn new() -> Result<Self> { … }` wrapper added in Task 3 from **both** `src/audio/playback.rs` and `src/audio/capture.rs` (keep `open`, `is_alive`, `current_device`). This avoids dead-code warnings.

- [ ] **Step 5: Build and test**

Run: `cargo build`
Expected: compiles cleanly, no dead-code warnings for `new`.

Run: `cargo test --lib`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/audio/playback.rs src/audio/capture.rs
git commit -m "feat(audio): watchdog rebuilds streams on disconnect / device change"
```

---

### Task 5: i18n keys for the Settings panel

**Files:**
- Modify: `assets/i18n/en.json`, `pl.json`, `de.json`, `fr.json`, `es.json`, `it.json`, `uk.json`

**Interfaces:**
- Produces translation keys: `config.title`, `config.audio`, `config.output`, `config.input`, `config.auto`, `config.active`, `config.fallback`, `profiles.settings`.

- [ ] **Step 1: Add the keys to every seeded locale**

In each file, change the last line `  "audio.unavailable": "…"` to add a trailing comma, then insert the 8 new keys before the closing `}`. Use these exact values per file:

`assets/i18n/en.json`:
```json
  "audio.unavailable": "Audio unavailable, retrying",
  "config.title": "Settings",
  "config.audio": "Audio devices",
  "config.output": "Output",
  "config.input": "Input",
  "config.auto": "Auto (system default)",
  "config.active": "Active:",
  "config.fallback": "fallback",
  "profiles.settings": "Settings"
}
```

`assets/i18n/pl.json`:
```json
  "config.title": "Ustawienia",
  "config.audio": "Urządzenia audio",
  "config.output": "Wyjście",
  "config.input": "Wejście",
  "config.auto": "Automatyczne (domyślne systemowe)",
  "config.active": "Aktywne:",
  "config.fallback": "zastępcze",
  "profiles.settings": "Ustawienia"
```

`assets/i18n/de.json`:
```json
  "config.title": "Einstellungen",
  "config.audio": "Audiogeräte",
  "config.output": "Ausgabe",
  "config.input": "Eingang",
  "config.auto": "Automatisch (Systemstandard)",
  "config.active": "Aktiv:",
  "config.fallback": "Ersatz",
  "profiles.settings": "Einstellungen"
```

`assets/i18n/fr.json`:
```json
  "config.title": "Paramètres",
  "config.audio": "Périphériques audio",
  "config.output": "Sortie",
  "config.input": "Entrée",
  "config.auto": "Auto (défaut système)",
  "config.active": "Actif :",
  "config.fallback": "secours",
  "profiles.settings": "Paramètres"
```

`assets/i18n/es.json`:
```json
  "config.title": "Ajustes",
  "config.audio": "Dispositivos de audio",
  "config.output": "Salida",
  "config.input": "Entrada",
  "config.auto": "Auto (predeterminado del sistema)",
  "config.active": "Activo:",
  "config.fallback": "alternativo",
  "profiles.settings": "Ajustes"
```

`assets/i18n/it.json`:
```json
  "config.title": "Impostazioni",
  "config.audio": "Dispositivi audio",
  "config.output": "Uscita",
  "config.input": "Ingresso",
  "config.auto": "Auto (predefinito di sistema)",
  "config.active": "Attivo:",
  "config.fallback": "ripiego",
  "profiles.settings": "Impostazioni"
```

`assets/i18n/uk.json`:
```json
  "config.title": "Налаштування",
  "config.audio": "Аудіопристрої",
  "config.output": "Вихід",
  "config.input": "Вхід",
  "config.auto": "Авто (системний за замовчуванням)",
  "config.active": "Активний:",
  "config.fallback": "резервний",
  "profiles.settings": "Налаштування"
```

> For `pl/de/fr/es/it/uk`, remember to add the trailing comma to whatever was the previous last key line, exactly as shown for `en.json`.

- [ ] **Step 2: Verify the locale key-sync test still passes**

Run: `cargo test --lib i18n::`
Expected: PASS — `english_has_all_keys_others_match` confirms all 7 locales have identical key sets and counts.

- [ ] **Step 3: Commit**

```bash
git add assets/i18n
git commit -m "i18n: add Settings panel keys across all seeded locales"
```

---

### Task 6: `ConfigPanel` UI

**Files:**
- Create: `src/ui/config_panel.rs`
- Modify: `src/ui/mod.rs`

**Interfaces:**
- Consumes: `device::{self, DevicePref, choose_target}` (Task 1); `Settings.{input_device,output_device}` (Task 2); `I18n` keys (Task 5).
- Produces: `struct ConfigPanel { pub visible: bool }` with `new()`, `toggle()`, and `show(&mut self, ctx, settings: &mut Settings, i18n: &I18n) -> bool` (returns `true` when a selection changed).

- [ ] **Step 1: Create the panel module**

Create `src/ui/config_panel.rs`:

```rust
use crate::audio::device::{self, DevicePref};
use crate::config::Settings;
use crate::i18n::I18n;

/// User-facing "Settings" window (F12). Currently hosts audio device
/// selection; designed to grow additional sections over time.
pub struct ConfigPanel {
    pub visible: bool,
}

impl ConfigPanel {
    pub fn new() -> Self {
        Self { visible: false }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    /// Draw the panel when visible. Returns `true` if the user changed a
    /// device selection, so the caller can persist settings.
    pub fn show(&mut self, ctx: &egui::Context, settings: &mut Settings, i18n: &I18n) -> bool {
        if !self.visible {
            return false;
        }

        // Enumerate fresh each frame the window is open, so hot-plugged devices
        // appear without any manual refresh.
        let outputs = device::list_output_devices();
        let inputs = device::list_input_devices();
        let out_default = device::default_output_name();
        let in_default = device::default_input_name();

        let mut changed = false;
        let mut open = self.visible;

        egui::Window::new(i18n.t("config.title"))
            .resizable(true)
            .default_width(340.0)
            .open(&mut open)
            .show(ctx, |ui| {
                ui.heading(i18n.t("config.audio"));
                ui.add_space(6.0);
                changed |= device_selector(
                    ui,
                    "cfg_output",
                    i18n.t("config.output"),
                    &outputs,
                    out_default.as_deref(),
                    &mut settings.output_device,
                    i18n,
                );
                ui.add_space(12.0);
                changed |= device_selector(
                    ui,
                    "cfg_input",
                    i18n.t("config.input"),
                    &inputs,
                    in_default.as_deref(),
                    &mut settings.input_device,
                    i18n,
                );
            });

        self.visible = open;
        changed
    }
}

/// One labelled device dropdown ("Auto" + each device) plus a line showing the
/// currently-active device and a "(fallback)" marker when a pinned device is
/// missing. Returns true if the selection changed.
fn device_selector(
    ui: &mut egui::Ui,
    id_salt: &str,
    label: &str,
    devices: &[String],
    default: Option<&str>,
    setting: &mut Option<String>,
    i18n: &I18n,
) -> bool {
    let mut changed = false;

    ui.label(label);

    let selected_text = match setting.as_deref() {
        None => i18n.t("config.auto").to_string(),
        Some(name) => name.to_string(),
    };

    egui::ComboBox::from_id_salt(id_salt)
        .selected_text(selected_text)
        .width(300.0)
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(setting.is_none(), i18n.t("config.auto"))
                .clicked()
                && setting.is_some()
            {
                *setting = None;
                changed = true;
            }
            for dev in devices {
                let is_sel = setting.as_deref() == Some(dev.as_str());
                if ui.selectable_label(is_sel, dev).clicked() && !is_sel {
                    *setting = Some(dev.clone());
                    changed = true;
                }
            }
        });

    // Show what will actually be used (and flag a fallback).
    let pref = DevicePref::from_setting(setting);
    if let Some(target) = device::choose_target(&pref, devices, default) {
        let mut text = format!("{} {}", i18n.t("config.active"), target.name);
        if target.is_fallback {
            text.push_str(&format!(" ({})", i18n.t("config.fallback")));
        }
        ui.label(egui::RichText::new(text).small().weak());
    }

    changed
}
```

- [ ] **Step 2: Export the module**

In `src/ui/mod.rs`, add the module declaration and re-export. Add `pub mod config_panel;` after `pub mod pad;` and `pub use config_panel::ConfigPanel;` after the `dev_panel` re-export, so the file reads:

```rust
pub mod config_panel;
pub mod dev_panel;
pub mod layout;
pub mod pad;
pub mod renderer;
pub mod visualizer;
pub mod widgets;

pub use config_panel::ConfigPanel;
pub use dev_panel::DevPanel;
pub use layout::{FaceLayout, compute as compute_layout};
pub use pad::{Pad, PadMode};
pub use renderer::Renderer;
pub use visualizer::Visualizer;
pub use widgets::{draw_keycap, draw_kid_face, gloss_overlay};
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: compiles cleanly (the panel is defined but not yet wired into `App` — that's Task 7).

- [ ] **Step 4: Commit**

```bash
git add src/ui/config_panel.rs src/ui/mod.rs
git commit -m "feat(ui): ConfigPanel — F12 Settings window with audio device selectors"
```

---

### Task 7: Wire the panel — F12 toggle, global render, Profiles button

**Files:**
- Modify: `src/app.rs` (imports 12–15; struct field ~43; `new()` ~143; `draw_profiles` ~558; `ui()` 1088–1110)

**Interfaces:**
- Consumes: `ConfigPanel` (Task 6).
- Produces: F12 toggles the panel from any screen; the panel renders globally; a ⚙ Settings button on the Profiles screen opens it.

- [ ] **Step 1: Import and add the field**

In the `use crate::ui::{…}` block (lines 12–15), add `ConfigPanel`:

```rust
use crate::ui::{
    ConfigPanel, DevPanel, Pad, PadMode, Renderer, Visualizer, compute_layout, draw_keycap,
    draw_kid_face, gloss_overlay,
};
```

Add the field to the `App` struct, right after `dev_panel: DevPanel,` (line 43):

```rust
    config_panel: ConfigPanel,
```

- [ ] **Step 2: Initialize it in `App::new`**

In the struct literal inside `App::new` (near line 143, after `dev_panel: DevPanel::new(),`):

```rust
            config_panel: ConfigPanel::new(),
```

- [ ] **Step 3: Add the global F12 toggle + panel render in `ui()`**

In the `eframe::App for App` `ui` method (lines 1088–1110), add the F12 handler right after `self.frame_count += 1;` (line 1090):

```rust
        if ui.input(|i| i.key_pressed(Key::F12)) {
            self.config_panel.toggle();
        }
```

And render the panel after the `match self.screen { … }` block, before `self.save_pending_screenshot(ui.ctx());`:

```rust
        if self.config_panel.show(ui.ctx(), &mut self.settings, &self.i18n) {
            self.save_settings();
        }
```

- [ ] **Step 4: Add the ⚙ Settings entry button on the Profiles screen**

In `draw_profiles`, immediately after the language-picker `egui::Area` block (after line 568, before the "Preload avatar textures" comment), add:

```rust
        // Settings entry (opens the F12 panel), anchored top-left.
        let settings_label = format!("⚙ {}", self.i18n.t("profiles.settings"));
        let mut open_settings = false;
        egui::Area::new(egui::Id::new("settings_entry"))
            .anchor(Align2::LEFT_TOP, [16.0, 16.0])
            .show(&ctx, |ui| {
                if ui.button(settings_label).clicked() {
                    open_settings = true;
                }
            });
        if open_settings {
            self.config_panel.visible = true;
        }
```

- [ ] **Step 5: Build and test**

Run: `cargo build`
Expected: compiles cleanly.

Run: `cargo test --lib`
Expected: PASS (all tests from Tasks 1, 2, 5, plus pre-existing).

- [ ] **Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat(app): wire ConfigPanel — global F12 toggle + Profiles Settings button"
```

---

### Task 8: Manual verification

**Files:** none (runtime verification).

- [ ] **Step 1: Build a release-ish run**

Run: `cargo run`
Expected: app launches to the Profiles screen.

- [ ] **Step 2: Panel reachability**

- Press **F12** on the Profiles screen → the "Settings" window appears; press F12 again → it closes.
- Confirm the **⚙ Settings** button (top-left of Profiles) opens the same window.
- Enter a profile/session, press **F12** → panel still toggles (global).

- [ ] **Step 3: Device selection + persistence**

- In the panel, pick a specific **Output** device → the "Active:" line updates. Quit and relaunch → the selection is still applied (persisted in `settings.json`).
- Set Output back to **Auto (system default)**.

- [ ] **Step 4: Reproduce the original bug + confirm recovery**

- Enter a session so audio is live. Play a pad to confirm sound.
- Switch the macOS output to a bluetooth speaker, then disconnect it (and/or sleep→wake). Within ~1–2s, playing a pad should produce sound again on the new default device (no restart).
- Pin the bluetooth device in the panel, disconnect it → "Active:" shows the default with **(fallback)**; reconnect it → within ~1–2s it reclaims the pinned device.

- [ ] **Step 5: Final commit (if any tweaks were needed)**

```bash
git add -A
git commit -m "chore(audio): manual-verification tweaks for device recovery"
```

---

## Notes / deviations from spec

- The spec mentioned a **"Rescan" button**; it is intentionally omitted because the panel re-enumerates devices every frame it is open, so the lists are always live and a manual rescan would be a no-op. The `config.rescan` key is therefore not added. (Easy to add later if a manual trigger is ever wanted.)
- Hardware-touching code (`open`, enumeration, the `maintain_audio` method) is verified by build + Task 8 manual steps rather than unit tests, consistent with the pre-existing untested `Capture`/`Playback`. All decision logic that *can* be unit-tested (`choose_target`, `needs_rebuild`, settings serde) is covered in Tasks 1–2.
- The spec mentioned a global **status line (`audio_status`)** inside the panel. Instead, each device selector shows a per-device **"Active: … (fallback)"** line, which is clearer and keeps `ConfigPanel::show` from needing extra `App` state. The global `audio_status` still renders on the session screen's bottom status line as today.
