//! Drawing. The only file that turns state into pixels.
//!
//! Gameplay happens in a fixed 960x720 play field; the window is
//! whatever size Hyprland decides. This module bridges the two with a
//! **letterbox**: one uniform scale factor for both axes, centred, with
//! bars on whichever pair of edges has slack. A non-uniform stretch
//! would be easier, but a stretched play field means the ball moves
//! faster horizontally than vertically at the same speed, which players
//! feel immediately even if they cannot name it.

use std::sync::OnceLock;

use omarcade_core::ease;
use omarcade_core::text::{text, text_width, GLYPH_H};
use omarcade_core::{Canvas, Color, Sprite, Theme};

use crate::art;
use crate::items::{Item, ItemKind};
use crate::state::{Ball, Brick, GameState, Phase, Tier, FIELD_H, FIELD_W, LEVELS};

/// Maps play-field coordinates onto the window.
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    scale: f32,
    off_x: f32,
    off_y: f32,
}

impl Viewport {
    /// Shift the whole viewport by `d` field units.
    ///
    /// ⚠️ **This is the ONLY place screen shake exists.** It moves the
    /// drawing; the field's own coordinates never change, so physics
    /// cannot see it and a ball can never collide with something it does
    /// not visually touch. Every other function in this file is unaware
    /// shake is a feature — which is exactly what centralising the
    /// coordinates in `Viewport` bought.
    fn shifted(mut self, d: crate::geom::Vec2) -> Self {
        self.off_x += d.x * self.scale;
        self.off_y += d.y * self.scale;
        self
    }

    /// Fit the play field inside `(w, h)`, preserving aspect ratio.
    pub fn fit(w: u32, h: u32) -> Self {
        // A zero-sized window would give a zero or NaN scale; clamp to
        // something harmless since a frame may still be requested.
        let w = w.max(1) as f32;
        let h = h.max(1) as f32;
        let scale = (w / FIELD_W).min(h / FIELD_H);
        Viewport {
            scale,
            off_x: (w - FIELD_W * scale) / 2.0,
            off_y: (h - FIELD_H * scale) / 2.0,
        }
    }

    fn x(&self, x: f32) -> i32 {
        (self.off_x + x * self.scale).round() as i32
    }

    fn y(&self, y: f32) -> i32 {
        (self.off_y + y * self.scale).round() as i32
    }

    /// Float variants, for objects that move. The integer ones round to
    /// the nearest pixel, which is right for static geometry and wrong
    /// for anything animating.
    fn fx(&self, x: f32) -> f32 {
        self.off_x + x * self.scale
    }

    fn fy(&self, y: f32) -> f32 {
        self.off_y + y * self.scale
    }

    fn flen(&self, v: f32) -> f32 {
        (v * self.scale).max(1.0)
    }

    fn len(&self, v: f32) -> u32 {
        // At least one pixel: a thin object that rounds to zero would
        // vanish entirely at small window sizes.
        ((v * self.scale).round() as i32).max(1) as u32
    }

    fn rect(&self, r: crate::geom::Rect, canvas: &mut Canvas<'_>, color: Color) {
        canvas.fill_rect(self.x(r.x), self.y(r.y), self.len(r.w), self.len(r.h), color);
    }
}

/// Brick colours by row, taken live from the theme.
pub fn palette(theme: &Theme) -> [Color; 6] {
    [theme.red, theme.orange, theme.yellow, theme.green, theme.cyan, theme.blue]
}

