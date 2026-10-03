#!/bin/sh
# Record Vowel Runner gameplay and turn it into docs/images/arcade/runner.gif
# (plus a still, runner.png),
# framed by runner-frame.png (from `cargo run --bin genarcade`).
#
#   docs/images/arcade/record-runner.sh [start-s] [length-s] [still-offset-s]
#
# Linux only: needs Xvfb, ffmpeg and a release build (`cargo build
# --release`). The game runs on a virtual display with a throwaway profile
# library, in demo mode (RONDELEK_GAME_AUTOPLAY): it plays itself the way a
# child who knows every vowel would, so the take shows the game played well.
# Each game deals a fresh course, so the full take is kept as runner-take.mp4
# in the temp folder: look through it, then re-cut that same take with a
# better start/length (TAKE=…/runner-take.mp4 skips recording).
#
# Earlier GIFs are kept beside this one for history (runner-v0.3.gif/.png).
set -eu

START=${1:-10}
LEN=${2:-8}
STILL=${3:-3.15}
ROOT="$(pwd)"
GAME="${GAME:-$ROOT/target/release/rondelek-game}"
FRAME="$ROOT/docs/images/arcade/runner-frame.png"
OUT="$ROOT/docs/images/arcade/runner.gif"
STILL_OUT="$ROOT/docs/images/arcade/runner.png"
WORK="$(mktemp -d)"
export XDG_DATA_HOME="$WORK/data" XDG_CONFIG_HOME="$WORK/config"
mkdir -p "$XDG_DATA_HOME" "$XDG_CONFIG_HOME"

if [ -n "${TAKE:-}" ]; then
  cp "$TAKE" "$WORK/runner-take.mp4"
else
  DISPLAY_NUM=:91
  Xvfb "$DISPLAY_NUM" -screen 0 1280x720x24 >/dev/null 2>&1 &
  XVFB_PID=$!
  trap 'kill "$XVFB_PID" 2>/dev/null || true' EXIT
  sleep 1
  export DISPLAY="$DISPLAY_NUM"

  # RONDELEK_GAME_FRAMES skips the pre-game screens and quits on its own.
  (cd "$WORK" && RONDELEK_GAME_AUTOPLAY=1 RONDELEK_GAME_FRAMES=6000 \
    "$GAME" runner >/dev/null 2>&1) &
  GAME_PID=$!
  sleep 3

  ffmpeg -y -loglevel error -f x11grab -video_size 1280x720 -framerate 30 \
    -draw_mouse 0 -i "$DISPLAY_NUM" -t 40 -pix_fmt yuv420p "$WORK/runner-take.mp4"
  kill "$GAME_PID" 2>/dev/null || true
fi

# Gameplay padded out to the frame's size with the picture in its 640×360
# hole at (20,20), the frame laid on top, then a GIF with a Bayer-dithered
# 128-colour palette: retro, and much smaller than error diffusion on a
# scrolling scene.
FW=$(ffprobe -v error -show_entries stream=width -of csv=p=0 "$FRAME")
FH=$(ffprobe -v error -show_entries stream=height -of csv=p=0 "$FRAME")
ffmpeg -y -loglevel error -ss "$START" -t "$LEN" -i "$WORK/runner-take.mp4" -i "$FRAME" \
  -filter_complex "[0:v]fps=12,scale=640:360:flags=lanczos,pad=$FW:$FH:20:20[g];[g][1:v]overlay=0:0,split[x][y];[x]palettegen=max_colors=128:stats_mode=diff[p];[y][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle" \
  -loop 0 "$OUT"

# A still from the full-quality take (not the dithered GIF), STILL seconds
# into the clip: the README shows it to viewers who prefer reduced motion.
STILL_AT=$(awk "BEGIN { print $START + $STILL }")
ffmpeg -y -loglevel error -ss "$STILL_AT" -i "$WORK/runner-take.mp4" -i "$FRAME" \
  -filter_complex "[0:v]scale=640:360:flags=lanczos,pad=$FW:$FH:20:20[g];[g][1:v]overlay=0:0" \
  -frames:v 1 "$STILL_OUT"
echo "wrote $OUT ($(du -h "$OUT" | cut -f1)) and $STILL_OUT"
echo "full take: $WORK/runner-take.mp4"
