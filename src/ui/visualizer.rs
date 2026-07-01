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
            let c = self.fft_scratch[i];
            self.magnitudes[i] = (c.norm() / self.fft_size as f32).sqrt();
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
            let scaled = (avg * 4.0).min(1.0);
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

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgb(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
    )
}