/// Draw the whole frame.
pub fn draw(state: &mut GameState, canvas: &mut Canvas<'_>, theme: &Theme) {
    // Refresh the palette cache the effects read, so a LIVE theme change
    // reaches the chips. `Brick` stores an index rather than a colour for
    // the same reason; this is what lets `physics` throw chips in a
    // brick's own colour without ever learning what a `Theme` is.
    //
    // ⚠️ This refresh is not the only place the palette is set — see
    // `GameState::set_palette`, called at construction. Relying on render
    // alone left every effect before the first frame drawing in the grey
    // placeholder, which is exactly how the level-clear cascade came out
    // monochrome in `dump_frame`: that harness runs all its physics first
    // and renders once at the end.
    state.palette = palette(theme);

    // ⚠️ Shake is applied to the VIEWPORT and nowhere else. The letterbox
    // is drawn BEFORE the shift and stays put — an effect that moves
    // everything is invisible, because nothing is left standing still to
    // see it against. The bars are that reference.
    let offset = state.shake.offset(&mut state.effect_rng);
    let vp = Viewport::fit(canvas.width(), canvas.height()).shifted(offset);

    // The letterbox bars are darker than the field, so the play area
    // reads as a distinct surface rather than the window just being an
    // odd shape.
    canvas.clear(theme.darker_background);
    canvas.fill_rect(
        vp.x(0.0),
        vp.y(0.0),
        vp.len(FIELD_W),
        vp.len(FIELD_H),
        theme.background,
    );

    let pal = palette(theme);
    // ⚠️ During the cascade's BUILD stage the next field arrives row by
    // row rather than appearing all at once — "builds are cheap and they
    // make ten levels feel like a journey". Rows above the build front are
    // in; rows below have not arrived yet.
    // ⚠️ The front sweeps the BRICK FIELD's height, not the window's. The
    // bricks occupy only the top ~280 of 720 units, so sweeping FIELD_H
    // put every row in during the first third of the build and the rest of
    // the stage showed nothing arriving.
    let brick_span = crate::state::BRICK_TOP
        + crate::state::BRICK_ROWS as f32
            * (crate::state::BRICK_H + crate::state::BRICK_GAP);
    let build_front = match state.clear_progress() {
        Some((true, t)) => Some(t * brick_span),
        // During the WAVE stage the old field is already gone (the player
        // destroyed the last brick) and the wave is drawn as chips, so
        // there is nothing to hold back.
        Some((false, _)) => Some(0.0),
        None => None,
    };
    for brick in &state.bricks {
        if !brick.alive() {
            continue;
        }
        if let Some(front) = build_front {
            if brick.rect.center().y > front {
                continue;
            }
        }
        draw_brick(brick, canvas, theme, &vp, pal[brick.color_index % pal.len()]);
    }

    vp.rect(state.paddle.rect(), canvas, theme.foreground);

    for item in &state.items {
        draw_item(item, canvas, theme, &vp);
    }

    draw_chips(state, canvas, &vp);

    // Balls are hidden once the game is over — nothing is in play.
    if state.phase != Phase::Lost && state.phase != Phase::Won {
        // Every trail first, then every ball, so a ball is never drawn
        // underneath another ball's trail.
        for ball in &state.balls {
            draw_trail(ball, canvas, theme, &vp, state.ball_speed());
        }

        for ball in &state.balls {
            // Sub-pixel, unlike the bricks: these are the things on screen
            // that move every frame, and snapping them to whole pixels is
            // exactly what makes 60fps motion look like 30.
            let r = ball.rect();
            canvas.fill_rect_f(vp.fx(r.x), vp.fy(r.y), vp.flen(r.w), vp.flen(r.h), theme.accent);
        }
    }

    draw_hud(state, canvas, theme, &vp);
    draw_phase_message(state, canvas, theme, &vp);
}

