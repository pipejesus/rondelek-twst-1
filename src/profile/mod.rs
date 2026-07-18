//! A *profile* is one child. Profiles live in an app-managed library under the OS
//! data dir; each profile folder holds a `profile.json`, an optional square
//! `avatar.png`, and a `sessions/` subfolder of timestamped sessions.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::session::Session;
use crate::util::{format_timestamp, new_uid, now_secs, short_uid};

const PROFILE_MANIFEST: &str = "profile.json";
const AVATAR_FILE: &str = "avatar.png";
const CALIBRATION_FILE: &str = "calibration.json";
const SESSIONS_DIR: &str = "sessions";
const AVATAR_SIZE: u32 = 256;
const MAX_NAME_LEN: usize = 80;

/// Root of the profile library, e.g. `~/Library/Application Support/rondelek/profiles`.
pub fn library_root() -> PathBuf {
    let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("rondelek").join("profiles")
}

/// Trim, strip control characters, collapse whitespace and cap the length. The
/// result is what we store and display; it never touches a filesystem path.
pub fn sanitize_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut last_was_space = false;
    for c in raw.trim().chars() {
        if c.is_control() {
            continue;
        }
        if c.is_whitespace() {
            if !last_was_space {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(c);
            last_was_space = false;
        }
    }
    out.chars()
        .take(MAX_NAME_LEN)
        .collect::<String>()
        .trim()
        .to_string()
}

/// A filesystem-safe slug derived from a name: lowercase ASCII alphanumerics,
/// other runs collapsed to single `-`. Falls back to `profile` when empty.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.extend(c.to_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let s = out.trim_matches('-');
    let s: String = s.chars().take(32).collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() {
        "profile".to_string()
    } else {
        s
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfileManifest {
    /// Stable id for cross-referencing (notes, etc.). Persisted via serde.
    #[allow(dead_code)]
    pub uid: String,
    /// Display name (sanitised, but may contain spaces/unicode).
    pub name: String,
    /// Avatar filename relative to the profile folder, if set.
    pub avatar: Option<String>,
    pub created: u64,
}

#[derive(Clone, Debug)]
pub struct Profile {
    pub dir: PathBuf,
    pub manifest: ProfileManifest,
}

/// Lightweight info about a stored session (without loading its samples).
#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub dir: PathBuf,
    pub folder: String,
    pub created: u64,
    /// Session uid, for future cross-referencing (notes, search).
    #[allow(dead_code)]
    pub uid: String,
}

impl Profile {
    fn manifest_path(dir: &Path) -> PathBuf {
        dir.join(PROFILE_MANIFEST)
    }

    /// Create a new profile folder (`<slug>-<short_uid>`) and write its manifest.
    /// `avatar_src`, if given, is decoded and stored as a square `avatar.png`.
    pub fn create(name: &str, avatar_src: Option<&Path>) -> Result<Self> {
        let name = sanitize_name(name);
        let uid = new_uid();
        let folder = format!("{}-{}", slug(&name), short_uid(&uid));
        let dir = library_root().join(folder);
        std::fs::create_dir_all(dir.join(SESSIONS_DIR))
            .context("Failed to create profile folder")?;

        let mut profile = Self {
            dir,
            manifest: ProfileManifest {
                uid,
                name,
                avatar: None,
                created: now_secs(),
            },
        };
        if let Some(src) = avatar_src {
            profile.set_avatar(src)?;
        }
        profile.save_manifest()?;
        Ok(profile)
    }

    pub fn load(dir: PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(Self::manifest_path(&dir))
            .context("Failed to read profile.json")?;
        let manifest: ProfileManifest =
            serde_json::from_str(&content).context("Failed to parse profile.json")?;
        Ok(Self { dir, manifest })
    }

    pub fn save_manifest(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(&self.manifest)
            .context("Failed to serialize profile manifest")?;
        std::fs::write(Self::manifest_path(&self.dir), json)
            .context("Failed to write profile.json")?;
        Ok(())
    }

    pub fn name(&self) -> &str {
        &self.manifest.name
    }

    pub fn avatar_path(&self) -> Option<PathBuf> {
        self.manifest.avatar.as_ref().map(|f| self.dir.join(f))
    }

    /// Decode an image file (PNG/JPEG/WebP) and store it as a square `avatar.png`.
    pub fn set_avatar(&mut self, src: &Path) -> Result<()> {
        let img = image::open(src).context("Failed to read image")?;
        self.store_avatar(img)
    }

