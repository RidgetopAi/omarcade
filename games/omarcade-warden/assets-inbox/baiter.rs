// ★ CLAUDE'S DRAFT, FOR BRIAN TO APPROVE OR REDRAW (W3, 2026-10-05).
// A flat, wide, thin saucer that matches your speed — 24x10 units: twice a
// Lander's width, two-thirds its height. Brian's spec: "Large targets". The three rim lights are the shimmer:
// in play they cycle colour, frame to frame.
// Exported in tools/vector-playground.html's exact format: Import it there
// to move any point, then re-export over this file.

use omarcade_core::{Canvas, Color, Shape, Transform};

/// underglow — authored in tools/vector-playground.html.
pub const BAITER_UNDERGLOW: Shape = Shape::new(&[
    (-5.40, 2.16),
    (5.40, 2.16),
    (3.51, 4.32),
    (-3.51, 4.32),
]);

/// hull — authored in tools/vector-playground.html.
pub const BAITER_HULL: Shape = Shape::new(&[
    (-12.15, 0.00),
    (-9.45, -2.16),
    (-4.05, -3.10),
    (4.05, -3.10),
    (9.45, -2.16),
    (12.15, 0.00),
    (9.45, 2.29),
    (4.05, 3.24),
    (-4.05, 3.24),
    (-9.45, 2.29),
]);

/// canopy — authored in tools/vector-playground.html.
pub const BAITER_CANOPY: Shape = Shape::new(&[
    (-4.32, -2.16),
    (-2.97, -4.59),
    (0.00, -5.67),
    (2.97, -4.59),
    (4.32, -2.16),
]);

/// rim — authored in tools/vector-playground.html.
pub const BAITER_RIM: Shape = Shape::new(&[
    (-11.74, -0.61),
    (11.74, -0.61),
    (11.74, 0.74),
    (-11.74, 0.74),
]);

/// light-left — authored in tools/vector-playground.html.
pub const BAITER_LIGHT_LEFT: Shape = Shape::new(&[
    (-8.03, 0.07),
    (-6.75, -1.22),
    (-5.47, 0.07),
    (-6.75, 1.35),
]);

/// light-mid — authored in tools/vector-playground.html.
pub const BAITER_LIGHT_MID: Shape = Shape::new(&[
    (-1.28, 0.07),
    (0.00, -1.22),
    (1.28, 0.07),
    (0.00, 1.35),
]);

/// light-right — authored in tools/vector-playground.html.
pub const BAITER_LIGHT_RIGHT: Shape = Shape::new(&[
    (5.47, 0.07),
    (6.75, -1.22),
    (8.03, 0.07),
    (6.75, 1.35),
]);

/// Draw baiter at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_baiter(canvas: &mut Canvas<'_>, t: &Transform) {
    BAITER_UNDERGLOW.fill(canvas, t, Color::rgb(255, 170, 40));
    BAITER_HULL.fill(canvas, t, Color::rgb(0, 178, 196));
    BAITER_CANOPY.fill(canvas, t, Color::rgb(170, 250, 255));
    BAITER_RIM.fill(canvas, t, Color::rgb(0, 92, 120));
    BAITER_LIGHT_LEFT.fill(canvas, t, Color::rgb(255, 60, 210));
    BAITER_LIGHT_MID.fill(canvas, t, Color::rgb(255, 60, 210));
    BAITER_LIGHT_RIGHT.fill(canvas, t, Color::rgb(255, 60, 210));
}
