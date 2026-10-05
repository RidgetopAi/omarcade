// ★ CLAUDE'S DRAFT, FOR BRIAN TO APPROVE OR REDRAW (W3, 2026-10-05).
// A small hot cross a Bomber leaves behind — 5x5 units. It cannot be shot;
// in play it pulses so it reads as live.
// Exported in tools/vector-playground.html's exact format: Import it there
// to move any point, then re-export over this file.

use omarcade_core::{Canvas, Color, Shape, Transform};

/// cross — authored in tools/vector-playground.html.
pub const MINE_CROSS: Shape = Shape::new(&[
    (-0.80, -2.60),
    (0.80, -2.60),
    (0.80, -0.80),
    (2.60, -0.80),
    (2.60, 0.80),
    (0.80, 0.80),
    (0.80, 2.60),
    (-0.80, 2.60),
    (-0.80, 0.80),
    (-2.60, 0.80),
    (-2.60, -0.80),
    (-0.80, -0.80),
]);

/// core — authored in tools/vector-playground.html.
pub const MINE_CORE: Shape = Shape::new(&[
    (-1.15, 0.00),
    (0.00, -1.15),
    (1.15, 0.00),
    (0.00, 1.15),
]);

/// Draw mine at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_mine(canvas: &mut Canvas<'_>, t: &Transform) {
    MINE_CROSS.fill(canvas, t, Color::rgb(255, 96, 40));
    MINE_CORE.fill(canvas, t, Color::rgb(255, 240, 185));
}
