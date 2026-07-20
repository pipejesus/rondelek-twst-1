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

/// De-duplicate names, preserving first-seen order. The ALSA host enumerates each
/// physical card through many PCM "hints" (front, plughw, sysdefault, …), so a
/// single microphone otherwise shows up a dozen times in the picker.
fn dedup_preserving_order(names: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    names.filter(|n| seen.insert(n.clone())).collect()
}

/// Names of all available output devices (best-effort; empty on error). Duplicate
/// names (same device via different backends/PCMs) are collapsed to one.
pub fn list_output_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.output_devices() {
        Ok(devs) => dedup_preserving_order(
            devs.filter_map(|d| d.description().ok().map(|desc| desc.name().to_string())),
        ),
        Err(_) => Vec::new(),
    }
}

/// Names of all available input devices (best-effort; empty on error). Duplicate
/// names (same device via different backends/PCMs) are collapsed to one.
pub fn list_input_devices() -> Vec<String> {
    let host = cpal::default_host();
    match host.input_devices() {
        Ok(devs) => dedup_preserving_order(
            devs.filter_map(|d| d.description().ok().map(|desc| desc.name().to_string())),
        ),
        Err(_) => Vec::new(),
    }
}

pub fn default_output_name() -> Option<String> {
    cpal::default_host()
        .default_output_device()
        .and_then(|d| d.description().ok().map(|desc| desc.name().to_string()))
}

pub fn default_input_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .and_then(|d| d.description().ok().map(|desc| desc.name().to_string()))
}

pub fn output_device_by_name(name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    host.output_devices().ok()?.find(|d| {
        d.description()
            .ok()
            .map(|desc| desc.name() == name)
            .unwrap_or(false)
    })
}

pub fn input_device_by_name(name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    host.input_devices().ok()?.find(|d| {
        d.description()
            .ok()
            .map(|desc| desc.name() == name)
            .unwrap_or(false)
    })
}

/// Resolve a preference to an actual capture device **handle**. `Auto` (and a
/// pinned-but-missing device) resolve to the system default device *directly* —
/// crucially **not** by looking its name up in `input_devices()`, because on some
/// hosts (ALSA/PipeWire) the default device's name ("Default Audio Device") is not
/// present in the enumerated list, so a name lookup would fail.
pub fn resolve_input(pref: &DevicePref, available: &[String]) -> Option<cpal::Device> {
    match pref {
        DevicePref::Pinned(name) if available.iter().any(|d| d == name) => {
            input_device_by_name(name)
        }
        _ => cpal::default_host().default_input_device(),
    }
}

/// Resolve a preference to an actual output device handle (see [`resolve_input`]).
pub fn resolve_output(pref: &DevicePref, available: &[String]) -> Option<cpal::Device> {
    match pref {
        DevicePref::Pinned(name) if available.iter().any(|d| d == name) => {
            output_device_by_name(name)
        }
        _ => cpal::default_host().default_output_device(),
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
    fn dedup_collapses_repeats_preserving_order() {
        let out = dedup_preserving_order(
            ["Mic", "Mic", "USB", "Mic", "USB"]
                .into_iter()
                .map(String::from),
        );
        assert_eq!(out, vec!["Mic".to_string(), "USB".to_string()]);
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
