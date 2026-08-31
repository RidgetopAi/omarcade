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

/// An 8-bit-per-channel colour with a blend weight.
///
/// Kept separate from the packed `u32` so the pixel format lives in
/// exactly one place — [`Color::to_u32`] — rather than being open-coded
/// at every call site.
///
/// `a` is **not** window transparency: softbuffer presents opaque
/// buffers, so there is nothing behind the frame to show through. It is
/// how much of this colour to mix into what is already on the canvas,
/// applied at draw time. That is what trails, glow, fades and dimmed
/// overlays are made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
    /// Draws nothing. Useful as a "no colour" sentinel in tables.
    pub const TRANSPARENT: Color = Color::rgba(0, 0, 0, 0);

    /// Fully opaque.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    /// `a` is the blend weight: 0 draws nothing, 255 fully replaces.
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Color { r, g, b, a }
    }

    /// The same colour at a different blend weight.
    pub const fn with_alpha(self, a: u8) -> Self {
        Color { a, ..self }
    }

    /// True when this colour writes pixels unchanged, which lets the
    /// draw path skip blending entirely. The overwhelmingly common case.
    pub const fn is_opaque(self) -> bool {
        self.a == 255
    }

    /// Mix towards `other` by `t` in `0.0..=1.0`, per channel.
    ///
    /// For palette transitions — a brick fading to its hit colour, a
    /// theme swap easing in — rather than for per-pixel compositing.
    pub fn lerp(self, other: Color, t: f32) -> Color {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color {
            r: mix(self.r, other.r),
            g: mix(self.g, other.g),
            b: mix(self.b, other.b),
            a: mix(self.a, other.a),
        }
    }

    /// Pull a colour toward grey, keeping its perceived brightness.
    ///
    /// `amount` is 0.0 (unchanged) to 1.0 (fully neutral). The grey it
    /// moves toward is the colour's own **luminance**, not the average of
    /// its channels: the eye is far more sensitive to green than to blue,
    /// so a naive average sends greens noticeably darker and blues
    /// lighter. Preserving luminance means desaturating changes the hue
    /// without changing how bright the surface reads.
    ///
    /// This exists because theme-derived surfaces can be far more
    /// saturated than their role wants. Measured across the installed
    /// Omarchy themes, road tarmac derived from theme slots ranged from
    /// chroma 0.000 to 0.273 — some themes put a vividly coloured road on
    /// screen. Capping saturation keeps every theme recolouring the scene
    /// while stopping a surface that should read as tarmac from reading as
    /// paint.
    pub fn desaturated(self, amount: f32) -> Color {
        let amount = amount.clamp(0.0, 1.0);
        // Rec. 709 luma, the same weighting used to judge contrast.
        let y = 0.2126 * self.r as f32 + 0.7152 * self.g as f32 + 0.0722 * self.b as f32;
        let mix = |c: u8| (c as f32 + (y - c as f32) * amount).round().clamp(0.0, 255.0) as u8;
        Color { r: mix(self.r), g: mix(self.g), b: mix(self.b), a: self.a }
    }

    /// Pack into softbuffer's pixel layout: `0x00RRGGBB`.
    ///
    /// The high byte stays zero: softbuffer presents opaque buffers, so
    /// alpha has already been resolved by the draw path before a pixel
    /// reaches here.
    pub const fn to_u32(self) -> u32 {
        (self.r as u32) << 16 | (self.g as u32) << 8 | (self.b as u32)
    }

    /// Unpack a canvas pixel so it can be blended against.
    const fn from_u32(v: u32) -> Self {
        Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    /// `self` composited over `dst` at `self.a`.
    ///
    /// Integer maths on purpose: this runs per pixel, and the rounding
    /// term keeps a 50% blend of 0 and 255 at 128 rather than 127.
    fn over(self, dst: Color) -> Color {
        let a = self.a as u32;
        let inv = 255 - a;
        let ch = |s: u8, d: u8| (((s as u32 * a) + (d as u32 * inv) + 127) / 255) as u8;
        Color::rgb(ch(self.r, dst.r), ch(self.g, dst.g), ch(self.b, dst.b))
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

        // Opaque is the overwhelmingly common case and gets the fast
        // path: a memset per row, no read-back, no per-pixel maths.
        if color.is_opaque() {
            for row in y0..y1 {
                let start = row * stride;
                self.buffer[start + x0..start + x1].fill(packed);
            }
            return;
        }

        if color.a == 0 {
            return;
        }

        for row in y0..y1 {
            let start = row * stride;
            for px in &mut self.buffer[start + x0..start + x1] {
                *px = color.over(Color::from_u32(*px)).to_u32();
            }
        }
    }

    /// Fill a rectangle at fractional coordinates, anti-aliasing the edges.
    ///
    /// The physics is continuous but [`fill_rect`](Self::fill_rect)
    /// snaps to whole pixels, so a ball crossing a pixel boundary at
    /// 60fps judders even though nothing is wrong with the simulation.
    /// This spreads a partly-covered edge pixel proportionally instead,
    /// which is what makes motion read as smooth.
    ///
    /// Costs more than the integer path — use it for things that *move*,
    /// not for a static background.
    pub fn fill_rect_f(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        // Every coordinate must be finite and the extent positive. A NaN
        // reaching the bounds maths below would silently produce an empty
        // or enormous range, so it is rejected here rather than clamped.
        if !(x.is_finite() && y.is_finite() && w.is_finite() && h.is_finite()) {
            return;
        }
        if w <= 0.0 || h <= 0.0 || color.a == 0 {
            return;
        }

        let (x0, x1) = (x, x + w);
        let (y0, y1) = (y, y + h);

        // Touched pixel range, clipped to the canvas.
        let px0 = (x0.floor().max(0.0)) as i64;
        let py0 = (y0.floor().max(0.0)) as i64;
        let px1 = (x1.ceil().min(self.width as f32)) as i64;
        let py1 = (y1.ceil().min(self.height as f32)) as i64;
        if px0 >= px1 || py0 >= py1 {
            return;
        }

        let stride = self.width as usize;
        for py in py0..py1 {
            // How much of this pixel row the rect covers, in 0.0..=1.0.
            let cy = (y1.min(py as f32 + 1.0) - y0.max(py as f32)).clamp(0.0, 1.0);
            if cy <= 0.0 {
                continue;
            }
            let start = py as usize * stride;

            for px in px0..px1 {
                let cx = (x1.min(px as f32 + 1.0) - x0.max(px as f32)).clamp(0.0, 1.0);
                if cx <= 0.0 {
                    continue;
                }

                let cover = cx * cy * (color.a as f32 / 255.0);
                let a = (cover * 255.0).round() as u8;
                if a == 0 {
                    continue;
                }

                let i = start + px as usize;
                let src = color.with_alpha(a);
                self.buffer[i] = src.over(Color::from_u32(self.buffer[i])).to_u32();
            }
        }
    }

    /// Darken or tint the whole canvas by drawing `color` over it.
    ///
    /// For a game-over dim or a pause veil: one call rather than a
    /// full-screen `fill_rect` a game has to size itself.
    ///
    /// **The most expensive call here.** Every pixel is read, blended and
    /// written, with no memset fast path. Measured at 960x720 it costs
    /// roughly 7x an opaque frame — 3.2% of a 60fps budget, so one per
    /// frame is fine and three stacked is a real cost. Prefer it for
    /// states (paused, game over) over per-frame effects.
    pub fn veil(&mut self, color: Color) {
        self.fill_rect(0, 0, self.width, self.height, color);
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

    fn chroma(c: Color) -> f32 {
        let (r, g, b) = (c.r as f32, c.g as f32, c.b as f32);
        let mx = r.max(g).max(b);
        let mn = r.min(g).min(b);
        if mx == 0.0 { 0.0 } else { (mx - mn) / mx }
    }

    fn luma(c: Color) -> f32 {
        0.2126 * c.r as f32 + 0.7152 * c.g as f32 + 0.0722 * c.b as f32
    }

    #[test]
    fn desaturating_removes_hue() {
        let green = Color::rgb(0xa7, 0xc0, 0x80);
        assert!(chroma(green) > 0.3);
        assert!(chroma(green.desaturated(0.75)) < chroma(green) * 0.3);
        assert!(chroma(green.desaturated(1.0)) < 0.01, "fully desaturated must be grey");
    }

    #[test]
    fn desaturating_by_zero_changes_nothing() {
        let c = Color::rgb(0xa7, 0xc0, 0x80);
        let d = c.desaturated(0.0);
        assert_eq!((c.r, c.g, c.b), (d.r, d.g, d.b));
    }

    /// The reason this greys toward LUMINANCE rather than the channel
    /// average: the eye weights green ~10x more than blue, so a naive
    /// average sends greens darker and blues lighter. A road that changed
    /// brightness when it was desaturated would trade one artefact for
    /// another.
    #[test]
    fn desaturating_preserves_perceived_brightness() {
        for c in [
            Color::rgb(0xa7, 0xc0, 0x80), // green
            Color::rgb(0x7f, 0xbb, 0xb3), // teal
            Color::rgb(0xe6, 0x7e, 0x80), // red
            Color::rgb(0x21, 0x27, 0x2c), // near-black
        ] {
            let before = luma(c);
            for amount in [0.25f32, 0.5, 0.75, 1.0] {
                let after = luma(c.desaturated(amount));
                assert!(
                    (after - before).abs() < 1.5,
                    "desaturating {c:?} by {amount} moved luma {before:.1} -> {after:.1}",
                );
            }
        }
    }

    #[test]
    fn desaturating_leaves_alpha_alone() {
        let c = Color::rgba(0xa7, 0xc0, 0x80, 0x40);
        assert_eq!(c.desaturated(1.0).a, 0x40);
    }

    /// Out-of-range input is clamped rather than producing nonsense, the
    /// same as `lerp` does.
    #[test]
    fn desaturation_amount_is_clamped() {
        let c = Color::rgb(0xa7, 0xc0, 0x80);
        assert_eq!(
            (c.desaturated(4.0).r, c.desaturated(4.0).g),
            (c.desaturated(1.0).r, c.desaturated(1.0).g),
        );
        assert_eq!(c.desaturated(-2.0).g, c.g);
    }
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

#[cfg(test)]
mod blend_tests {
    use super::*;

    fn canvas_of(w: u32, h: u32, fill: Color) -> Vec<u32> {
        vec![fill.to_u32(); (w * h) as usize]
    }

    #[test]
    fn rgb_is_opaque_and_rgba_is_not() {
        assert!(Color::rgb(1, 2, 3).is_opaque());
        assert!(!Color::rgba(1, 2, 3, 128).is_opaque());
        assert_eq!(Color::rgb(1, 2, 3).a, 255);
    }

    #[test]
    fn opaque_fill_replaces_the_pixel_exactly() {
        let mut buf = canvas_of(4, 4, Color::BLACK);
        let mut c = Canvas::new(&mut buf, 4, 4);
        c.fill_rect(0, 0, 4, 4, Color::WHITE);
        assert!(buf.iter().all(|&p| p == Color::WHITE.to_u32()));
    }

    #[test]
    fn a_fully_transparent_fill_draws_nothing() {
        let mut buf = canvas_of(4, 4, Color::BLACK);
        let before = buf.clone();
        let mut c = Canvas::new(&mut buf, 4, 4);
        c.fill_rect(0, 0, 4, 4, Color::rgba(255, 255, 255, 0));
        assert_eq!(buf, before);
    }

    #[test]
    fn half_alpha_white_over_black_lands_mid_grey() {
        let mut buf = canvas_of(2, 2, Color::BLACK);
        let mut c = Canvas::new(&mut buf, 2, 2);
        c.fill_rect(0, 0, 2, 2, Color::rgba(255, 255, 255, 128));
        // 128/255 of the way from 0 to 255, with rounding.
        let got = Color::from_u32(buf[0]);
        assert_eq!(got.r, 128, "got {got:?}");
        assert_eq!(got.r, got.g);
        assert_eq!(got.g, got.b);
    }

    #[test]
    fn blending_is_idempotent_at_full_alpha() {
        // An a=255 colour through the blend path must equal the fast path.
        let mut a = canvas_of(2, 2, Color::rgb(10, 20, 30));
        let mut b = a.clone();
        Canvas::new(&mut a, 2, 2).fill_rect(0, 0, 2, 2, Color::rgb(200, 100, 50));
        Canvas::new(&mut b, 2, 2).fill_rect(0, 0, 2, 2, Color::rgba(200, 100, 50, 255));
        assert_eq!(a, b);
    }

    #[test]
    fn repeated_veils_darken_monotonically() {
        // The trail/fade primitive: each pass must move towards the veil
        // colour and never overshoot past it.
        let mut buf = canvas_of(1, 1, Color::WHITE);
        let mut last = 255u8;
        for _ in 0..12 {
            Canvas::new(&mut buf, 1, 1).veil(Color::rgba(0, 0, 0, 40));
            let v = Color::from_u32(buf[0]).r;
            assert!(v <= last, "veil brightened the canvas: {last} -> {v}");
            last = v;
        }
        assert!(last < 100, "twelve veils should have darkened it, got {last}");
    }

    #[test]
    fn subpixel_rect_covers_a_whole_pixel_fully() {
        let mut buf = canvas_of(3, 3, Color::BLACK);
        let mut c = Canvas::new(&mut buf, 3, 3);
        c.fill_rect_f(1.0, 1.0, 1.0, 1.0, Color::WHITE);
        assert_eq!(Color::from_u32(buf[1 * 3 + 1]).r, 255, "centre must be full");
        assert_eq!(Color::from_u32(buf[0]).r, 0, "corner must be untouched");
    }

    #[test]
    fn subpixel_rect_anti_aliases_a_half_covered_pixel() {
        let mut buf = canvas_of(3, 1, Color::BLACK);
        let mut c = Canvas::new(&mut buf, 3, 1);
        // Covers x from 0.5 to 1.5: half of pixel 0, half of pixel 1.
        c.fill_rect_f(0.5, 0.0, 1.0, 1.0, Color::WHITE);
        let p0 = Color::from_u32(buf[0]).r;
        let p1 = Color::from_u32(buf[1]).r;
        assert!((120..=135).contains(&p0), "half coverage should be ~128, got {p0}");
        assert!((120..=135).contains(&p1), "half coverage should be ~128, got {p1}");
    }

    #[test]
    fn subpixel_motion_is_gradual_not_stepped() {
        // The whole point: nudging by a fraction of a pixel must change
        // the image. With integer fill_rect it would not.
        let mut seen = Vec::new();
        for i in 0..5 {
            let mut buf = canvas_of(4, 1, Color::BLACK);
            Canvas::new(&mut buf, 4, 1).fill_rect_f(1.0 + i as f32 * 0.2, 0.0, 1.0, 1.0, Color::WHITE);
            seen.push(Color::from_u32(buf[1]).r);
        }
        assert!(seen.windows(2).any(|w| w[0] != w[1]), "sub-pixel moves produced identical frames: {seen:?}");
    }

    #[test]
    fn degenerate_subpixel_rects_are_ignored() {
        let mut buf = canvas_of(2, 2, Color::BLACK);
        let before = buf.clone();
        let mut c = Canvas::new(&mut buf, 2, 2);
        c.fill_rect_f(0.0, 0.0, 0.0, 5.0, Color::WHITE);
        c.fill_rect_f(0.0, 0.0, -3.0, 5.0, Color::WHITE);
        c.fill_rect_f(f32::NAN, 0.0, 1.0, 1.0, Color::WHITE);
        c.fill_rect_f(0.0, f32::INFINITY, 1.0, 1.0, Color::WHITE);
        assert_eq!(buf, before, "a degenerate rect must draw nothing, not panic");
    }

    #[test]
    fn subpixel_rect_clips_off_canvas() {
        let mut buf = canvas_of(2, 2, Color::BLACK);
        // Entirely off to the left: nothing drawn.
        Canvas::new(&mut buf, 2, 2).fill_rect_f(-50.0, 0.0, 10.0, 10.0, Color::WHITE);
        assert!(buf.iter().all(|&p| p == Color::BLACK.to_u32()));
        // Partly overlapping from the right: the visible sliver draws.
        Canvas::new(&mut buf, 2, 2).fill_rect_f(1.5, -1.0, 10.0, 10.0, Color::WHITE);
        assert!(Color::from_u32(buf[1]).r > 0, "the visible sliver must draw");
    }

    #[test]
    fn color_lerp_moves_between_two_colours() {
        let a = Color::rgb(0, 0, 0);
        let b = Color::rgb(255, 100, 50);
        assert_eq!(a.lerp(b, 0.0), a);
        assert_eq!(a.lerp(b, 1.0), b);
        let mid = a.lerp(b, 0.5);
        assert_eq!(mid.r, 128);
        assert_eq!(mid.g, 50);
    }
}
