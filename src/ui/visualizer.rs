use crate::config::{Settings, Theme};
use crate::util::lerp;
use egui::{Color32, Painter, Pos2, Rect};
use rustfft::{FftPlanner, num_complex::Complex};

/// One frame's audio from both taps. The two streams can run at different sample
/// rates (capture device vs output device), so each carries its own. Either may
/// be empty (no mic, or nothing playing).
pub struct AudioFrame<'a> {
    pub input: &'a [f32],
    pub playback: &'a [f32],
    // Sample rates are part of the interface for visualizers that need a time or
    // frequency axis (e.g. a waveform scope); the spectrum is bin-based and
    // ignores them, so they read as unused until such a visualizer exists.
    #[allow(dead_code)]
    pub input_rate: u32,
    #[allow(dead_code)]
    pub playback_rate: u32,
}

/// A swappable audio visualizer. Implementations receive both taps each frame
/// and choose what to render; the app owns one behind a `Box<dyn Visualizer>`,
/// so new styles (waveform, VU meter, …) can be dropped in later.
pub trait Visualizer {
    /// Ingest the latest audio. Called only on frames with new samples.
    fn update(&mut self, frame: &AudioFrame, settings: &Settings);
    /// Render the current state into `rect`.
    fn draw(&self, painter: &Painter, rect: Rect, theme: &Theme, settings: &Settings);
    /// Fill with a representative pattern for screenshots (no live audio needed).
    /// Default is a no-op; visualizers that can, override it.
    fn demo_fill(&mut self, _num_bars: usize) {}
    /// Supply the active profile's calibrated vowel targets (`None` = use the
    /// scaled reference set). Default is a no-op; the vowel visualizer overrides.
    fn set_calibration(&mut self, _prototypes: Option<crate::audio::vowel::Prototypes>) {}
}

/// The default visualizer: a retro dot-matrix FFT spectrum.
pub struct SpectrumVisualizer {
    fft_buffer: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    window: Vec<f32>,
    magnitudes: Vec<f32>,
    smoothed: Vec<f32>,
    fft_size: usize,
}

impl SpectrumVisualizer {
    pub fn new() -> Self {
        let fft_size = 1024;
        let window: Vec<f32> = (0..fft_size)
            .map(|i| {
                0.5 * (1.0
                    - (2.0 * std::f32::consts::PI * i as f32 / (fft_size as f32 - 1.0)).cos())
            })
            .collect();

        Self {
            fft_buffer: vec![Complex::new(0.0, 0.0); fft_size],
            fft_scratch: vec![Complex::new(0.0, 0.0); fft_size],
            window,
            magnitudes: vec![0.0; fft_size / 2],
            smoothed: vec![0.0; 256],
            fft_size,
        }
    }

    fn process(&mut self, samples: &[f32], settings: &Settings) {
        let num_bars = settings.visualizer_num_bars;
        if num_bars > self.smoothed.len() {
            self.smoothed.resize(num_bars, 0.0);
        }

        if samples.len() < self.fft_size {
            return;
        }

        let start = samples.len().saturating_sub(self.fft_size);
        let chunk = &samples[start..];

        for (i, &sample) in chunk.iter().enumerate() {
            self.fft_buffer[i] = Complex::new(sample * self.window[i], 0.0);
        }

        self.fft_scratch.copy_from_slice(&self.fft_buffer);

        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(self.fft_size);
        fft.process(&mut self.fft_scratch);

        for i in 0..self.magnitudes.len() {
            // Linear amplitude per bin; the dB mapping below turns this into a
            // perceptual (log) height. (Was a sqrt() curve with a fixed ×4 gain,
            // which crushed normal speech into the bottom few rows.)
            self.magnitudes[i] = self.fft_scratch[i].norm() / self.fft_size as f32;
        }

        let num_bars = num_bars.min(self.magnitudes.len());
        if num_bars == 0 {
            return;
        }

        let num_bins = self.magnitudes.len();

        for bar in 0..num_bars {
            // Logarithmic frequency bands: low frequencies (where speech energy
            // lives) spread across many bars instead of clumping into the first
            // few, so the spectrum fills all bars at any bar count.
            let (start_idx, end_idx) = bar_bin_range(bar, num_bars, num_bins);
            let band = &self.magnitudes[start_idx..end_idx];
            let avg: f32 = band.iter().sum::<f32>() / band.len() as f32;
            let scaled = level_from_magnitude(avg, settings.visualizer_floor_db);
            let decay = settings.visualizer_decay;
            let smoothing = settings.visualizer_smoothing;

            if scaled > self.smoothed[bar] {
                self.smoothed[bar] = lerp(self.smoothed[bar], scaled, 1.0 - smoothing);
            } else {
                self.smoothed[bar] = lerp(self.smoothed[bar], scaled, decay);
            }
        }
    }
}

