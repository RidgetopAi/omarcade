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
PANEL_W = 106  # inner face width in cells
PANEL_H = 30   # inner face height


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
    mark = scaled(MARK, 2)          # 88 x 32 — taller than the panel
    mark = mark[4:26]               # crop the thin outer flanks, keep the peaks
    centre(g, mark, 0)
    centre(g, text_rows("RIDGETOPAI", scale=1), 23)


def sign3_mandrel(g):
    """Sign 3 — Mandrel. Set big: one word, so it gets the whole panel."""
    centre(g, text_rows("MANDREL", scale=2), 8)


def sign4_omarcade(g):
    """Sign 4 — Omarcade, in the style of the Omarchy wordmark.

    The Omarchy mark is a 3-cell stroke with 3-cell gaps. Doubling the
    5x7 font gives a 2-cell stroke, which is the closest this font comes
    to that weight without redrawing letterforms by hand.
    """
    centre(g, text_rows("OMARCADE", scale=2), 8)


def sign6_choice(g):
    """Sign 6 — my choice.

    A circuit sign rather than another brand: "NEXT LAP" over a chequered
    band. It is the one sign that says something about the RACE instead of
    about a product, which is what the back straight is short of — and a
    chequer reads at distance where a word does not, so it still carries
    something once the text has shrunk past legibility.
    """
    centre(g, text_rows("NEXT LAP", scale=2), 4)
    # A chequered band under the words: 3-cell squares, two rows deep.
    band_top, sq = 22, 3
    for r in range(sq * 2):
        for c in range(PANEL_W):
            if ((c // sq) + (r // sq)) % 2 == 0:
                g[band_top + r][c] = 'O'


if __name__ == '__main__':
    render("SIGN 2 — RIDGETOPAI", sign2_ridgetop)
    render("SIGN 3 — MANDREL", sign3_mandrel)
    render("SIGN 4 — OMARCADE", sign4_omarcade)
    render("SIGN 6 — NEXT LAP", sign6_choice)
