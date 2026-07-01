use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

pub struct Capture {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    sample_rate: u32,
    alive: Arc<AtomicBool>,
    current_device: String,
}

impl Capture {
    /// Build an input stream on `device`, folding multi-channel frames to mono.
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

    pub fn drain(&mut self) -> Vec<f32> {
        match self.buffer.lock() {
            Ok(mut buf) => {
                let len = buf.len();
                let drained: Vec<f32> = buf.drain(..).collect();
                buf.reserve(len.min(8192));
                drained
            }
            Err(_) => Vec::new(),
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
fn fold_to_mono(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    frame.iter().sum::<f32>() / frame.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_frame_passes_through() {
        assert_eq!(fold_to_mono(&[0.5]), 0.5);
    }

    #[test]
    fn stereo_frame_averages_channels() {
        assert_eq!(fold_to_mono(&[0.2, 0.4]), 0.3);
        assert_eq!(fold_to_mono(&[-1.0, 1.0]), 0.0);
    }

    #[test]
    fn empty_frame_is_silent() {
        assert_eq!(fold_to_mono(&[]), 0.0);
    }
}
