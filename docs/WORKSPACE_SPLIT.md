# The workspace split: why `rondelek` became three crates

Date: 2026-07-20. Reference doc for whoever hits something weird while
testing the first release built from this layout — read this before
re-deriving the reasoning from scratch.

## The bug this fixes

Windows release builds failed at the link step only (Linux and macOS ARM
built fine; macOS x86 has an unrelated pre-existing problem, not covered
here):

```
error: linking with `link.exe` failed: exit code: 1169
= note: libwindows-9d534c1a9dd9a424.rlib(user32.dll) : error LNK2005:
        ShowCursor already defined in libraylib_sys-960663ee646cca24.rlib(rcore.obj)
        ...fatal error LNK1169: one or more multiply defined symbols found
```

**Not a GitHub Actions problem, not a compiler-setup problem.** raylib's C
core (`rcore.c`, compiled into `raylib-sys`) defines its own global function
named `ShowCursor`. Older versions of the `windows` crate — pulled in
transitively by `eframe` → `egui-winit` → `winit`/`accesskit_windows`/
`arboard` — bundle a whole-DLL import library for `user32.dll` that *also*
defines a symbol named `ShowCursor`. MSVC's linker won't allow two
definitions of the same symbol name in one binary; ELF/Mach-O linkers on
Linux/macOS don't have this failure mode, which is why those platforms were
silent about it.

The two symbols only collided because both dependency stacks — raylib (for
the voice-games) and eframe/winit (for the sampler UI) — were compiled into
**one executable**. The game runner used to spawn itself
(`Command::new(current_exe()).arg("--game")`), so `rondelek.exe` always
contained both.

## The fix: three crates instead of one

```
rondelek-twst-1/            (workspace root — no [package], just [workspace])
├── Cargo.toml               workspace manifest + [workspace.package] shared metadata
├── core/                    rondelek-core   — lib only, zero GUI-toolkit deps
│   └── src/{audio,config,profile,session,util,games.rs,lib.rs}
├── app/                     rondelek        — the eframe sampler UI
│   └── src/{app.rs,camera/,i18n/,ui/,bin/genskin.rs,main.rs}
└── game/                    rondelek-game   — the raylib voice-games runner
    └── src/{lib.rs,runner.rs,voice.rs,main.rs}
```

`core` depends on neither `app` nor `game`; `app` and `game` both depend on
`core` but never on each other. That's the whole trick: raylib is a
dependency of `game` only, the `eframe`/`winit`/`egui-winit` stack is a
dependency of `app` only, and Cargo compiles per-crate dependency graphs —
so `raylib-sys` is never even *fetched* for an `app` build, never mind
linked. This isn't relying on the linker to dead-strip unused code (fragile,
codegen-unit-dependent); the crate simply isn't in the dependency graph.

Verified with:

```sh
cargo tree -p rondelek --target x86_64-pc-windows-msvc -i raylib-sys
# error: package ID specification `raylib-sys` did not match any packages

cargo tree -p rondelek-game --target x86_64-pc-windows-msvc -i eframe
cargo tree -p rondelek-game --target x86_64-pc-windows-msvc -i winit
cargo tree -p rondelek-game --target x86_64-pc-windows-msvc -i accesskit-windows
cargo tree -p rondelek-game --target x86_64-pc-windows-msvc -i arboard
# all four: "did not match any packages"
```

One residual thing worth knowing: `rondelek-game` *does* pull in the
`windows` crate transitively, via `cpal`'s WASAPI backend:

```sh
cargo tree -p rondelek-game --target x86_64-pc-windows-msvc -i windows
# windows v0.62.2
# └── cpal v0.18.1
#     └── rondelek-core v0.1.0
#         └── rondelek-game v0.1.0
```

