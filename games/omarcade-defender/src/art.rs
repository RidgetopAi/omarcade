//! The ship, as points.
//!
//! Authored in `tools/vector-playground.html` and exported from it. Edit
//! it there rather than here: the numbers below are a drawing, and a
//! drawing is easier to change with a pointer than with a text editor.
//!
//! Pieces are filled bottom to top, and they deliberately OVERLAP rather
//! than share edges. Two shapes meeting on an exact diagonal leave a
//! visible hairline, because each is anti-aliased honestly and two
//! half-covered edges reach 75% rather than 100%. Drawing the hull whole
//! and laying detail on top of it is what avoids that.

use omarcade_core::{Canvas, Color, Shape, Transform};

/// The body: nose to tail, including the tail fin.
pub const HULL: Shape = Shape::new(&[
    (-14.50, -7.00),
    (-11.50, -7.00),
    (-9.00, -3.50),
    (-7.00, -3.50),
    (-3.50, -3.50),
    (-0.50, -3.50),
    (2.00, -4.50),
    (3.50, -5.00),
    (5.00, -5.00),
    (7.00, -5.00),
    (10.50, -4.00),
    (14.00, -3.00),
    (19.00, -1.00),
    (19.50, -0.50),
    (19.00, 0.50),
    (13.00, 1.00),
    (7.50, 1.00),
    (2.50, 1.00),
    (-2.00, 1.00),
    (-6.00, 1.00),
    (-11.00, 1.00),
    (-12.00, 0.50),
    (-13.00, 0.00),
    (-14.50, -1.00),
]);

/// The dark underside, which is what gives the hull its roundness.
pub const WING_SHADOW: Shape = Shape::new(&[
    (1.50, -1.50),
    (-5.00, -1.50),
    (-7.00, -1.00),
    (-14.50, -1.00),
    (-13.50, -0.50),
    (-12.50, 0.00),
    (-11.00, 1.00),
    (-8.00, 1.00),
    (-3.50, 1.00),
    (1.00, 2.00),
    (2.00, 3.00),
    (2.00, 1.50),
    (3.00, 1.00),
    (6.00, 1.50),
    (7.50, 1.50),
    (9.50, 1.50),
    (14.00, 1.00),
    (18.50, 0.50),
    (6.50, -1.50),
    (4.00, -1.50),
]);

/// The wing, hanging below the body.
pub const WING: Shape = Shape::new(&[
    (4.50, -1.50),
    (-1.50, -1.50),
    (-3.50, -1.50),
    (-3.00, -0.50),
    (-1.50, 4.00),
    (1.50, 4.00),
    (6.50, 4.00),
    (3.00, 3.50),
    (2.00, 3.00),
    (1.00, 2.00),
    (0.50, 0.50),
    (2.00, -0.50),
    (3.50, -1.00),
]);

/// The canopy glass — the brightest thing on the ship, and what carries
/// the eye toward the nose.
pub const COCKPIT_WINDOW: Shape = Shape::new(&[
    (3.50, -5.00),
    (5.00, -5.00),
    (7.00, -5.00),
    (10.50, -4.00),
    (14.00, -3.00),
    (13.50, -2.00),
    (12.50, -2.00),
    (11.00, -2.00),
    (8.50, -2.50),
    (6.00, -3.00),
    (1.00, -3.50),
    (-5.50, -3.50),
    (1.00, -4.50),
]);

pub const BACK_WING: Shape = Shape::new(&[
    (-6.50, -1.50),
    (-8.50, -0.50),
    (-10.00, 0.00),
    (-11.50, 0.00),
    (-13.50, -0.50),
    (-15.00, -1.50),
    (-14.50, -1.50),
]);

pub const TAIL_SHADOW: Shape = Shape::new(&[
    (-14.50, -6.50),
    (-14.50, -2.00),
    (-3.00, -2.00),
    (-10.50, -2.50),
    (-11.00, -3.50),
    (-12.00, -6.00),
    (-9.00, -2.50),
    (-11.50, -6.50),
]);

pub const SHADOW_TOP_BOTTOM: Shape = Shape::new(&[
    (7.00, -1.50),
    (-6.50, -1.50),
    (-8.00, -1.00),
    (-2.00, -1.00),
    (6.00, -1.00),
    (9.50, -1.00),
]);

/// The engine. Small on purpose — it is drawn as light, not paint.
pub const EXHAUST: Shape = Shape::new(&[
    (-3.00, -1.00),
    (-3.50, 0.00),
    (-2.50, 1.00),
    (-2.50, 0.00),
]);

