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
    /// Whether this pixel belongs to a rolling surface.
    ///
    /// A sprite resolves characters to colours at build time and then
    /// forgets the grid, so the roll animation has no way to ask "was
    /// this a tread?" later. One bool per pixel, decided once, is
    /// cheaper than carrying the whole character grid around and far
    /// cheaper than matching on colour — two different parts of a car
    /// can legitimately share a tone.
    tread: bool,
}

/// A grid of pixels, drawable at any scale.
#[derive(Debug, Clone, PartialEq)]
pub struct Sprite {
    pixels: Vec<Pixel>,
    width: u16,
    height: u16,
    /// The rows the tread occupies, inclusive, if there is any.
    ///
    /// Computed once at build time so the roll animation can wrap the
    /// tread inside its own band rather than the whole sprite —
    /// otherwise scrolling rubber climbs out of the tyre and across the
    /// bodywork.
    tread_span: Option<(u16, u16)>,
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
        Sprite::from_rows_with_tread(rows, palette, &[])
    }

    /// Build, marking some characters as rolling surface.
    ///
    /// `tread_chars` are the letters that scroll under
    /// [`Sprite::draw_ground_rolling`] — the tyre tread on a car, the
    /// links on a tank track. Everything else is painted on and stays
    /// put. Kept separate from the palette because it is a question
    /// about MOTION, not colour: tread usually shares a tone with
    /// something static, and matching on colour would animate both.
    pub fn from_rows_with_tread(
        rows: &[&str],
        palette: &[PaletteEntry],
        tread_chars: &[char],
    ) -> Result<Sprite, SpriteError> {
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
                pixels.push(Pixel {
                    x: x as u16,
                    y: y as u16,
                    color,
                    tread: tread_chars.contains(&ch),
                });
            }
        }

        let tread_span = pixels
            .iter()
            .filter(|p| p.tread)
            .fold(None, |acc: Option<(u16, u16)>, p| {
                Some(match acc {
                    None => (p.y, p.y),
                    Some((lo, hi)) => (lo.min(p.y), hi.max(p.y)),
                })
            });

        Ok(Sprite {
            pixels,
            width: width as u16,
            height: rows.len() as u16,
            tread_span,
        })
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

    /// [`Sprite::new`], marking some characters as rolling surface.
    pub fn new_with_tread(
        rows: &[&str],
        palette: &[PaletteEntry],
        tread_chars: &[char],
    ) -> Sprite {
        match Sprite::from_rows_with_tread(rows, palette, tread_chars) {
            Ok(s) => s,
            Err(e) => panic!("malformed sprite: {e}"),
        }
    }

    /// How many pixels are marked as rolling surface.
    pub fn tread_ink(&self) -> usize {
        self.pixels.iter().filter(|p| p.tread).count()
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// How many pixels actually draw. Transparent cells are not counted.
    /// The bounding box of the sprite's INK: `(x0, y0, x1, y1)`,
    /// inclusive, or `None` for a sprite with no lit pixels.
    ///
    /// Art is authored on a grid that is usually bigger than the drawing
    /// on it, and the difference matters the moment something is
    /// positioned or scaled by size. A 160-wide gantry whose ink spans 126
    /// columns, scaled by its GRID width to span a road, draws a structure
    /// only 79% of the road wide and floating clear of both verges. A
    /// billboard whose posts run past the bottom of its panel, scaled by
    /// its whole HEIGHT, comes out two thirds the intended size with posts
    /// too short to see.
    ///
    /// Computed rather than authored so a redraw with different padding is
    /// followed automatically — a hardcoded bound silently goes wrong the
    /// first time the art changes.
    pub fn ink_bounds(&self) -> Option<(u16, u16, u16, u16)> {
        let first = self.pixels.first()?;
        let mut b = (first.x, first.y, first.x, first.y);
        for p in &self.pixels {
            b.0 = b.0.min(p.x);
            b.1 = b.1.min(p.y);
            b.2 = b.2.max(p.x);
            b.3 = b.3.max(p.y);
        }
        Some(b)
    }

    /// How far the ink's horizontal centre sits from the grid's, in
    /// pixels. Zero when the padding is symmetric.
    ///
    /// Anything that centres this sprite on something — a road's centre
    /// line, a lane — has to correct by this, or lopsided padding puts the
    /// drawing off by exactly this much while the sprite looks correctly
    /// placed.
    pub fn ink_centre_bias(&self) -> f32 {
        match self.ink_bounds() {
            Some((x0, _, x1, _)) => {
                (x0 as f32 + x1 as f32 + 1.0) / 2.0 - self.width as f32 / 2.0
            }
            None => 0.0,
        }
    }

    /// How many blank rows sit below the ink.
    ///
    /// [`Sprite::draw_ground`] stands a sprite on its grid's bottom edge,
    /// so trailing blank rows hang the drawing in the air by exactly this
    /// many scaled pixels. Adding this back to the ground line is what
    /// puts the object ON the ground rather than above it.
    pub fn ink_foot_gap(&self) -> f32 {
        match self.ink_bounds() {
            Some((_, _, _, y1)) => (self.height - 1 - y1) as f32,
            None => 0.0,
        }
    }

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

    /// Draw under a [`Pose`], anchored on the ground like
    /// [`Sprite::draw_ground`].
    ///
    /// This is the one a game calls: a car sits ON the road at a screen
    /// row, and leans, turns and squats about that contact point rather
    /// than about its own top-left corner.
    pub fn draw_ground_posed(
        &self,
        canvas: &mut Canvas<'_>,
        cx: f32,
        ground_y: f32,
        scale: f32,
        pose: Pose,
        tint: Option<(Color, f32)>,
    ) {
        if !(cx.is_finite() && ground_y.is_finite() && scale.is_finite()) || scale <= 0.0 {
            return;
        }
        if !pose.is_finite() {
            return;
        }

        let w = self.width as f32;
        let h = self.height as f32;
        // Turning squashes width. A car angling away from the camera
        // shows less of its back, and that foreshortening is most of
        // what sells the turn — see `Pose::turn`.
        let squash = 1.0 - pose.turn.abs() * MAX_SQUASH;
        // Squat compresses height about the contact patch.
        let squish = 1.0 - pose.squat * MAX_SQUAT;

        for p in &self.pixels {
            // Height above the sprite's own base, measured to the cell's
            // TOP edge — the same cell-not-point reasoning as the
            // horizontal axis below.
            let up = h - p.y as f32;
            // Lean shears by height: the wheels stay planted and the
            // body tilts over them. Shearing about the BASE rather than
            // the centre is what makes it read as banking rather than
            // as the whole car sliding sideways.
            //
            // `up / h` is how far up the sprite this pixel sits, 0 at
            // the wheels and 1 at the roof; multiplying by the WIDTH
            // makes a full lean move the roof by a fixed fraction of the
            // car's own width. Width rather than height because that is
            // what the eye compares a tilt against — a tall narrow
            // sprite sheared by its height leans absurdly far.
            let shear = pose.lean * (up / h) * w * LEAN_SHEAR;
            // Offset from the sprite's centreline, so squash pulls the
            // silhouette inward symmetrically.
            //
            // `w / 2.0`, not `(w - 1.0) / 2.0`: a pixel is a CELL, not a
            // point, so the sprite's centre is the middle of its
            // bounding box and not the middle of its outermost pixel
            // CENTRES. Getting this wrong offsets the whole sprite by
            // half a source pixel — invisible at a glance, but it
            // destroys the sub-pixel coverage that makes fractional
            // scaling smooth, and it would shift every car sideways the
            // moment cornering was switched on.
            let from_centre = p.x as f32 - w / 2.0;

            let px = cx + (from_centre * squash + shear) * scale;
            let py = ground_y - up * squish * scale;

            let color = match tint {
                Some((t, amount)) => p.color.lerp(t, amount),
                None => p.color,
            };
            // Cells are widened by the same factors they are spaced by,
            // or a squashed sprite draws as a comb of gaps instead of a
            // solid body.
            canvas.fill_rect_f(px, py, scale * squash, scale * squish, color);
        }
    }

    /// Draw posed, with the tread scrolling.
    ///
    /// `roll` is a phase in source pixels: pass a value that accumulates
    /// with road speed and the tread appears to rotate. Only pixels
    /// marked as tread move; everything painted on the car stays put.
    ///
    /// The tread wraps within the WHEEL'S OWN vertical extent rather
    /// than the sprite's, so rubber never climbs out of the tyre and
    /// onto the bodywork. That extent is measured per column, because a
    /// car's left and right wheels need not be the same height and a
    /// single sprite may carry several rolling surfaces.
    pub fn draw_ground_rolling(
        &self,
        canvas: &mut Canvas<'_>,
        cx: f32,
        ground_y: f32,
        scale: f32,
        pose: Pose,
        roll: f32,
        tint: Option<(Color, f32)>,
    ) {
        if !(cx.is_finite() && ground_y.is_finite() && scale.is_finite()) || scale <= 0.0 {
            return;
        }
        if !pose.is_finite() || !roll.is_finite() {
            return;
        }
        // No tread, or standing still: this is exactly the posed draw,
        // and going through one code path keeps them from drifting.
        if self.tread_span.is_none() || roll == 0.0 {
            self.draw_ground_posed(canvas, cx, ground_y, scale, pose, tint);
            return;
        }
        let (t_lo, t_hi) = self.tread_span.unwrap();
        let span = (t_hi - t_lo + 1) as f32;

        let w = self.width as f32;
        let h = self.height as f32;
        let squash = 1.0 - pose.turn.abs() * MAX_SQUASH;
        let squish = 1.0 - pose.squat * MAX_SQUAT;

        for p in &self.pixels {
            // A tread pixel is drawn at a scrolled row, wrapped inside
            // the tread band. rem_euclid rather than `%` so a negative
            // roll (reversing) wraps instead of going out of range.
            let src_y = if p.tread {
                let off = (p.y as f32 - t_lo as f32 + roll).rem_euclid(span);
                t_lo as f32 + off
            } else {
                p.y as f32
            };

            let up = h - src_y;
            let shear = pose.lean * (up / h) * w * LEAN_SHEAR;
            let from_centre = p.x as f32 - w / 2.0;

            let px = cx + (from_centre * squash + shear) * scale;
            let py = ground_y - up * squish * scale;

            let color = match tint {
                Some((t, amount)) => p.color.lerp(t, amount),
                None => p.color,
            };
            canvas.fill_rect_f(px, py, scale * squash, scale * squish, color);
        }
    }

}

