# Rondelek — TODO / deferred work

Tracked items intentionally left for later batches.

## Avatars / camera
- **AVIF avatar input.** Currently PNG/JPG/WebP only (pure-Rust, portable). AVIF
  decoding needs a native library (dav1d) — revisit if there's demand and the
  Windows build cost is acceptable.

## Internationalization
- **Remaining European languages.** Framework + picker already list ~36 languages;
  fully translated: en, pl, de, fr, es, it, uk. The rest fall back to English.
  Add locale files under `assets/i18n/<code>.json` (keys must match `en.json`).
- Consider local-time (vs UTC) session-folder timestamps if therapists want it
  (would add a date/time dependency).

## Sessions / profiles
- **Session search** across a profile (the timestamped folders + manifest UIDs are
  already in place to support it).
- **Notes attached to profiles/sessions** via their UIDs (uids already stored in
  `profile.json` / `session.json`).
- Real artwork for the character avatars (`assets/avatars/`, placeholders now).
- Game profile picker: a "back" key from the control screen to the picker, and
  a friendly "add a child in Rondelek first" message when the library is empty
  (needs translated text in the game, which has none yet).
- CI: bump `actions/checkout` and `softprops/action-gh-release` to versions
  that run on Node 24 (GitHub warns Node 20 is deprecated); `ubuntu-latest`
  moves to Ubuntu 26 on 2026-10-19, so recheck the apt package list then.
- Re-enable the macOS/Linux release targets in `release.yml` when wanted.
- Prune old merged local/remote branches (`feat/*`, `fix/skinning-better`, …).
- UI tests beyond the settings page (Home/Hub navigation) with `egui_kittest`.

## Nice-to-have
- Per-pad labels (e.g. "ma", "pa") shown on the pads instead of the key letter.
