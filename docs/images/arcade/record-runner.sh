#!/bin/sh
# Record Vowel Runner gameplay and turn it into docs/images/arcade/runner.gif
# (plus a still, runner.png),
# framed by runner-frame.png (from `cargo run --bin genarcade`).
#
#   docs/images/arcade/record-runner.sh [start-s] [length-s] [still-offset-s]
#
# Linux only: needs Xvfb, xdotool, ffmpeg and a release build
# (`cargo build --release`). The game runs on a virtual display with a
# throwaway profile library; xdotool holds the vowel keys (A jumps, E ducks,
# I shoots), which the game accepts in place of a voice. Timing on a virtual
# display varies run to run, so the full take is kept as runner-take.mp4 in
# the temp folder: look through it and re-run with a better start/length.
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

DISPLAY_NUM=:91
Xvfb "$DISPLAY_NUM" -screen 0 1280x720x24 >/dev/null 2>&1 &
XVFB_PID=$!
trap 'kill "$XVFB_PID" 2>/dev/null || true' EXIT
sleep 1
export DISPLAY="$DISPLAY_NUM"

# RONDELEK_GAME_FRAMES skips the pre-game screens and quits on its own.
(cd "$WORK" && RONDELEK_GAME_FRAMES=4000 "$GAME" runner >/dev/null 2>&1) &
sleep 3
xdotool mousemove 640 360  # keyboard focus follows the pointer (no WM)

ffmpeg -y -loglevel error -f x11grab -video_size 1280x720 -framerate 30 \
  -draw_mouse 0 -i "$DISPLAY_NUM" -t 30 -pix_fmt yuv420p "$WORK/runner-take.mp4" &
FFMPEG_PID=$!

press() { xdotool keydown "$1"; sleep "$2"; xdotool keyup "$1"; sleep "$3"; }
i=0
while [ "$i" -lt 9 ]; do
  press a 0.35 0.55
  press i 0.3 0.45
  press i 0.3 0.6
  press e 0.9 0.4
  press a 0.35 0.2
  press a 0.3 0.5
  i=$((i + 1))
done
wait "$FFMPEG_PID"

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
