// ★ CLAUDE'S DRAFT, FOR BRIAN TO APPROVE OR REDRAW (W4, 2026-10-06).
// A starburst that drifts and never shoots — 13x13 units, about a Lander's
// size (the original's Pod is 8x8 px to the Lander's 10x8). Purple spikes,
// a red four-point star inside, a hot core: it reads as a shell that is
// full of something. Shoot it and up to six Swarmers burst out.
// Exported in tools/vector-playground.html's exact format: Import it there
// to move any point, then re-export over this file.

use omarcade_core::{Canvas, Color, Shape, Transform};

/// burst — authored in tools/vector-playground.html.
pub const POD_BURST: Shape = Shape::new(&[
    (0.00, -6.60),
    (1.26, -3.05),
    (4.67, -4.67),
    (3.05, -1.26),
    (6.60, 0.00),
    (3.05, 1.26),
    (4.67, 4.67),
    (1.26, 3.05),
    (0.00, 6.60),
    (-1.26, 3.05),
    (-4.67, 4.67),
    (-3.05, 1.26),
    (-6.60, 0.00),
    (-3.05, -1.26),
    (-4.67, -4.67),
    (-1.26, -3.05),
]);

/// star — authored in tools/vector-playground.html.
pub const POD_STAR: Shape = Shape::new(&[
    (3.11, -3.11),
    (1.60, 0.00),
    (3.11, 3.11),
    (0.00, 1.60),
    (-3.11, 3.11),
    (-1.60, 0.00),
    (-3.11, -3.11),
    (0.00, -1.60),
]);

/// core — authored in tools/vector-playground.html.
pub const POD_CORE: Shape = Shape::new(&[
    (1.50, 0.00),
    (1.06, 1.06),
    (0.00, 1.50),
    (-1.06, 1.06),
    (-1.50, 0.00),
    (-1.06, -1.06),
    (0.00, -1.50),
    (1.06, -1.06),
]);

/// Draw pod at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_pod(canvas: &mut Canvas<'_>, t: &Transform) {
    POD_BURST.fill(canvas, t, Color::rgb(168, 60, 255));
    POD_STAR.fill(canvas, t, Color::rgb(255, 40, 72));
    POD_CORE.fill(canvas, t, Color::rgb(255, 220, 120));
}
