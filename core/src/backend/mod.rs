//! The seam between games and the platform.
//!
//! Everything a game needs to draw and to read input lives here, and
//! nothing in this module names a platform type. A game imports
//! [`Canvas`], [`InputEvent`], [`Key`] and [`Color`]; it never imports
//! winit or softbuffer, and its `Cargo.toml` does not list them.
//!
//! Swapping the shipping backend (winit + softbuffer) for the later
//! layer-shell one means writing a second [`Backend`] impl. No game
//! changes. That constraint is what keeps this module small: if a
//! signature here could not be honoured by a layer-shell surface, it
//! does not belong.

/// An 8-bit-per-channel opaque colour.
///
/// Kept separate from the packed `u32` so the pixel format lives in
/// exactly one place — [`Color::to_u32`] — rather than being open-coded
/// at every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0 };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255 };

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b }
    }

    /// Pack into softbuffer's pixel layout: `0x00RRGGBB`.
    ///
    /// The high byte is unused (not alpha — softbuffer presents opaque
    /// buffers), so it stays zero.
    pub const fn to_u32(self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | (self.b as u32)
    }
}

/// A mutable view over a frame's pixels.
///
/// Borrowed, never owned: the backend hands one of these out per frame
/// wrapping whatever buffer it got from the compositor. `Canvas`
/// allocates nothing and outlives nothing.
pub struct Canvas<'a> {
    buffer: &'a mut [u32],
    width: u32,
    height: u32,
}

impl<'a> Canvas<'a> {
    /// Wrap a backend-owned buffer.
    ///
    /// # Panics
    /// If `buffer.len()` is smaller than `width * height` — a backend bug,
    /// and one worth failing loudly on rather than rendering garbage.
    pub fn new(buffer: &'a mut [u32], width: u32, height: u32) -> Self {
        let need = (width as usize) * (height as usize);
        assert!(
            buffer.len() >= need,
            "canvas buffer too small: {} px for {width}x{height} ({need} px)",
            buffer.len(),
        );
        Canvas { buffer, width, height }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Paint every pixel.
    pub fn clear(&mut self, color: Color) {
        self.buffer[..(self.width as usize) * (self.height as usize)].fill(color.to_u32());
    }

    /// Fill an axis-aligned rectangle, clipped to the canvas.
    ///
    /// Coordinates are signed and the rect is clipped, not validated: a
    /// ball at `x = -3` or hanging off the right edge draws its visible
    /// part and nothing else. Games should not have to bounds-check
    /// before every draw, and a panic mid-frame is never the right
    /// answer to an object leaving the play field.
    pub fn fill_rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color) {
        if w == 0 || h == 0 {
            return;
        }

        // Clip to [0, width) x [0, height) in i64 so a huge w/h cannot
        // overflow the addition before we have clamped it.
        let x0 = x.max(0) as i64;
        let y0 = y.max(0) as i64;
        let x1 = ((x as i64) + (w as i64)).min(self.width as i64);
        let y1 = ((y as i64) + (h as i64)).min(self.height as i64);

        if x0 >= x1 || y0 >= y1 {
            return; // fully off-canvas
        }

        let (x0, x1) = (x0 as usize, x1 as usize);
        let (y0, y1) = (y0 as usize, y1 as usize);
        let stride = self.width as usize;
        let packed = color.to_u32();

        for row in y0..y1 {
            let start = row * stride;
            self.buffer[start + x0..start + x1].fill(packed);
        }
    }
}

/// Keys the suite actually binds.
///
/// Deliberately our own enum rather than a re-export. A
/// `winit::keyboard::KeyCode` here would leak the platform straight
/// through the seam and make the layer-shell backend a breaking change.
/// Unmapped keys are simply not delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Left,
    Right,
    Up,
    Down,
    Space,
    Enter,
    Escape,
    P,
}

/// What the platform tells the game about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    KeyDown(Key),
    KeyUp(Key),
    /// The window manager asked us to close (Super+Q, title-bar X).
    /// Distinct from Escape: the game may refuse Escape, never this.
    CloseRequested,
    Resized { width: u32, height: u32 },
}

/// A game: state that reacts to input and paints frames.
///
/// The backend drives this, not the other way round — games do not own
/// a loop. That inversion is what lets the same game run under an
/// ordinary window today and a layer-shell surface later.
pub trait Game {
    /// Handle one input event. Return `false` to quit.
    fn on_input(&mut self, event: InputEvent) -> bool;

