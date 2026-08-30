//! Pixel art as data.
//!
//! A sprite is authored as a grid of characters plus a palette:
//!
//! ```text
//!     const CAR: &[&str] = &[
//!         "..RRRR..",
//!         ".RWWWWR.",
//!         "BBRRRRBB",
//!     ];
//! ```
//!
//! `.` is transparent; every other character indexes a palette the
//! caller supplies. That keeps the art readable in the source, reviewable
//! row by row, and correctable in words — "the rear wheels are one row
//! too high" is a change anyone can make without opening an editor.
//!
//! **Scaling is fractional, never integer.** A sprite approaching from
//! the horizon grows continuously — 0.30x, 0.31x, 0.32x — and integer
//! nearest-neighbour snaps that to 1x, 2x, 3x, so cars visibly pop
//! between sizes as they approach. That popping is the most obvious
//! artifact in amateur pseudo-3D racers, and avoiding it is most of what
//! "modern feel" means here. Each source pixel is drawn as a sub-pixel
//! rect, so the silhouette stays chunky while the motion stays smooth.
//!
//! Nothing here allocates per frame: a [`Sprite`] is built once and
//! drawn many times.

use crate::backend::{Canvas, Color};

/// The character that means "draw nothing here".
pub const TRANSPARENT: char = '.';

/// A palette entry: the character used in the grid, and its colour.
pub type PaletteEntry = (char, Color);

/// One authored pixel: where it sits in the grid, and what colour.
///
/// Transparent cells are dropped at build time rather than tested per
/// draw, so drawing cost scales with the sprite's *ink*, not its bounding
/// box — which matters for a car that is mostly empty corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Pixel {
    x: u16,
    y: u16,
    color: Color,
}

/// A grid of pixels, drawable at any scale.
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    pixels: Vec<Pixel>,
    width: u16,
    height: u16,
}

/// What went wrong while parsing a sprite grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpriteError {
    /// A row was a different length to the first.
    ///
    /// Ragged art is almost always a typo, and it silently shifts every
    /// pixel after it — so it is an error rather than something to pad.
    RaggedRow { row: usize, expected: usize, found: usize },
    /// A character with no palette entry.
    ///
    /// Deliberately fatal. The 5x7 font skips unknown glyphs silently and
    /// that is how "BEST" once shipped as "EST"; art with a typo would
    /// otherwise render as a hole nobody notices until it is in a
    /// screenshot.
    UnknownChar { row: usize, col: usize, ch: char },
    Empty,
}

impl std::fmt::Display for SpriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpriteError::RaggedRow { row, expected, found } => write!(
                f,
                "row {row} is {found} chars, expected {expected} — every row must be the same width"
            ),
            SpriteError::UnknownChar { row, col, ch } => write!(
                f,
                "row {row} col {col}: {ch:?} is not in the palette"
            ),
            SpriteError::Empty => write!(f, "sprite has no rows"),
        }
    }
}

impl std::error::Error for SpriteError {}

impl Sprite {
    /// Build from a character grid and a palette.
    ///
    /// # Errors
    /// Ragged rows and unpalletted characters both fail loudly — see
    /// [`SpriteError`].
    pub fn from_rows(rows: &[&str], palette: &[PaletteEntry]) -> Result<Sprite, SpriteError> {
        if rows.is_empty() {
            return Err(SpriteError::Empty);
        }

        let width = rows[0].chars().count();
        if width == 0 {
            return Err(SpriteError::Empty);
        }

        let mut pixels = Vec::new();
        for (y, row) in rows.iter().enumerate() {
            let len = row.chars().count();
            if len != width {
                return Err(SpriteError::RaggedRow { row: y, expected: width, found: len });
            }
            for (x, ch) in row.chars().enumerate() {
                if ch == TRANSPARENT {
                    continue;
                }
                let color = palette
                    .iter()
                    .find(|(c, _)| *c == ch)
                    .map(|(_, col)| *col)
                    .ok_or(SpriteError::UnknownChar { row: y, col: x, ch })?;
                // A fully transparent palette colour draws nothing, so
                // there is no reason to carry it.
                if color.a == 0 {
                    continue;
                }
                pixels.push(Pixel { x: x as u16, y: y as u16, color });
            }
        }

        Ok(Sprite { pixels, width: width as u16, height: rows.len() as u16 })
    }

