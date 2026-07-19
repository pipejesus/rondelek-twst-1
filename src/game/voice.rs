//! Bridges the microphone + vowel detector into per-frame [`VoiceInput`].
//!
//! The gate (smoothing, show threshold, margin) deliberately mirrors
//! `VowelVisualizer::best_index` so a vowel that lights up in the sampler's
//! visualizer also triggers in a game — one mental model for the kid.

use super::VoiceInput;
use crate::audio::Capture;
use crate::audio::device::{self, DevicePref};
use crate::audio::vowel::{VowelCalibration, VowelDetector};
use crate::config::Settings;

/// Analysis window fed to the detector, matching the visualizer (~46 ms).
const WINDOW: usize = 2048;
/// After an onset fires, no new onset for this long — one sustained "aaa"
/// must read as one jump even if the gate flickers.
const REFRACTORY_SECS: f32 = 0.15;

pub struct VoiceBridge {
    capture: Option<Capture>,
    detector: VowelDetector,
    buf: Vec<f32>,
    scores: [f32; 6],
    level: f32,
    held: Option<usize>,
    refractory: f32,
    // Copied thresholds (the game process never re-reads settings mid-run).
    voicing_threshold: f32,
    smoothing: f32,
    show_threshold: f32,
    margin_threshold: f32,
}

impl VoiceBridge {
    pub fn new(settings: &Settings, calibration: Option<VowelCalibration>) -> Self {
        let pref = DevicePref::from_setting(&settings.input_device);
        let capture = device::resolve_input(&pref, &device::list_input_devices())
            .and_then(|dev| Capture::open(&dev).ok());
        if capture.is_none() {
            eprintln!("game: no microphone — keyboard vowels (A E I O U Y) only");
        }
        let mut detector = VowelDetector::new();
        detector.set_calibration(calibration.as_ref());

        Self {
            capture,
            detector,
            buf: Vec::with_capacity(WINDOW * 2),
            scores: [0.0; 6],
            level: 0.0,
            held: None,
            refractory: 0.0,
            voicing_threshold: settings.vowel_voicing_threshold,
            smoothing: settings.vowel_smoothing,
            show_threshold: settings.vowel_show_threshold,
            margin_threshold: settings.vowel_margin_threshold,
        }
    }

    /// Drain the mic, update the detector, and derive this frame's input.
    /// `kb_held` (keyboard fallback) overrides the voice gate when present.
    pub fn poll(&mut self, dt: f32, kb_held: Option<usize>) -> VoiceInput {
        if let Some(cap) = &mut self.capture {
            let chunk = cap.drain();
            if !chunk.is_empty() {
                self.level = chunk.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
                self.buf.extend_from_slice(&chunk);
                let excess = self.buf.len().saturating_sub(WINDOW);
                if excess > 0 {
                    self.buf.drain(..excess);
                }
                if self.buf.len() >= WINDOW {
                    let rate = cap.sample_rate();
                    let res = self
                        .detector
                        .analyze(&self.buf, rate, self.voicing_threshold);
                    // Normalize the EMA to the sampler's ~30 FPS cadence so
                    // the F12 "Smoothing" slider feels identical here even
                    // though this loop runs at 60 FPS.
                    let keep = self.smoothing.clamp(0.0, 0.98).powf(30.0 * dt);
                    let a = 1.0 - keep;
                    for i in 0..6 {
                        self.scores[i] += (res.scores[i] - self.scores[i]) * a;
                    }
                }
            }
        }

        let active =
            kb_held.or_else(|| gate(&self.scores, self.show_threshold, self.margin_threshold));
        let onset = self.step_edge(active, dt);
        VoiceInput {
            held: active,
            onset,
            scores: self.scores,
            level: self.level,
        }
    }

    /// Rising-edge detection with a refractory period. Separated from `poll`
    /// so it can be unit-tested without audio.
    fn step_edge(&mut self, active: Option<usize>, dt: f32) -> Option<usize> {
        self.refractory = (self.refractory - dt).max(0.0);
        let onset = match (self.held, active) {
            (prev, Some(v)) if prev != Some(v) && self.refractory <= 0.0 => {
                self.refractory = REFRACTORY_SECS;
                Some(v)
            }
            _ => None,
        };
        self.held = active;
        onset
    }
}

/// The on/off decision on smoothed scores — the same rule as the sampler's
/// vowel visualizer: loudest vowel wins only if it clears the show threshold
/// AND beats the runner-up by the margin.
fn gate(scores: &[f32; 6], show_threshold: f32, margin_threshold: f32) -> Option<usize> {
    let (idx, &val) = scores
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))?;
    let second = scores
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != idx)
        .map(|(_, &v)| v)
        .fold(0.0f32, f32::max);
    (val >= show_threshold && (val - second) >= margin_threshold).then_some(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bridge() -> VoiceBridge {
        // No capture/calibration needed for edge-logic tests.
        VoiceBridge::new_for_test()
    }

    impl VoiceBridge {
        fn new_for_test() -> Self {
            Self {
                capture: None,
                detector: VowelDetector::new(),
                buf: Vec::new(),
                scores: [0.0; 6],
                level: 0.0,
                held: None,
                refractory: 0.0,
                voicing_threshold: 0.012,
                smoothing: 0.5,
                show_threshold: 0.4,
                margin_threshold: 0.15,
            }
        }
    }

    #[test]
    fn gate_requires_threshold_and_margin() {
        assert_eq!(gate(&[0.3, 0.0, 0.0, 0.0, 0.0, 0.0], 0.4, 0.15), None);
        assert_eq!(gate(&[0.6, 0.5, 0.0, 0.0, 0.0, 0.0], 0.4, 0.15), None);
        assert_eq!(gate(&[0.6, 0.2, 0.0, 0.0, 0.0, 0.0], 0.4, 0.15), Some(0));
    }

    #[test]
    fn sustained_vowel_fires_exactly_one_onset() {
        let mut b = bridge();
        let mut onsets = 0;
        for _ in 0..60 {
            if b.step_edge(Some(0), 1.0 / 60.0).is_some() {
                onsets += 1;
            }
        }
        assert_eq!(onsets, 1);
    }

    #[test]
    fn flicker_within_refractory_does_not_retrigger() {
        let mut b = bridge();
        assert_eq!(b.step_edge(Some(0), 0.016), Some(0));
        assert_eq!(b.step_edge(None, 0.016), None);
        // Gate flickers back on 32 ms later — still inside the 150 ms window.
        assert_eq!(b.step_edge(Some(0), 0.016), None);
        // After the refractory expires, a fresh onset is allowed.
        assert_eq!(b.step_edge(None, 0.2), None);
        assert_eq!(b.step_edge(Some(0), 0.016), Some(0));
    }

    #[test]
    fn switching_vowels_fires_new_onset_after_refractory() {
        let mut b = bridge();
        assert_eq!(b.step_edge(Some(0), 0.016), Some(0));
        for _ in 0..12 {
            b.step_edge(Some(0), 0.016);
        }
        // a → e switch, refractory long expired.
        assert_eq!(b.step_edge(Some(1), 0.016), Some(1));
    }
}
