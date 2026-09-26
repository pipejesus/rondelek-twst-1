#!/bin/sh
# Prove an AppImage actually runs: render one app frame and a few game frames
# on a virtual display and check that both screenshots were written.
#
#   packaging/appimage/smoke.sh <file.AppImage>
#
# Needs xvfb-run and Mesa's software GL (CI: xvfb, libgl1-mesa-dri). X11 and
# GL are loaded at runtime (dlopen), so `ldd` looking clean proves nothing;
# drawing a frame does. Uses a throwaway profile library, never real data.
set -eu

IMG="$(readlink -f "$1")"
WORK="$(mktemp -d)"
export XDG_DATA_HOME="$WORK/data" XDG_CONFIG_HOME="$WORK/config"
mkdir -p "$XDG_DATA_HOME/rondelek/profiles/maya-demo1" "$XDG_CONFIG_HOME"
echo '{"uid":"demo-1","name":"Maya","avatar":null,"character":"bear","created":0}' \
  > "$XDG_DATA_HOME/rondelek/profiles/maya-demo1/profile.json"
# raylib saves a screenshot into the current directory first (see game::snap).
cd "$WORK"

echo "== app"
RONDELEK_SHOT="$WORK/app.png" timeout 120 \
  xvfb-run -a -s "-screen 0 1280x800x24" "$IMG"
test -s "$WORK/app.png" || { echo "the app did not render a frame"; exit 1; }

echo "== game (--game, standalone with the who's-playing picker)"
RONDELEK_GAME_SCREEN=profiles RONDELEK_GAME_FRAMES=30 RONDELEK_GAME_SHOT="$WORK/game.png" \
  timeout 120 xvfb-run -a -s "-screen 0 1280x800x24" "$IMG" --game
test -s "$WORK/game.png" || { echo "the game did not render a frame"; exit 1; }

echo "smoke test passed: app and game both rendered"
rm -rf "$WORK"
