use anyhow::{Context, Result};
use std::io::Cursor;
use std::path::Path;

pub struct Sample {
    pub buf: Vec<f32>,
    pub has_data: bool,
    sample_rate: u32,
}

impl Sample {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            buf: Vec::new(),
            has_data: false,
            sample_rate,
        }
    }

    #[allow(dead_code)]
    pub fn push(&mut self, sample: f32) {
        self.buf.push(sample);
        self.has_data = true;
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.has_data = false;
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
    }

    pub fn encode_wav_bytes(&self) -> Result<Vec<u8>> {
        let mut cursor = Cursor::new(Vec::new());
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).context("Failed to create WAV writer")?;

        for &sample in &self.buf {
            let clamped = (sample * i16::MAX as f32).clamp(i16::MIN as f32, i16::MAX as f32) as i16;
            writer
                .write_sample(clamped)
                .context("Failed to write WAV sample")?;
        }

        writer.finalize().context("Failed to finalize WAV")?;
        Ok(cursor.into_inner())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let bytes = self.encode_wav_bytes()?;
        std::fs::write(path, bytes).context("Failed to write WAV file")?;
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self> {
        let mut reader = hound::WavReader::open(path).context("Failed to open WAV file")?;
        let spec = reader.spec();
        let mut buf = Vec::new();

        match spec.sample_format {
            hound::SampleFormat::Int => {
                for sample in reader.samples::<i16>() {
                    let s = sample.context("Failed to read sample")?;
                    buf.push(s as f32 / i16::MAX as f32);
                }
            }
            hound::SampleFormat::Float => {
                for sample in reader.samples::<f32>() {
                    let s = sample.context("Failed to read sample")?;
                    buf.push(s);
                }
            }
        }

        Ok(Self {
            buf,
            has_data: true,
            sample_rate: spec.sample_rate,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_roundtrip_preserves_rate_and_samples() {
        let mut sample = Sample::new(48000);
        for i in 0..1000 {
            sample.push(((i as f32 / 1000.0) * 2.0 - 1.0) * 0.9);
        }

        let mut path = std::env::temp_dir();
        path.push(format!("rondelek_wav_{}.wav", std::process::id()));
        sample.save(&path).unwrap();

        let loaded = Sample::load(&path).unwrap();
        assert_eq!(loaded.sample_rate(), 48000);
        assert_eq!(loaded.buf.len(), 1000);
        // 16-bit quantisation tolerance.
        for (a, b) in sample.buf.iter().zip(loaded.buf.iter()) {
            assert!((a - b).abs() < 1.0 / 32767.0 + 1e-4);
        }

        std::fs::remove_file(&path).ok();
    }
}