/// One falling power-up.
///
/// ⚠️ **An item must never be mistaken for a ball.** A player who dives for
/// a falling bomb thinking it is the ball will call it a bug, and they will
/// be right to. Three cues separate them, and no single one is trusted:
///
/// * **Shape.** Wide and flat — a capsule, twice as wide as tall — against
///   the ball's small square.
/// * **Motion.** Items fall straight down at roughly half ball speed, and
///   they never bounce.
/// * **No trail.** The trail is the ball's signature; nothing else has one.
///
/// A fourth cue separates the two KINDS from each other: a grow is drawn
/// with a bar across it reading as "wider", a bomb with a gap reading as
/// "cut". Colour agrees with that but never carries it alone — a player
/// should not have to learn which colour is bad.
fn draw_item(item: &Item, canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    let r = item.rect();
    let bad = item.kind.is_bad();

    // ⚠️ **Colour is the weakest cue here and is never trusted alone.** Two
    // attempts at picking theme slots both failed by LOOKING at them (the
    // `items` scene in dump_frame): `green` resolved to the same yellow as
    // brick row 3, and `magenta` came out a pink barely distinguishable from
    // `red`. A game cannot choose what a theme puts in its slots, so any
    // rule of the form "good is X, bad is Y" is one theme away from being
    // wrong. What actually separates them is the GLYPH — an unbroken bar
    // versus a bar with a bite out of it — and that is theme-proof.
    //
    // The border is what makes the glyph legible against whatever the body
    // colour turns out to be, and it is why an item never disappears into a
    // brick row even when their colours collide.
    let body = if bad { theme.red } else { theme.green };

    // A dark surround first, one unit proud on every side, so the item has
    // an edge no matter what is behind it.
    canvas.fill_rect_f(
        vp.fx(r.x - 2.0),
        vp.fy(r.y - 2.0),
        vp.flen(r.w + 4.0),
        vp.flen(r.h + 4.0),
        theme.background,
    );
    canvas.fill_rect_f(vp.fx(r.x), vp.fy(r.y), vp.flen(r.w), vp.flen(r.h), body);

    // The glyph inside: a bar that spans the item for a grow, a bar with a
    // bite out of the middle for a bomb.
    let mark = theme.background.with_alpha(200);
    let bar_h = (r.h * 0.22).max(1.0);
    let bar_y = r.y + r.h * 0.5 - bar_h * 0.5;
    let inset = r.w * 0.18;

    match item.kind {
        // Two stubs with a gap — the paddle, cut.
        ItemKind::Bomb => {
            let seg = (r.w - inset * 2.0) * 0.32;
            canvas.fill_rect_f(vp.fx(r.x + inset), vp.fy(bar_y), vp.flen(seg), vp.flen(bar_h), mark);
            canvas.fill_rect_f(
                vp.fx(r.x + r.w - inset - seg),
                vp.fy(bar_y),
                vp.flen(seg),
                vp.flen(bar_h),
                mark,
            );
        }
        // One unbroken bar — the paddle, whole and wide.
        ItemKind::Grow(_) => {
            canvas.fill_rect_f(
                vp.fx(r.x + inset),
                vp.fy(bar_y),
                vp.flen(r.w - inset * 2.0),
                vp.flen(bar_h),
                mark,
            );
        }
        // ⚠️ A HORSESHOE, deliberately nothing like a bar. Grow and bomb
        // are both horizontal strokes, so a third horizontal glyph would
        // be a fourth thing to squint at. The legs point DOWN, toward the
        // paddle that does the catching.
        ItemKind::Magnet => {
            let leg_w = (r.w * 0.13).max(1.0);
            let top_y = r.y + r.h * 0.24;
            // Narrower than the other glyphs: a horseshoe is a TALL shape,
            // and letting it span the full width made it read as a table.
            let span = (r.w - inset * 2.0) * 0.72;
            let arch_x = r.x + (r.w - span) / 2.0;
            // The arch across the top.
            canvas.fill_rect_f(
                vp.fx(arch_x),
                vp.fy(top_y),
                vp.flen(span),
                vp.flen(bar_h),
                mark,
            );
            // Two legs hanging from its ends.
            let leg_h = r.h * 0.44;
            let leg_y = top_y + bar_h;
            canvas.fill_rect_f(
                vp.fx(arch_x),
                vp.fy(leg_y),
                vp.flen(leg_w),
                vp.flen(leg_h),
                mark,
            );
            canvas.fill_rect_f(
                vp.fx(arch_x + span - leg_w),
                vp.fy(leg_y),
                vp.flen(leg_w),
                vp.flen(leg_h),
                mark,
            );
        }
        // The Omarchy mark itself — the rarest item in the bag wearing the
        // real logo, not a stand-in. This replaced a deliberate placeholder
        // disc; see `art::OMARCHY_MARK` for why the art needed no authoring.
        //
        // ⚠️ The mark is SQUARE (15x15) and the item box is WIDE (34x16), so
        // it is scaled to the box HEIGHT and centred in the width, never
        // stretched to fill. Scaling x and y independently would break the
        // logo's 1:1 cells into uneven rectangles — a squashed trademark is
        // the one thing this must not look like. It reads as the only square
        // among three wide bars, which is its own kind of "this one is rare".
        ItemKind::Omarchy => {
            // ⚠️ Built ONCE. `Sprite::new` allocates and parses the grid;
            // doing that per frame would put an allocation in the render
            // loop for every falling item.
            static MARK_SPRITE: OnceLock<Sprite> = OnceLock::new();
            let sprite = MARK_SPRITE.get_or_init(art::omarchy_sprite);

            // Fill the item's height exactly, in field units.
            let scale = r.h / art::MARK_H;
            // ⚠️ `draw_tinted` anchors TOP-LEFT, unlike the bars above which
            // are positioned from their own edges. Centring in the wide slot
            // is the caller's job — get this wrong and the mark sits in the
            // corner rather than the middle.
            let x = r.x + (r.w - art::MARK_W * scale) / 2.0;

            // Tinted fully to `mark`, so the glyph carries the meaning and
            // the theme supplies the colour — the same rule the bars follow.
            // The source SVG's green gradient is an export artifact, not part
            // of the mark's identity, so none of it survives here.
            sprite.draw_tinted(
                canvas,
                vp.fx(x),
                vp.fy(r.y),
                vp.flen(scale),
                Some((mark, 1.0)),
            );
        }
    }
}