    fn store_avatar(&mut self, img: image::DynamicImage) -> Result<()> {
        let square = square_avatar(img);
        let path = self.dir.join(AVATAR_FILE);
        square
            .save_with_format(&path, image::ImageFormat::Png)
            .context("Failed to write avatar.png")?;
        self.manifest.avatar = Some(AVATAR_FILE.to_string());
        self.save_manifest()
    }

    /// Update the display name (sanitised) and persist. The on-disk folder name
    /// is intentionally left unchanged — the manifest is the source of truth for
    /// the display name.
    pub fn set_name(&mut self, raw: &str) -> Result<()> {
        self.manifest.name = sanitize_name(raw);
        self.save_manifest()
    }

    /// Remove the stored avatar (file + manifest field), reverting to the default
    /// face. A no-op if no avatar is set.
    pub fn clear_avatar(&mut self) -> Result<()> {
        if let Some(file) = self.manifest.avatar.take() {
            let path = self.dir.join(file);
            if path.exists() {
                std::fs::remove_file(&path).context("Failed to remove avatar file")?;
            }
        }
        self.save_manifest()
    }

    fn calibration_path(dir: &Path) -> PathBuf {
        dir.join(CALIBRATION_FILE)
    }

    /// Load this profile's vowel calibration, if it has been calibrated with the
    /// current format. Missing, unreadable, or outdated (e.g. the pre-MFCC
    /// formant format) files simply mean "not calibrated".
    pub fn load_calibration(&self) -> Option<crate::audio::vowel::VowelCalibration> {
        let content = std::fs::read_to_string(Self::calibration_path(&self.dir)).ok()?;
        serde_json::from_str::<crate::audio::vowel::VowelCalibration>(&content)
            .ok()
            .filter(|c| c.is_valid())
    }

    /// Persist a vowel calibration for this profile.
    pub fn save_calibration(&self, cal: &crate::audio::vowel::VowelCalibration) -> Result<()> {
        let json = serde_json::to_string_pretty(cal).context("Failed to serialize calibration")?;
        std::fs::write(Self::calibration_path(&self.dir), json)
            .context("Failed to write calibration.json")?;
        Ok(())
    }

    fn sessions_dir(&self) -> PathBuf {
        self.dir.join(SESSIONS_DIR)
    }

    /// All sessions for this profile, newest first.
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir(self.sessions_dir()) {
            for entry in entries.flatten() {
                let dir = entry.path();
                if !dir.is_dir() {
                    continue;
                }
                let folder = entry.file_name().to_string_lossy().into_owned();
                // Read uid/created from the manifest if present.
                let (uid, created) = std::fs::read_to_string(dir.join("session.json"))
                    .ok()
                    .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                    .map(|v| {
                        (
                            v.get("uid")
                                .and_then(|u| u.as_str())
                                .unwrap_or("")
                                .to_string(),
                            v.get("created").and_then(|c| c.as_u64()).unwrap_or(0),
                        )
                    })
                    .unwrap_or_default();
                out.push(SessionInfo {
                    dir,
                    folder,
                    created,
                    uid,
                });
            }
        }
        out.sort_by(|a, b| b.created.cmp(&a.created).then(b.folder.cmp(&a.folder)));
        out
    }

    /// Create a fresh timestamped session under this profile.
    pub fn new_session(&self) -> Result<Session> {
        let base = self.sessions_dir();
        let ts = format_timestamp(now_secs());
        let mut dir = base.join(&ts);
        if dir.exists() {
            dir = base.join(format!("{ts}-{}", short_uid(&new_uid())));
        }
        std::fs::create_dir_all(&dir).context("Failed to create session folder")?;
        Session::create(dir)
    }
}

/// Centre-crop to a square and resize to `AVATAR_SIZE`.
fn square_avatar(img: image::DynamicImage) -> image::DynamicImage {
    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    let x = (w - side) / 2;
    let y = (h - side) / 2;
    img.crop_imm(x, y, side, side).resize_exact(
        AVATAR_SIZE,
        AVATAR_SIZE,
        image::imageops::FilterType::Lanczos3,
    )
}

