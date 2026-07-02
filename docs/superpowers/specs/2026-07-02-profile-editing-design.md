# Profile Editing — Design Spec

**Date:** 2026-07-02
**Status:** Draft — awaiting user review

## Problem

Profiles can be **created** (name + photo, via upload or webcam) but never
**edited**. Once a profile exists, its name and avatar are fixed. Users need to
correct a typo'd name, swap a photo, or clear a photo back to the default.

## Scope

Make **all user-editable profile data** editable after creation. The profile
manifest (`ProfileManifest`, `src/profile/mod.rs`) stores four fields:

| Field     | Editable? | Notes                                              |
|-----------|-----------|----------------------------------------------------|
| `name`    | ✅        | Display name (sanitised).                          |
| `avatar`  | ✅        | The photo. Can be replaced **or removed**.         |
| `uid`     | ❌        | Stable internal id — must never change.            |
| `created` | ❌        | Creation timestamp — immutable.                    |

So editing covers **name + avatar**, which is exactly the data the existing
new-profile form already collects.

**Out of scope (YAGNI):** profile deletion, renaming the on-disk folder,
avatar cropping/rotation UI.

## Decisions (assumed; user was away when confirming — flagged for review)

1. **Entry point:** an **"Edit profile" button on the Sessions screen**, placed
   under the avatar+name heading. The profile is already selected and displayed
   there, so it's the natural spot. (Alternatives considered: per-card pencil
   icon on the Profiles list; both. Rejected for crowding the card and
   complicating its click target.)
2. **Photo removal:** edit mode gets a **"Remove photo"** action that clears the
   avatar back to the default illustrated kid-face — full control over the
   stored data, including clearing it.

## Approach

**Reuse the existing new-profile form for both create and edit.** The form
(`draw_new_profile`) already renders the avatar preview, Upload / Take-photo
buttons, name field, and error line. We add a *mode* flag; the mode only changes
the title, the primary-button label, whether "Remove photo" appears, and what
happens on submit. This avoids a second near-identical screen.

The `AppScreen::NewProfile` variant is renamed to `AppScreen::ProfileForm` to
reflect its dual purpose (targeted clarity improvement in the code being
touched).

### Form mode + avatar state

Two small additions to `App`'s form state (replacing `form_avatar_src`):

```rust
enum FormMode { Create, Edit }

enum AvatarChoice {
    Keep,            // Create: no avatar yet. Edit: keep the existing one.
    New(PathBuf),    // A freshly picked / captured image to import.
    Remove,          // Edit: clear back to the default face.
}
```

`App` fields: `form_mode: FormMode`, `form_avatar: AvatarChoice`
(plus the existing `form_name`, `form_error`, `camera`).

**Preview resolution** in the form:
- `New(p)` → show `p`
- `Remove` → default kid-face
- `Keep` → Create: default face; Edit: the profile's current `avatar_path()`

Upload and camera actions set `AvatarChoice::New(path)` in both modes.
"Remove photo" (edit only) sets `AvatarChoice::Remove`.

### Data-layer methods (`src/profile/mod.rs`)

`set_avatar` already exists and is public. Add:

```rust
/// Update the display name (sanitised) and persist. The on-disk folder name
/// is intentionally left unchanged.
pub fn set_name(&mut self, raw: &str) -> Result<()>;

/// Remove the stored avatar file (if any) and clear the manifest field.
pub fn clear_avatar(&mut self) -> Result<()>;
```

Both call `save_manifest()`. `uid` and `created` are never touched. The folder
name (`<slug>-<short_uid>`) is **not** renamed on a name change — it is internal
and the display name comes from the manifest; renaming risks breaking open
session paths for no user-visible benefit.

## Data flow

**Enter edit** (`begin_edit_profile`, from the Sessions "Edit profile" button):
prefill `form_name` from the current profile, `form_avatar = Keep`, clear error,
`form_mode = Edit`, `screen = ProfileForm`.

**Submit** (`submit_profile_form`, replacing `create_profile_from_form`):
1. Sanitise name; if empty → set `form.name_required` error, stop.
2. **Create:** as today — `Profile::create(name, new_image_if_any)`, refresh,
   `select_profile`.
3. **Edit:** take `current_profile`; `set_name`; then per `AvatarChoice`:
   `New(p)` → `set_avatar(p)`; `Remove` → `clear_avatar()`; `Keep` → nothing.
   Evict the stale avatar texture from `tex_cache` (see below), refresh
   profiles, then `select_profile(updated)` → returns to Sessions with fresh
   data.
4. On any `Err`, show it in `form_error` and stay on the form.

**Cancel:** Create → back to Profiles (as today); Edit → back to Sessions.

### Texture cache invalidation

`tex_cache` is keyed by the avatar file path. Edit reuses the same
`avatar.png` filename, so after `set_avatar`/`clear_avatar` the cached texture is
stale. On successful edit, remove the profile's avatar path from `tex_cache` so
the new image (or default face) renders. This is the one easy-to-miss bug and is
called out explicitly.

## i18n

Source of truth is `assets/i18n/en.json`; **all 7 seeded locales** (en, pl, de,
fr, es, it, uk) must stay key-synced (there is a test enforcing this). New keys:

| Key                 | English         |
|---------------------|-----------------|
| `form.edit_title`   | Edit profile    |
| `form.save`         | Save            |
| `form.remove_photo` | Remove photo    |
| `sessions.edit`     | Edit profile    |

Reused: `form.upload`, `form.take_photo`, `form.cancel`, `form.name_hint`,
`form.name_required`. Title switches `form.title` ↔ `form.edit_title`; primary
button switches `form.create` ↔ `form.save`.

## Error handling

Same model as create: filesystem/image errors surface into `form_error` and the
user stays on the form. Empty name is validated before any write. Removing a
non-existent avatar file is a no-op (not an error).

## Testing (TDD)

Data-layer unit tests in `src/profile/mod.rs` (mirroring the existing
`create_profile_and_session_roundtrip` style, cleaning up temp dirs):

- **rename preserves identity:** create → `set_name("New Name")` → reload from
  disk → name updated, `uid` and `created` unchanged, sessions dir intact.
- **clear_avatar:** create with an avatar → `clear_avatar()` → `avatar` is
  `None`, `avatar.png` gone from disk, reload confirms.
- **set_avatar replace:** create with avatar A → `set_avatar(B)` → reload shows
  the avatar still present (file rewritten at same path).

The form/screen wiring in `app.rs` has no existing unit-test coverage and is
verified manually (run the app: edit a name, replace a photo, remove a photo,
cancel) — consistent with how the create flow is verified today.

## Files touched

- `src/profile/mod.rs` — add `set_name`, `clear_avatar`; new tests.
- `src/app.rs` — `FormMode`/`AvatarChoice`, rename screen variant, add
  `begin_edit_profile`, rename `create_profile_from_form` →
  `submit_profile_form` with edit branch, "Edit profile" button in
  `draw_sessions`, "Remove photo" button + title/label switches in the form,
  preview resolution, `tex_cache` eviction.
- `assets/i18n/*.json` (×7) — 4 new keys each.