This is safe. `windows v0.62.2` no longer depends on the old
`windows_x86_64_msvc` crate (checked — not in the tree), which means it uses
the modern per-function `raw-dylib` import mechanism instead of the old
whole-DLL bundled `.lib` blobs. cpal only calls COM/audio APIs
(`IMMDeviceEnumerator`, `IAudioClient`, …), never `UI::WindowsAndMessaging`
where `ShowCursor` lives, so no import thunk for `ShowCursor` is ever
generated from that path. If a future dependency bump ever pulls in
`windows_x86_64_msvc` (or any `windows-sys`/`windows` version old enough to
use bundled DLL-level `.lib`s) *and* that dependency touches
`UI::WindowsAndMessaging`, re-run the `cargo tree -i windows_x86_64_msvc`
check above — that's the early-warning sign this bug could come back.

## File moves

Everything moved with `git mv` (shows as clean renames in `git status`), so
`git log --follow` still works on any of these files.

| Old path | New path | Crate |
|---|---|---|
| `src/audio/*` | `core/src/audio/*` | core |
| `src/config/*` | `core/src/config/*` | core |
| `src/profile/mod.rs` | `core/src/profile/mod.rs` | core |
| `src/session/mod.rs` | `core/src/session/mod.rs` | core |
| `src/util/mod.rs` | `core/src/util/mod.rs` | core |
| `src/app.rs` | `app/src/app.rs` | app |
| `src/camera/mod.rs` | `app/src/camera/mod.rs` | app |
| `src/i18n/mod.rs` | `app/src/i18n/mod.rs` | app |
| `src/ui/*` | `app/src/ui/*` | app |
| `src/bin/genskin.rs` | `app/src/bin/genskin.rs` | app |
| `src/main.rs` | `app/src/main.rs` | app (rewritten — see below) |
| `src/game/mod.rs` | `game/src/lib.rs` | game (renamed mod.rs→lib.rs, crate root) |
| `src/game/runner.rs` | `game/src/runner.rs` | game |
| `src/game/voice.rs` | `game/src/voice.rs` | game |

New files: `Cargo.toml` (rewritten to a workspace manifest),
`core/Cargo.toml`, `app/Cargo.toml`, `game/Cargo.toml`, `core/src/lib.rs`,
`core/src/games.rs`, `game/src/main.rs`.

No `[[bin]]`/`[lib]` sections were added to the per-crate `Cargo.toml`s —
Cargo's defaults already do the right thing (package name, hyphens turned to
underscores for the lib target) so they'd have been redundant.

## Dependency assignment

Each crate's `Cargo.toml` only lists what its own code (`grep -rl
'crate_name::'`) actually imports — nothing was carried over "just in case."

- **core**: `egui` (bare — just `Color32`/`Key` value types for
  `Settings`/`Theme`, no windowing code, safe), `cpal`, `hound`,
  `spectrograms`, `non-empty-slice`, `serde`, `serde_json`, `dirs`, `anyhow`,
  `image` (profile avatar resize), `uuid`, `sys-locale` (moved here — see
  below).
- **app**: `rondelek-core`, `eframe`, `egui`, `rustfft` (visualizer FFT),
  `serde`/`serde_json`/`dirs`/`anyhow` (used directly by `ui/skin.rs`),
  `rfd`, `image`, `nokhwa`, `zip`, `winit`. `ringbuf` was already unused
  anywhere in the codebase before this refactor (confirmed by grep) — left
  in `app`'s `Cargo.toml` unchanged rather than removed, since dropping an
  unrelated dead dependency wasn't part of this task.
- **game**: `rondelek-core`, `raylib`, `anyhow`.

## Two things that had to move with genuine logic changes, not just files

### 1. `default_language()` — `Settings` can't call into `app`'s i18n anymore

`core/src/config/settings.rs` used to call `crate::i18n::detect_system_lang()`
to pick a sane default UI language. `i18n` is app-only now (it has the
embedded translation JSON via `include_str!`, which stays in `app` since it's
a pure UI concern). `core` can't depend on `app` (wrong direction — would
reintroduce the exact problem this split fixes if `app` ever depended back).

Fix: `core/src/config/settings.rs` now has its own inline
`SUPPORTED_LANGS: &[&str]` (just the codes, no endonym/display name) and
does the `sys_locale::get_locale()` → primary subtag → allow-list check
itself. It's marked `// ponytail:` and must be kept in sync by hand with
`app/src/i18n/mod.rs`'s `EUROPEAN_LANGS` list. **If you add a new supported
language, update both places** — `EUROPEAN_LANGS` in
`app/src/i18n/mod.rs` (display name) and `SUPPORTED_LANGS` in
`core/src/config/settings.rs` (bare code, for the default-detection check).

