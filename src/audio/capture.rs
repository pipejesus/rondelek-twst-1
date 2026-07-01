use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct Capture {
    stream: Option<cpal::Stream>,
    buffer: Arc<Mutex<VecDeque<f32>>>,
    sample_rate: u32,
}

impl Capture {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input audio device found")?;

        let supported = device
            .default_input_config()
            .context("Failed to get default input config")?;

        let sample_rate: u32 = supported.sample_rate();
        let channels = supported.channels();

        let buffer = Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
        let buf_clone = Arc::clone(&buffer);

        let config = cpal::StreamConfig {
            channels,
            sample_rate: supported.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };

        // The device may capture in stereo (or more). The rest of the app works
        // with a single mono stream, so fold each interleaved frame down to one
        // sample by averaging its channels before buffering.
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
                |err| eprintln!("Capture error: {err}"),
                None,
            )
            .context("Failed to build input stream")?;

        stream.play().context("Failed to start input stream")?;

        Ok(Self {
            stream: Some(stream),
            buffer,
            sample_rate,
        })
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
