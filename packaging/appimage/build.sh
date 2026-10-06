#!/bin/sh
# Assemble the Linux AppImage from two already-built release binaries.
#
#   packaging/appimage/build.sh <bin-dir> <version> <out-dir>
#
# e.g. packaging/appimage/build.sh target/release v0.2.0 dist
# writes <out-dir>/rondelek-<version>-x86_64.AppImage and its .zsync.
# Run from the repo root. Used by the `linux` job in
# .github/workflows/release.yml; see README.md here for the why.
set -eu

BIN_DIR=$1
VERSION=$2
OUT_DIR=$3
HERE="$(dirname "$(readlink -f "$0")")"
NAME="rondelek-$VERSION-x86_64.AppImage"
WORK="$(mktemp -d)"
APPDIR="$WORK/AppDir"

# Both binaries go side by side: the app starts the game from its own folder.
install -Dm755 "$BIN_DIR/rondelek" "$APPDIR/usr/bin/rondelek"
install -Dm755 "$BIN_DIR/rondelek-game" "$APPDIR/usr/bin/rondelek-game"
strip "$APPDIR/usr/bin/rondelek" "$APPDIR/usr/bin/rondelek-game"

# appimagetool wants the desktop entry and icon at the AppDir root; desktop
# integrators (Gear Lever, AppImageLauncher) read the usr/share copies.
install -Dm755 "$HERE/AppRun" "$APPDIR/AppRun"
install -Dm644 "$HERE/rondelek.desktop" "$APPDIR/rondelek.desktop"
install -Dm644 "$HERE/rondelek.desktop" "$APPDIR/usr/share/applications/rondelek.desktop"
install -Dm644 assets/icon/rondelek.png "$APPDIR/rondelek.png"
install -Dm644 assets/icon/rondelek.png "$APPDIR/usr/share/icons/hicolor/512x512/apps/rondelek.png"
install -Dm644 README.md "$APPDIR/usr/share/doc/rondelek/README.md"
install -Dm644 LICENSE "$APPDIR/usr/share/doc/rondelek/LICENSE"

if command -v desktop-file-validate >/dev/null; then
  desktop-file-validate "$APPDIR/rondelek.desktop"
fi

# appimagetool is itself an AppImage; APPIMAGETOOL can point at a local copy.
TOOL="${APPIMAGETOOL:-$WORK/appimagetool}"
if [ ! -x "$TOOL" ]; then
  curl -fsSL --retry 5 --retry-all-errors -o "$TOOL" \
    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage
  chmod +x "$TOOL"
fi
TOOL="$(readlink -f "$TOOL")"

# The runtime is the small program at the front of every AppImage (the static
# type2 runtime: needs no libfuse2 on the user's machine). appimagetool would
# fetch it itself, without retries; a flaky download shouldn't sink a release.
RUNTIME="$WORK/runtime-x86_64"
curl -fsSL --retry 5 --retry-all-errors -o "$RUNTIME" \
  https://github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64

mkdir -p "$OUT_DIR"
# -u embeds update information: AppImageUpdate / Gear Lever fetch the newest
# release's .zsync and download only the changed blocks. Both files are
# uploaded to the release. zsyncmake writes the .zsync into the *current*
# directory, so run from the output folder to get it next to the AppImage.
cd "$OUT_DIR"
ARCH=x86_64 "$TOOL" --comp zstd --runtime-file "$RUNTIME" \
  -u "gh-releases-zsync|pipejesus|rondelek-twst-1|latest|rondelek-*-x86_64.AppImage.zsync" \
  "$APPDIR" "$NAME"

test -s "$NAME.zsync" || { echo "no $NAME.zsync was written"; exit 1; }
rm -rf "$WORK"
ls -la
