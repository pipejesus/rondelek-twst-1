# Profile Editing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users edit an existing profile's name and photo (replace or remove), reusing the existing new-profile form.

**Architecture:** The new-profile form becomes a shared create/edit form driven by a `FormMode` flag and an `AvatarChoice` enum. A new "Edit profile" button on the Sessions screen enters edit mode pre-filled with the profile's data. Two new data-layer methods (`set_name`, `clear_avatar`) persist the changes; `uid`/`created` are never touched and the on-disk folder is never renamed.

**Tech Stack:** Rust, egui/eframe (immediate-mode UI), `image` crate, serde/serde_json.

## Global Constraints

- **Editable data = `name` + `avatar` only.** `ProfileManifest.uid` and `.created` are immutable; never write them during edit.
- **Do not rename the on-disk profile folder** (`<slug>-<short_uid>`) on a name change — the manifest is the source of truth for the display name.
- **i18n key sync is enforced by a test.** `assets/i18n/en.json` is the source of truth; every one of the 7 seeded locales (en, pl, de, fr, es, it, uk) must contain the exact same key set (same count). Adding a key to one locale requires adding it to all seven.
- **Formatting gate:** `cargo fmt --all -- --check` runs in the pre-commit hook. Run `cargo fmt --all` before every commit.
- Follow existing egui patterns in `src/app.rs` (per-frame local `do_*` flags collected during layout, then acted on after the closure).

---

### Task 1: Data-layer methods — `set_name` + `clear_avatar`

**Files:**
- Modify: `src/profile/mod.rs` (add two methods to `impl Profile`, after `set_avatar`/`store_avatar` around line 172)
- Test: `src/profile/mod.rs` (add tests to the existing `#[cfg(test)] mod tests` block, near line 276)

**Interfaces:**
- Consumes: existing `Profile { dir, manifest }`, `ProfileManifest { uid, name, avatar, created }`, `sanitize_name`, `save_manifest`, `avatar_path`, `set_avatar`, the `AVATAR_FILE` const, `now_secs` (from `crate::util`), and the `image` crate.
- Produces (used by Task 3):
  - `pub fn set_name(&mut self, raw: &str) -> anyhow::Result<()>`
  - `pub fn clear_avatar(&mut self) -> anyhow::Result<()>`

- [ ] **Step 1: Write the failing tests**

Add these three tests inside `mod tests` in `src/profile/mod.rs` (the block already has `use super::*;`):

