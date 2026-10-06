//! Frame timing for performance work (`RONDELEK_GAME_STATS=1`): where each
//! gameplay frame's time goes, printed to stderr when the game ends.
//!
//! Measure the GPU's share with the frame cap off (`RONDELEK_GAME_FPS=0`) and
//! vsync off (`__GL_SYNC_TO_VBLANK=0` on NVIDIA, `vblank_mode=0` on Mesa): then
//! `present` is the time spent waiting for the GPU to finish the frame.

use std::time::Duration;

/// Frames left out at the start: shader compiles and first uploads.
const WARMUP: usize = 30;

/// One frame's split, in milliseconds.
#[derive(Clone, Copy)]
struct Frame {
    update: f32,
    /// Building the frame on the CPU (draw calls issued, not yet finished).
    draw: f32,
    /// `EndDrawing`: flush, swap, and the frame cap's wait.
    present: f32,
}

#[derive(Default)]
pub struct FrameStats {
    frames: Vec<Frame>,
}

impl FrameStats {
    pub fn push(&mut self, update: Duration, draw: Duration, present: Duration) {
        let ms = |d: Duration| d.as_secs_f32() * 1000.0;
        self.frames.push(Frame {
            update: ms(update),
            draw: ms(draw),
            present: ms(present),
        });
    }

    /// One line: frame-time percentiles and the mean of each part.
    pub fn summary(&self) -> String {
        let frames = self.frames.get(WARMUP..).unwrap_or_default();
        if frames.is_empty() {
            return format!("{} frames: too few to time", self.frames.len());
        }
        let mut total: Vec<f32> = frames
            .iter()
            .map(|f| f.update + f.draw + f.present)
            .collect();
        total.sort_by(f32::total_cmp);
        let pct = |p: f32| total[((total.len() - 1) as f32 * p).round() as usize];
        let mean =
            |part: fn(&Frame) -> f32| frames.iter().map(part).sum::<f32>() / frames.len() as f32;
        let frame_mean = total.iter().sum::<f32>() / total.len() as f32;
        format!(
            "{} frames · frame {:.2} ms ({:.0} fps; p50 {:.2}, p95 {:.2}, p99 {:.2}) · \
             update {:.3} ms · draw {:.3} ms · present {:.2} ms",
            frames.len(),
            frame_mean,
            1000.0 / frame_mean,
            pct(0.5),
            pct(0.95),
            pct(0.99),
            mean(|f| f.update),
            mean(|f| f.draw),
            mean(|f| f.present),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warmup_frames_are_left_out() {
        let mut s = FrameStats::default();
        for _ in 0..WARMUP {
            s.push(Duration::from_secs(1), Duration::ZERO, Duration::ZERO);
        }
        for _ in 0..10 {
            s.push(
                Duration::from_millis(1),
                Duration::from_millis(2),
                Duration::from_millis(3),
            );
        }
        let line = s.summary();
        assert!(line.starts_with("10 frames · frame 6.00 ms"), "{line}");
    }
}
