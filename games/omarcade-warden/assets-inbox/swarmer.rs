// ★ CLAUDE'S DRAFT, FOR BRIAN TO APPROVE OR REDRAW (W4, 2026-10-06).
// A tiny teardrop that comes in packs — 7x5 units, about half a Lander
// (the original's is 6x4 px to the Lander's 10x8). Point first, drawn
// facing LEFT; `.flipped()` faces it right. Red body, orange glow at the
// heavy end so a pack reads as a scatter of sparks.
// Exported in tools/vector-playground.html's exact format: Import it there
// to move any point, then re-export over this file.

use omarcade_core::{Canvas, Color, Shape, Transform};

/// body — authored in tools/vector-playground.html.
pub const SWARMER_BODY: Shape = Shape::new(&[
    (-3.80, 0.00),
    (-0.71, -1.92),
    (0.57, -2.48),
    (1.96, -2.27),
    (3.01, -1.34),
    (3.40, 0.00),
    (3.01, 1.34),
    (1.96, 2.27),
    (0.57, 2.48),
    (-0.71, 1.92),
]);

/// glow — authored in tools/vector-playground.html.
pub const SWARMER_GLOW: Shape = Shape::new(&[
    (0.20, 0.00),
    (1.10, -1.20),
    (2.30, -1.05),
    (2.75, 0.00),
    (2.30, 1.05),
    (1.10, 1.20),
]);

/// Draw swarmer at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_swarmer(canvas: &mut Canvas<'_>, t: &Transform) {
    SWARMER_BODY.fill(canvas, t, Color::rgb(235, 36, 36));
    SWARMER_GLOW.fill(canvas, t, Color::rgb(255, 160, 40));
}
