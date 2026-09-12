#!/usr/bin/env bash
#
# Regenerate the cabinet's game art.
#
# The cabinet shows a screenshot of each game on its screen. These are
# not screenshots: they are frames rendered by each game's own
# dump_frame, headless and deterministic, so they cannot drift from what
# the game actually looks like and they never depend on what the
# compositor was doing.
#
# Re-run this whenever a game's look changes. It is the only way these
# files should ever be produced.
#
#   tools/make-cabinet-art.sh
#
# ⚠️ SCENES ARE CHOSEN FOR THUMBNAIL LEGIBILITY, NOT FOR THE BEST FRAME.
# The cabinet draws these small, so what matters is whether the picture
# reads as a game in motion at that size — which is a different question
# from how good the frame looks at full resolution, and the scene names
# do not tell you which you are getting. Each choice below was made by
# rendering the candidates and looking at them:
#
#   pixel-break  multiball  the full brick field edge to edge, the orange
#                           row carrying the only real colour, six ball
#                           trails and debris in flight. `midgame`, the
#                           obvious pick, is mostly empty black and reads
#                           as a blank box when small.
#   volley       rally      the ball mid-flight WITH a motion trail and an
#                           honest 4-6 scoreline. Volley is structurally
#                           sparse — two paddles, a ball, a dashed line —
#                           and no scene changes that. Brian: keep it
#                           honest, it is what the game is.
#   racer        drive      the player's car with a rival close ahead and
#                           the pack at mid-distance, under the wallpaper
#                           sky. Reads as a racing game instantly.
#
# ⚠️ THE RACER'S SKY IS THE LIVE OMARCHY WALLPAPER, so this render
# depends on the theme that is set when it runs. That is deliberate —
# the committed art should look like the suite's own default — but it
# does mean a themed machine regenerates slightly different art.

set -euo pipefail

cd "$(dirname "$0")/.."

OUT=docs/cabinet
mkdir -p "$OUT"

echo "building the dumpers (release: the racer's sky decode is slow in debug)"
cargo build --release -q \
  -p omarcade-pixel-break -p omarcade-volley -p omarcade-racer \
  --example dump_frame

render() {
  local pkg="$1" scene="$2" dest="$3"
  # ⚠️ Scene THEN path for all three. dump_art takes them the other way
  # round; that tool is not used here.
  cargo run --release -q -p "$pkg" --example dump_frame -- "$scene" "$dest"
}

render omarcade-pixel-break multiball "$OUT/pixel-break.png"
render omarcade-volley      rally     "$OUT/volley.png"
render omarcade-racer       drive     "$OUT/racer.png"

# ⚠️ SHRINK THEM. dump_frame writes STORED-MODE deflate — valid PNG, no
# compression — so every frame lands at ~2 MB. That is the right
# tradeoff for a throwaway render (it keeps the games dependency-free)
# and the wrong one for files that live in the repo forever and get
# cloned by everyone who installs the plugin.
#
# Optional: without a compressor the art is still correct, just large.
# Never let a missing tool block regenerating the art.
if command -v magick >/dev/null 2>&1; then
  echo "compressing"
  for f in "$OUT"/*.png; do
    magick "$f" -strip -define png:compression-level=9 "$f"
  done
elif command -v optipng >/dev/null 2>&1; then
  echo "compressing"
  optipng -quiet -o2 "$OUT"/*.png
else
  echo "⚠️ no magick or optipng — art is correct but ~2 MB per file"
fi

# The README image, from the same source. It has been a 1426-byte broken
# file since afd804a, and it is the first thing anyone sees.
cp "$OUT/pixel-break.png" docs/pixel-break.png

echo
echo "wrote:"
ls -la "$OUT"/*.png docs/pixel-break.png | awk '{printf "  %-34s %8.1f KB\n", $9, $5/1024}'
echo
echo "⚠️ LOOK AT THEM. A dump_frame scene cannot assert its own content —"
echo "   a frame with nothing in it renders, writes and reports success."