/// Brick chips, as added light.
///
/// ⚠️ **Drawn here rather than through `ParticlePool::draw`.** That method
/// writes in RAW FIELD COORDINATES, which is right for a game whose canvas
/// is the field — and wrong for this one, which letterboxes. Calling it
/// would put every chip in the correct place at exactly one window size
/// and visibly adrift at all the others.
///
/// Additive, so chips brighten what is under them and read as light rather
/// than as translucent stickers. That is also what the playground tuned
/// against; on plain alpha these numbers would look wrong.
fn draw_chips(state: &GameState, canvas: &mut Canvas<'_>, vp: &Viewport) {
    for p in state.chips.particles() {
        let half = p.size * 0.5;
        canvas.fill_rect_add_f(
            vp.fx(p.pos.x - half),
            vp.fy(p.pos.y - half),
            vp.flen(p.size),
            vp.flen(p.size),
            p.fade(),
        );
    }
}

/// One brick, with its damage written into its shape.
///
/// ⚠️ **Remaining hits are read from the SHAPE, never from a colour.** A
/// player should be able to glance at a brick and know it is nearly gone
/// without having learned a palette. Two devices carry it:
///
/// * **The brick shrinks as it is damaged**, anchored so it visibly loses
///   material from a corner rather than shrinking evenly toward its middle.
///   That is what "breaking pieces off" looks like at this size.
/// * **The tier keeps its border for as long as it lives.** A reinforced
///   brick keeps its seam and an armoured one its heavy frame right down to
///   the last hit, so a chipped armoured brick still reads as armoured
///   rather than as a small plain one.
///
/// Nothing here picks a colour of its own: the row colour arrives as an
/// argument and the border comes from the theme.
fn draw_brick(
    brick: &Brick,
    canvas: &mut Canvas<'_>,
    theme: &Theme,
    vp: &Viewport,
    color: Color,
) {
    let r = brick.rect;
    let damage = brick.damage();

    // Undamaged bricks take the cheap path — the overwhelmingly common
    // case, and identical to what shipped before tiers existed.
    if damage <= 0.0 && brick.tier == Tier::Plain {
        vp.rect(r, canvas, color);
        return;
    }

    // Lose up to a third of each side across the brick's whole life. More
    // than that and a damaged brick stops reading as a brick.
    let shrink = damage * 0.34;
    let w = r.w * (1.0 - shrink);
    let h = r.h * (1.0 - shrink);
    // Anchored top-left rather than centred: material comes off the bottom
    // right corner, which reads as a piece breaking away instead of the
    // whole brick receding.
    canvas.fill_rect_f(vp.fx(r.x), vp.fy(r.y), vp.flen(w), vp.flen(h), color);

    match brick.tier {
        Tier::Plain => {}
        // A seam across the middle: one line, and it survives the first hit.
        // ⚠️ Thin on purpose. Judged against dump_frame's `tiers` scene: at
        // 14% of a 28-unit brick the seam reads as two separate stripes
        // rather than as one brick that is scored across the middle.
        Tier::Reinforced => {
            let seam = (h * 0.07).max(1.0);
            canvas.fill_rect_f(
                vp.fx(r.x),
                vp.fy(r.y + h * 0.5 - seam * 0.5),
                vp.flen(w),
                vp.flen(seam),
                theme.background.with_alpha(150),
            );
        }
        // A heavy frame, drawn as four edges so the row colour still shows
        // through the middle and the brick keeps its identity.
        Tier::Armoured => {
            // ⚠️ Also judged by looking: at 18% the frame eats the middle
            // and an armoured brick stops showing its row colour, so it no
            // longer reads as part of the row it belongs to.
            let t = (h * 0.11).max(1.0);
            let edge = theme.foreground.with_alpha(120);
            canvas.fill_rect_f(vp.fx(r.x), vp.fy(r.y), vp.flen(w), vp.flen(t), edge);
            canvas.fill_rect_f(vp.fx(r.x), vp.fy(r.y + h - t), vp.flen(w), vp.flen(t), edge);
            canvas.fill_rect_f(vp.fx(r.x), vp.fy(r.y), vp.flen(t), vp.flen(h), edge);
            canvas.fill_rect_f(vp.fx(r.x + w - t), vp.fy(r.y), vp.flen(t), vp.flen(h), edge);
        }
    }
}