```rust
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
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib profile::tests::set_name_preserves_identity profile::tests::clear_avatar_removes_file_and_field profile::tests::set_avatar_replaces_existing`
Expected: FAIL — compile error `no method named set_name`/`clear_avatar found for struct Profile` (the two new methods don't exist yet; `set_avatar_replaces_existing` may compile but is grouped here to run together).

- [ ] **Step 3: Implement the two methods**

In `src/profile/mod.rs`, inside `impl Profile`, immediately after the `store_avatar` method (around line 172), add:

```rust
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
```

(`Result` and `context` are already imported at the top of the file via `use anyhow::{Context, Result};`.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib profile::tests::set_name_preserves_identity profile::tests::clear_avatar_removes_file_and_field profile::tests::set_avatar_replaces_existing`
Expected: PASS (3 passed).

- [ ] **Step 5: Format and commit**

```bash
cargo fmt --all
git add src/profile/mod.rs
git commit -m "feat(profile): set_name + clear_avatar edit methods

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: i18n keys for edit UI

**Files:**
- Modify: `assets/i18n/en.json`, `assets/i18n/pl.json`, `assets/i18n/de.json`, `assets/i18n/fr.json`, `assets/i18n/es.json`, `assets/i18n/it.json`, `assets/i18n/uk.json`

**Interfaces:**
- Produces (used by Task 3): translation keys `form.edit_title`, `form.save`, `form.remove_photo`, `sessions.edit`.

- [ ] **Step 1: Verify the key-sync test passes before changes**

Run: `cargo test --lib i18n::tests::english_has_all_keys_others_match`
Expected: PASS (baseline — all locales currently in sync).

- [ ] **Step 2: Add the four keys to every locale**

Add each key below to the matching JSON file. Place the three `form.*` keys next to the existing `form.*` block and `sessions.edit` next to the `sessions.*` block. **Watch JSON commas** — every entry except the last in the object needs a trailing comma.

`assets/i18n/en.json`:
```json
  "form.edit_title": "Edit profile",
  "form.save": "Save",
  "form.remove_photo": "Remove photo",
  "sessions.edit": "Edit profile",
```

`assets/i18n/pl.json`:
```json
  "form.edit_title": "Edytuj profil",
  "form.save": "Zapisz",
  "form.remove_photo": "Usuń zdjęcie",
  "sessions.edit": "Edytuj profil",
```

`assets/i18n/de.json`:
```json
  "form.edit_title": "Profil bearbeiten",
  "form.save": "Speichern",
  "form.remove_photo": "Foto entfernen",
  "sessions.edit": "Profil bearbeiten",
```

`assets/i18n/fr.json`:
```json
  "form.edit_title": "Modifier le profil",
  "form.save": "Enregistrer",
  "form.remove_photo": "Supprimer la photo",
  "sessions.edit": "Modifier le profil",
```

`assets/i18n/es.json`:
```json
  "form.edit_title": "Editar perfil",
  "form.save": "Guardar",
  "form.remove_photo": "Quitar foto",
  "sessions.edit": "Editar perfil",
```

`assets/i18n/it.json`:
```json
  "form.edit_title": "Modifica profilo",
  "form.save": "Salva",
  "form.remove_photo": "Rimuovi foto",
  "sessions.edit": "Modifica profilo",
```

`assets/i18n/uk.json`:
```json
  "form.edit_title": "Редагувати профіль",
  "form.save": "Зберегти",
  "form.remove_photo": "Видалити фото",
  "sessions.edit": "Редагувати профіль",
```

- [ ] **Step 3: Run the key-sync test to verify all locales stay in sync**

Run: `cargo test --lib i18n::tests::english_has_all_keys_others_match`
Expected: PASS. If it fails with "locale X missing key" or "has extra/old keys", a file is missing one of the four keys or has a JSON syntax error — fix that file.

- [ ] **Step 4: Commit**

```bash
git add assets/i18n/*.json
git commit -m "i18n: add profile-edit keys across all seeded locales

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Edit UI wiring in `src/app.rs`

Turns the new-profile form into a shared create/edit form and adds the Sessions-screen entry point. This is one deliverable: all edits below keep the crate compiling and warning-free only when applied together, so complete all steps before running the build in Step 13.

**Files:**
- Modify: `src/app.rs`

**Interfaces:**
- Consumes: `Profile::set_name`, `Profile::clear_avatar`, `Profile::set_avatar`, `Profile::avatar_path` (Task 1); i18n keys (Task 2); existing `Profile::create`, `profile::sanitize_name`, `select_profile`, `refresh_profiles`, `texture_from_path`, `tex_cache`, `current_profile`.
- Produces: no cross-task interface (final task).

- [ ] **Step 1: Rename the `NewProfile` screen variant and add the form enums**

Replace the `AppScreen` enum (lines 23–34) with:

```rust
/// Top-level screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppScreen {
    /// Pick or create a child profile (+ language picker).
    Profiles,
    /// Create or edit a child profile (name + avatar).
    ProfileForm,
    /// A profile's sessions: resume a past one or start fresh.
    Sessions,
    /// The sampler.
    Session,
}

/// Whether the profile form is creating a new profile or editing an existing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FormMode {
    Create,
    Edit,
}

/// The avatar decision captured by the profile form.
#[derive(Clone, Debug)]
enum AvatarChoice {
    /// Create: no avatar yet. Edit: keep the profile's existing avatar.
    Keep,
    /// A freshly picked or captured image to import on submit.
    New(PathBuf),
    /// Edit: clear the avatar back to the default face on submit.
    Remove,
}
```

- [ ] **Step 2: Update the `App` struct form fields**

Replace lines 56–60:

```rust
    // New-profile form state.
    form_name: String,
    form_avatar_src: Option<PathBuf>,
    form_error: Option<String>,
    camera: Option<crate::camera::CameraSession>,
```

with:

```rust
    // Profile form state (shared by create + edit).
    form_mode: FormMode,
    form_name: String,
    form_avatar: AvatarChoice,
    form_error: Option<String>,
    camera: Option<crate::camera::CameraSession>,
```

- [ ] **Step 3: Update the initializer**

Replace lines 155–157:

```rust
            form_name: String::new(),
            form_avatar_src: None,
            form_error: None,
