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
/// The ship.
///
/// ★ `exhaust` IS THE FLARE'S INTENSITY, 0.0 AT REST. The hull flare has
/// always been drawn — it is the warmth visible in Brian's no-thrust
/// frame — but it was a CONSTANT, so the ship looked equally lit whether
/// the engine was firing or not. Measuring his frames, thrust roughly
/// DOUBLES the warm pixel count (93 -> ~196), and about half of that
/// arrives as a trailing cloud; the rest is the ship's own tail getting
/// hotter. This is that half.
///
/// ⚠️ THE LIVES ROW PASSES 0.0 AND MUST KEEP DOING SO. A row of little
/// ships showing live exhaust would read as five engines burning in the
/// HUD.
pub fn draw_ship(canvas: &mut Canvas<'_>, t: &Transform, exhaust: f32) {
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
    // ★ THE FLARE. At rest this is the original constant, byte for byte,
    // so a ship sitting still looks exactly as it always has. Under
    // thrust it grows toward the pale-yellow hot end AND grows in size,
    // because a flare that only changed colour stayed the same shape and
    // read as a recolour rather than as more fire.
    let e = exhaust.clamp(0.0, 1.0);
    let flare = Color::rgb(
        248 + ((255 - 248) as f32 * e) as u8,
        140 + ((236 - 140) as f32 * e) as u8,
        18 + ((170 - 18) as f32 * e) as u8,
    );
    if e > 0.0 {
        // ⚠️ MULTIPLIES THE EXISTING SCALE. `Transform::scaled` REPLACES
        // the scale rather than compounding it, so passing the growth
        // factor alone would draw the hot flare at 1.55px instead of at
        // `art::SCALE` times 1.55 — a flare that got SMALLER under
        // thrust. Read the builder, do not assume it multiplies.
        let hot = t.scaled(t.scale * (1.0 + EXHAUST_GROWTH * e));
        EXHAUST.fill_add(canvas, &hot, flare);
    }
    EXHAUST.fill_add(canvas, t, flare);
}

/// How much bigger the hull flare gets at full thrust.
const EXHAUST_GROWTH: f32 = 0.55;

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

// =====================================================================
// THE HUMANOIDS
//
// The people on the surface, and the reason any of this matters —
// Defender is a PROTECTION game with shooting in it, not a shooter with
// civilians standing around.
//
// ★ APPROVED AS DRAWN. A green head on a magenta body is the arcade's
// own palette, verified against a screenshot of the machine; an earlier
// read of this art as "wrong colours" was mine, made from memory, and
// was wrong (see LESSONS L063). Scaled 2.5x and otherwise untouched.
// =====================================================================

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

// =====================================================================
// W3: THE BAITER, THE BOMBER AND THE MINE
//
// ★ CLAUDE'S DRAFTS, APPROVED BY BRIAN ("approved", 2026-10-05) from the
// art_review render. Shapes pasted verbatim from assets-inbox/*.rs, which
// still import into tools/vector-playground.html; redraw there and paste
// the consts back over these.
//
// ⚠️ THE DRAW FUNCTIONS ARE NOT THE PLAYGROUND'S. Each takes the one thing
// that animates in play — the Baiter's shimmering rim lights, the
// Bomber's palette cycle, the mine's pulse — so the playground's fixed
// colours are the frame-zero colours, nothing more. Every colour is a
// fixed threat colour, never the theme's (L065).
// =====================================================================

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

/// cross — authored in tools/vector-playground.html.
pub const MINE_CROSS: Shape = Shape::new(&[
    (-0.80, -2.60),
    (0.80, -2.60),
    (0.80, -0.80),
    (2.60, -0.80),
    (2.60, 0.80),
    (0.80, 0.80),
    (0.80, 2.60),
    (-0.80, 2.60),
    (-0.80, 0.80),
    (-2.60, 0.80),
    (-2.60, -0.80),
    (-0.80, -0.80),
]);

/// core — authored in tools/vector-playground.html.
pub const MINE_CORE: Shape = Shape::new(&[
    (-1.15, 0.00),
    (0.00, -1.15),
    (1.15, 0.00),
    (0.00, 1.15),
]);

