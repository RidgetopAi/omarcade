use omarcade_core::{Canvas, Color, Shape, Transform};

/// lander-body — authored in tools/vector-playground.html.
pub const MUTANT_LANDER_BODY: Shape = Shape::new(&[
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

/// humanoid-head — authored in tools/vector-playground.html.
pub const MUTANT_HUMANOID_HEAD: Shape = Shape::new(&[
    (-2.00, -5.00),
    (-2.00, -4.00),
    (-2.00, 1.00),
    (-1.00, 3.00),
    (0.00, 3.00),
    (1.00, -1.00),
    (1.00, -5.00),
]);

/// lander-left-shadow — authored in tools/vector-playground.html.
pub const MUTANT_LANDER_LEFT_SHADOW: Shape = Shape::new(&[
    (-5.00, -4.00),
    (-4.00, -2.00),
    (-5.00, -1.00),
    (-2.00, 3.00),
    (-5.00, 2.00),
    (-6.00, -1.00),
]);

/// lander-right-shadow — authored in tools/vector-playground.html.
pub const MUTANT_LANDER_RIGHT_SHADOW: Shape = Shape::new(&[
    (5.00, -4.00),
    (4.00, -2.00),
    (5.00, -1.00),
    (2.00, 3.00),
    (5.00, 2.00),
    (6.00, -1.00),
]);

/// lander-shadow-leg1 — authored in tools/vector-playground.html.
pub const MUTANT_LANDER_SHADOW_LEG1: Shape = Shape::new(&[
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
pub const MUTANT_LANDER_SHADOW_LEG3: Shape = Shape::new(&[
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
pub const MUTANT_LANDER_SHADOW_LEG2L: Shape = Shape::new(&[
    (-1.00, 5.00),
    (-1.00, 7.00),
    (-1.00, 8.00),
    (-1.00, 10.00),
    (0.00, 10.00),
    (0.00, 9.00),
    (0.00, 6.00),
]);

/// humanoid-arm2 — authored in tools/vector-playground.html.
pub const MUTANT_HUMANOID_ARM2: Shape = Shape::new(&[
    (-2.00, 0.00),
    (-2.00, 3.00),
    (-1.00, 3.00),
    (-1.00, 0.00),
]);

/// humanoid-arm1 — authored in tools/vector-playground.html.
pub const MUTANT_HUMANOID_ARM1: Shape = Shape::new(&[
    (0.00, -1.00),
    (2.00, -1.00),
    (2.00, 3.00),
    (0.00, 3.00),
]);

/// humanoid-body — authored in tools/vector-playground.html.
pub const MUTANT_HUMANOID_BODY: Shape = Shape::new(&[
    (-1.00, 3.00),
    (1.00, 3.00),
    (1.00, 10.00),
    (-1.00, 10.00),
]);

/// humanoid-head-shadow — authored in tools/vector-playground.html.
pub const MUTANT_HUMANOID_HEAD_SHADOW: Shape = Shape::new(&[
    (1.00, -5.00),
    (2.00, -5.00),
    (2.00, -1.00),
    (1.00, -1.00),
    (1.00, 0.00),
    (1.00, 0.00),
]);

/// lander-left-window — authored in tools/vector-playground.html.
pub const MUTANT_LANDER_LEFT_WINDOW: Shape = Shape::new(&[
    (-2.00, -3.00),
    (-3.00, -3.00),
    (-3.00, -2.00),
    (-2.00, -1.00),
]);

/// lander-right-window — authored in tools/vector-playground.html.
pub const MUTANT_LANDER_RIGHT_WINDOW: Shape = Shape::new(&[
    (2.00, -3.00),
    (3.00, -3.00),
    (3.00, -2.00),
    (2.00, -1.00),
]);

/// Draw the mutant at `t`.
///
/// Pieces are filled bottom to top and deliberately OVERLAP rather
/// than sharing edges: two shapes meeting on an exact diagonal leave
/// a hairline, because each is anti-aliased honestly and two
/// half-covered edges reach 75%, not 100%.
pub fn draw_mutant(canvas: &mut Canvas<'_>, t: &Transform) {
    MUTANT_LANDER_BODY.fill(canvas, t, Color::rgb(30, 218, 16));
    MUTANT_HUMANOID_HEAD.fill(canvas, t, Color::rgb(187, 0, 255));
    MUTANT_LANDER_LEFT_SHADOW.fill(canvas, t, Color::rgb(0, 143, 2));
    MUTANT_LANDER_RIGHT_SHADOW.fill(canvas, t, Color::rgb(0, 143, 2));
    MUTANT_LANDER_SHADOW_LEG1.fill(canvas, t, Color::rgb(0, 143, 2));
    MUTANT_LANDER_SHADOW_LEG3.fill(canvas, t, Color::rgb(0, 143, 2));
    MUTANT_LANDER_SHADOW_LEG2L.fill(canvas, t, Color::rgb(0, 143, 2));
    MUTANT_HUMANOID_ARM2.fill(canvas, t, Color::rgb(255, 0, 234));
    MUTANT_HUMANOID_ARM1.fill(canvas, t, Color::rgb(255, 0, 234));
    MUTANT_HUMANOID_BODY.fill(canvas, t, Color::rgb(87, 68, 0));
    MUTANT_HUMANOID_HEAD_SHADOW.fill(canvas, t, Color::rgb(75, 5, 133));
    MUTANT_LANDER_LEFT_WINDOW.fill(canvas, t, Color::rgb(255, 174, 0));
    MUTANT_LANDER_RIGHT_WINDOW.fill(canvas, t, Color::rgb(255, 174, 0));
}