impl Default for SpectrumVisualizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Visualizer for SpectrumVisualizer {
    /// Feed whichever tap is louder this frame — the mic while recording, the
    /// playback monitor while a clip plays — so the display follows the sound.
    fn update(&mut self, frame: &AudioFrame, settings: &Settings) {
        let source = louder_window(frame.input, frame.playback, self.fft_size);
        self.process(source, settings);
    }

    /// Render the spectrum as a retro dot-matrix display: a grid of round dots
    /// that light from the bottom up per column, EP-133 LCD style.
    fn draw(&self, painter: &Painter, rect: Rect, theme: &Theme, settings: &Settings) {
        let cols = settings.visualizer_num_bars.min(self.smoothed.len());
        if cols == 0 || rect.width() <= 4.0 || rect.height() <= 4.0 {
            return;
        }

        let pad = (rect.width().min(rect.height()) * 0.04).clamp(4.0, 14.0);
        let inner = rect.shrink(pad);

        // Choose a dot-grid resolution that keeps dots roughly square.
        let rows = ((inner.height() / (inner.width() / cols as f32)).round() as usize).clamp(6, 18);

        let cell_w = inner.width() / cols as f32;
        let cell_h = inner.height() / rows as f32;
        let dot_r = (cell_w.min(cell_h) * 0.34).max(0.8);

        for c in 0..cols {
            let level = self.smoothed[c].clamp(0.0, 1.0);
            let lit_rows = (level * rows as f32).round() as usize;
            let cx = inner.left() + (c as f32 + 0.5) * cell_w;

            for r in 0..rows {
                // r counted from the bottom.
                let cy = inner.bottom() - (r as f32 + 0.5) * cell_h;
                let center = Pos2::new(cx, cy);

                if r < lit_rows {
                    let h = r as f32 / (rows as f32 - 1.0).max(1.0);
                    let color = if h < 0.5 {
                        lerp_color(theme.visualizer_bar_low, theme.visualizer_bar_mid, h / 0.5)
                    } else {
                        lerp_color(
                            theme.visualizer_bar_mid,
                            theme.visualizer_bar_high,
                            (h - 0.5) / 0.5,
                        )
                    };
                    painter.circle_filled(center, dot_r, color);
                } else {
                    painter.circle_filled(center, dot_r * 0.85, theme.visualizer_dot_off);
                }
            }
        }
    }

    /// Fill the bars with a representative arch pattern. Used only by the
    /// screenshot harness so captures show the screen alive without a live mic.
    fn demo_fill(&mut self, num_bars: usize) {
        if num_bars > self.smoothed.len() {
            self.smoothed.resize(num_bars, 0.0);
        }
        for (i, v) in self.smoothed.iter_mut().take(num_bars).enumerate() {
            let x = i as f32 / num_bars.max(1) as f32;
            let arch = (std::f32::consts::PI * x).sin();
            *v = (0.15 + 0.85 * arch).clamp(0.0, 1.0);
        }
    }
}

/// Upper edge (dB) of the display window. Sounds at or above this fill the
/// column; the floor is user-tunable (dev panel). A window rather than a hard
/// gain means loud input tops out gracefully instead of clipping the display,
/// while normal speech still lands in the lively middle.
const CEIL_DB: f32 = -12.0;

/// Pick whichever tap carries more energy over its last `n` samples. This makes
/// the spectrum follow the microphone while recording and the playback monitor
/// while a clip plays, with no coupling to the UI mode. Ties favour the mic.
fn louder_window<'a>(input: &'a [f32], playback: &'a [f32], n: usize) -> &'a [f32] {
    let energy = |s: &[f32]| {
        let start = s.len().saturating_sub(n);
        s[start..].iter().map(|x| x * x).sum::<f32>()
    };
    if energy(playback) > energy(input) {
        playback
    } else {
        input
    }
}

/// FFT-bin range `[start, end)` for bar `bar` of `num_bars`, log-spaced from bin
/// 1 (bin 0 is DC and skipped) to `num_bins`. Because the bins map linearly to
/// frequency, log-spacing the edges gives each bar a geometrically wider band —
/// so the low, energy-dense end of the spectrum spreads across many bars and the
/// display fills at any bar count. Every band holds at least one bin.
fn bar_bin_range(bar: usize, num_bars: usize, num_bins: usize) -> (usize, usize) {
    debug_assert!(num_bars >= 1 && num_bins >= 2);
    let top = num_bins as f32;
    // edge(0) = 1, edge(num_bars) = num_bins, geometric in between.
    let edge = |b: usize| top.powf(b as f32 / num_bars as f32);
    let start = (edge(bar) as usize).clamp(1, num_bins - 1);
    let end = (edge(bar + 1) as usize).clamp(start + 1, num_bins);
    (start, end)
}

