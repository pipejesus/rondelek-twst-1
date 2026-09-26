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
/// Deleted profiles are moved here (inside the library root), not erased.
/// `list_profiles` skips it because it has no `profile.json`.
const TRASH_DIR: &str = ".trash";
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
    /// Avatar photo filename relative to the profile folder, if set.
    #[serde(default)]
    pub avatar: Option<String>,
    /// Built-in character avatar (app asset name), used when there is no photo.
    /// Stored by name, so redrawing the character art updates every profile.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
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

    /// Create a new profile folder (`<slug>-<short_uid>`) in the user's library
    /// and write its manifest. `avatar_src`, if given, is decoded and stored as a
    /// square `avatar.png`.
    pub fn create(name: &str, avatar_src: Option<&Path>) -> Result<Self> {
        Self::create_in(&library_root(), name, avatar_src)
    }

    /// [`Self::create`] under an explicit library `root` (tests use a temp dir so
    /// they never touch the real library).
    pub fn create_in(root: &Path, name: &str, avatar_src: Option<&Path>) -> Result<Self> {
        let name = sanitize_name(name);
        let uid = new_uid();
        let folder = format!("{}-{}", slug(&name), short_uid(&uid));
        let dir = root.join(folder);
        std::fs::create_dir_all(dir.join(SESSIONS_DIR))
            .context("Failed to create profile folder")?;

        let mut profile = Self {
            dir,
            manifest: ProfileManifest {
                uid,
                name,
                avatar: None,
                character: None,
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
        self.manifest.character = None;
        self.save_manifest()
    }

    /// The built-in character avatar, if one is chosen (and there is no photo).
    pub fn character(&self) -> Option<&str> {
        self.manifest.character.as_deref()
    }

    /// Use a built-in character as the avatar. Removes any photo first, so the
    /// two never compete.
    pub fn set_character(&mut self, name: &str) -> Result<()> {
        self.remove_avatar_file()?;
        self.manifest.character = Some(name.to_string());
        self.save_manifest()
    }

    fn remove_avatar_file(&mut self) -> Result<()> {
        if let Some(file) = self.manifest.avatar.take() {
            let path = self.dir.join(file);
            if path.exists() {
                std::fs::remove_file(&path).context("Failed to remove avatar file")?;
            }
        }
        Ok(())
    }

    /// "Delete" this profile by moving its whole folder (sessions, recordings,
    /// calibration) into the library's `.trash/`, so a mistake can still be
    /// undone by hand. Returns where it went.
    pub fn move_to_trash(self) -> Result<PathBuf> {
        let root = self
            .dir
            .parent()
            .context("Profile folder has no parent")?
            .to_path_buf();
        let trash = root.join(TRASH_DIR);
        std::fs::create_dir_all(&trash).context("Failed to create trash folder")?;
        let folder = self
            .dir
            .file_name()
            .context("Profile folder has no name")?
            .to_string_lossy()
            .into_owned();
        let dest = trash.join(format!("{folder}-{}", now_secs()));
        std::fs::rename(&self.dir, &dest).context("Failed to move profile to trash")?;
        Ok(dest)
    }

    /// Update the display name (sanitised) and persist. The on-disk folder name
    /// is intentionally left unchanged — the manifest is the source of truth for
    /// the display name.
    pub fn set_name(&mut self, raw: &str) -> Result<()> {
        self.manifest.name = sanitize_name(raw);
        self.save_manifest()
    }

    /// Remove the stored avatar (photo and/or character), reverting to the
    /// default face. A no-op if none is set.
    pub fn clear_avatar(&mut self) -> Result<()> {
        self.remove_avatar_file()?;
        self.manifest.character = None;
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
    list_profiles_in(&library_root())
}

/// [`list_profiles`] under an explicit library `root`.
pub fn list_profiles_in(root: &Path) -> Vec<Profile> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
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

    /// A throwaway library root, removed when dropped (even if the test
    /// panics), so tests never write into the user's real profile library.
    struct TempLibrary(PathBuf);

    impl TempLibrary {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("rondelek-lib-test-{}", new_uid()));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }

        fn create(&self, name: &str, avatar: Option<&Path>) -> Profile {
            Profile::create_in(&self.0, name, avatar).unwrap()
        }
    }

    impl Drop for TempLibrary {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    #[test]
    fn character_and_photo_replace_each_other() {
        let src = std::env::temp_dir().join(format!("rondelek_char_{}.png", new_uid()));
        image::RgbaImage::from_pixel(8, 8, image::Rgba([1, 2, 3, 255]))
            .save(&src)
            .unwrap();
        let lib = TempLibrary::new();
        let mut profile = lib.create("Char Kid", Some(&src));
        let photo = profile.avatar_path().unwrap();

        profile.set_character("fox").unwrap();
        assert_eq!(profile.character(), Some("fox"));
        assert!(profile.avatar_path().is_none());
        assert!(!photo.exists(), "photo file removed");

        profile.set_avatar(&src).unwrap();
        assert!(profile.avatar_path().is_some());
        assert_eq!(profile.character(), None);

        profile.set_character("owl").unwrap();
        profile.clear_avatar().unwrap();
        let reloaded = Profile::load(profile.dir.clone()).unwrap();
        assert_eq!(reloaded.character(), None);
        assert!(reloaded.avatar_path().is_none());
        std::fs::remove_file(&src).ok();
    }

    #[test]
    fn manifests_without_character_still_load() {
        let lib = TempLibrary::new();
        let dir = lib.0.join("old-kid-1234");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("profile.json"),
            r#"{ "uid": "u1", "name": "Old Kid", "avatar": null, "created": 5 }"#,
        )
        .unwrap();
        let p = Profile::load(dir).unwrap();
        assert_eq!(p.name(), "Old Kid");
        assert_eq!(p.character(), None);
    }

    #[test]
    fn trash_moves_the_whole_profile_out_of_the_list() {
        let lib = TempLibrary::new();
        let profile = lib.create("Gone Kid", None);
        profile.new_session().unwrap();
        let dest = profile.move_to_trash().unwrap();
        assert!(list_profiles_in(&lib.0).is_empty());
        assert!(dest.starts_with(lib.0.join(TRASH_DIR)));
        assert!(dest.join("profile.json").exists(), "kept, recoverable");
        assert!(dest.join("sessions").is_dir());
    }

    #[test]
    fn tests_use_a_private_library() {
        let lib = TempLibrary::new();
        let profile = lib.create("Isolated Kid", None);
        assert!(profile.dir.starts_with(&lib.0));
        assert!(!profile.dir.starts_with(library_root()));
        let listed = list_profiles_in(&lib.0);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name(), "Isolated Kid");
    }

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
        let lib = TempLibrary::new();
        let profile = lib.create("Cal Kid", None);
        let dir = profile.dir.clone();
        assert!(profile.load_calibration().is_none(), "starts uncalibrated");

        // Six vowels' worth of distinct MFCC-shaped frames.
        let per_vowel: Vec<Vec<Vec<f32>>> = (0..VOWELS.len())
            .map(|v| (0..20).map(|_| vec![v as f32; N_MFCC]).collect())
            .collect();
        let per_rms: Vec<Vec<f32>> = vec![vec![0.02; 20]; VOWELS.len()];
        let cal = build_calibration(&per_vowel, &per_rms, 44_100, Some("Test Mic".into()), 123)
            .expect("built");
        profile.save_calibration(&cal).unwrap();

        let loaded = Profile::load(dir.clone())
            .unwrap()
            .load_calibration()
            .expect("calibration loads back");
        assert_eq!(loaded, cal);
        assert!(loaded.is_valid());
    }

    #[test]
    fn create_profile_and_session_roundtrip() {
        // Create under a private temp library and verify the on-disk structure.
        let lib = TempLibrary::new();
        let profile = lib.create("Test Kid", None);
        assert!(profile.dir.join("profile.json").exists());
        assert!(profile.dir.join("sessions").is_dir());
        assert!(!profile.manifest.uid.is_empty());

        let session = profile.new_session().unwrap();
        assert!(session.dir.join("session.json").exists());
        assert!(!session.manifest.uid.is_empty());

        let sessions = profile.list_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].uid, session.manifest.uid);
    }

    #[test]
    fn set_name_preserves_identity() {
        let lib = TempLibrary::new();
        let mut profile = lib.create("Old Name", None);
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
    }

    #[test]
    fn clear_avatar_removes_file_and_field() {
        let src = std::env::temp_dir().join(format!("rondelek_avc_{}.png", now_secs()));
        image::RgbaImage::from_pixel(8, 8, image::Rgba([10, 20, 30, 255]))
            .save(&src)
            .unwrap();

        let lib = TempLibrary::new();
        let mut profile = lib.create("Avatar Kid", Some(&src));
        let dir = profile.dir.clone();
        let avatar_path = profile.avatar_path().expect("avatar should be set");
        assert!(avatar_path.exists());

        profile.clear_avatar().unwrap();
        assert!(profile.manifest.avatar.is_none());
        assert!(!avatar_path.exists());

        let reloaded = Profile::load(dir.clone()).unwrap();
        assert!(reloaded.manifest.avatar.is_none());

        std::fs::remove_file(&src).ok();
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

        let lib = TempLibrary::new();
        let mut profile = lib.create("Swap Kid", Some(&src_a));
        let dir = profile.dir.clone();
        assert!(profile.avatar_path().unwrap().exists());

        profile.set_avatar(&src_b).unwrap();
        let reloaded = Profile::load(dir.clone()).unwrap();
        assert_eq!(reloaded.manifest.avatar.as_deref(), Some(AVATAR_FILE));
        assert!(reloaded.avatar_path().unwrap().exists());

        std::fs::remove_file(&src_a).ok();
        std::fs::remove_file(&src_b).ok();
    }
}