```

with:

```rust
            form_mode: FormMode::Create,
            form_name: String::new(),
            form_avatar: AvatarChoice::Keep,
            form_error: None,
```

- [ ] **Step 4: Update `begin_new_profile` and add `begin_edit_profile`**

Replace the whole `begin_new_profile` method (lines 426–431) with:

```rust
    fn begin_new_profile(&mut self) {
        self.form_mode = FormMode::Create;
        self.form_name.clear();
        self.form_avatar = AvatarChoice::Keep;
        self.form_error = None;
        self.screen = AppScreen::ProfileForm;
    }

    /// Enter the form pre-filled with the current profile's data for editing.
    fn begin_edit_profile(&mut self) {
        let Some(profile) = self.current_profile.as_ref() else {
            return;
        };
        self.form_mode = FormMode::Edit;
        self.form_name = profile.name().to_string();
        self.form_avatar = AvatarChoice::Keep;
        self.form_error = None;
        self.screen = AppScreen::ProfileForm;
    }
```

- [ ] **Step 5: Update `upload_avatar_dialog`**

Replace line 438 (`self.form_avatar_src = Some(path);`) with:

```rust
            self.form_avatar = AvatarChoice::New(path);
```

- [ ] **Step 6: Replace `create_profile_from_form` with `submit_profile_form`**

Replace the whole `create_profile_from_form` method (lines 442–456) with:

```rust
    /// Submit the profile form: create a new profile or apply edits to the
    /// current one, depending on `form_mode`.
    fn submit_profile_form(&mut self) {
        let name = profile::sanitize_name(&self.form_name);
        if name.is_empty() {
            self.form_error = Some(self.i18n.t("form.name_required").to_string());
            return;
        }
        match self.form_mode {
            FormMode::Create => {
                let avatar = match &self.form_avatar {
                    AvatarChoice::New(p) => Some(p.clone()),
                    _ => None,
                };
                match Profile::create(&self.form_name, avatar.as_deref()) {
                    Ok(profile) => {
                        self.refresh_profiles();
                        self.select_profile(profile);
                    }
                    Err(e) => self.form_error = Some(format!("{e}")),
                }
            }
            FormMode::Edit => {
                let Some(mut profile) = self.current_profile.take() else {
                    self.screen = AppScreen::Profiles;
                    return;
                };
                // Evict any cached texture for this avatar path before rewriting
                // it, so the new image (or default face) renders after save.
                if let Some(path) = profile.avatar_path() {
                    self.tex_cache.remove(&path);
                }
                let result = profile.set_name(&self.form_name).and_then(|()| {
                    match &self.form_avatar {
                        AvatarChoice::New(p) => profile.set_avatar(p),
                        AvatarChoice::Remove => profile.clear_avatar(),
                        AvatarChoice::Keep => Ok(()),
                    }
                });
                match result {
                    Ok(()) => {
                        self.refresh_profiles();
                        self.select_profile(profile);
                    }
                    Err(e) => {
                        self.form_error = Some(format!("{e}"));
                        self.current_profile = Some(profile);
                    }
                }
            }
        }
    }
```

- [ ] **Step 7: Update the camera-capture assignment**

In `camera_modal`, replace line 565 (`Ok(path) => self.form_avatar_src = Some(path),`) with:

```rust
                    Ok(path) => self.form_avatar = AvatarChoice::New(path),