/// List all profiles in the library, sorted by name (case-insensitive).
pub fn list_profiles() -> Vec<Profile> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(library_root()) {
        for entry in entries.flatten() {
            let dir = entry.path();
            if dir.is_dir()
                && let Ok(profile) = Profile::load(dir)
            {
                out.push(profile);
            }
        }
    }
    out.sort_by_key(|p| p.name().to_lowercase());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_collapses_and_trims() {
        assert_eq!(sanitize_name("  Maya\t\n  Smith  "), "Maya Smith");
        assert_eq!(sanitize_name("Łukasz"), "Łukasz");
        assert_eq!(sanitize_name(""), "");
    }

    #[test]
    fn slug_is_path_safe() {
        assert_eq!(slug("Maya Smith"), "maya-smith");
        assert_eq!(slug("../../etc/passwd"), "etc-passwd");
        assert_eq!(slug("***"), "profile");
        assert_eq!(slug("Łukasz 7"), "ukasz-7");
    }

    #[test]
    fn calibration_round_trips() {
        use crate::audio::vowel::{N_MFCC, VOWELS, build_calibration};
        let profile = Profile::create("Cal Kid", None).unwrap();
        let dir = profile.dir.clone();
        assert!(profile.load_calibration().is_none(), "starts uncalibrated");

        // Six vowels' worth of distinct MFCC-shaped frames.
        let per_vowel: Vec<Vec<Vec<f32>>> = (0..VOWELS.len())
            .map(|v| (0..20).map(|_| vec![v as f32; N_MFCC]).collect())
            .collect();
        let cal =
            build_calibration(&per_vowel, 44_100, Some("Test Mic".into()), 123).expect("built");
        profile.save_calibration(&cal).unwrap();

        let loaded = Profile::load(dir.clone())
            .unwrap()
            .load_calibration()
            .expect("calibration loads back");
        assert_eq!(loaded, cal);
        assert!(loaded.is_valid());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn create_profile_and_session_roundtrip() {
        // Redirect the library to a temp dir via a fixed name; create under it
        // and verify structure. (Uses the real library_root; cleaned up after.)
        let profile = Profile::create("Test Kid", None).unwrap();
        assert!(profile.dir.join("profile.json").exists());
        assert!(profile.dir.join("sessions").is_dir());
        assert!(!profile.manifest.uid.is_empty());

        let session = profile.new_session().unwrap();
        assert!(session.dir.join("session.json").exists());
        assert!(!session.manifest.uid.is_empty());

        let sessions = profile.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].uid, session.manifest.uid);

        std::fs::remove_dir_all(&profile.dir).ok();
    }

    #[test]
    fn set_name_preserves_identity() {
        let mut profile = Profile::create("Old Name", None).unwrap();
        let uid = profile.manifest.uid.clone();
        let created = profile.manifest.created;
        let dir = profile.dir.clone();

        profile.set_name("New Name").unwrap();

        // Reload from disk to confirm persistence and that identity is intact.
        let reloaded = Profile::load(dir.clone()).unwrap();
        assert_eq!(reloaded.name(), "New Name");
        assert_eq!(reloaded.manifest.uid, uid);
        assert_eq!(reloaded.manifest.created, created);
        assert!(dir.join("sessions").is_dir());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clear_avatar_removes_file_and_field() {
        let src = std::env::temp_dir().join(format!("rondelek_avc_{}.png", now_secs()));
        image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255]))
            .save(&src)
            .unwrap();

        let mut profile = Profile::create("Avatar Kid", Some(&src)).unwrap();
        let dir = profile.dir.clone();
        let avatar_path = profile.avatar_path().expect("avatar should be set");
        assert!(avatar_path.exists());

        profile.clear_avatar().unwrap();
        assert!(profile.manifest.avatar.is_none());
        assert!(!avatar_path.exists());

        let reloaded = Profile::load(dir.clone()).unwrap();
        assert!(reloaded.manifest.avatar.is_none());

        std::fs::remove_file(&src).ok();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn set_avatar_replaces_existing() {
        let src_a = std::env::temp_dir().join(format!("rondelek_ava_{}.png", now_secs()));
        let src_b = std::env::temp_dir().join(format!("rondelek_avb_{}.png", now_secs()));
        image::RgbaImage::from_pixel(8, 8, image::Rgba([1, 2, 3, 255]))
            .save(&src_a)
            .unwrap();
        image::RgbaImage::from_pixel(8, 8, image::Rgba([9, 8, 7, 255]))
            .save(&src_b)
            .unwrap();

        let mut profile = Profile::create("Swap Kid", Some(&src_a)).unwrap();
        let dir = profile.dir.clone();
        assert!(profile.avatar_path().unwrap().exists());

        profile.set_avatar(&src_b).unwrap();
        let reloaded = Profile::load(dir.clone()).unwrap();
        assert_eq!(reloaded.manifest.avatar.as_deref(), Some(AVATAR_FILE));
        assert!(reloaded.avatar_path().unwrap().exists());

        std::fs::remove_file(&src_a).ok();
        std::fs::remove_file(&src_b).ok();
        std::fs::remove_dir_all(&dir).ok();
    }
}