    /// Build, panicking on malformed art.
    ///
    /// For sprites written as constants in the source, where a typo is a
    /// bug to fix now rather than a condition to handle. A test should
    /// construct every sprite the game ships so this fires in CI rather
    /// than on someone's machine.
    pub fn new(rows: &[&str], palette: &[PaletteEntry]) -> Sprite {
        match Sprite::from_rows(rows, palette) {
            Ok(s) => s,
            Err(e) => panic!("malformed sprite: {e}"),
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// How many pixels actually draw. Transparent cells are not counted.
    pub fn ink(&self) -> usize {
        self.pixels.len()
    }

    /// Draw with the top-left at `(x, y)`, each source pixel `scale`
    /// units across.
    ///
    /// `scale` is an `f32` and fractional values are the point: at 0.4
    /// the sprite is drawn at 40% size with anti-aliased edges rather
    /// than snapped to whole pixels.
    pub fn draw(&self, canvas: &mut Canvas<'_>, x: f32, y: f32, scale: f32) {
        self.draw_tinted(canvas, x, y, scale, None);
    }

    /// Draw centred horizontally on `x`, with `y` as the BOTTOM edge.
    ///
    /// The anchor a scaled sprite in a pseudo-3D scene actually wants:
    /// a car sits ON the road at a given screen row, and grows upward
    /// and outward from there as it approaches. Anchoring at the
    /// top-left instead makes it appear to sink as it scales.
    pub fn draw_ground(&self, canvas: &mut Canvas<'_>, cx: f32, ground_y: f32, scale: f32) {
        let w = self.width as f32 * scale;
        let h = self.height as f32 * scale;
        self.draw(canvas, cx - w / 2.0, ground_y - h, scale);
    }

    /// Draw, optionally mixing every pixel toward `tint`.
    ///
    /// One call rather than a second sprite per lighting state: distance
    /// haze, a brake-light flash, or a car dimmed by dusk are all the
    /// same operation. `tint` is `(colour, amount)` with amount in
    /// `0.0..=1.0`.
    pub fn draw_tinted(
        &self,
        canvas: &mut Canvas<'_>,
        x: f32,
        y: f32,
        scale: f32,
        tint: Option<(Color, f32)>,
    ) {
        // A sprite scaled to nothing, or off in a NaN, draws nothing
        // rather than looping over every pixel to no effect.
        if !(x.is_finite() && y.is_finite() && scale.is_finite()) || scale <= 0.0 {
            return;
        }

        for p in &self.pixels {
            let color = match tint {
                Some((t, amount)) => p.color.lerp(t, amount),
                None => p.color,
            };
            canvas.fill_rect_f(
                x + p.x as f32 * scale,
                y + p.y as f32 * scale,
                scale,
                scale,
                color,
            );
        }
    }

    /// Draw mirrored left-to-right.
    ///
    /// Cars, road signs and scenery are usually authored facing one way
    /// and needed both; mirroring at draw time keeps one source of truth
    /// for the art.
    pub fn draw_flipped(&self, canvas: &mut Canvas<'_>, x: f32, y: f32, scale: f32) {
        if !(x.is_finite() && y.is_finite() && scale.is_finite()) || scale <= 0.0 {
            return;
        }
        let w = self.width as f32;
        for p in &self.pixels {
            canvas.fill_rect_f(
                x + (w - 1.0 - p.x as f32) * scale,
                y + p.y as f32 * scale,
                scale,
                scale,
                p.color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);
    const WHITE: Color = Color::rgb(255, 255, 255);
    const BLACK: Color = Color::rgb(0, 0, 0);

    fn palette() -> Vec<PaletteEntry> {
        vec![('R', RED), ('W', WHITE), ('B', BLACK)]
    }

    fn canvas_of(w: u32, h: u32) -> Vec<u32> {
        vec![0; (w * h) as usize]
    }

    fn painted(buf: &[u32]) -> usize {
        buf.iter().filter(|&&p| p != 0).count()
    }

    #[test]
    fn a_grid_becomes_pixels() {
        let s = Sprite::new(&["RW", "BR"], &palette());
        assert_eq!(s.width(), 2);
        assert_eq!(s.height(), 2);
        assert_eq!(s.ink(), 4);
    }

    #[test]
    fn transparent_cells_are_dropped_not_stored() {
        // Drawing cost should scale with a sprite's INK, not its bounding
        // box — a car is mostly empty corners.
        let s = Sprite::new(&["R..R", "....", "R..R"], &palette());
        assert_eq!(s.width(), 4);
        assert_eq!(s.height(), 3);
        assert_eq!(s.ink(), 4, "only the four corners draw");
    }

    #[test]
    fn a_ragged_row_is_an_error() {
        // Almost always a typo, and it silently shifts every later pixel.
        let err = Sprite::from_rows(&["RRR", "RR"], &palette()).unwrap_err();
        assert_eq!(err, SpriteError::RaggedRow { row: 1, expected: 3, found: 2 });
    }

    #[test]
    fn an_unpalletted_char_is_an_error_not_a_hole() {
        // The "BEST shipped as EST" failure mode: a silent skip turns a
        // typo into a hole nobody notices until it is in a screenshot.
        let err = Sprite::from_rows(&["RRR", "RZR"], &palette()).unwrap_err();
        assert_eq!(err, SpriteError::UnknownChar { row: 1, col: 1, ch: 'Z' });
    }

    #[test]
    fn an_empty_sprite_is_an_error() {
        assert_eq!(Sprite::from_rows(&[], &palette()).unwrap_err(), SpriteError::Empty);
        assert_eq!(Sprite::from_rows(&[""], &palette()).unwrap_err(), SpriteError::Empty);
    }

    #[test]
    fn a_fully_transparent_palette_colour_is_dropped() {
        let pal = vec![('R', RED), ('X', Color::TRANSPARENT)];
        let s = Sprite::new(&["RXR"], &pal);
        assert_eq!(s.ink(), 2);
    }

    #[test]
    fn drawing_at_scale_one_lands_pixel_for_pixel() {
        let s = Sprite::new(&["RR", "RR"], &palette());
        let mut buf = canvas_of(8, 8);
        {
            let mut c = Canvas::new(&mut buf, 8, 8);
            s.draw(&mut c, 2.0, 3.0, 1.0);
        }
        assert_eq!(painted(&buf), 4);
        assert_eq!(buf[3 * 8 + 2], RED.to_u32());
        assert_eq!(buf[4 * 8 + 3], RED.to_u32());
        assert_eq!(buf[3 * 8 + 4], 0, "must not bleed right");
    }

    #[test]
    fn scaling_up_covers_proportionally_more() {
        let s = Sprite::new(&["R"], &palette());
        let count_at = |scale: f32| {
            let mut buf = canvas_of(32, 32);
            {
                let mut c = Canvas::new(&mut buf, 32, 32);
                s.draw(&mut c, 4.0, 4.0, scale);
            }
            painted(&buf)
        };
        assert_eq!(count_at(1.0), 1);
        assert_eq!(count_at(4.0), 16);
        assert_eq!(count_at(8.0), 64);
    }

    /// The property the whole module exists for: a sprite must grow
    /// SMOOTHLY, not snap between integer sizes. Integer nearest-
    /// neighbour would give identical output across a range of scales and
    /// then jump; sub-pixel coverage changes continuously.
    #[test]
    fn fractional_scales_are_actually_distinct() {
        let s = Sprite::new(&["RR", "RR"], &palette());
        let ink_at = |scale: f32| {
            let mut buf = canvas_of(64, 64);
            {
                let mut c = Canvas::new(&mut buf, 64, 64);
                s.draw(&mut c, 10.0, 10.0, scale);
            }
            // Sum the RED channel — the sprite's colour. Partial
            // coverage darkens edge pixels, so this changes continuously
            // even across scales where the pixel COUNT is identical.
            // (Summing the blue channel would read zero for a red
            // sprite, which is a measurement bug, not a scaling one.)
            buf.iter().map(|p| ((p >> 16) & 0xff) as u64).sum::<u64>()
        };

        let a = ink_at(3.0);
        let b = ink_at(3.3);
        let c = ink_at(3.6);
        assert!(a < b && b < c, "coverage must grow with scale: {a} {b} {c}");
    }

    #[test]
    fn a_sprite_scaled_to_nothing_draws_nothing() {
        let s = Sprite::new(&["RR", "RR"], &palette());
        let mut buf = canvas_of(16, 16);
        {
            let mut c = Canvas::new(&mut buf, 16, 16);
            s.draw(&mut c, 4.0, 4.0, 0.0);
            s.draw(&mut c, 4.0, 4.0, -2.0);
            s.draw(&mut c, f32::NAN, 4.0, 1.0);
            s.draw(&mut c, 4.0, 4.0, f32::NAN);
        }
        assert_eq!(painted(&buf), 0);
    }

    #[test]
    fn drawing_off_canvas_clips_rather_than_panicking() {
        let s = Sprite::new(&["RRRR", "RRRR"], &palette());
        let mut buf = canvas_of(8, 8);
        {
            let mut c = Canvas::new(&mut buf, 8, 8);
            s.draw(&mut c, -2.0, -1.0, 1.0); // straddles the corner
            s.draw(&mut c, 100.0, 100.0, 1.0); // fully off
        }
        // Some of the first draw is visible; nothing panicked.
        assert!(painted(&buf) > 0);
    }

    #[test]
    fn ground_anchoring_puts_the_sprite_above_the_line() {
        // A car sits ON the road at a given row and grows upward as it
        // approaches. Top-left anchoring would make it sink instead.
        let s = Sprite::new(&["RR", "RR"], &palette());
        let mut buf = canvas_of(32, 32);
        {
            let mut c = Canvas::new(&mut buf, 32, 32);
            s.draw_ground(&mut c, 16.0, 20.0, 2.0);
        }
        // 2x2 at scale 2 = 4x4 px, bottom edge on row 20, so rows 16..20.
        for y in 16..20 {
            assert!(buf[y * 32 + 16] != 0, "row {y} should be painted");
        }
        assert_eq!(buf[20 * 32 + 16], 0, "nothing at or below the ground line");
    }

    #[test]
    fn ground_anchoring_keeps_the_centre_fixed_as_it_scales() {
        // The other half: a car must not drift sideways as it approaches.
        let s = Sprite::new(&["RRRR"], &palette());
        let centre_of = |scale: f32| {
            let mut buf = canvas_of(64, 64);
            {
                let mut c = Canvas::new(&mut buf, 64, 64);
                s.draw_ground(&mut c, 32.0, 40.0, scale);
            }
            let painted: Vec<usize> =
                (0..64 * 64).filter(|&i| buf[i] != 0).map(|i| i % 64).collect();
            let lo = *painted.iter().min().unwrap() as f32;
            let hi = *painted.iter().max().unwrap() as f32;
            (lo + hi) / 2.0
        };
        let small = centre_of(2.0);
        let large = centre_of(6.0);
        assert!((small - large).abs() < 1.5, "centre drifted: {small} -> {large}");
    }

    #[test]
    fn a_tint_moves_every_pixel_toward_the_tint_colour() {
        let s = Sprite::new(&["R"], &palette());
        let mut buf = canvas_of(4, 4);
        {
            let mut c = Canvas::new(&mut buf, 4, 4);
            s.draw_tinted(&mut c, 0.0, 0.0, 1.0, Some((WHITE, 1.0)));
        }
        assert_eq!(buf[0], WHITE.to_u32(), "fully tinted is the tint colour");

        let mut buf = canvas_of(4, 4);
        {
            let mut c = Canvas::new(&mut buf, 4, 4);
            s.draw_tinted(&mut c, 0.0, 0.0, 1.0, Some((WHITE, 0.0)));
        }
        assert_eq!(buf[0], RED.to_u32(), "zero tint is untouched");
    }

    #[test]
    fn flipping_mirrors_left_to_right() {
        // Art is authored facing one way and needed both.
        let s = Sprite::new(&["RB"], &palette());
        let mut buf = canvas_of(4, 4);
        {
            let mut c = Canvas::new(&mut buf, 4, 4);
            s.draw_flipped(&mut c, 0.0, 0.0, 1.0);
        }
        // Unflipped would be R at x=0; flipped puts B there.
        assert_eq!(buf[0], BLACK.to_u32());
        assert_eq!(buf[1], RED.to_u32());
    }
}
