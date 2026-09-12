#!/usr/bin/env bash
#
# Regenerate preview.png — the marketplace card image.
#
# This is the picture on plugins.omarchy.org, and for most people it is
# the entire first impression: the browse page shows 106 plugins in the
# games category and a card is a thumbnail, a title and one line. A
# plugin with no preview renders as a flat placeholder with its initials.
#
#   tools/make-preview.sh
#
# ⚠️ THREE GAMES, NOT ONE, AND NOT THE CABINET. Brian's call, and the
# reasoning is the differentiator: anyone can ship one arcade game, and
# the suite is the thing nobody else in the ecosystem has. A single
# racer frame is the prettiest picture here and it undersells three
# games as one; a shot of the cabinet sells the shelf rather than what
# is on it. A row of three says what this actually is in one glance.
#
# ⚠️ RENDERED BY THE GAMES THEMSELVES, never screenshotted, for the same
# reason tools/make-cabinet-art.sh is: a frame that comes out of the
# game's own headless renderer cannot drift from what the game looks
# like. A hand-made marketing image can, and would, and nobody would
# notice until someone installed it.
#
# The scenes match the cabinet art, chosen by rendering the candidates
# and looking at them (see make-cabinet-art.sh for why each one):
#   pixel-break  multiball   the full brick field, six balls in flight
#   volley       rally       mid-rally with a trail and an honest 4-6
#   racer        drive       a rival close ahead, under the wallpaper sky
#
# ⚠️ THE RACER'S SKY IS THE LIVE OMARCHY WALLPAPER, so this render
# depends on the theme set when it runs. Deliberate — the committed art
# should look like the suite's own default — but it means a themed
# machine produces slightly different art. Set OMARCADE_BACKGROUND to
# pin it.

set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."

OUT=preview.png
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

command -v magick >/dev/null 2>&1 || {
  echo "make-preview.sh: needs ImageMagick (magick) to compose the card" >&2
  exit 1
}

echo "building the dumpers (release: the racer's sky decode is slow in debug)"
cargo build --release -q \
  -p omarcade-pixel-break -p omarcade-volley -p omarcade-racer \
  --example dump_frame

render() {
  # ⚠️ Scene THEN path for all three. dump_art takes them the other way
  # round; that tool is not used here.
  cargo run --release -q -p "$1" --example dump_frame -- "$2" "$3"
}

echo "rendering the three games"
render omarcade-pixel-break multiball "$TMP/1.png"
render omarcade-volley      rally     "$TMP/2.png"
render omarcade-racer       drive     "$TMP/3.png"

for f in "$TMP"/1.png "$TMP"/2.png "$TMP"/3.png; do
  [[ -s $f ]] || { echo "make-preview.sh: $f did not render" >&2; exit 1; }
done

# ---------------------------------------------------------------------
# The card.
#
# 1600x900. Omacircuit's preview is 1844x1084 (~1.7:1) and the browse
# page crops cards to a wide banner, so 16:9 sits inside both without
# the composition depending on exactly where the crop lands.
#
# Each game is 960x720 (4:3). Three of them side by side at full height
# would be far wider than 16:9, so they are scaled to a common height
# and CENTRE-CROPPED to equal thirds: each panel keeps its middle, which
# is where every one of these scenes puts its subject.
# ---------------------------------------------------------------------
W=1600
H=900
PANEL=$(( W / 3 ))      # 533, and the remainder goes to the last panel
BAND=96                 # the title band across the bottom

art_h=$(( H - BAND ))

# ⚠️ EVERY DECISION BELOW CAME FROM LOOKING AT A RENDER, NOT FROM
# REASONING ABOUT ONE. The first attempt put the band at the TOP (extent
# pushes art down, it does not reserve space below it), set the name in
# a serif face directly over the brick field where it was unreadable,
# and centre-cropped each panel — which cut Pixel Break's paddle off and
# left Volley as a mostly-empty rectangle, because these scenes do NOT
# all put their subject in the middle.
echo "composing ${W}x${H}"

# Where each game's subject actually is, so the crop keeps it:
#   pixel-break  the bricks are TOP, the paddle is BOTTOM — keep all of
#                it by fitting the whole frame, not cropping to fill
#   volley       sparse by nature; fitting it whole keeps the ball, the
#                net and the scoreline that make it read as a game
#   racer        the road fills the lower two thirds and the wallpaper
#                sky the upper one; fitting whole keeps the car
#
# ⇒ FIT, DO NOT FILL. `-resize x${art_h}` without `^` scales the whole
# 4:3 frame into the panel height and letterboxes the sides against the
# card background. Nothing is cut off, and three 4:3 frames in a 16:9
# card leaves exactly the margin the hairlines need.
fit() {
  magick "$1" -resize "x${art_h}" \
    -background '#0d1117' -gravity center -extent "${2}x${art_h}" "$3"
}

