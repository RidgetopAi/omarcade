use omarcade_core::{Canvas, Color, Shape, Transform};

/// body-outline — authored in tools/vector-playground.html.
pub const HUMANOID_BODY_OUTLINE: Shape = Shape::new(&[
    (-1.25, 1.25),
    (-1.25, 6.25),
    (1.25, 6.25),
    (1.25, 2.50),
    (2.50, 1.25),
    (2.50, -1.25),
    (1.25, -1.25),
    (1.25, -5.00),
    (0.00, -5.00),
    (-1.25, -5.00),
    (-2.50, -5.00),
    (-2.50, 1.25),
]);

/// head — authored in tools/vector-playground.html.
pub const HUMANOID_HEAD: Shape = Shape::new(&[
    (1.25, -5.00),
    (-2.50, -5.00),
    (-2.50, -1.25),
    (1.25, -1.25),
]);

/// left-arm — authored in tools/vector-playground.html.
pub const HUMANOID_LEFT_ARM: Shape = Shape::new(&[
    (-2.50, -1.25),
    (-2.50, 1.25),
    (-1.25, 2.50),
    (-1.25, -1.25),
]);

/// right-arm — authored in tools/vector-playground.html.
pub const HUMANOID_RIGHT_ARM: Shape = Shape::new(&[
    (1.25, -1.25),
    (1.25, -2.50),
    (2.50, -2.50),
    (2.50, 1.25),
    (1.25, 2.50),
]);

/// light — authored in tools/vector-playground.html.
pub const HUMANOID_LIGHT: Shape = Shape::new(&[
    (-2.50, -3.75),
    (-1.25, -3.75),
    (-1.25, -1.25),
    (-2.50, -1.25),
]);

/// Draw the humanoid at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_humanoid(canvas: &mut Canvas<'_>, t: &Transform) {
    HUMANOID_BODY_OUTLINE.fill(canvas, t, Color::rgb(87, 68, 0));
    HUMANOID_HEAD.fill(canvas, t, Color::rgb(0, 255, 0));
    HUMANOID_LEFT_ARM.fill(canvas, t, Color::rgb(255, 0, 234));
    HUMANOID_RIGHT_ARM.fill(canvas, t, Color::rgb(255, 0, 234));
    HUMANOID_LIGHT.fill(canvas, t, Color::rgb(255, 221, 0));
}