/// The Baiter's rim lights cycle through these, each light a step behind
/// the last — the original's 3-frame iridescent shimmer.
const BAITER_SHIMMER: [Color; 3] =
    [Color::rgb(255, 60, 210), Color::rgb(120, 255, 240), Color::rgb(255, 230, 90)];

/// Draw a Baiter. `shimmer` advances the rim lights one colour per step.
pub fn draw_baiter(canvas: &mut Canvas<'_>, t: &Transform, shimmer: u32) {
    BAITER_UNDERGLOW.fill(canvas, t, Color::rgb(255, 170, 40));
    BAITER_HULL.fill(canvas, t, Color::rgb(0, 178, 196));
    BAITER_CANOPY.fill(canvas, t, Color::rgb(170, 250, 255));
    BAITER_RIM.fill(canvas, t, Color::rgb(0, 92, 120));
    for (i, light) in [BAITER_LIGHT_LEFT, BAITER_LIGHT_MID, BAITER_LIGHT_RIGHT].iter().enumerate() {
        let c = BAITER_SHIMMER[(shimmer as usize + i) % BAITER_SHIMMER.len()];
        light.fill(canvas, t, c);
    }
}

/// The Bomber's body cycles through these (the original's TIECOL), so a
/// squad shimmers in and out of step with itself.
const BOMBER_PALETTE: [Color; 4] = [
    Color::rgb(232, 40, 165),
    Color::rgb(255, 110, 60),
    Color::rgb(180, 70, 255),
    Color::rgb(255, 60, 100),
];

/// Draw a Bomber. `cycle` steps the body through its palette.
pub fn draw_bomber(canvas: &mut Canvas<'_>, t: &Transform, cycle: u32) {
    BOMBER_FIN_LEFT.fill(canvas, t, Color::rgb(150, 20, 110));
    BOMBER_FIN_RIGHT.fill(canvas, t, Color::rgb(150, 20, 110));
    BOMBER_BODY.fill(canvas, t, BOMBER_PALETTE[cycle as usize % BOMBER_PALETTE.len()]);
    BOMBER_PANEL.fill(canvas, t, Color::rgb(84, 0, 62));
    BOMBER_CORE.fill(canvas, t, Color::rgb(255, 222, 120));
}

/// Draw a mine. `pulse` (0..1) brightens the core: a live thing, because
/// it cannot be shot and must never be mistaken for debris.
pub fn draw_mine(canvas: &mut Canvas<'_>, t: &Transform, pulse: f32) {
    MINE_CROSS.fill(canvas, t, Color::rgb(255, 96, 40));
    let dim = Color::rgb(255, 150, 60);
    MINE_CORE.fill(canvas, t, dim.lerp(Color::rgb(255, 250, 220), pulse.clamp(0.0, 1.0)));
}

/// Half-width and half-height of a piece, in art units: the bounds of
/// every layer together. Hitboxes are derived from this, so the art and
/// the collision cannot drift apart (docs/warden-plan.md recommendation 2).
pub const fn half_extents(layers: &[Shape<'_>]) -> (f32, f32) {
    let (mut w, mut h) = (0.0f32, 0.0f32);
    let mut i = 0;
    while i < layers.len() {
        let pts = layers[i].points;
        let mut j = 0;
        while j < pts.len() {
            let (x, y) = pts[j];
            let (ax, ay) = (if x < 0.0 { -x } else { x }, if y < 0.0 { -y } else { y });
            if ax > w {
                w = ax;
            }
            if ay > h {
                h = ay;
            }
            j += 1;
        }
        i += 1;
    }
    (w, h)
}

pub const BAITER_LAYERS: [Shape<'static>; 7] = [
    BAITER_UNDERGLOW,
    BAITER_HULL,
    BAITER_CANOPY,
    BAITER_RIM,
    BAITER_LIGHT_LEFT,
    BAITER_LIGHT_MID,
    BAITER_LIGHT_RIGHT,
];
pub const BOMBER_LAYERS: [Shape<'static>; 5] =
    [BOMBER_FIN_LEFT, BOMBER_FIN_RIGHT, BOMBER_BODY, BOMBER_PANEL, BOMBER_CORE];
pub const MINE_LAYERS: [Shape<'static>; 2] = [MINE_CROSS, MINE_CORE];