/// How far a full turn pulls the silhouette in, as a fraction of width.
///
/// Not 1.0: a car turned hard is still a car, and squashing it to
/// nothing reads as the sprite vanishing rather than as the car angling.
/// 0.34 keeps a hard turn recognisably the same vehicle.
const MAX_SQUASH: f32 = 0.34;

/// How far the top of a fully leaned sprite travels sideways, as a
/// fraction of the sprite's OWN HEIGHT.
///
/// Proportional rather than a fixed offset per row, so lean is an angle
/// and not a distance. A constant per-row shear tuned on a short sprite
/// tears a tall one apart: at 0.30 per row a 40-tall car's roof
/// travelled twelve source pixels and the rear wing smeared into a
/// streak. Expressed this way the same value means the same visual
/// angle whatever the sprite's size.
const LEAN_SHEAR: f32 = 0.16;

/// How far a full squat compresses height.
const MAX_SQUAT: f32 = 0.12;

/// The fastest the tread may scroll, in source pixels per frame.
///
/// Above about one row per frame a scrolling pattern aliases and
/// visually REVERSES — the wagon-wheel effect from film. There is no
/// fixing that with more speed; the eye simply cannot resolve it. So the
/// rate pins here and the car reads as "very fast" instead of appearing
/// to roll backwards, which is what the arcade originals did too.
pub const MAX_ROLL_PER_FRAME: f32 = 0.9;

