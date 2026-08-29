//! Drawing. The only file that turns state into pixels.
//!
//! Gameplay happens in a fixed 960x720 play field; the window is
//! whatever size Hyprland decides. This module bridges the two with a
//! **letterbox**: one uniform scale factor for both axes, centred, with
//! bars on whichever pair of edges has slack. A non-uniform stretch
//! would be easier, but a stretched Breakout means the ball moves
//! faster horizontally than vertically at the same speed, which players
//! feel immediately even if they cannot name it.

use omarcade_core::{Canvas, Color, Theme};

use crate::state::{GameState, Phase, FIELD_H, FIELD_W};

/// Maps play-field coordinates onto the window.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    scale: f32,
    off_x: f32,
    off_y: f32,
}

impl Viewport {
    /// Fit the play field inside `(w, h)`, preserving aspect ratio.
    pub fn fit(w: u32, h: u32) -> Self {
        // A zero-sized window would give a zero or NaN scale; clamp to
        // something harmless since a frame may still be requested.
        let w = w.max(1) as f32;
        let h = h.max(1) as f32;
        let scale = (w / FIELD_W).min(h / FIELD_H);
        Viewport {
            scale,
            off_x: (w - FIELD_W * scale) / 2.0,
            off_y: (h - FIELD_H * scale) / 2.0,
        }
    }

    fn x(&self, x: f32) -> i32 {
        (self.off_x + x * self.scale).round() as i32
    }

    fn y(&self, y: f32) -> i32 {
        (self.off_y + y * self.scale).round() as i32
    }

    fn len(&self, v: f32) -> u32 {
        // At least one pixel: a thin object that rounds to zero would
        // vanish entirely at small window sizes.
        ((v * self.scale).round() as i32).max(1) as u32
    }

    fn rect(&self, r: crate::geom::Rect, canvas: &mut Canvas<'_>, color: Color) {
        canvas.fill_rect(self.x(r.x), self.y(r.y), self.len(r.w), self.len(r.h), color);
    }
}

/// Brick colours by row, taken live from the theme.
fn palette(theme: &Theme) -> [Color; 6] {
    [theme.red, theme.orange, theme.yellow, theme.green, theme.cyan, theme.blue]
}

/// Draw the whole frame.
pub fn draw(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme) {
    let vp = Viewport::fit(canvas.width(), canvas.height());

    // The letterbox bars are darker than the field, so the play area
    // reads as a distinct surface rather than the window just being an
    // odd shape.
    canvas.clear(theme.darker_background);
    canvas.fill_rect(
        vp.x(0.0),
        vp.y(0.0),
        vp.len(FIELD_W),
        vp.len(FIELD_H),
        theme.background,
    );

    let pal = palette(theme);
    for brick in &state.bricks {
        if brick.alive {
            vp.rect(brick.rect, canvas, pal[brick.color_index % pal.len()]);
        }
    }

    vp.rect(state.paddle.rect(), canvas, theme.foreground);

    // The ball is hidden once the game is over — nothing is in play.
    if state.phase != Phase::Lost && state.phase != Phase::Won {
        vp.rect(state.ball.rect(), canvas, theme.accent);
    }

    draw_hud(state, canvas, theme, &vp);
    draw_phase_message(state, canvas, theme, &vp);
}

fn draw_hud(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    let scale = (vp.scale * 3.0).max(1.0) as u32;
    text(canvas, &format!("SCORE {}", state.score), vp.x(24.0), vp.y(30.0), scale, theme.foreground);

    let lives = format!("LIVES {}", state.lives);
    let width = text_width(&lives, scale) as f32;
    text(
        canvas,
        &lives,
        vp.x(FIELD_W - 24.0) - width as i32,
        vp.y(30.0),
        scale,
        theme.light_foreground,
    );
}

fn draw_phase_message(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    let (msg, color) = match state.phase {
        Phase::Ready => ("PRESS SPACE", theme.light_foreground),
        Phase::Playing => return,
        Phase::Won => ("YOU WIN - ENTER", theme.green),
        Phase::Lost => ("GAME OVER - ENTER", theme.red),
    };

    let scale = (vp.scale * 4.0).max(1.0) as u32;
    let w = text_width(msg, scale) as f32;
    let x = vp.x(FIELD_W / 2.0) - (w / 2.0) as i32;
    let y = vp.y(FIELD_H / 2.0);
    text(canvas, msg, x, y, scale, color);

    // The best score, under the verdict. Only once a game has ended and
    // only if there is one — a first-ever run has nothing to beat, and an
    // empty "BEST 0" would just be noise.
    if state.best == 0 {
        return;
    }

    let beaten = state.score >= state.best;
    let line = if beaten {
        format!("NEW BEST {}", state.best)
    } else {
        format!("BEST {}", state.best)
    };

    let small = (vp.scale * 2.0).max(1.0) as u32;
    let lw = text_width(&line, small) as f32;
    text(
        canvas,
        &line,
        vp.x(FIELD_W / 2.0) - (lw / 2.0) as i32,
        y + (scale * GLYPH_H * 2) as i32,
        small,
        if beaten { theme.yellow } else { theme.light_foreground },
    );
}

// ---------------------------------------------------------------------
// Text
//
// Canvas draws rectangles and nothing else, so glyphs are 5x7 bitmaps
// painted as one rect per pixel. That is enough for a HUD and keeps the
// game free of any font dependency — a real font stack would be a large
// amount of machinery for eighteen characters.
// ---------------------------------------------------------------------

