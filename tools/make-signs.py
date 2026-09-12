#!/usr/bin/env python3
"""Compose the billboard panels and emit the Rust art for `art.rs`.

WHY THIS EXISTS AS A TOOL RATHER THAN HAND-DRAWN ART
----------------------------------------------------
The Omarchy sign was transcribed by hand from an SVG that was already a
pixel grid (`<rect>`s on a lattice). That worked because nothing had to
be *decided* — every cell was already stated in the file.

These five are different. Each needs text set in the game's own 5x7 font,
centred on a panel, sometimes beside a traced mark. Doing that by hand
means counting columns, and a miscount shows up as a sign that is one
pixel off-centre in a game nobody is looking at that closely — until they
are. So the composition is computed, and this script is the source of
truth for it.

Re-run after changing any panel:

    python3 tools/make-signs.py > /tmp/signs.txt

then paste the arrays into `games/omarcade-racer/src/art.rs`.

WHAT IT WILL NOT DO
-------------------
It will not rasterise arbitrary art down to panel size. The RidgetopAi
logo's *text* was tried that way and came out as mush: at this scale a
letter stroke lands below one cell, which is the same failure the
Omarchy sign's own comment warns about ("the stroke IS the resolution").
Only the traced MOUNTAIN survives, because a bold silhouette does. The
words are set in the 5x7 font instead, where every stroke is exactly one
cell by construction.
"""

