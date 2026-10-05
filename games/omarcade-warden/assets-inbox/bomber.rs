// ★ CLAUDE'S DRAFT, FOR BRIAN TO APPROVE OR REDRAW (W3, 2026-10-05).
// A small armoured box with stub fins — 14x10 units. In play the body's
// colour cycles through the palette (the original's TIECOL).
// Exported in tools/vector-playground.html's exact format: Import it there
// to move any point, then re-export over this file.

use omarcade_core::{Canvas, Color, Shape, Transform};

/// fin-left — authored in tools/vector-playground.html.
pub const BOMBER_FIN_LEFT: Shape = Shape::new(&[
    (-4.40, -2.00),
    (-7.00, -1.00),
    (-7.00, 1.00),
    (-4.40, 2.00),
]);

/// fin-right — authored in tools/vector-playground.html.
pub const BOMBER_FIN_RIGHT: Shape = Shape::new(&[
    (4.40, -2.00),
    (7.00, -1.00),
    (7.00, 1.00),
    (4.40, 2.00),
]);

/// body — authored in tools/vector-playground.html.
pub const BOMBER_BODY: Shape = Shape::new(&[
    (-4.00, -5.00),
    (4.00, -5.00),
    (5.00, -4.00),
    (5.00, 4.00),
    (4.00, 5.00),
    (-4.00, 5.00),
    (-5.00, 4.00),
    (-5.00, -4.00),
]);

/// panel — authored in tools/vector-playground.html.
pub const BOMBER_PANEL: Shape = Shape::new(&[
    (-3.00, -3.00),
    (3.00, -3.00),
    (3.00, 3.00),
    (-3.00, 3.00),
]);

/// core — authored in tools/vector-playground.html.
pub const BOMBER_CORE: Shape = Shape::new(&[
    (-2.00, 0.00),
    (0.00, -2.00),
    (2.00, 0.00),
    (0.00, 2.00),
]);

/// Draw bomber at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_bomber(canvas: &mut Canvas<'_>, t: &Transform) {
    BOMBER_FIN_LEFT.fill(canvas, t, Color::rgb(150, 20, 110));
    BOMBER_FIN_RIGHT.fill(canvas, t, Color::rgb(150, 20, 110));
    BOMBER_BODY.fill(canvas, t, Color::rgb(232, 40, 165));
    BOMBER_PANEL.fill(canvas, t, Color::rgb(84, 0, 62));
    BOMBER_CORE.fill(canvas, t, Color::rgb(255, 222, 120));
}
