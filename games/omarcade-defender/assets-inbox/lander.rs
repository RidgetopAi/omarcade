use omarcade_core::{Canvas, Color, Shape, Transform};

/// lander-body — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LANDER_BODY: Shape = Shape::new(&[
    (-1.00, -2.50),
    (-2.50, -2.00),
    (-3.00, -0.50),
    (-2.50, 1.00),
    (-1.00, 1.50),
    (-2.50, 3.00),
    (-3.00, 4.00),
    (-3.00, 5.00),
    (-2.00, 5.00),
    (-2.00, 4.00),
    (-1.50, 3.00),
    (-0.50, 2.50),
    (-0.50, 3.50),
    (-0.50, 5.00),
    (0.50, 5.00),
    (0.50, 4.50),
    (0.50, 3.50),
    (0.50, 2.50),
    (1.50, 3.00),
    (2.00, 4.00),
    (2.00, 5.00),
    (3.00, 5.00),
    (3.00, 4.00),
    (2.50, 3.00),
    (1.00, 1.50),
    (2.50, 1.00),
    (3.00, -0.50),
    (2.50, -2.00),
    (1.00, -2.50),
]);

/// lander-light — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LANDER_LIGHT: Shape = Shape::new(&[
    (-1.00, -2.50),
    (-0.50, -2.00),
    (-2.50, -2.00),
    (0.00, -1.50),
    (2.50, -2.00),
    (0.50, -2.00),
    (1.00, -2.50),
]);

/// left-window — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LEFT_WINDOW: Shape = Shape::new(&[
    (-1.00, -0.50),
    (-1.00, 0.50),
    (-0.50, 0.50),
    (-0.50, -0.50),
]);

/// right-window — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_RIGHT_WINDOW: Shape = Shape::new(&[
    (0.50, -0.50),
    (0.50, 0.50),
    (1.00, 0.50),
    (1.00, -0.50),
]);

/// lander-left-shadow — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LANDER_LEFT_SHADOW: Shape = Shape::new(&[
    (-2.50, -2.00),
    (-2.00, -1.00),
    (-2.50, -0.50),
    (-1.00, 1.50),
    (-2.50, 1.00),
    (-3.00, -0.50),
]);

/// lander-right-shadow — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LANDER_RIGHT_SHADOW: Shape = Shape::new(&[
    (2.50, -2.00),
    (2.00, -1.00),
    (2.50, -0.50),
    (1.00, 1.50),
    (2.50, 1.00),
    (3.00, -0.50),
]);

/// lander-shadow-leg1 — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LANDER_SHADOW_LEG1: Shape = Shape::new(&[
    (-0.50, 2.50),
    (-1.00, 2.50),
    (-1.50, 2.50),
    (-2.00, 3.50),
    (-2.00, 4.00),
    (-2.50, 5.00),
    (-2.00, 5.00),
    (-1.50, 4.00),
    (-1.50, 3.00),
]);

/// lander-shadow-leg3 — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LANDER_SHADOW_LEG3: Shape = Shape::new(&[
    (0.50, 2.50),
    (1.50, 2.50),
    (2.00, 3.50),
    (2.00, 4.00),
    (2.50, 5.00),
    (2.00, 5.00),
    (1.50, 4.00),
    (1.50, 3.00),
]);

/// lander-shadow-leg2l — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LANDER_SHADOW_LEG2L: Shape = Shape::new(&[
    (-0.50, 2.50),
    (-0.50, 3.50),
    (-0.50, 4.00),
    (-0.50, 5.00),
    (0.00, 5.00),
    (0.00, 4.50),
    (0.00, 3.00),
]);

/// lander-body-light1 — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LANDER_BODY_LIGHT1: Shape = Shape::new(&[
    (-1.50, 0.50),
    (-1.00, 1.00),
    (-0.50, 1.00),
    (-0.50, 0.50),
]);

/// lander-body-light2 — authored in tools/vector-playground.html.
pub const UNNAMED_SHAPE_LANDER_BODY_LIGHT2: Shape = Shape::new(&[
    (0.50, 0.50),
    (1.50, 0.50),
    (1.00, 1.00),
    (0.50, 1.00),
]);

/// Draw unnamed_shape at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_unnamed_shape(canvas: &mut Canvas<'_>, t: &Transform) {
    UNNAMED_SHAPE_LANDER_BODY.fill(canvas, t, Color::rgb(30, 218, 16));
    UNNAMED_SHAPE_LANDER_LIGHT.fill(canvas, t, Color::rgb(255, 230, 66));
    UNNAMED_SHAPE_LEFT_WINDOW.fill(canvas, t, Color::rgb(0, 0, 0));
    UNNAMED_SHAPE_RIGHT_WINDOW.fill(canvas, t, Color::rgb(0, 0, 0));
    UNNAMED_SHAPE_LANDER_LEFT_SHADOW.fill(canvas, t, Color::rgb(0, 143, 2));
    UNNAMED_SHAPE_LANDER_RIGHT_SHADOW.fill(canvas, t, Color::rgb(0, 143, 2));
    UNNAMED_SHAPE_LANDER_SHADOW_LEG1.fill(canvas, t, Color::rgb(0, 143, 2));
    UNNAMED_SHAPE_LANDER_SHADOW_LEG3.fill(canvas, t, Color::rgb(0, 143, 2));
    UNNAMED_SHAPE_LANDER_SHADOW_LEG2L.fill(canvas, t, Color::rgb(0, 143, 2));
    UNNAMED_SHAPE_LANDER_BODY_LIGHT1.fill(canvas, t, Color::rgb(255, 221, 0));
    UNNAMED_SHAPE_LANDER_BODY_LIGHT2.fill(canvas, t, Color::rgb(255, 213, 0));
}