/// One ball's recent path, fading out behind it.
///
/// Cheap on purpose: ten alpha quads measured at well under a tenth of a
/// millisecond at 960x720. The expensive full-screen veil is not used
/// here — a trail is a local effect and should cost like one.
///
/// Takes a `Ball`, not the state: each ball owns its own trail, and drawing
/// from a shared one would produce a line that whips between balls.
fn draw_trail(
    ball: &Ball,
    canvas: &mut Canvas<'_>,
    theme: &Theme,
    vp: &Viewport,
    speed: f32,
) {
    let n = ball.trail.len();
    if n < 2 {
        return;
    }

    // Skip index 0: that is where the ball itself is drawn.
    for (i, pos) in ball.trail.iter().enumerate().skip(1) {
        let t = i as f32 / n as f32;

        // Fade and shrink together. Either alone reads as a bug — a
        // constant-size fading trail looks like ghosting, and a shrinking
        // opaque one looks like a string of beads.
        // ⚠️ Brightness tracks SPEED, so level 10 reads as fast rather
        // than merely being fast. Length does too, but it is bounded by
        // how far the ball travels between frames; brightness is free.
        let alpha = (ease::out_cubic(1.0 - t) * crate::state::trail_alpha_for(speed)) as u8;
        if alpha == 0 {
            continue;
        }
        let size = ball.radius * 2.0 * ease::lerp(1.0, 0.35, t);
        let off = (ball.radius * 2.0 - size) / 2.0;

        canvas.fill_rect_f(
            vp.fx(pos.x - ball.radius + off),
            vp.fy(pos.y - ball.radius + off),
            vp.flen(size),
            vp.flen(size),
            theme.accent.with_alpha(alpha),
        );
    }
}

fn draw_hud(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    let scale = (vp.scale * 3.0).max(1.0) as u32;
    text(canvas, &format!("SCORE {}", state.score), vp.x(24.0), vp.y(30.0), scale, theme.foreground);

    // ⚠️ The level, centred between score and lives.
    //
    // Its absence was reported from real play: with ten levels built and
    // working, a cleared field silently became the next one and the game
    // looked like a single level that kept resetting. Every test asserted
    // `level` had incremented — and every one of them passed. Nothing on
    // screen said so, which is a different question from whether the code
    // was right.
    //
    // The slash is in the 5x7 font (A-Z 0-9 space - + . : /), so this
    // renders without the glyph work the plan expected to need.
    let level = format!("LEVEL {}/{}", state.level, LEVELS);
    let level_w = text_width(&level, scale) as f32;
    text(
        canvas,
        &level,
        vp.x(FIELD_W / 2.0) - (level_w / 2.0) as i32,
        vp.y(30.0),
        scale,
        theme.light_foreground,
    );

    let lives = format!("LIVES {}", state.lives);
    let width = text_width(&lives, scale) as f32;
    text(
        canvas,
        &lives,
        vp.x(FIELD_W - 24.0) - width as i32,
        vp.y(30.0),
        scale,
        theme.light_foreground,
    );
}