const GLYPH_W: u32 = 5;
const GLYPH_H: u32 = 7;
/// Gap between characters, in glyph pixels.
const GLYPH_SPACING: u32 = 1;

/// Row bitmaps, most significant bit = leftmost of five columns.
fn glyph(c: char) -> Option<[u8; 7]> {
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
        ' ' => [0; 7],
        _ => return None,
    })
}

/// Pixel width of `s` at `scale`.
pub fn text_width(s: &str, scale: u32) -> u32 {
    let n = s.chars().count() as u32;
    if n == 0 {
        return 0;
    }
    (n * GLYPH_W + (n - 1) * GLYPH_SPACING) * scale
}

/// Draw `s` with its top-left at `(x, y)`.
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
    fn viewport_letterboxes_a_wide_window() {
        // Wider than 4:3, so bars go on the left and right.
        let vp = Viewport::fit(1920, 720);
        assert!((vp.scale - 1.0).abs() < 1e-5, "scale limited by height");
        assert!(vp.off_x > 0.0, "horizontal bars expected");
        assert!(vp.off_y.abs() < 1e-5, "no vertical bars");
    }

    #[test]
    fn viewport_letterboxes_a_tall_window() {
        let vp = Viewport::fit(960, 1440);
        assert!((vp.scale - 1.0).abs() < 1e-5, "scale limited by width");
        assert!(vp.off_y > 0.0, "vertical bars expected");
        assert!(vp.off_x.abs() < 1e-5);
    }

    /// The field must land centred and fully inside the window at any
    /// size — this is where letterbox off-by-ones show up.
    #[test]
    fn field_is_centred_and_inside_the_window_at_many_sizes() {
        for &(w, h) in &[
            (1261, 701), // what Hyprland actually gave us last session
            (960, 720),
            (1920, 1080),
            (640, 480),
            (300, 1000),
            (1000, 300),
        ] {
            let vp = Viewport::fit(w, h);
            let left = vp.x(0.0);
            let right = vp.x(FIELD_W);
            let top = vp.y(0.0);
            let bottom = vp.y(FIELD_H);

            assert!(left >= 0, "{w}x{h}: left {left} off-window");
            assert!(top >= 0, "{w}x{h}: top {top} off-window");
            assert!(right <= w as i32, "{w}x{h}: right {right} > {w}");
            assert!(bottom <= h as i32, "{w}x{h}: bottom {bottom} > {h}");

            // Centred: margins equal within a pixel of rounding.
            let mx = (left - (w as i32 - right)).abs();
            let my = (top - (h as i32 - bottom)).abs();
            assert!(mx <= 1, "{w}x{h}: horizontal margins differ by {mx}");
            assert!(my <= 1, "{w}x{h}: vertical margins differ by {my}");
        }
    }

    #[test]
    fn aspect_ratio_is_preserved() {
        let vp = Viewport::fit(1261, 701);
        let w = vp.x(FIELD_W) - vp.x(0.0);
        let h = vp.y(FIELD_H) - vp.y(0.0);
        let want = FIELD_W / FIELD_H;
        let got = w as f32 / h as f32;
        assert!((got - want).abs() < 0.01, "aspect {got} != {want}");
    }

    #[test]
    fn zero_sized_window_does_not_panic_or_nan() {
        let vp = Viewport::fit(0, 0);
        assert!(vp.scale.is_finite());
        assert!(vp.x(10.0).is_positive() || vp.x(10.0) == 0);
    }

    #[test]
    fn thin_objects_never_round_away_to_nothing() {
        let vp = Viewport::fit(100, 75); // heavy downscale
        assert!(vp.len(1.0) >= 1, "a 1-unit object must still be visible");
    }

    #[test]
    fn text_width_matches_glyph_layout() {
        assert_eq!(text_width("", 1), 0);
        assert_eq!(text_width("A", 1), GLYPH_W);
        // Two glyphs plus one space between them.
        assert_eq!(text_width("AB", 1), GLYPH_W * 2 + GLYPH_SPACING);
        assert_eq!(text_width("A", 3), GLYPH_W * 3);
    }

    /// Every character the HUD can render must have a glyph, or words
    /// silently lose letters.
    #[test]
    fn all_hud_characters_have_glyphs() {
        for s in [
            "SCORE 0123456789",
            "LIVES 3",
            "PRESS SPACE",
            "YOU WIN - ENTER",
            "GAME OVER - ENTER",
            "BEST 980",
            "NEW BEST 600",
        ] {
            for c in s.chars() {
                assert!(glyph(c).is_some(), "no glyph for {c:?} in {s:?}");
            }
        }
    }

    /// The list above is written by hand, so it only covers strings someone
    /// remembered to add — "BEST" shipped missing its B because of exactly
    /// that. Requiring the whole printable set makes any future string safe
    /// by construction instead of by vigilance.
    #[test]
    fn the_full_printable_set_has_glyphs() {
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -".chars() {
            assert!(glyph(c).is_some(), "no glyph for {c:?}");
        }
    }

    /// A character with no glyph must not be silently skipped mid-word.
    /// This pins the behaviour that hid the missing B: `text` advances the
    /// cursor for unknown characters, so a gap appears rather than the
    /// remaining letters sliding left as if nothing were wrong.
    #[test]
    fn an_unknown_character_still_occupies_its_cell() {
        let scale = 1;
        // '@' has no glyph; the string must still measure as 3 characters.
        assert_eq!(text_width("A@B", scale), text_width("ABC", scale));
    }
}