```

- [ ] **Step 8: Rename `draw_new_profile` and fix the avatar preview**

Change the method signature (line 795) from `fn draw_new_profile(&mut self, ui: &mut Ui) {` to `fn draw_profile_form(&mut self, ui: &mut Ui) {`.

Then replace the avatar-texture block (lines 800–803):

```rust
        let avatar_tex = self
            .form_avatar_src
            .clone()
            .and_then(|p| self.texture_from_path(&ctx, &p));
```

with:

```rust
        // Resolve which image the preview should show:
        //  - New(p)  → the freshly chosen image
        //  - Remove  → none (default face)
        //  - Keep    → Edit: the profile's existing avatar; Create: none
        let preview_path = match &self.form_avatar {
            AvatarChoice::New(p) => Some(p.clone()),
            AvatarChoice::Remove => None,
            AvatarChoice::Keep => self
                .current_profile
                .as_ref()
                .filter(|_| self.form_mode == FormMode::Edit)
                .and_then(|p| p.avatar_path()),
        };
        let avatar_tex = preview_path.and_then(|p| self.texture_from_path(&ctx, &p));
```

- [ ] **Step 9: Rename the submit flag and switch the title text**

In `draw_profile_form`, replace the flag declarations (lines 805–808):

```rust
        let mut do_upload = false;
        let mut do_camera = false;
        let mut do_create = false;
        let mut do_cancel = false;
```

with:

```rust
        let mut do_upload = false;
        let mut do_camera = false;
        let mut do_remove_photo = false;
        let mut do_submit = false;
        let mut do_cancel = false;
```

Then replace the title label (lines 812–817):

```rust
            ui.label(
                egui::RichText::new(self.i18n.t("form.title"))
                    .color(self.theme.text_primary)
                    .size(24.0)
                    .strong(),
            );
```

with:

```rust
            let title_key = match self.form_mode {
                FormMode::Create => "form.title",
                FormMode::Edit => "form.edit_title",
            };
            ui.label(
                egui::RichText::new(self.i18n.t(title_key))
                    .color(self.theme.text_primary)
                    .size(24.0)
                    .strong(),
            );
```

- [ ] **Step 10: Add the "Remove photo" button (edit mode only)**

Immediately after the Upload/Take-photo `allocate_ui_with_layout` block closes (after line 851, `);`) and before `ui.add_space(16.0);` (line 853), insert:

```rust
            // Edit mode: allow clearing the photo back to the default face,
            // shown only when there is actually a photo to remove.
            let has_photo = matches!(self.form_avatar, AvatarChoice::New(_))
                || (self.form_mode == FormMode::Edit
                    && matches!(self.form_avatar, AvatarChoice::Keep)
                    && self
                        .current_profile
                        .as_ref()
                        .is_some_and(|p| p.avatar_path().is_some()));
            if self.form_mode == FormMode::Edit && has_photo {
                ui.add_space(8.0);
                if ui
                    .add_sized(
                        [298.0, 32.0],
                        egui::Button::new(self.i18n.t("form.remove_photo")),
                    )
                    .clicked()
                {
                    do_remove_photo = true;
                }
            }
```

- [ ] **Step 11: Switch the primary button label + flag**

Replace the primary-button block (lines 876–882):

```rust
                    let create = egui::Button::new(
                        egui::RichText::new(self.i18n.t("form.create")).color(Color32::WHITE),
                    )
                    .fill(ORANGE);
                    if ui.add_sized([145.0, 40.0], create).clicked() {
                        do_create = true;
                    }
```

with:

```rust
                    let submit_key = match self.form_mode {
                        FormMode::Create => "form.create",
                        FormMode::Edit => "form.save",
                    };
                    let submit = egui::Button::new(
                        egui::RichText::new(self.i18n.t(submit_key)).color(Color32::WHITE),
                    )
                    .fill(ORANGE);
                    if ui.add_sized([145.0, 40.0], submit).clicked() {
                        do_submit = true;
                    }
```

- [ ] **Step 12: Wire the post-layout actions (remove/cancel/submit)**

Replace the tail action block (lines 890–901):

```rust
        if do_upload {
            self.upload_avatar_dialog();
        }
        if do_camera {
            self.open_camera();
        }
        if do_cancel {
            self.screen = AppScreen::Profiles;
        }
        if do_create {
            self.create_profile_from_form();
        }
    }
```

with:

```rust
        if do_upload {
            self.upload_avatar_dialog();
        }
        if do_camera {
            self.open_camera();
        }
        if do_remove_photo {
            self.form_avatar = AvatarChoice::Remove;
        }
        if do_cancel {
            // Create returns to the profile list; edit returns to the profile's
            // sessions screen (the profile is still selected).
            self.screen = match self.form_mode {
                FormMode::Create => AppScreen::Profiles,
                FormMode::Edit => AppScreen::Sessions,
            };
        }
        if do_submit {
            self.submit_profile_form();
        }
    }
```

- [ ] **Step 13: Add the "Edit profile" button to `draw_sessions`**

In `draw_sessions`, add an `edit_profile` flag. Replace the flag declarations (lines 920–922):

```rust
        let mut back = false;
        let mut new_session = false;
        let mut open_idx: Option<usize> = None;