/// Map a linear amplitude to a `0.0..=1.0` display height on a decibel scale
/// between `floor_db` and [`CEIL_DB`]. Silence → 0, loud → 1. Display-only.
fn level_from_magnitude(avg: f32, floor_db: f32) -> f32 {
    let db = 20.0 * (avg + 1e-9).log10();
    // Guard the window so a misconfigured floor can never divide by ~zero.
    let span = (CEIL_DB - floor_db).max(1.0);
    ((db - floor_db) / span).clamp(0.0, 1.0)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_is_empty() {
        assert_eq!(level_from_magnitude(0.0, -60.0), 0.0);
    }

    #[test]
    fn at_ceiling_is_full() {
        // Amplitude whose dB equals CEIL_DB should map to the top.
        let amp = 10f32.powf(CEIL_DB / 20.0);
        assert!((level_from_magnitude(amp, -60.0) - 1.0).abs() < 1e-4);
    }

    #[test]
    fn at_floor_is_zero() {
        let floor = -60.0;
        let amp = 10f32.powf(floor / 20.0);
        assert!(level_from_magnitude(amp, floor).abs() < 1e-4);
    }

    #[test]
    fn midwindow_is_mid_height() {
        // Halfway (in dB) between floor and ceiling → ~0.5.
        let floor = -60.0;
        let mid_db = (floor + CEIL_DB) / 2.0;
        let amp = 10f32.powf(mid_db / 20.0);
        assert!((level_from_magnitude(amp, floor) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn lower_floor_gives_more_movement() {
        // Same quiet input reads higher with a lower (more sensitive) floor.
        let quiet = 0.001;
        let sensitive = level_from_magnitude(quiet, -75.0);
        let flat = level_from_magnitude(quiet, -45.0);
        assert!(sensitive > flat);
    }

    #[test]
    fn degenerate_floor_above_ceiling_is_safe() {
        // Must not panic or divide by ~zero if a config sets floor above ceiling.
        let v = level_from_magnitude(0.5, 0.0);
        assert!((0.0..=1.0).contains(&v));
    }

    #[test]
    fn bands_cover_full_range() {
        let n = 512;
        let bars = 36;
        assert_eq!(bar_bin_range(0, bars, n).0, 1); // skips DC, starts at bin 1
        assert_eq!(bar_bin_range(bars - 1, bars, n).1, n); // last band reaches the top
    }

    #[test]
    fn bands_are_nonempty_and_nondecreasing() {
        let n = 512;
        for bars in [16usize, 36, 100, 256] {
            let mut prev_start = 0;
            for b in 0..bars {
                let (lo, hi) = bar_bin_range(b, bars, n);
                assert!(hi > lo, "empty band at {b}/{bars}");
                assert!(lo < n && hi <= n, "out of range at {b}/{bars}");
                assert!(lo >= prev_start, "start went backwards at {b}/{bars}");
                prev_start = lo;
            }
        }
    }

    #[test]
    fn active_fraction_is_bar_count_independent() {
        // The bar whose band first reaches a fixed frequency bin should sit at
        // roughly the same FRACTION of the display regardless of bar count —
        // exactly the property the old linear binning lacked (there the fraction
        // shrank as bars grew, so the spectrum clumped at the left).
        let n = 512;
        let target = 93; // ~4 kHz at 44.1 kHz / 1024-pt FFT — the speech ceiling
        let frac = |bars: usize| {
            (0..bars)
                .find(|&b| bar_bin_range(b, bars, n).1 > target)
                .unwrap() as f32
                / bars as f32
        };
        assert!((frac(36) - frac(256)).abs() < 0.05);
    }

    #[test]
    fn tone_lands_away_from_the_left_edge() {
        // End-to-end: a 500 Hz tone (mid speech range) should light a bar near
        // the middle of a 128-bar display, not clump against the left edge the
        // way linear binning did.
        let sr = 44100;
        let mut viz = SpectrumVisualizer::new();
        let settings = crate::config::Settings {
            visualizer_num_bars: 128,
            ..crate::config::Settings::default()
        };

        let samples: Vec<f32> = (0..2048)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 500.0 * i as f32 / sr as f32).sin())
            .collect();
        let frame = AudioFrame {
            input: &samples,
            input_rate: sr,
            playback: &[],
            playback_rate: 0,
        };
        for _ in 0..40 {
            viz.update(&frame, &settings); // let the attack settle
        }

        let bars = 128;
        let (peak_bar, &peak) = viz.smoothed[..bars]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();
        assert!(peak > 0.3, "500 Hz tone barely registered (peak {peak})");
        assert!(
            peak_bar > bars / 8,
            "peak at bar {peak_bar}/{bars} — clumped left"
        );
    }

    #[test]
    fn louder_window_picks_the_stronger_tap() {
        let n = 8;
        let quiet = vec![0.01_f32; 16];
        let loud = vec![0.5_f32; 16];
        // Playback louder → playback wins; mic louder → mic wins.
        assert_eq!(louder_window(&quiet, &loud, n).as_ptr(), loud.as_ptr());
        assert_eq!(louder_window(&loud, &quiet, n).as_ptr(), loud.as_ptr());
        // Silence on both → default to the mic (input).
        let silent = vec![0.0_f32; 16];
        assert_eq!(
            louder_window(&quiet, &silent, n).as_ptr(),
            quiet.as_ptr(),
            "input should win when playback is silent"
        );
    }
}