/// How a sprite is oriented: leaning, turning, and loaded.
///
/// A pseudo-3D racer needs a car that banks into a corner and angles
/// away as it turns, and drawing a sprite per angle is a great deal of
/// art to author and keep in sync. All three of these are pure
/// arithmetic on where each source pixel lands, so they cost nothing
/// extra to draw and they compose.
///
/// What this deliberately does NOT do is rotate. A car seen from behind
/// and rotated would show its SIDE, and there is no side in a
/// rear-view grid — no transform can invent one. Squash is the honest
/// approximation, and it is the one the arcade originals used.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pose {
    /// Bank angle, `-1.0` (left) to `1.0` (right).
    ///
    /// Shears the sprite horizontally by height above its base, so the
    /// wheels stay planted and the body tilts over them.
    pub lean: f32,
    /// Heading, `-1.0` (hard left) to `1.0` (hard right).
    ///
    /// Compresses width, so the car reads as angled away from the
    /// camera. The sign is carried for callers that want it; the
    /// squash itself is symmetric.
    pub turn: f32,
    /// Weight on the suspension, `0.0` (unloaded) to `1.0` (hard).
    ///
    /// Compresses height about the contact patch. Small, but it is what
    /// makes a lean read as physics rather than as a slider.
    pub squat: f32,
}