fn draw_phase_message(state: &GameState, canvas: &mut Canvas<'_>, theme: &Theme, vp: &Viewport) {
    // ⚠️ `Ready` means two different things to a player: "you lost a ball"
    // and "you cleared the level". Showing one message for both is what
    // made a ten-level game read as one level resetting.
    let advanced = format!("LEVEL {} - SPACE", state.level);
    let (msg, color): (&str, _) = match state.phase {
        Phase::Ready if state.just_advanced => (&advanced, theme.accent),
        Phase::Ready => ("PRESS SPACE", theme.light_foreground),
        Phase::Playing => return,
        // ⚠️ No text during the cascade. The effect IS the message — a
        // banner over it would compete with the thing the player just
        // earned, and the "LEVEL n - SPACE" line arrives a beat later when
        // the field has finished building.
        Phase::Clearing => return,
        Phase::Won => ("YOU WIN - ENTER", theme.green),
        Phase::Lost => ("GAME OVER - ENTER", theme.red),
    };

    let scale = (vp.scale * 4.0).max(1.0) as u32;
    let w = text_width(msg, scale) as f32;
    let x = vp.x(FIELD_W / 2.0) - (w / 2.0) as i32;
    let y = vp.y(FIELD_H / 2.0);
    text(canvas, msg, x, y, scale, color);

    // The best score, under the verdict. Only once a game has ended and
    // only if there is one — a first-ever run has nothing to beat, and an
    // empty "BEST 0" would just be noise.
    if state.best == 0 {
        return;
    }

    let beaten = state.score >= state.best;
    let line = if beaten {
        format!("NEW BEST {}", state.best)
    } else {
        format!("BEST {}", state.best)
    };

    let small = (vp.scale * 2.0).max(1.0) as u32;
    let lw = text_width(&line, small) as f32;
    text(
        canvas,
        &line,
        vp.x(FIELD_W / 2.0) - (lw / 2.0) as i32,
        y + (scale * GLYPH_H * 2) as i32,
        small,
        if beaten { theme.yellow } else { theme.light_foreground },
    );
}

#[cfg(test)]
mod tests {

    /// ⚠️ Three labels share the HUD row now. At the largest scale the
    /// viewport can produce they must not overlap, or the level readout
    /// eats the score.
    #[test]
    fn the_hud_row_never_collides() {
        use crate::state::LEVELS;
        for scale in 1..=8u32 {
            // The widest each label can get: max score, max lives, last level.
            let score = text_width("SCORE 9999999", scale) as f32;
            let level = text_width(&format!("LEVEL {LEVELS}/{LEVELS}"), scale) as f32;
            let lives = text_width("LIVES 9", scale) as f32;

            // In field units at scale 1 the row spans 24..FIELD_W-24.
            let usable = FIELD_W - 48.0;
            let total = score + level + lives;
            if scale <= 3 {
                assert!(
                    total < usable,
                    "scale {scale}: {total} of labels does not fit in {usable}"
                );
            }
            // The centred label must never start before the score ends.
            let level_start = FIELD_W / 2.0 - level / 2.0;
            if scale <= 3 {
                assert!(
                    level_start > 24.0 + score,
                    "scale {scale}: LEVEL starts at {level_start}, SCORE ends at {}",
                    24.0 + score
                );
            }
        }
    }

    #[test]
    fn every_hud_and_message_character_has_a_glyph() {
        use crate::state::LEVELS;
        let samples = [
            format!("LEVEL {LEVELS}/{LEVELS}"),
            "LEVEL 1 - SPACE".to_string(),
            "SCORE 1234567".to_string(),
            "LIVES 3".to_string(),
            "PRESS SPACE".to_string(),
            "YOU WIN - ENTER".to_string(),
            "GAME OVER - ENTER".to_string(),
        ];
        for sample in samples {
            assert_eq!(
                omarcade_core::text::unrenderable(&sample),
                None,
                "{sample:?} has a character the 5x7 font cannot draw"
            );
        }
    }

    use super::*;
    use omarcade_core::text::glyph;

