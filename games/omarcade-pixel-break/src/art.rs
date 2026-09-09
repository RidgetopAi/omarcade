//! Authored pixel art.
//!
//! One asset so far: the Omarchy mark, granted by the rarest item in the
//! bag. It lives here rather than in `render.rs` because that file is
//! about *drawing* and this one is about *data* — the same split the
//! racer already makes in its own `art.rs`.
//!
//! ⚠️ **Nothing in `state.rs` may reference this module.** Two examples
//! (`probe_select`, `probe_brick`) pull `state.rs` via `#[path]` WITHOUT
//! `render.rs`, so a `crate::art::` reference from state would fail in
//! those two alone — and only in examples, which is the slowest possible
//! place to find out. Art is presentation; keep it on the render side.

use omarcade_core::backend::Color;
use omarcade_core::sprite::TRANSPARENT;
use omarcade_core::Sprite;

/// The Omarchy mark, at native resolution.
///
/// ⚠️ **This is not a hand-drawn approximation of the logo — it IS the
/// logo.** The source is vendored at `docs/omarchy-logo-hackerman.svg`:
/// a 1200-unit square whose every path coordinate is a multiple of 80,
/// which is to say a 15x15 grid. Rendered at 15x15 its alpha channel
/// holds only 0 or 255 — no anti-aliasing anywhere — and rendered at
/// 600px, all 225 cells are uniformly full or uniformly empty. The mark
/// was drawn on this grid; we are reading it off, not redrawing it.
///
/// To re-derive these rows if the source is ever updated:
///
/// ```text
///     rsvg-convert -w 15 -h 15 docs/omarchy-logo-hackerman.svg -o /tmp/m.png
/// ```
///
/// then read the ALPHA channel — not brightness. The SVG carries a green
/// gradient that goes dark at the bottom, and thresholding on brightness
/// silently drops the last four rows.
///
/// That matters if it is ever edited: the asymmetries are real. The
/// one-row notch in the left wall at row 7, the three-cell gap in row 2,
/// and the break in the bottom row are all in the source path. They look
/// like mistakes and are not — `the_marks_asymmetries_are_intact` pins
/// each one.
///
/// `#` is ink, `.` is transparent. 95 of 225 cells carry ink — an open,
/// light glyph whose ink is almost entirely one-cell-wide WALLS. That
/// openness is exactly why it is not drawn as-is on a falling item; see
/// [`inverted_rows`].
pub const OMARCHY_MARK: &[&str] = &[
    "###############",
    "#......#......#",
    "#.######...##.#",
    "#.#.........#.#",
    "#.#.........#.#",
    "#.#.........#.#",
    "#.#.........#.#",
    "###.........#.#",
    "#.#.........#.#",
    "#.#.........#.#",
    "#.#.........#.#",
    "#.#.........#.#",
    "#.###########.#",
    "#......#......#",
    "########.######",
];

/// The mark's width and height in source pixels.
///
/// Read off the art rather than written down twice, so a future edit to
/// the grid cannot leave a stale constant behind.
pub const MARK_W: f32 = OMARCHY_MARK[0].len() as f32;
pub const MARK_H: f32 = OMARCHY_MARK.len() as f32;

/// The mark with ink and space swapped.
///
/// ⚠️ **This is what actually gets drawn on a falling item, and the
/// reason is worth keeping.** The mark's ink is its WALLS — one cell
/// wide. On a 16-unit-tall item those walls come out around one screen
/// pixel, and at that width they read as faint scratches rather than as
/// a glyph. Worse, the biggest shape left is the mark's empty middle, so
/// the eye takes the *body colour* as the figure and the logo reads as a
/// picture frame around nothing. Rendered side by side against the SVG
/// at full size, the difference is not subtle.
///
/// Inverting fixes it without touching the art: the same 15x15 grid, the
/// same silhouette, but the glyph's MASS carries it instead of its
/// linework — 130 inked cells instead of 95, dominated by one solid
/// centre. At full size your eye already reads the thick dark strokes as
/// the figure; this just makes that true at 16 pixels too.
///
/// Derived from [`OMARCHY_MARK`] rather than written out a second time,
/// so the two can never drift apart.
fn inverted_rows() -> Vec<String> {
    OMARCHY_MARK
        .iter()
        .map(|row| {
            row.chars()
                .map(|c| if c == TRANSPARENT { '#' } else { TRANSPARENT })
                .collect()
        })
        .collect()
}

