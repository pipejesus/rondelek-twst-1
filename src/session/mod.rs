//! A *session* is a folder on disk holding one child's set of recordings plus a
//! `session.json` manifest. One installation can hold many sessions, and a guide
//! can reopen any previous one. Samples are stored as `pad_NN.wav` alongside the
//! manifest.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::audio::Sample;
use crate::config::NUM_SAMPLES;
use crate::util::{new_uid, now_secs};

const MANIFEST_NAME: &str = "session.json";

fn wav_name(idx: usize) -> String {
    format!("pad_{:02}.wav", idx + 1)
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PadEntry {
    pub label: String,
    /// WAV filename relative to the session folder, if a recording exists.
    pub file: Option<String>,
    pub has_sample: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionManifest {
    /// Stable id for cross-referencing (notes, etc.). Older manifests without one
    /// get a fresh uid on load.
    #[serde(default = "new_uid")]
    pub uid: String,
    pub name: String,
    pub created: u64,
    pub modified: u64,
    pub pads: Vec<PadEntry>,
}

impl SessionManifest {
    fn new(name: String) -> Self {
        let now = now_secs();
        Self {
            uid: new_uid(),
            name,
            created: now,
            modified: now,
            pads: vec![PadEntry::default(); NUM_SAMPLES],
        }
    }

    /// Ensure the pad list always has exactly `NUM_SAMPLES` entries (tolerates
    /// manifests written by a different build).
    fn normalize(&mut self) {
        self.pads.resize(NUM_SAMPLES, PadEntry::default());
    }
}

#[derive(Clone, Debug)]
pub struct Session {
    pub dir: PathBuf,
    pub manifest: SessionManifest,
}

impl Session {
    fn manifest_path(dir: &Path) -> PathBuf {
        dir.join(MANIFEST_NAME)
    }

    /// Create a brand-new session in `dir` (created if needed). The session name
    /// defaults to the folder's own name.
    pub fn create(dir: PathBuf) -> Result<Self> {
        std::fs::create_dir_all(&dir).context("Failed to create session folder")?;
        let name = dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Session".to_string());
        let session = Self {
            dir,
            manifest: SessionManifest::new(name),
        };
        session.save_manifest()?;
        Ok(session)
    }

    /// Open an existing session folder. If no manifest is present (e.g. the user
    /// pointed at a plain folder), a fresh one is created in place.
    pub fn open(dir: PathBuf) -> Result<Self> {
        let path = Self::manifest_path(&dir);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let mut manifest: SessionManifest =
                    serde_json::from_str(&content).context("Failed to parse session.json")?;
                manifest.normalize();
                Ok(Self { dir, manifest })
            }
            Err(_) => Self::create(dir),
        }
    }

    #[allow(dead_code)] // used by session search (planned)
    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn save_manifest(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.manifest)
            .context("Failed to serialize session manifest")?;
        std::fs::write(Self::manifest_path(&self.dir), json)
            .context("Failed to write session.json")?;
        Ok(())
    }

    /// Persist a recorded sample for `idx` and update the manifest.
    pub fn save_sample(&mut self, idx: usize, sample: &Sample) -> Result<()> {
        let name = wav_name(idx);
        sample.save(&self.dir.join(&name))?;
        if let Some(entry) = self.manifest.pads.get_mut(idx) {
            entry.file = Some(name);
            entry.has_sample = true;
        }
        self.manifest.modified = now_secs();
        self.save_manifest()
    }

    /// Load all pad samples referenced by the manifest. Missing/unreadable files
    /// yield an empty sample at `default_rate` so indexing stays aligned.
    pub fn load_samples(&self, default_rate: u32) -> Vec<Sample> {
        (0..NUM_SAMPLES)
            .map(|idx| {
                let entry = self.manifest.pads.get(idx);
                let file = entry.and_then(|e| e.file.as_ref());
                if let Some(file) = file {
                    let path = self.dir.join(file);
                    if path.exists()
                        && let Ok(sample) = Sample::load(&path)
                    {
                        return sample;
                    }
                }
                Sample::new(default_rate)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("rondelek_test_{}_{}", tag, now_secs()));
        p
    }

    #[test]
    fn create_writes_manifest_with_all_pads() {
        let dir = temp_dir("create");
        let session = Session::create(dir.clone()).unwrap();
        assert_eq!(session.manifest.pads.len(), NUM_SAMPLES);
        assert!(Session::manifest_path(&dir).exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_and_reload_sample_roundtrips() {
        let dir = temp_dir("roundtrip");
        let mut session = Session::create(dir.clone()).unwrap();

        let mut sample = Sample::new(44100);
        for i in 0..256 {
            sample.push((i as f32 / 256.0) * 2.0 - 1.0);
        }
        session.save_sample(3, &sample).unwrap();

        // Reopen from disk and confirm the recording is restored.
        let reopened = Session::open(dir.clone()).unwrap();
        assert!(reopened.manifest.pads[3].has_sample);
        let loaded = reopened.load_samples(44100);
        assert!(loaded[3].has_data);
        assert_eq!(loaded[3].buf.len(), 256);
        assert!(!loaded[0].has_data);

        std::fs::remove_dir_all(&dir).ok();
    }
}
