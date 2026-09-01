//! A 5x7 bitmap font, painted one rectangle per pixel.
//!
//! [`Canvas`] draws rectangles and nothing else, and a real font stack
//! would be a large dependency for a HUD's worth of capitals and digits.
//! Every game in the suite draws its score, its prompts and its game-over
//! line with this; it lived as an identical copy in Breakout and in Pong
//! until the racer became the third consumer, which is the rule `geom`
//! moved to core on.
//!
//! Unknown characters are SKIPPED SILENTLY by [`text`] — which is how
//! Breakout once shipped "BEST" as "EST". [`unrenderable`] exists so a
//! game can assert every string it can display is covered, rather than
//! discovering a missing glyph by reading the screen.

use crate::backend::{Canvas, Color};

/// Glyph width in font pixels.
pub const GLYPH_W: u32 = 5;
/// Glyph height in font pixels.
pub const GLYPH_H: u32 = 7;
/// Gap between characters, in font pixels.
pub const GLYPH_SPACING: u32 = 1;

/// Row bitmaps for `c`, most significant bit the leftmost of five columns.
///
/// Covers A-Z (either case), 0-9, space and dash. Anything else is `None`.
pub fn glyph(c: char) -> Option<[u8; GLYPH_H as usize]> {
    Some(match c.to_ascii_uppercase() {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        ' ' => [0; GLYPH_H as usize],
        _ => return None,
    })
}

/// The first character of `s` the font cannot draw, if any.
///
/// For a game's coverage test: every string the game can display, run
/// through this, must come back `None`.
pub fn unrenderable(s: &str) -> Option<char> {
    s.chars().find(|&c| glyph(c).is_none())
}

/// Rendered width of `s`, in pixels, at `scale`.
pub fn text_width(s: &str, scale: u32) -> u32 {
    let n = s.chars().count() as u32;
    if n == 0 {
        return 0;
    }
    (n * GLYPH_W + (n - 1) * GLYPH_SPACING) * scale
}

/// Draw `s` with its top-left at `(x, y)`, each font pixel `scale` wide.
pub fn text(canvas: &mut Canvas<'_>, s: &str, x: i32, y: i32, scale: u32, color: Color) {
    let scale = scale.max(1);
    let advance = ((GLYPH_W + GLYPH_SPACING) * scale) as i32;

    for (i, ch) in s.chars().enumerate() {
        let Some(rows) = glyph(ch) else { continue };
        let gx = x + i as i32 * advance;
        for (row, bits) in rows.iter().enumerate() {
            for col in 0..GLYPH_W {
                // Bit 4 is the leftmost column.
                if bits & (1 << (GLYPH_W - 1 - col)) != 0 {
                    canvas.fill_rect(
                        gx + (col * scale) as i32,
                        y + (row as u32 * scale) as i32,
                        scale,
                        scale,
                        color,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_font_covers_capitals_digits_space_and_dash() {
        for ch in ('A'..='Z').chain('a'..='z').chain('0'..='9').chain([' ', '-']) {
            assert!(glyph(ch).is_some(), "font is missing {ch:?}");
        }
        assert_eq!(unrenderable("SCORE 10-9 BEST"), None);
        assert_eq!(unrenderable("LAP 1/3"), Some('/'));
        assert_eq!(unrenderable("87.5"), Some('.'));
    }

    #[test]
    fn text_width_matches_the_glyph_layout() {
        assert_eq!(text_width("", 1), 0);
        assert_eq!(text_width("A", 1), GLYPH_W);
        // Two glyphs plus one space between them.
        assert_eq!(text_width("AB", 1), GLYPH_W * 2 + GLYPH_SPACING);
        assert_eq!(text_width("A", 3), GLYPH_W * 3);
    }

    /// Paint a glyph and read the pixels back. The shape is checked
    /// against the bitmap by hand, not by calling `glyph` again, so a
    /// bit-order slip in `text` cannot pass by being symmetrical with
    /// itself.
    #[test]
    fn a_glyph_paints_its_bitmap_left_to_right() {
        let (w, h) = (8u32, 9u32);
        let mut buf = vec![0u32; (w * h) as usize];
        {
            let mut canvas = Canvas::new(&mut buf, w, h);
            text(&mut canvas, "J", 1, 1, 1, Color::WHITE);
        }
        let lit = |x: u32, y: u32| buf[(y * w + x) as usize] != 0;
        // 'J' row 0 is 00111: columns 2..5 of the glyph, which starts at x=1.
        assert!(!lit(1, 1) && !lit(2, 1) && lit(3, 1) && lit(4, 1) && lit(5, 1));
        // Row 5 is 10010: the hook's left foot at column 0 and a stem at column 3.
        assert!(lit(1, 6) && !lit(2, 6) && !lit(3, 6) && lit(4, 6) && !lit(5, 6));
        // Nothing outside the cell.
        assert!(!lit(0, 1) && !lit(6, 1) && !lit(1, 0) && !lit(1, 8));
        let count = buf.iter().filter(|&&p| p != 0).count();
        assert_eq!(count, 3 + 1 + 1 + 1 + 1 + 2 + 2, "wrong number of pixels for J");
    }

    /// Scale multiplies every dimension, and characters advance by the
    /// glyph plus the gap.
    #[test]
    fn scale_and_advance_agree_with_text_width() {
        // 'H' lights both edge columns, so its extent IS the cell's.
        let s = "HH";
        let scale = 3;
        let w = text_width(s, scale) + 2;
        let h = GLYPH_H * scale + 2;
        let mut buf = vec![0u32; (w * h) as usize];
        {
            let mut canvas = Canvas::new(&mut buf, w, h);
            text(&mut canvas, s, 1, 1, scale, Color::WHITE);
        }
        let lit_cols: Vec<u32> = (0..w)
            .filter(|&x| (0..h).any(|y| buf[(y * w + x) as usize] != 0))
            .collect();
        // Drawn from x=1, the rightmost lit column is x = text_width.
        assert_eq!(*lit_cols.last().unwrap(), text_width(s, scale), "advance is off");
        // The first H spans 1..=15; the gap is 16..=18; the second H
        // starts at 1 + (5+1)*3 = 19.
        assert!(!lit_cols.contains(&16) && !lit_cols.contains(&18) && lit_cols.contains(&19));
        let lit_rows: Vec<u32> = (0..h)
            .filter(|&y| (0..w).any(|x| buf[(y * w + x) as usize] != 0))
            .collect();
        assert_eq!(lit_rows.len() as u32, GLYPH_H * scale, "scale did not multiply height");
    }
}