```

with:

```rust
        let mut back = false;
        let mut edit_profile = false;
        let mut new_session = false;
        let mut open_idx: Option<usize> = None;
```

Then insert the button between the name heading and the "New session" button. Replace lines 960–963:

```rust
                ui.add_space(16.0);

                let new_btn = egui::Button::new(
                    egui::RichText::new(self.i18n.t("sessions.new"))
```

with:

```rust
                ui.add_space(10.0);
                if ui
                    .add_sized(
                        [200.0, 32.0],
                        egui::Button::new(format!("✎ {}", self.i18n.t("sessions.edit"))),
                    )
                    .clicked()
                {
                    edit_profile = true;
                }
                ui.add_space(16.0);

                let new_btn = egui::Button::new(
                    egui::RichText::new(self.i18n.t("sessions.new"))
```

Then wire the action. Replace the tail block (lines 993–999):

```rust
        if back {
            self.go_to_profiles();
        } else if new_session {
            self.start_new_session();
        } else if let Some(i) = open_idx {
            self.open_session_info(i);
        }
```

with:

```rust
        if back {
            self.go_to_profiles();
        } else if edit_profile {
            self.begin_edit_profile();
        } else if new_session {
            self.start_new_session();
        } else if let Some(i) = open_idx {
            self.open_session_info(i);
        }
```

- [ ] **Step 14: Update the screen dispatch in `ui`**

In the `impl eframe::App for App` `ui` method, replace the match arm (line 1186):

```rust
            AppScreen::NewProfile => self.draw_new_profile(ui),
```

with:

```rust
            AppScreen::ProfileForm => self.draw_profile_form(ui),
```

- [ ] **Step 15: Build, lint, and test**

Run: `cargo build`
Expected: compiles with no errors and no warnings (no leftover `form_avatar_src`, `create_profile_from_form`, `draw_new_profile`, or `NewProfile` references; all `AvatarChoice`/`FormMode` variants are used).

Run: `cargo clippy --all-targets -- -D warnings`
Expected: no warnings.

Run: `cargo test`
Expected: PASS — all existing tests plus the three from Task 1 and the i18n sync test.

- [ ] **Step 16: Manual verification**

Run: `cargo run`
Verify, entering from the Sessions screen of a profile via the new "✎ Edit profile" button:
1. **Rename:** change the name, click **Save** → returns to Sessions, heading shows the new name; the Profiles list also shows it (go Back to confirm).
2. **Replace photo:** Edit → **Upload picture** (or **Take photo**), pick an image, **Save** → new avatar shows on Sessions and Profiles (confirms texture-cache eviction works — the old image does not linger).
3. **Remove photo:** Edit a profile that has a photo → **Remove photo** (preview reverts to the default face) → **Save** → default face shows on Sessions and Profiles.
4. **Cancel:** Edit → change something → **Cancel** → returns to Sessions with the profile unchanged.
5. **Empty name guard:** Edit → clear the name → **Save** → inline "Please enter a name." error, stays on the form.
6. **Create still works:** from Profiles → **New profile** → title reads "New profile", primary button reads "Create", no "Remove photo" button appears; creating works as before.

- [ ] **Step 17: Format and commit**

```bash
cargo fmt --all
git add src/app.rs
git commit -m "feat(app): profile editing — shared create/edit form + Sessions entry

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage:** name edit (Task 1 `set_name`, Task 3 submit) ✓; avatar replace (Task 3 `New` → `set_avatar`) ✓; avatar remove (Task 1 `clear_avatar`, Task 3 `Remove` + button) ✓; `uid`/`created` immutable (Task 1 tests assert) ✓; no folder rename (Task 1 `set_name` doc + constraint) ✓; Sessions entry point (Task 3 Step 13) ✓; reuse form via `FormMode` (Task 3) ✓; texture-cache eviction (Task 3 Step 6) ✓; i18n 4 keys ×7 locales (Task 2) ✓; screen rename `NewProfile`→`ProfileForm` (Task 3) ✓.
- **Type consistency:** `set_name`/`clear_avatar`/`set_avatar` signatures match between Task 1 and Task 3 callers; `AvatarChoice::{Keep,New,Remove}` and `FormMode::{Create,Edit}` used consistently; flag renamed `do_create`→`do_submit` everywhere it appears.