    /// Advance simulation by `dt` seconds.
    fn update(&mut self, dt: f32);

    /// Paint the current state.
    fn render(&mut self, canvas: &mut Canvas<'_>);
}

pub mod winit_soft;

/// A platform that can host a [`Game`].
///
/// Intentionally one method. Everything else a backend does — creating a
/// surface, resizing buffers, translating events, pacing frames — is its
/// own business and must not appear in this signature, or games would
/// start depending on it.
pub trait Backend {
    type Error;

    /// Run `game` to completion, returning when it quits.
    fn run<G: Game>(self, game: G) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Color = Color::rgb(255, 0, 0);

    fn canvas_of(w: u32, h: u32) -> Vec<u32> {
        vec![0; (w * h) as usize]
    }

    fn count_set(buf: &[u32]) -> usize {
        buf.iter().filter(|&&p| p != 0).count()
    }

    #[test]
    fn color_packs_to_0x00rrggbb() {
        assert_eq!(Color::rgb(0x7f, 0xbb, 0xb3).to_u32(), 0x007fbbb3);
        assert_eq!(Color::BLACK.to_u32(), 0x0000_0000);
        assert_eq!(Color::WHITE.to_u32(), 0x00ff_ffff);
    }

    #[test]
    fn clear_paints_every_pixel() {
        let mut buf = canvas_of(4, 3);
        Canvas::new(&mut buf, 4, 3).clear(RED);
        assert_eq!(count_set(&buf), 12);
    }

    #[test]
    fn fill_rect_lands_where_asked() {
        let mut buf = canvas_of(10, 10);
        Canvas::new(&mut buf, 10, 10).fill_rect(2, 3, 4, 2, RED);
        assert_eq!(count_set(&buf), 8);
        // row 3, cols 2..6
        assert_eq!(buf[3 * 10 + 2], RED.to_u32());
        assert_eq!(buf[3 * 10 + 5], RED.to_u32());
        assert_eq!(buf[3 * 10 + 6], 0, "must not paint past the right edge");
        assert_eq!(buf[2 * 10 + 2], 0, "must not paint the row above");
    }

    #[test]
    fn negative_origin_clips_instead_of_panicking() {
        let mut buf = canvas_of(10, 10);
        // Straddles the top-left corner: only the 2x2 at (0,0) is visible.
        Canvas::new(&mut buf, 10, 10).fill_rect(-3, -3, 5, 5, RED);
        assert_eq!(count_set(&buf), 4);
        assert_eq!(buf[0], RED.to_u32());
        assert_eq!(buf[2], 0);
    }

    #[test]
    fn rect_hanging_off_the_far_edge_clips() {
        let mut buf = canvas_of(10, 10);
        Canvas::new(&mut buf, 10, 10).fill_rect(8, 8, 100, 100, RED);
        assert_eq!(count_set(&buf), 4);
    }

    #[test]
    fn fully_offscreen_draws_nothing() {
        let mut buf = canvas_of(10, 10);
        {
            let mut c = Canvas::new(&mut buf, 10, 10);
            c.fill_rect(50, 50, 4, 4, RED);
            c.fill_rect(-20, 0, 4, 4, RED);
            c.fill_rect(0, -20, 4, 4, RED);
        }
        assert_eq!(count_set(&buf), 0);
    }

    #[test]
    fn zero_sized_rect_draws_nothing() {
        let mut buf = canvas_of(10, 10);
        {
            let mut c = Canvas::new(&mut buf, 10, 10);
            c.fill_rect(1, 1, 0, 5, RED);
            c.fill_rect(1, 1, 5, 0, RED);
        }
        assert_eq!(count_set(&buf), 0);
    }

    #[test]
    fn extreme_coords_do_not_overflow() {
        let mut buf = canvas_of(10, 10);
        {
            let mut c = Canvas::new(&mut buf, 10, 10);
            // i32::MAX + huge w would wrap if the clip used i32 maths.
            c.fill_rect(i32::MAX, 0, u32::MAX, 4, RED);
            c.fill_rect(i32::MIN, 0, u32::MAX, 4, RED);
        }
        // The second spans the whole width once clamped: 10 px * 4 rows.
        assert_eq!(count_set(&buf), 40);
    }
}
