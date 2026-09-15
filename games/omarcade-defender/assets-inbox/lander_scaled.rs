use omarcade_core::{Canvas, Color, Shape, Transform};

/// lander-body — authored in tools/vector-playground.html.
pub const LANDER_BODY: Shape = Shape::new(&[
    (-2.00, -5.00),
    (-5.00, -4.00),
    (-6.00, -1.00),
    (-5.00, 2.00),
    (-2.00, 3.00),
    (-5.00, 6.00),
    (-6.00, 8.00),
    (-6.00, 10.00),
    (-4.00, 10.00),
    (-4.00, 8.00),
    (-3.00, 6.00),
    (-1.00, 5.00),
    (-1.00, 7.00),
    (-1.00, 10.00),
    (1.00, 10.00),
    (1.00, 9.00),
    (1.00, 7.00),
    (1.00, 5.00),
    (3.00, 6.00),
    (4.00, 8.00),
    (4.00, 10.00),
    (6.00, 10.00),
    (6.00, 8.00),
    (5.00, 6.00),
    (2.00, 3.00),
    (5.00, 2.00),
    (6.00, -1.00),
    (5.00, -4.00),
    (2.00, -5.00),
]);

/// lander-light — authored in tools/vector-playground.html.
pub const LANDER_LIGHT: Shape = Shape::new(&[
    (-2.00, -5.00),
    (-1.00, -4.00),
    (-5.00, -4.00),
    (0.00, -3.00),
    (5.00, -4.00),
    (1.00, -4.00),
    (2.00, -5.00),
]);

/// left-window — authored in tools/vector-playground.html.
pub const LANDER_LEFT_WINDOW: Shape = Shape::new(&[
    (-2.00, -1.00),
    (-2.00, 1.00),
    (-1.00, 1.00),
    (-1.00, -1.00),
]);

/// right-window — authored in tools/vector-playground.html.
pub const LANDER_RIGHT_WINDOW: Shape = Shape::new(&[
    (1.00, -1.00),
    (1.00, 1.00),
    (2.00, 1.00),
    (2.00, -1.00),
]);

/// lander-left-shadow — authored in tools/vector-playground.html.
pub const LANDER_LEFT_SHADOW: Shape = Shape::new(&[
    (-5.00, -4.00),
    (-4.00, -2.00),
    (-5.00, -1.00),
    (-2.00, 3.00),
    (-5.00, 2.00),
    (-6.00, -1.00),
]);

/// lander-right-shadow — authored in tools/vector-playground.html.
pub const LANDER_RIGHT_SHADOW: Shape = Shape::new(&[
    (5.00, -4.00),
    (4.00, -2.00),
    (5.00, -1.00),
    (2.00, 3.00),
    (5.00, 2.00),
    (6.00, -1.00),
]);

/// lander-shadow-leg1 — authored in tools/vector-playground.html.
pub const LANDER_SHADOW_LEG1: Shape = Shape::new(&[
    (-1.00, 5.00),
    (-2.00, 5.00),
    (-3.00, 5.00),
    (-4.00, 7.00),
    (-4.00, 8.00),
    (-5.00, 10.00),
    (-4.00, 10.00),
    (-3.00, 8.00),
    (-3.00, 6.00),
]);

/// lander-shadow-leg3 — authored in tools/vector-playground.html.
pub const LANDER_SHADOW_LEG3: Shape = Shape::new(&[
    (1.00, 5.00),
    (3.00, 5.00),
    (4.00, 7.00),
    (4.00, 8.00),
    (5.00, 10.00),
    (4.00, 10.00),
    (3.00, 8.00),
    (3.00, 6.00),
]);

/// lander-shadow-leg2l — authored in tools/vector-playground.html.
pub const LANDER_SHADOW_LEG2L: Shape = Shape::new(&[
    (-1.00, 5.00),
    (-1.00, 7.00),
    (-1.00, 8.00),
    (-1.00, 10.00),
    (0.00, 10.00),
    (0.00, 9.00),
    (0.00, 6.00),
]);

/// lander-body-light1 — authored in tools/vector-playground.html.
pub const LANDER_BODY_LIGHT1: Shape = Shape::new(&[
    (-3.00, 1.00),
    (-2.00, 2.00),
    (-1.00, 2.00),
    (-1.00, 1.00),
]);

/// lander-body-light2 — authored in tools/vector-playground.html.
pub const LANDER_BODY_LIGHT2: Shape = Shape::new(&[
    (1.00, 1.00),
    (3.00, 1.00),
    (2.00, 2.00),
    (1.00, 2.00),
]);

/// Draw the lander at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_lander(canvas: &mut Canvas<'_>, t: &Transform) {
    LANDER_BODY.fill(canvas, t, Color::rgb(30, 218, 16));
    LANDER_LIGHT.fill(canvas, t, Color::rgb(255, 230, 66));
    LANDER_LEFT_WINDOW.fill(canvas, t, Color::rgb(0, 0, 0));
    LANDER_RIGHT_WINDOW.fill(canvas, t, Color::rgb(0, 0, 0));
    LANDER_LEFT_SHADOW.fill(canvas, t, Color::rgb(0, 143, 2));
    LANDER_RIGHT_SHADOW.fill(canvas, t, Color::rgb(0, 143, 2));
    LANDER_SHADOW_LEG1.fill(canvas, t, Color::rgb(0, 143, 2));
    LANDER_SHADOW_LEG3.fill(canvas, t, Color::rgb(0, 143, 2));
    LANDER_SHADOW_LEG2L.fill(canvas, t, Color::rgb(0, 143, 2));
    LANDER_BODY_LIGHT1.fill(canvas, t, Color::rgb(255, 221, 0));
    LANDER_BODY_LIGHT2.fill(canvas, t, Color::rgb(255, 213, 0));
}