/// Build the mark as a drawable sprite.
///
/// The palette is a single entry and the colour in it is a placeholder:
/// every draw goes through [`Sprite::draw_tinted`] with the theme's own
/// colour mixed at full strength, because **colour cannot be trusted in
/// a theme-reactive game — the glyph carries the meaning.** The source
/// SVG's green gradient is an artifact of how it was exported for
/// download, not part of the mark's identity, so dropping it costs
/// nothing.
///
/// ⚠️ Built once and drawn many times. `Sprite` does not allocate per
/// frame, but this does — do not call it inside the render loop.
///
/// Panics only if the art above is malformed (ragged rows, or a
/// character with no palette entry), which is a programming error in
/// this file and is caught by the tests below.
pub fn omarchy_sprite() -> Sprite {
    let rows = inverted_rows();
    let refs: Vec<&str> = rows.iter().map(|s| s.as_str()).collect();
    Sprite::new(&refs, &[('#', Color::rgb(255, 255, 255))])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mark_is_a_clean_square_grid() {
        // The whole reason this asset needed no authoring: it is exactly
        // 15x15. If a future edit breaks that, the centring maths in
        // render.rs silently stops being centred.
        assert_eq!(OMARCHY_MARK.len(), 15, "the mark is 15 rows");
        for (y, row) in OMARCHY_MARK.iter().enumerate() {
            assert_eq!(row.len(), 15, "row {y} is not 15 wide");
        }
        assert_eq!(MARK_W, 15.0);
        assert_eq!(MARK_H, 15.0);
        assert_eq!(MARK_W, MARK_H, "the mark must stay square");
    }

    #[test]
    fn the_mark_uses_only_known_characters() {
        for (y, row) in OMARCHY_MARK.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                assert!(
                    ch == '#' || ch == '.',
                    "unexpected character {ch:?} at row {y} col {x}"
                );
            }
        }
    }

    #[test]
    fn the_art_has_the_ink_it_should() {
        // 95 of 225 in the true mark. This is the check that would fail
        // if the rows were ever silently replaced with a redrawn
        // approximation.
        let inked: usize = OMARCHY_MARK.iter().map(|r| r.matches('#').count()).sum();
        assert_eq!(inked, 95, "the mark has 95 inked cells");
    }

    #[test]
    fn the_sprite_is_the_inverse_and_covers_every_cell() {
        let sprite = omarchy_sprite();
        assert_eq!(sprite.width(), 15);
        assert_eq!(sprite.height(), 15);

        // What gets drawn is the INVERSE, so its ink is the mark's empty
        // space: 130 cells, and the two must sum to the whole 15x15 grid
        // with nothing counted twice or lost. That sum is the real
        // guarantee — it holds no matter how the art is edited.
        let inked: usize = OMARCHY_MARK.iter().map(|r| r.matches('#').count()).sum();
        assert_eq!(sprite.ink(), 130, "the drawn sprite has 130 inked cells");
        assert_eq!(
            sprite.ink() + inked,
            15 * 15,
            "the mark and its inverse must tile the grid exactly"
        );
    }

    #[test]
    fn inverting_twice_returns_the_original() {
        // The cheapest possible proof that the inversion is faithful:
        // invert the inverse and the true mark comes back. If this ever
        // fails, the derived rows have stopped being a pure swap.
        let once = inverted_rows();
        let twice: Vec<String> = once
            .iter()
            .map(|row| {
                row.chars()
                    .map(|c| if c == TRANSPARENT { '#' } else { TRANSPARENT })
                    .collect()
            })
            .collect();
        assert_eq!(twice, OMARCHY_MARK, "double inversion is not the identity");
    }

    #[test]
    fn the_marks_asymmetries_are_intact() {
        // These three cells are the mark's character and the most likely
        // casualty of a well-meaning "tidy up". Named explicitly so a
        // change to any of them fails loudly instead of passing as art.
        let row = |y: usize| OMARCHY_MARK[y].as_bytes();

        // The notch in the left edge: at COLUMN 1, row 7 is inked where
        // rows 6 and 8 are open — the left wall closes for exactly one
        // row, halfway down. (Column 2 is inked all the way down; it is
        // the inner wall, not the notch.)
        assert_eq!(row(7)[1], b'#', "row 7 closes the left notch");
        assert_eq!(row(6)[1], b'.', "row 6 is open there");
        assert_eq!(row(8)[1], b'.', "row 8 is open there");

        // The three-cell gap in row 2, at columns 8..11.
        assert_eq!(&OMARCHY_MARK[2][8..11], "...", "row 2 keeps its gap");

        // The break in the bottom row.
        assert_eq!(row(14)[8], b'.', "the bottom row is broken, not solid");
    }
}