As a consequence, `app/src/i18n/mod.rs`'s old `detect_system_lang()` and
`is_supported()` functions became genuinely dead code (their only caller was
that `crate::i18n::detect_system_lang()` call) and were deleted, along with
the now-unused `sys-locale` dependency in `app/Cargo.toml`.

### 2. `GAMES` registry moved from `game` to `core`

`pub const GAMES: &[(&str, &str)]` (the `(id, i18n_key)` list the sampler UI
iterates to build the game-picker menu) used to live in `src/game/mod.rs`.
The sampler (`app`) needs to read it to draw the menu, but `app` must never
depend on `game` (that's raylib again). It's plain data with zero raylib
dependency of its own, so it moved to `core/src/games.rs` —
`rondelek_core::games::GAMES`. `game`'s own `run()` never actually used the
constant (it just string-matches `id` directly), so nothing there changed
behaviorally.

## The CLI/process contract changed

**Old:** `rondelek --game <id> --profile <dir>` (same binary, self-exec).

**New:** `rondelek-game <id> --profile <dir>` (separate binary, sibling of
`rondelek`/`rondelek.exe` in the same directory).

`app/src/app.rs::launch_game()` now does:

```rust
let game_bin = if cfg!(windows) { "rondelek-game.exe" } else { "rondelek-game" };
let sibling = std::env::current_exe()?.with_file_name(game_bin);
Command::new(sibling).arg(id) /* + --profile <dir> */ .spawn()
```

**This means `rondelek-game(.exe)` must always ship in the same directory as
`rondelek(.exe)`.** If you're testing a build by hand (not via the packaged
release archive), copying just `target/release/rondelek` somewhere and
running it will fail to launch games with a "file not found" spawn error —
`rondelek-game` has to be copied alongside it. `release.yml` was updated to
package both binaries together in the same archive; if you build a local
dev/test package manually, remember to include both.

## release.yml changes

Both the Unix and Windows packaging steps now copy `rondelek-game`/
`rondelek-game.exe` into the staged release directory alongside `rondelek`.
The `cargo build --release --locked --target ...` step itself was
**unchanged** — building at the workspace root with no `-p` flag already
builds every member (`core`, `app`, `game`), so both binaries (plus the dev
tool `genskin`, which is *not* packaged) come out of that one command.

## Things worth checking specifically when testing the Windows build

1. **The link error itself is gone.** This was the whole point — confirm
   the Windows release job in `release.yml` reaches the package step.
2. **Game launch works from the packaged zip.** Extract the release zip
   fresh (don't reuse a build directory where `rondelek-game.exe` might be
   sitting in a different folder) and confirm picking a game from the
   sampler UI actually opens the raylib window. This exercises the new
   sibling-binary spawn path end to end.
3. **Language auto-detection on first run.** The `default_language()`
   duplication (see above) is the one piece of actual logic that changed,
   not just moved. It's behavior-preserving by construction, but it's new
   code, so a quick check on a non-English Windows locale is worth doing
   once.
4. **`genskin.exe` should NOT be in the release zip** — it's a dev-only
   asset generator (see `docs/SKINS.md`), intentionally excluded from
   packaging.

## Verifying the split from a clean checkout

```sh
cargo build --workspace           # builds core + app + game (+ genskin)
cargo test --workspace --locked   # exactly what ci.yml runs — 76 tests, 3 crates
cargo tree -p rondelek -i raylib-sys                       # must error "did not match"
cargo tree -p rondelek-game -i eframe                      # must error "did not match"
```

If either `cargo tree -i` query above ever starts *succeeding* (i.e. prints
a dependency path instead of erroring), someone introduced a `core`/`app`/
`game` boundary violation — probably a new `use` statement reaching across
crates it shouldn't — and the Windows link error is likely to come back.