    #[test]
    fn viewport_letterboxes_a_wide_window() {
        // Wider than 4:3, so bars go on the left and right.
        let vp = Viewport::fit(1920, 720);
        assert!((vp.scale - 1.0).abs() < 1e-5, "scale limited by height");
        assert!(vp.off_x > 0.0, "horizontal bars expected");
        assert!(vp.off_y.abs() < 1e-5, "no vertical bars");
    }

    #[test]
    fn viewport_letterboxes_a_tall_window() {
        let vp = Viewport::fit(960, 1440);
        assert!((vp.scale - 1.0).abs() < 1e-5, "scale limited by width");
        assert!(vp.off_y > 0.0, "vertical bars expected");
        assert!(vp.off_x.abs() < 1e-5);
    }

    /// The field must land centred and fully inside the window at any
    /// size — this is where letterbox off-by-ones show up.
    #[test]
    fn field_is_centred_and_inside_the_window_at_many_sizes() {
        for &(w, h) in &[
            (1261, 701), // what Hyprland actually gave us last session
            (960, 720),
            (1920, 1080),
            (640, 480),
            (300, 1000),
            (1000, 300),
        ] {
            let vp = Viewport::fit(w, h);
            let left = vp.x(0.0);
            let right = vp.x(FIELD_W);
            let top = vp.y(0.0);
            let bottom = vp.y(FIELD_H);

            assert!(left >= 0, "{w}x{h}: left {left} off-window");
            assert!(top >= 0, "{w}x{h}: top {top} off-window");
            assert!(right <= w as i32, "{w}x{h}: right {right} > {w}");
            assert!(bottom <= h as i32, "{w}x{h}: bottom {bottom} > {h}");

            // Centred: margins equal within a pixel of rounding.
            let mx = (left - (w as i32 - right)).abs();
            let my = (top - (h as i32 - bottom)).abs();
            assert!(mx <= 1, "{w}x{h}: horizontal margins differ by {mx}");
            assert!(my <= 1, "{w}x{h}: vertical margins differ by {my}");
        }
    }

    #[test]
    fn aspect_ratio_is_preserved() {
        let vp = Viewport::fit(1261, 701);
        let w = vp.x(FIELD_W) - vp.x(0.0);
        let h = vp.y(FIELD_H) - vp.y(0.0);
        let want = FIELD_W / FIELD_H;
        let got = w as f32 / h as f32;
        assert!((got - want).abs() < 0.01, "aspect {got} != {want}");
    }

    #[test]
    fn zero_sized_window_does_not_panic_or_nan() {
        let vp = Viewport::fit(0, 0);
        assert!(vp.scale.is_finite());
        assert!(vp.x(10.0).is_positive() || vp.x(10.0) == 0);
    }

    #[test]
    fn thin_objects_never_round_away_to_nothing() {
        let vp = Viewport::fit(100, 75); // heavy downscale
        assert!(vp.len(1.0) >= 1, "a 1-unit object must still be visible");
    }

    /// Every character the HUD can render must have a glyph, or words
    /// silently lose letters.
    #[test]
    fn all_hud_characters_have_glyphs() {
        for s in [
            "SCORE 0123456789",
            "LIVES 3",
            "PRESS SPACE",
            "YOU WIN - ENTER",
            "GAME OVER - ENTER",
            "BEST 980",
            "NEW BEST 600",
        ] {
            for c in s.chars() {
                assert!(glyph(c).is_some(), "no glyph for {c:?} in {s:?}");
            }
        }
    }

    /// The list above is written by hand, so it only covers strings someone
    /// remembered to add — "BEST" shipped missing its B because of exactly
    /// that. Requiring the whole printable set makes any future string safe
    /// by construction instead of by vigilance.
    #[test]
    fn the_full_printable_set_has_glyphs() {
        for c in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -".chars() {
            assert!(glyph(c).is_some(), "no glyph for {c:?}");
        }
    }

    /// A character with no glyph must not be silently skipped mid-word.
    /// This pins the behaviour that hid the missing B: `text` advances the
    /// cursor for unknown characters, so a gap appears rather than the
    /// remaining letters sliding left as if nothing were wrong.
    #[test]
    fn an_unknown_character_still_occupies_its_cell() {
        let scale = 1;
        // '@' has no glyph; the string must still measure as 3 characters.
        assert_eq!(text_width("A@B", scale), text_width("ABC", scale));
    }
}