# The 5x7 font, transcribed from core/src/text.rs. Kept in sync by the
# `the_font_matches_core` test in art.rs rather than by memory.
FONT = {
    'A': [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
    'B': [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
    'C': [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
    'D': [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
    'E': [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
    'F': [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
    'G': [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
    'H': [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
    'I': [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    'J': [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
    'K': [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
    'L': [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
    'M': [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
    'N': [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
    'O': [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    'P': [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
    'Q': [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
    'R': [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
    'S': [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
    'T': [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
    'U': [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
    'V': [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
    'W': [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
    'X': [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001],
    'Y': [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
    'Z': [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
    ' ': [0, 0, 0, 0, 0, 0, 0],
}

# The traced RidgetopAi mountain, rasterised from
# ~/livestream/overlays/logo.svg with its <text> removed, at 44 wide.
# ⚠️ THE TEXT IS DELIBERATELY NOT HERE — see the module docstring.
MARK = [
    "......................O.....................",
    ".....................OOO....................",
    "....................OOOOO...................",
    "............OO.....OOOO.OO..................",
    "...........OOOO...OOOOO..OO.................",
    "..........OOO.O..OOOOOO....O................",
    ".........OOO..OOOOOOOOO.....O.....O.........",
    "........OOOO...OOO..OOO......O...OOO........",
    ".......OOOOO.O..OO....O.......O.OOOOO.......",
    "......OOOOO..OO..OO...O....OOOOO.OO.OO......",
    ".....OOOOOO..O....OO......OOOOOOO.O..OO.....",
    "....OOO.OO..O......OO....OO.....OO....OO....",
    "...OO...............OO..OO.......OO....OO...",
    "..OO.................O.OO..........O.....O..",
    ".OO...................OO..................OO",
    "O.....................O....................O",
]

# The blank billboard's panel face, from art.rs. Everything drawn here is
# painted INSIDE this rectangle; the frame, border and posts are never
# touched, so every sign stays the same physical object.
# ⚠️ ONE WIDTH FOR ALL FOUR, AND IT IS WIDER THAN THE BLANK SIGN'S 58.
#
# The widest content is "OMARCADE" and "NEXT LAP" at scale 2 — 94 cells of
# ink. The Omarchy sign already set the precedent for what to do about
# that: it grew the panel rather than downscaling the mark, because at
# this size downscaling drops whole strokes and turns letters into mush.
#
# All four share ONE width so they read as the same object standing in
# different places, rather than as four differently-sized signs. The cost
# is a little empty face on the narrower ones, which is what a real
# billboard looks like anyway.
# ⚠️ MEASURED FROM BILLBOARD_OMARCHY, NOT CHOSEN. That sign's panel face
# (the K run) is 87 cells wide and 30 tall, and its frame, border and
# posts are exactly where they are because Brian approved them there.
# These signs are the SAME OBJECT with a different face painted on, so
# every number here is read off that sprite rather than invented.
# ⚠️ AND THEN WIDENED AGAIN, FOR THE SAME REASON OMARCHY WAS.
#
# The Omarchy sign grew from the blank sign's 58-cell face to 87 because
# its mark would not fit otherwise, and downscaling drops strokes. The
# widest content here is "OMARCADE" / "NEXT LAP" at scale 2, which needs
# 94 cells of ink — 7 more than 87 holds with any padding worth having.
#
# So the face grows to 104 and ALL FOUR share it. The alternative was
# setting those two at scale 1 while MANDREL stayed at 2, which makes
# four signs that are visibly not the same kind of thing.
PANEL_W = 104  # inner face width, widened from Omarchy's 87
PANEL_H = 26   # inner face height — the K rows, 15..40 inclusive

# The full sprite grid. Everything outside the face — frame, border,
# posts — is copied from BILLBOARD_OMARCHY unchanged; only the face is
# wider, so the widening inserts columns and moves nothing else.
OMARCHY_FACE_W = 87
WIDEN = PANEL_W - OMARCHY_FACE_W


def text_rows(s, scale=1, spacing=1):
    """Render a string in the 5x7 font, optionally scaled up."""
    rows = []
    for r in range(7):
        line = []
        for ch in s:
            bits = FONT.get(ch.upper(), FONT[' '])[r]
            for c in range(5):
                on = (bits >> (4 - c)) & 1
                line.extend(['O' if on else '.'] * scale)
            line.extend(['.'] * spacing * scale)
        # trim the trailing inter-letter gap
        if spacing:
            line = line[: len(line) - spacing * scale]
        for _ in range(scale):
            rows.append(''.join(line))
    return rows


def blank(w, h):
    return [['.'] * w for _ in range(h)]


def stamp(grid, art, top, left):
    """Paint `art` (list of strings) onto `grid` at (top,left). '.' is clear."""
    for r, line in enumerate(art):
        for c, ch in enumerate(line):
            if ch == '.':
                continue
            y, x = top + r, left + c
            if 0 <= y < len(grid) and 0 <= x < len(grid[0]):
                grid[y][x] = ch


def centre(grid, art, top):
    w = max(len(l) for l in art)
    stamp(grid, art, top, (len(grid[0]) - w) // 2)


def render(name, build):
    """Build one panel and print it as Rust-ready rows."""
    g = blank(PANEL_W, PANEL_H)
    build(g)
    print(f"--- {name} ({PANEL_W}x{PANEL_H}) ---")
    for row in g:
        print(''.join(row))
    print()


def scaled(art, k):
    """Nearest-neighbour upscale by an INTEGER factor.

    ⚠️ Integer only. A fractional scale resamples, and resampling a
    1-cell stroke is how the logo's text turned to mush in the first
    place. Doubling duplicates whole cells and cannot lose a stroke.
    """
    out = []
    for line in art:
        row = ''.join(ch * k for ch in line)
        out.extend([row] * k)
    return out


def sign2_ridgetop(g):
    """Sign 2 — Brian's own logo: the traced mark over RIDGETOPAI.

    ⚠️ THE MARK IS DOUBLED AND THE WORDS ARE DOUBLED WITH IT. At native
    44 the mountain sat narrower than its own wordmark and read as a
    small picture with a big caption. A logo's mark should lead.
    """
    # ⚠️ The doubled mark is 32 rows and the face is 26, so it cannot be
    # used whole. Crop the thin outer flanks — the long shallow skirts
    # that carry no shape — and keep the peaks, which are the logo.
    mark = scaled(MARK, 2)[4:20]
    centre(g, mark, 0)
    # One clear row between the mark and the words: they are two things,
    # and a logo that touches its own wordmark reads as one smudge.
    centre(g, text_rows("RIDGETOPAI", scale=1), 17)


def sign3_mandrel(g):
    """Sign 3 — Mandrel. Set big: one word, so it gets the whole panel."""
    centre(g, text_rows("MANDREL", scale=2), (PANEL_H - 14) // 2)


def sign4_omarcade(g):
    """Sign 4 — Omarcade, in the style of the Omarchy wordmark.

    The Omarchy mark is a 3-cell stroke with 3-cell gaps. Doubling the
    5x7 font gives a 2-cell stroke, which is the closest this font comes
    to that weight without redrawing letterforms by hand.
    """
    centre(g, text_rows("OMARCADE", scale=2), (PANEL_H - 14) // 2)


def sign6_choice(g):
    """Sign 6 — my choice.

    A circuit sign rather than another brand: "NEXT LAP" over a chequered
    band. It is the one sign that says something about the RACE instead of
    about a product, which is what the back straight is short of — and a
    chequer reads at distance where a word does not, so it still carries
    something once the text has shrunk past legibility.
    """
    centre(g, text_rows("NEXT LAP", scale=2), 2)
    # A chequered band under the words: 3-cell squares, two deep, placed
    # against the BOTTOM of the face rather than at a typed-in row, so it
    # follows the panel instead of falling off it when the face resizes.
    sq = 3
    band_h = sq * 2
    band_top = PANEL_H - band_h - 1
    for r in range(band_h):
        for c in range(PANEL_W):
            if ((c // sq) + (r // sq)) % 2 == 0:
                g[band_top + r][c] = 'O'


# ---------------------------------------------------------------------
# Emitting the full sprite
# ---------------------------------------------------------------------

def omarchy_rows():
    """The shipped Omarchy sign, read straight out of art.rs.

    ⚠️ READ, NEVER RETYPED. The frame, the border, the posts and the one
    stray 'J' pixel are all exactly where Brian approved them. Copying
    them by hand is how a sign quietly stops matching its neighbours.
    """
    import re
    src = open('games/omarcade-racer/src/art.rs').read()
    m = re.search(r'pub const BILLBOARD_OMARCHY: &\[&str\] = &\[(.*?)\n\];', src, re.S)
    return re.findall(r'"([^"]*)"', m.group(1))


def full_sprite(face):
    """Wrap a 104x30 face in the real sign: frame, border, posts.

    Built by taking the Omarchy sprite and widening it — every row gets
    WIDEN extra columns inserted INSIDE the panel field, so the frame
    stays a frame and the posts stay under the ends of it.
    """
    base = omarchy_rows()
    out = []
    for line in base:
        # ⚠️ INSERT INSIDE THE PANEL, NOT AT THE MIDDLE OF THE ROW. The
        # posts sit at columns 56-58 and 130-132; splitting at the row's
        # midpoint (94) lands between them and pushes the right-hand post
        # outward by WIDEN while the left stays put, so the sign ends up
        # standing on legs of different spacing than its own frame.
        #
        # Column 94 is inside the panel field on EVERY row — for frame
        # and border rows it repeats that row's own character, for post
        # rows it repeats the blank between the posts — so duplicating it
        # widens the panel and leaves the frame, the border and both
        # posts exactly as drawn, which is the whole construction rule
        # the Omarchy sign was built by.
        cut = 94
        out.append(list(line[:cut] + line[cut] * WIDEN + line[cut:]))

    # Paint the face over the K field. Row 15 is the first face row and
    # column 51 its left edge, both measured off the sprite.
    for r in range(PANEL_H):
        for c in range(PANEL_W):
            # '.' on the face means "panel colour", not "transparent" —
            # the sign has a solid face and the art is painted onto it.
            out[15 + r][51 + c] = 'O' if face[r][c] == 'O' else 'K'
    return [''.join(row) for row in out]


def emit_rust(const_name, doc, face_rows):
    rows = full_sprite(face_rows)
    print(f"pub const {const_name}: &[&str] = &[")
    for r in rows:
        print(f'    "{r}",')
    print("];")
    print()


SIGNS = [
    ("BILLBOARD_RIDGETOP", sign2_ridgetop,
     "A billboard carrying the RidgetopAi mark."),
    ("BILLBOARD_MANDREL", sign3_mandrel,
     "A billboard carrying the Mandrel wordmark."),
    ("BILLBOARD_OMARCADE", sign4_omarcade,
     "A billboard carrying the Omarcade wordmark."),
    ("BILLBOARD_NEXTLAP", sign6_choice,
     "A circuit sign: NEXT LAP over a chequered band."),
]


if __name__ == '__main__':
    import sys
    if '--preview' in sys.argv:
        render("SIGN 2 — RIDGETOPAI", sign2_ridgetop)
        render("SIGN 3 — MANDREL", sign3_mandrel)
        render("SIGN 4 — OMARCADE", sign4_omarcade)
        render("SIGN 6 — NEXT LAP", sign6_choice)
    else:
        for name, build, doc in SIGNS:
            g = blank(PANEL_W, PANEL_H)
            build(g)
            emit_rust(name, doc, [''.join(r) for r in g])
