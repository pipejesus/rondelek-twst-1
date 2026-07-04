use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, StreamTrait};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// A mono clip being played back, with a per-frame read cursor.
type Source = (Vec<f32>, usize);
type Sources = Arc<Mutex<Vec<Source>>>;
/// Monitor tap: the mixed mono signal actually sent to the speakers, so a
/// visualizer can respond to playback (not just the microphone).
type Monitor = Arc<Mutex<VecDeque<f32>>>;

pub struct Playback {
    stream: Option<cpal::Stream>,
    sources: Sources,
    monitor: Monitor,
    sample_rate: u32,
    alive: Arc<AtomicBool>,
    current_device: String,
}

impl Playback {
    /// Build an output stream on `device`. The stream's error callback flips an
    /// `alive` flag so the watchdog can detect a disconnected device.
    pub fn open(device: &cpal::Device) -> Result<Self> {
        let current_device = device
            .description()
            .ok()
            .map(|d| d.name().to_string())
            .unwrap_or_default();

        let supported = device
            .default_output_config()
            .context("Failed to get default output config")?;

        let channels = supported.channels().max(1) as usize;
        let sample_rate: u32 = supported.sample_rate();

        let sources: Sources = Arc::new(Mutex::new(Vec::new()));
        let src_clone = Arc::clone(&sources);

        let monitor: Monitor = Arc::new(Mutex::new(VecDeque::with_capacity(8192)));
        let monitor_cb = Arc::clone(&monitor);

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
                        let mut mon = monitor_cb.lock().ok();
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
                            // Every channel in the frame gets the same mixed
                            // value, so playback speed/pitch stays independent
                            // of the output device's channel count.
                            for sample in frame.iter_mut() {
                                *sample = value;
                            }
                            // Tap the signal for the visualizer, but only while
                            // something is actually playing — when idle we push
                            // nothing, so the monitor stays empty and the mic
                            // keeps driving the display.
                            if active > 0
                                && let Some(mon) = mon.as_deref_mut()
                            {
                                if mon.len() > 32768 {
                                    mon.pop_front();
                                }
                                mon.push_back(value);
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
            monitor,
            sample_rate,
            alive,
            current_device,
        })
    }

    /// Drain the monitor tap — the mixed mono samples sent to the speakers since
    /// the last call. Empty when nothing is playing. Runs at [`output_rate`].
    pub fn drain_monitor(&mut self) -> Vec<f32> {
        match self.monitor.lock() {
            Ok(mut mon) => mon.drain(..).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Sample rate of the monitor tap (the output device rate).
    pub fn output_rate(&self) -> u32 {
        self.sample_rate
    }

    /// False once cpal has reported a stream error (e.g. device disconnected).
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Name of the device this stream was built on.
    pub fn current_device(&self) -> &str {
        &self.current_device
    }

    /// Queue a mono clip for playback, resampling from `src_rate` to the output
    /// device rate so pitch and duration are preserved.
    pub fn play(&mut self, buffer: Vec<f32>, src_rate: u32) {
        let buffer = resample_linear(buffer, src_rate, self.sample_rate);
        if let Ok(mut sources) = self.sources.lock() {
            sources.push((buffer, 0));
        }
    }

    pub fn stop(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.pause();
            drop(stream);
        }
    }
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Simple linear-interpolation resampler for a mono buffer. Returns the input
/// unchanged when the rates already match (or are degenerate).
pub fn resample_linear(input: Vec<f32>, src_rate: u32, dst_rate: u32) -> Vec<f32> {
    if src_rate == dst_rate || src_rate == 0 || dst_rate == 0 || input.len() < 2 {
        return input;
    }

    let ratio = dst_rate as f64 / src_rate as f64;
    let out_len = ((input.len() as f64) * ratio).round() as usize;
    if out_len == 0 {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(out_len);
    let step = src_rate as f64 / dst_rate as f64;
    for i in 0..out_len {
        let pos = i as f64 * step;
        let idx = pos.floor() as usize;
        let frac = (pos - idx as f64) as f32;
        let a = input[idx];
        let b = if idx + 1 < input.len() {
            input[idx + 1]
        } else {
            a
        };
        out.push(a + (b - a) * frac);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_same_rate_is_identity() {
        let input = vec![0.1, 0.2, 0.3, 0.4];
        let out = resample_linear(input.clone(), 44100, 44100);
        assert_eq!(input, out);
    }

    #[test]
    fn resample_doubling_rate_roughly_doubles_length() {
        let input = vec![0.0; 100];
        let out = resample_linear(input, 22050, 44100);
        assert_eq!(out.len(), 200);
    }

    #[test]
    fn resample_halving_rate_roughly_halves_length() {
        let input = vec![0.0; 100];
        let out = resample_linear(input, 48000, 24000);
        assert_eq!(out.len(), 50);
    }

    #[test]
    fn resample_preserves_constant_signal() {
        let input = vec![0.5; 50];
        let out = resample_linear(input, 16000, 44100);
        for v in out {
            assert!((v - 0.5).abs() < 1e-6);
        }
    }
}