fit "$TMP/1.png" "$PANEL" "$TMP/p1.png"
fit "$TMP/2.png" "$PANEL" "$TMP/p2.png"
fit "$TMP/3.png" "$(( W - PANEL * 2 ))" "$TMP/p3.png"

magick "$TMP/p1.png" "$TMP/p2.png" "$TMP/p3.png" +append "$TMP/row.png"

# ⚠️ A SEPARATOR, BECAUSE THE FIRST TWO PANELS BLEND INTO ONE RECTANGLE.
# Pixel Break and Volley are both mostly dark with a light subject, so
# side by side with no rule between them they read as a single wide
# scene rather than as two games — which loses the entire point of
# showing three. A render made that obvious and nothing else would have.
magick "$TMP/row.png" \
  -fill '#0d1117' -stroke none \
  -draw "rectangle $(( PANEL - 3 )),0 $(( PANEL + 2 )),${art_h}" \
  -draw "rectangle $(( PANEL * 2 - 3 )),0 $(( PANEL * 2 + 2 )),${art_h}" \
  "$TMP/row.png"

# The name band UNDER the art. `-extent` with north gravity keeps the
# row at the top and adds the band below it — the opposite of what
# south gravity did.
magick "$TMP/row.png" \
  -background '#0d1117' -gravity north -extent "${W}x${H}" "$TMP/card.png"

# The name, in the band, in the bar's own sans — never over the games.
# Wide tracking is the one typographic gesture that says "marquee"
# without a novelty face, the same gesture the cabinet's own marquee
# uses. A serif here read as a book cover.
# ⚠️ NAME A FONT THAT EXISTS ON THIS MACHINE, and check rather than
# assume: the first version looked for DejaVu-Sans-Book, found nothing,
# and the `${FONT:+}` expansion quietly collapsed to nothing — so the
# name rendered in ImageMagick's default SERIF, left-aligned, and looked
# like a book cover on an arcade card. A font that is absent fails
# SILENTLY here; only the render shows it.
#
# ⚠️ AND NO `exit` IN THE AWK. `awk '… {print; exit}'` closes the pipe on
# the first match, `magick -list font` takes SIGPIPE, and `set -o
# pipefail` makes that 141 kill the whole script — AFTER the font was
# found correctly. The script died one line past success with no error
# message at all. Read the whole list and take the first match instead.
FONT=$(magick -list font \
  | awk '/^ *Font: (Adwaita-Sans-Bold|Liberation-Sans-Bold|DejaVu-Sans-Bold)$/ {print $2}' \
  | head -1)
[[ -n $FONT ]] || echo "⚠️ no known sans font found; the name will render in the default face" >&2

# ⚠️ THE LABEL IS RENDERED SEPARATELY AND COMPOSITED, NOT ANNOTATED ONTO
# THE CARD. `-gravity south -annotate` centres correctly on a clean
# canvas (measured: x=658 of 1600, exact) and landed at x=125 on the
# real card — same command, same font, same string. The card carries
# state from the panel composition that moves the anchor, and chasing
# that with an offset is fitting a constant to one image.
#
# Rendering the text on its own transparent layer and centring THAT is
# positioning by construction: `-gravity center -composite` has one
# meaning and no history. Measured after: dead centre.
magick -background none -fill '#e6edf3' \
  ${FONT:+-font "$FONT"} \
  -pointsize 34 -kerning 14 \
  label:'OMARCADE' "$TMP/label.png"

magick "$TMP/card.png" \
  \( "$TMP/label.png" \) \
  -gravity south -geometry +0+28 -composite \
  "$TMP/card.png"

echo "compressing"
magick "$TMP/card.png" -strip -define png:compression-level=9 "$OUT"

echo
printf 'wrote %s  %.1f KB  %s\n' "$OUT" \
  "$(( $(stat -c%s "$OUT") ))e-3" \
  "$(magick identify -format '%wx%h' "$OUT")"
echo
echo "⚠️ LOOK AT IT. A dump_frame scene cannot assert its own content — a"
echo "   frame with nothing in it renders, writes and reports success."
