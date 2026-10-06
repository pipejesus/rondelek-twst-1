use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{InputCallbackInfo, SampleFormat};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The most mono samples the tap holds (about 0.7 s at 48 kHz) when nobody
/// drains it: the oldest go first.
const TAP_MAX_SAMPLES: usize = 32768;

pub struct Capture {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    sample_rate: u32,
    alive: Arc<AtomicBool>,
    current_device: String,
}

impl Capture {
    /// Build an input stream on `device`, converting whatever the hardware's
    /// native sample format is to mono `f32`. Hardware mics are commonly `i16`
    /// or `i32`, **not** `f32` — building an `f32`-only stream fails outright on
    /// those (e.g. "sample format f32 is not supported"), which is why every real
    /// mic must be opened in its native format and converted here.
    pub fn open(device: &cpal::Device) -> Result<Self> {
        let current_device = device
            .description()
            .ok()
            .map(|d| d.name().to_string())
            .unwrap_or_default();

        let supported = device
            .default_input_config()
            .context("Failed to get default input config")?;

        let sample_rate: u32 = supported.sample_rate();
        let channels = supported.channels().max(1) as usize;
        let sample_format = supported.sample_format();

        let buffer = Arc::new(Mutex::new(VecDeque::with_capacity(TAP_MAX_SAMPLES + 1)));
        let alive = Arc::new(AtomicBool::new(true));

        let config = cpal::StreamConfig {
            channels: supported.channels(),
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        // One arm per hardware sample format: build a typed input stream and fold
        // each interleaved frame down to a single mono `f32` sample.
        macro_rules! build {
            ($t:ty, $to_f32:expr) => {{
                let buf = Arc::clone(&buffer);
                let alive_cb = Arc::clone(&alive);
                let conv: fn($t) -> f32 = $to_f32;
                device.build_input_stream(
                    config.clone(),
                    move |data: &[$t], _: &InputCallbackInfo| {
                        // The real-time thread: no allocation here.
                        if let Ok(mut b) = buf.lock() {
                            for frame in data.chunks(channels) {
                                if b.len() > TAP_MAX_SAMPLES {
                                    let _ = b.pop_front();
                                }
                                b.push_back(fold_to_mono(frame.iter().map(|&s| conv(s))));
                            }
                        }
                    },
                    move |err| {
                        alive_cb.store(false, Ordering::Relaxed);
                        eprintln!("Capture error: {err}");
                    },
                    None,
                )
            }};
        }

        let stream = match sample_format {
            SampleFormat::F32 => build!(f32, |s| s),
            SampleFormat::I16 => build!(i16, |s| s as f32 / 32768.0),
            SampleFormat::I32 => build!(i32, |s| s as f32 / 2_147_483_648.0),
            SampleFormat::I8 => build!(i8, |s| s as f32 / 128.0),
            SampleFormat::U16 => build!(u16, |s| (s as f32 - 32768.0) / 32768.0),
            SampleFormat::U8 => build!(u8, |s| (s as f32 - 128.0) / 128.0),
            SampleFormat::F64 => build!(f64, |s| s as f32),
            other => return Err(anyhow!("Unsupported input sample format: {other:?}")),
        }
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

    /// False once cpal has reported a stream error (e.g. device disconnected).
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Name of the device this stream was built on.
    pub fn current_device(&self) -> &str {
        &self.current_device
    }

    /// Everything captured since the last drain, oldest first.
    pub fn drain(&mut self) -> Vec<f32> {
        let mut out = Vec::new();
        self.drain_into(&mut out);
        out
    }

    /// [`Capture::drain`], appended to `out` (no allocation once `out` has
    /// room). Returns how many samples it appended.
    pub fn drain_into(&mut self, out: &mut Vec<f32>) -> usize {
        match self.buffer.lock() {
            Ok(mut buf) => {
                let n = buf.len();
                out.extend(buf.drain(..));
                n
            }
            Err(_) => 0,
        }
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
            drop(stream);
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Average one interleaved frame's channels down to a single mono sample.
fn fold_to_mono(frame: impl ExactSizeIterator<Item = f32>) -> f32 {
    let n = frame.len();
    if n == 0 {
        return 0.0;
    }
    frame.sum::<f32>() / n as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_frame_passes_through() {
        assert_eq!(fold_to_mono([0.5].into_iter()), 0.5);
    }

    #[test]
    fn stereo_frame_averages_channels() {
        assert_eq!(fold_to_mono([0.2, 0.4].into_iter()), 0.3);
        assert_eq!(fold_to_mono([-1.0, 1.0].into_iter()), 0.0);
    }

    #[test]
    fn empty_frame_is_silent() {
        assert_eq!(fold_to_mono([].into_iter()), 0.0);
    }
}