impl Pose {
    /// Sitting square and unloaded. Renders exactly as
    /// [`Sprite::draw_ground`] would.
    pub const UPRIGHT: Pose = Pose { lean: 0.0, turn: 0.0, squat: 0.0 };

    /// Lean and turn together, which is what a car in a corner is doing.
    ///
    /// A real car banks INTO the direction it turns, so one input drives
    /// both by default and a caller who wants them apart can build the
    /// struct directly.
    pub fn cornering(amount: f32) -> Pose {
        let a = amount.clamp(-1.0, 1.0);
        Pose { lean: a, turn: a, squat: a.abs() * 0.5 }
    }

    /// Clamp every field to its documented range.
    ///
    /// Physics hands over values that overshoot at the edges, and an
    /// unclamped lean would shear a sprite off the screen rather than
    /// pinning at full bank.
    pub fn clamped(self) -> Pose {
        Pose {
            lean: self.lean.clamp(-1.0, 1.0),
            turn: self.turn.clamp(-1.0, 1.0),
            squat: self.squat.clamp(0.0, 1.0),
        }
    }

    fn is_finite(self) -> bool {
        self.lean.is_finite() && self.turn.is_finite() && self.squat.is_finite()
    }
}

/// Accumulates tread phase from road speed.
///
/// A wheel's tread scrolls at a rate set by how fast the car is
/// travelling. Keeping that as its own small type means a game advances
/// one value per car and hands it to the draw call, rather than
/// scattering phase arithmetic through the render code.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Roll {
    phase: f32,
}

impl Roll {
    pub const fn new() -> Roll {
        Roll { phase: 0.0 }
    }

    /// Advance by `speed` (units/second) over `dt` seconds.
    ///
    /// `pixels_per_unit` converts the game's own speed units into
    /// source pixels of tread travel — the one number to tune if the
    /// wheels look like they are spinning too fast or too slow for the
    /// speed on the HUD.
    ///
    /// The step is capped at [`MAX_ROLL_PER_FRAME`], so flooring it at
    /// 200mph makes the tread pin rather than strobe backwards.
    pub fn advance(&mut self, speed: f32, pixels_per_unit: f32, dt: f32) {
        if !(speed.is_finite() && pixels_per_unit.is_finite() && dt.is_finite()) {
            return;
        }
        let step = (speed * pixels_per_unit * dt).clamp(
            -MAX_ROLL_PER_FRAME,
            MAX_ROLL_PER_FRAME,
        );
        self.phase += step;
        // Kept small so the phase never drifts into the range where an
        // f32 loses sub-pixel precision. A car that has driven for an
        // hour must roll exactly as smoothly as one that just started.
        if self.phase.abs() > 4096.0 {
            self.phase = self.phase % 1.0;
        }
    }

