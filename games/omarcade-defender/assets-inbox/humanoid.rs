use omarcade_core::{Canvas, Color, Shape, Transform};

/// body-outline — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_BODY_OUTLINE: Shape = Shape::new(&[
    (-0.50, 0.50),
    (-0.50, 2.50),
    (0.50, 2.50),
    (0.50, 1.00),
    (1.00, 0.50),
    (1.00, -0.50),
    (0.50, -0.50),
    (0.50, -2.00),
    (0.00, -2.00),
    (-0.50, -2.00),
    (-1.00, -2.00),
    (-1.00, 0.50),
]);

/// head — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_HEAD: Shape = Shape::new(&[
    (0.50, -2.00),
    (-1.00, -2.00),
    (-1.00, -0.50),
    (0.50, -0.50),
]);

/// left-arm — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LEFT_ARM: Shape = Shape::new(&[
    (-1.00, -0.50),
    (-1.00, 0.50),
    (-0.50, 1.00),
    (-0.50, -0.50),
]);

/// right-arm — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_RIGHT_ARM: Shape = Shape::new(&[
    (0.50, -0.50),
    (0.50, -1.00),
    (1.00, -1.00),
    (1.00, 0.50),
    (0.50, 1.00),
]);

/// light — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LIGHT: Shape = Shape::new(&[
    (-1.00, -1.50),
    (-0.50, -1.50),
    (-0.50, -0.50),
    (-1.00, -0.50),
]);

/// Draw unnamed_shape at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_unnamed_shape(canvas: &mut Canvas<'_>, t: &Transform) {
    UNNAMED_SHAPE_BODY_OUTLINE.fill(canvas, t, Color::rgb(87, 68, 0));
    UNNAMED_SHAPE_HEAD.fill(canvas, t, Color::rgb(0, 255, 0));
    UNNAMED_SHAPE_LEFT_ARM.fill(canvas, t, Color::rgb(255, 0, 234));
    UNNAMED_SHAPE_RIGHT_ARM.fill(canvas, t, Color::rgb(255, 0, 234));
    UNNAMED_SHAPE_LIGHT.fill(canvas, t, Color::rgb(255, 221, 0));
}