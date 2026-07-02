use crate::config::{Settings, Theme};
use crate::util::lerp;
use egui::{Color32, Painter, Pos2, Rect};
use rustfft::{FftPlanner, num_complex::Complex};

pub struct Visualizer {
    fft_buffer: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    window: Vec<f32>,
    magnitudes: Vec<f32>,
    smoothed: Vec<f32>,
    fft_size: usize,
    sample_rate: u32,
}

impl Visualizer {
    pub fn new(sample_rate: u32) -> Self {
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
            sample_rate,
        }
    }

    pub fn process_samples(&mut self, samples: &[f32], settings: &Settings) {
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

        let bins_per_bar = self.magnitudes.len() / num_bars;

        for bar in 0..num_bars {
            let start_idx = bar * bins_per_bar;
            let end_idx = start_idx + bins_per_bar;
            let avg: f32 =
                self.magnitudes[start_idx..end_idx].iter().sum::<f32>() / bins_per_bar as f32;
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

    /// Render the spectrum as a retro dot-matrix display: a grid of round dots
    /// that light from the bottom up per column, EP-133 LCD style.
    pub fn draw(&self, painter: &Painter, rect: Rect, theme: &Theme, settings: &Settings) {
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

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Fill the bars with a representative arch pattern. Used only by the
    /// screenshot harness so captures show the screen alive without a live mic.
    pub fn demo_fill(&mut self, num_bars: usize) {
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
}