    /// The current phase, in source pixels.
    pub fn phase(self) -> f32 {
        self.phase
    }
}

impl Default for Pose {
    fn default() -> Self {
        Pose::UPRIGHT
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

    #[test]
    fn an_upright_pose_matches_plain_ground_drawing() {
        // The claim in Pose::UPRIGHT's docs, tested rather than trusted:
        // if the posed path drifted from the plain one, every car would
        // shift the moment cornering was wired up, and it would look
        // like a physics bug.
        let s = Sprite::new(&["RWR", "BRB", "RRR"], &palette());
        let render = |posed: bool| {
            let mut buf = canvas_of(64, 64);
            {
                let mut c = Canvas::new(&mut buf, 64, 64);
                if posed {
                    s.draw_ground_posed(&mut c, 32.0, 40.0, 3.0, Pose::UPRIGHT, None);
                } else {
                    s.draw_ground(&mut c, 32.0, 40.0, 3.0);
                }
            }
            buf
        };
        assert_eq!(render(true), render(false), "UPRIGHT must be a no-op");
    }

    #[test]
    fn leaning_moves_the_top_and_leaves_the_wheels() {
        // The property that makes a lean read as banking rather than as
        // the car sliding sideways.
        // Wide enough for the shear to be measurable: lean is a
        // fraction of the sprite's WIDTH, so a 3-wide test sprite leans
        // by half a pixel and proves nothing.
        let s = Sprite::new(
            &["RRRRRRRRRRRR", "RRRRRRRRRRRR", "RRRRRRRRRRRR", "RRRRRRRRRRRR"],
            &palette(),
        );
        let spread_at = |row_from_bottom: usize, lean: f32| {
            let mut buf = canvas_of(96, 96);
            {
                let mut c = Canvas::new(&mut buf, 96, 96);
                s.draw_ground_posed(&mut c, 48.0, 60.0, 4.0,
                    Pose { lean, turn: 0.0, squat: 0.0 }, None);
            }
            // Centre of mass of the painted pixels in one screen row.
            let y = 60 - 1 - row_from_bottom * 4;
            let xs: Vec<usize> = (0..96).filter(|&x| buf[y * 96 + x] != 0).collect();
            if xs.is_empty() { return None; }
            Some((xs[0] + xs[xs.len() - 1]) as f32 / 2.0)
        };

        let base_up = spread_at(0, 0.0).unwrap();
        let base_lean = spread_at(0, 1.0).unwrap();
        assert!((base_up - base_lean).abs() < 2.0,
            "the bottom row must stay put: {base_up} -> {base_lean}");

        let top_up = spread_at(3, 0.0).unwrap();
        let top_lean = spread_at(3, 1.0).unwrap();
        assert!(top_lean > top_up + 2.0,
            "the top row must travel: {top_up} -> {top_lean}");

        // And it must go the other way for a negative lean.
        let top_left = spread_at(3, -1.0).unwrap();
        assert!(top_left < top_up - 2.0, "leaning left must move left");
    }

    #[test]
    fn turning_narrows_the_silhouette_without_moving_it() {
        let s = Sprite::new(&["RRRRRRRR"], &palette());
        let width_of = |turn: f32| {
            let mut buf = canvas_of(128, 64);
            {
                let mut c = Canvas::new(&mut buf, 128, 64);
                s.draw_ground_posed(&mut c, 64.0, 40.0, 4.0,
                    Pose { lean: 0.0, turn, squat: 0.0 }, None);
            }
            let xs: Vec<usize> = (0..128 * 64).filter(|&i| buf[i] != 0)
                .map(|i| i % 128).collect();
            let lo = *xs.iter().min().unwrap() as f32;
            let hi = *xs.iter().max().unwrap() as f32;
            (hi - lo, (lo + hi) / 2.0)
        };
        let (w0, c0) = width_of(0.0);
        let (w1, c1) = width_of(1.0);
        assert!(w1 < w0, "a turned car must be narrower: {w0} -> {w1}");
        assert!((c0 - c1).abs() < 2.0, "but must not drift: {c0} -> {c1}");
        // Symmetric: turning either way squashes the same amount.
        let (wl, _) = width_of(-1.0);
        assert!((wl - w1).abs() < 2.0, "squash must be symmetric");
    }

    #[test]
    fn a_squashed_sprite_stays_solid() {
        // Spacing pixels closer without widening them draws a comb of
        // gaps — the sprite looks shredded rather than foreshortened.
        let s = Sprite::new(&["RRRRRRRRRRRR"], &palette());
        let mut buf = canvas_of(128, 64);
        {
            let mut c = Canvas::new(&mut buf, 128, 64);
            s.draw_ground_posed(&mut c, 64.0, 40.0, 4.0,
                Pose { lean: 0.0, turn: 1.0, squat: 0.0 }, None);
        }
        let row = 39;
        let xs: Vec<usize> = (0..128).filter(|&x| buf[row * 128 + x] != 0).collect();
        let lo = xs[0];
        let hi = xs[xs.len() - 1];
        let holes = (lo..=hi).filter(|&x| buf[row * 128 + x] == 0).count();
        assert_eq!(holes, 0, "squashed sprite has {holes} gaps in its body");
    }

    #[test]
    fn squatting_lowers_the_roof_and_keeps_the_wheels_down() {
        let s = Sprite::new(&["RRR", "RRR", "RRR", "RRR"], &palette());
        let top_of = |squat: f32| {
            let mut buf = canvas_of(96, 96);
            {
                let mut c = Canvas::new(&mut buf, 96, 96);
                s.draw_ground_posed(&mut c, 48.0, 60.0, 4.0,
                    Pose { lean: 0.0, turn: 0.0, squat }, None);
            }
            let ys: Vec<usize> = (0..96 * 96).filter(|&i| buf[i] != 0)
                .map(|i| i / 96).collect();
            (*ys.iter().min().unwrap(), *ys.iter().max().unwrap())
        };
        let (top0, bot0) = top_of(0.0);
        let (top1, bot1) = top_of(1.0);
        assert!(top1 > top0, "a squatting car must be shorter");
        assert!((bot1 as i32 - bot0 as i32).abs() <= 1, "wheels stay on the road");
    }

    #[test]
    fn a_pose_is_clamped_and_nan_draws_nothing() {
        let p = Pose { lean: 5.0, turn: -9.0, squat: 3.0 }.clamped();
        assert_eq!(p.lean, 1.0);
        assert_eq!(p.turn, -1.0);
        assert_eq!(p.squat, 1.0);

        let s = Sprite::new(&["RR", "RR"], &palette());
        let mut buf = canvas_of(32, 32);
        {
            let mut c = Canvas::new(&mut buf, 32, 32);
            s.draw_ground_posed(&mut c, 16.0, 20.0, 2.0,
                Pose { lean: f32::NAN, turn: 0.0, squat: 0.0 }, None);
            s.draw_ground_posed(&mut c, 16.0, 20.0, 2.0,
                Pose { lean: 0.0, turn: f32::INFINITY, squat: 0.0 }, None);
        }
        assert_eq!(painted(&buf), 0, "a bad pose must draw nothing, not garbage");
    }

    #[test]
    fn cornering_leans_into_the_turn() {
        // A car banks INTO the corner it is taking; opposite signs would
        // read as a car sliding out of one.
        let right = Pose::cornering(1.0);
        assert_eq!(right.lean.signum(), right.turn.signum());
        assert!(right.squat > 0.0, "cornering loads the suspension");
        let left = Pose::cornering(-1.0);
        assert_eq!(left.lean.signum(), left.turn.signum());
        assert!(left.squat > 0.0, "squat is unsigned");
        assert_eq!(Pose::cornering(0.0), Pose::UPRIGHT);
        // Overshoot from physics must pin, not shear off screen.
        assert_eq!(Pose::cornering(4.0).lean, 1.0);
    }

    /// A grid whose tread band is rows 2..=4.
    ///
    /// The tread rows must DIFFER from each other, or scrolling an
    /// identical pattern produces an identical image and a "the tread
    /// moves" test passes vacuously. This one alternates.
    fn wheelie() -> Sprite {
        Sprite::new_with_tread(
            &["BBBB", "BBBB", "RRBB", "BBRR", "RRBB", "BBBB"],
            &[('B', BLACK), ('R', WHITE)],
            &['R'],
        )
    }

    #[test]
    fn only_tread_characters_are_marked() {
        let s = wheelie();
        assert_eq!(s.ink(), 24);
        assert_eq!(s.tread_ink(), 6, "three rows of two");
    }

    #[test]
    fn a_sprite_with_no_tread_rolls_identically_to_posed() {
        // The fallback path. If these ever diverged, wheels-less sprites
        // would shift the moment roll was wired up.
        let s = Sprite::new(&["RW", "BR"], &palette());
        let render = |rolling: bool| {
            let mut buf = canvas_of(32, 32);
            {
                let mut c = Canvas::new(&mut buf, 32, 32);
                if rolling {
                    s.draw_ground_rolling(&mut c, 16.0, 24.0, 3.0, Pose::UPRIGHT, 7.5, None);
                } else {
                    s.draw_ground_posed(&mut c, 16.0, 24.0, 3.0, Pose::UPRIGHT, None);
                }
            }
            buf
        };
        assert_eq!(render(true), render(false));
    }

    #[test]
    fn zero_roll_changes_nothing() {
        let s = wheelie();
        let render = |roll: Option<f32>| {
            let mut buf = canvas_of(48, 48);
            {
                let mut c = Canvas::new(&mut buf, 48, 48);
                match roll {
                    Some(r) => s.draw_ground_rolling(&mut c, 24.0, 40.0, 4.0, Pose::UPRIGHT, r, None),
                    None => s.draw_ground_posed(&mut c, 24.0, 40.0, 4.0, Pose::UPRIGHT, None),
                }
            }
            buf
        };
        assert_eq!(render(Some(0.0)), render(None));
    }

    /// The property that keeps rubber on the tyre: however far the tread
    /// scrolls, it must never paint outside the rows it started in.
    #[test]
    fn tread_wraps_inside_the_wheel_and_never_escapes() {
        let s = wheelie();
        // Where the tread colour appears, in source rows.
        let tread_rows = |roll: f32| {
            let mut buf = canvas_of(64, 64);
            {
                let mut c = Canvas::new(&mut buf, 64, 64);
                s.draw_ground_rolling(&mut c, 32.0, 50.0, 4.0, Pose::UPRIGHT, roll, None);
            }
            let mut rows = std::collections::BTreeSet::new();
            for y in 0..64 {
                for x in 0..64 {
                    if buf[y * 64 + x] == WHITE.to_u32() {
                        // Screen row back to a source row.
                        rows.insert((50 - y) / 4);
                    }
                }
            }
            rows
        };
        let at_rest = tread_rows(0.0);
        assert!(!at_rest.is_empty(), "the test sprite must show tread");
        // Sweep well past a full wrap, forwards and backwards.
        for i in -40..=40 {
            let roll = i as f32 * 0.37;
            let rows = tread_rows(roll);
            assert!(
                rows.is_subset(&at_rest),
                "tread escaped its band at roll {roll}: {rows:?} vs {at_rest:?}",
            );
        }
    }

    #[test]
    fn tread_actually_moves() {
        // The other half: wrapping must not be achieved by not moving.
        let s = wheelie();
        let frame = |roll: f32| {
            let mut buf = canvas_of(64, 64);
            {
                let mut c = Canvas::new(&mut buf, 64, 64);
                s.draw_ground_rolling(&mut c, 32.0, 50.0, 4.0, Pose::UPRIGHT, roll, None);
            }
            buf
        };
        assert_ne!(frame(0.0), frame(1.0), "one row of roll must be visible");
    }

    #[test]
    fn a_full_wrap_returns_to_the_start() {
        // The tread band is 2 rows, so rolling by 2 is a whole cycle.
        let s = wheelie();
        let frame = |roll: f32| {
            let mut buf = canvas_of(64, 64);
            {
                let mut c = Canvas::new(&mut buf, 64, 64);
                s.draw_ground_rolling(&mut c, 32.0, 50.0, 4.0, Pose::UPRIGHT, roll, None);
            }
            buf
        };
        assert_eq!(frame(0.0), frame(3.0), "a full cycle must be seamless");
        assert_eq!(frame(0.0), frame(-3.0), "and seamless in reverse");
    }

    #[test]
    fn roll_is_capped_so_it_cannot_alias() {
        // Above ~1 row per frame a scrolling pattern reverses visually.
        // Flooring it must pin the rate, not strobe.
        let mut r = Roll::new();
        r.advance(100_000.0, 50.0, 1.0 / 60.0);
        assert!(
            r.phase().abs() <= MAX_ROLL_PER_FRAME + 1e-6,
            "one step went past the cap: {}",
            r.phase(),
        );
        // Reverse pins too.
        let mut back = Roll::new();
        back.advance(-100_000.0, 50.0, 1.0 / 60.0);
        assert!(back.phase() >= -MAX_ROLL_PER_FRAME - 1e-6);
    }

    #[test]
    fn roll_stays_precise_over_a_long_drive() {
        // An hour at full tilt must not accumulate into the range where
        // an f32 loses sub-pixel resolution — a car that has been
        // driving a while has to roll as smoothly as one that just
        // started.
        let mut r = Roll::new();
        for _ in 0..(60 * 60 * 60) {
            r.advance(300.0, 0.5, 1.0 / 60.0);
        }
        assert!(r.phase().is_finite());
        assert!(r.phase().abs() <= 4096.0, "phase ran away: {}", r.phase());
    }

    #[test]
    fn a_bad_roll_draws_nothing_rather_than_garbage() {
        let s = wheelie();
        let mut buf = canvas_of(48, 48);
        {
            let mut c = Canvas::new(&mut buf, 48, 48);
            s.draw_ground_rolling(&mut c, 24.0, 40.0, 4.0, Pose::UPRIGHT, f32::NAN, None);
        }
        assert_eq!(painted(&buf), 0);

        // And a NaN speed must not poison the accumulator.
        let mut r = Roll::new();
        r.advance(5.0, 1.0, 1.0 / 60.0);
        let before = r.phase();
        r.advance(f32::NAN, 1.0, 1.0 / 60.0);
        assert_eq!(r.phase(), before, "a bad step must be ignored, not stored");
    }

    #[test]
    fn rolling_composes_with_a_pose() {
        // Roll and lean are independent; using both must not cancel
        // either out.
        let s = wheelie();
        let frame = |pose: Pose, roll: f32| {
            let mut buf = canvas_of(96, 96);
            {
                let mut c = Canvas::new(&mut buf, 96, 96);
                s.draw_ground_rolling(&mut c, 48.0, 70.0, 5.0, pose, roll, None);
            }
            buf
        };
        let lean = Pose { lean: 1.0, turn: 0.0, squat: 0.0 };
        assert_ne!(frame(Pose::UPRIGHT, 1.0), frame(lean, 1.0), "lean must apply");
        assert_ne!(frame(lean, 0.0), frame(lean, 1.0), "roll must apply");
    }
}