pub const COCKPIT_FRAME1: Shape = Shape::new(&[
    (7.50, -5.00),
    (7.00, -4.50),
    (6.50, -3.00),
    (6.00, -3.00),
    (6.50, -5.00),
]);

/// ⚠️ Straightened from the original export, which crossed itself and
/// repeated two points. An even-odd fill renders a crossed outline's
/// overlap HOLLOW, so it came out as a smear rather than a strut.
pub const COCKPIT_FRAME2: Shape = Shape::new(&[
    (12.50, -3.50),
    (12.00, -3.50),
    (11.00, -2.00),
    (11.50, -2.00),
]);

pub const WING_HIGHLIGHT: Shape = Shape::new(&[
    (-2.50, -1.00),
    (2.00, -1.00),
    (0.50, 0.00),
    (0.00, 1.50),
    (1.50, 3.50),
    (-0.50, 3.50),
    (-1.50, 2.50),
    (-2.00, 1.00),
    (-2.50, 0.00),
]);

pub const GUN_MOUNT: Shape = Shape::new(&[
    (14.50, 0.50),
    (16.00, 0.00),
    (17.00, 1.00),
    (15.00, 1.00),
]);

/// ⚠️ Lifted 1.5 units from the original export, where it sat at y 1.5..2.0
/// and floated free below the hull's bottom edge at y 1.0. It now bites a
/// full unit into the hull, which is overlap rather than abutment.
pub const GUN: Shape = Shape::new(&[
    (15.00, 0.50),
    (15.50, 0.50),
    (17.00, 0.50),
    (19.00, 0.50),
    (20.00, 0.00),
    (18.00, 0.00),
    (15.00, 0.00),
]);

/// How big the ship is drawn, as a multiplier on the units above.
///
/// ⚠️ SIZED FROM A RENDER, NOT A GUESS, and the number is inherited rather
/// than re-derived. The placeholder's first version was 26px across and
/// vanished on a 960-wide screen; doubling it fixed that, and the result
/// is what Brian flew and approved. This art is authored so that the same
/// scale lands in the same place — about 68px nose to tail.
pub const SCALE: f32 = 2.0;

/// Draw the ship at `t`.
///
/// The order is the drawing: body, then the shading that rounds it, then
/// the details on top. The exhaust comes last and is ADDITIVE, because an
/// engine is light rather than paint — drawn earlier it was simply buried
/// under the wing.
pub fn draw_ship(canvas: &mut Canvas<'_>, t: &Transform) {
    HULL.fill(canvas, t, Color::rgb(45, 81, 225));
    WING_SHADOW.fill(canvas, t, Color::rgb(0, 5, 138));
    WING.fill(canvas, t, Color::rgb(140, 146, 222));
    COCKPIT_WINDOW.fill(canvas, t, Color::rgb(211, 198, 170));
    BACK_WING.fill(canvas, t, Color::rgb(140, 146, 222));
    TAIL_SHADOW.fill(canvas, t, Color::rgb(0, 5, 138));
    SHADOW_TOP_BOTTOM.fill(canvas, t, Color::rgb(10, 18, 21));
    COCKPIT_FRAME1.fill(canvas, t, Color::rgb(108, 103, 90));
    COCKPIT_FRAME2.fill(canvas, t, Color::rgb(108, 103, 90));
    WING_HIGHLIGHT.fill(canvas, t, Color::rgb(86, 118, 215));
    GUN_MOUNT.fill(canvas, t, Color::rgb(211, 198, 170));
    GUN.fill(canvas, t, Color::rgb(211, 198, 170));
    EXHAUST.fill_add(canvas, t, Color::rgb(248, 140, 18));
}

// =====================================================================
// THE ENEMIES
//
// Authored by Brian in tools/vector-playground.html and scaled to the
// size the render settled on — the Lander and the Mutant are both 12x15
// units, drawing at 24x30px beside the ship's 70x22.
//
// ★ THE MUTANT IS THE LANDER, NOT A SECOND DRAWING OF ONE. It was built
// by importing lander.rs back into the playground and drawing a humanoid
// into the pod, so its body, both shadows and all three legs are
// byte-identical to the Lander's. That is what makes the fusion read as
// a fusion on screen: a Mutant occupies exactly the space a Lander did.
// Verified as identical before and after scaling, not assumed.
// =====================================================================

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
