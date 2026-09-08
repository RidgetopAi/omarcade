//! Simulation: the only file that advances time.
//!
//! Three classic Breakout failures shape everything here, and each has
//! tests aimed squarely at it:
//!
//! 1. **Tunnelling.** At 420 units/s a 100ms hitch moves the ball 42
//!    units — clean through a 28-unit brick, with no frame ever seeing
//!    an overlap. Fixed timesteps bound per-tick movement instead.
//! 2. **Near-horizontal lock.** If a bounce leaves the ball with a tiny
//!    vertical component it crawls sideways forever and the game stops
//!    being a game. Outgoing angles are clamped.
//! 3. **Multi-brick ticks.** A ball can overlap two bricks at once;
//!    reflecting off both flips the velocity twice and sends it back
//!    the way it came. Only the deepest collision is resolved per tick.

use crate::geom::{Axis, Rect, Vec2};
use crate::items::MAGNET_HOLD_SECONDS;
// ⚠️ The steering formula, its floor and its clamp all live in `state`,
// not here. The magnet releases a ball with the same aim an ordinary
// return gives it, and `state::release_held` owns that release — while
// `state` cannot depend on `physics`, because two probes pull `state.rs`
// via `#[path]` without it. One copy, in the module both sides can reach.
use crate::state::{
    clamp_angle, Ball, GameState, Paddle, Phase, BRICK_COLS, BRICK_ROWS,
};

/// Simulation rate. High enough that per-tick movement (~1.75 units at
/// ball speed) is far smaller than the thinnest brick, which is what
/// makes tunnelling impossible rather than merely unlikely.
pub const FIXED_DT: f32 = 1.0 / 240.0;

/// Most fixed steps one frame may run.
///
/// Without this, a stalled frame — window dragged, machine resumed from
/// suspend — asks for hundreds of steps, which makes the next frame
/// slower, which asks for more steps: the spiral of death. Past this
/// limit we drop the surplus time and let the game run briefly slow
/// instead of freezing.
const MAX_STEPS_PER_FRAME: u32 = 8;


/// Tolerance for treating two penetration depths as equal.
const EPS: f32 = 1e-4;

/// The speed at which one fixed tick moves the ball a whole brick height —
/// the point where tunnelling stops being impossible and starts being
/// merely unlikely.
///
/// ⚠️ **DERIVED, never typed.** Written as a number it would go quietly
/// wrong the day `BRICK_H` or `FIXED_DT` changed: the assertion would still
/// pass while the thing it protects had moved. Computed from both, the
/// guard moves with them. This is L026, and Omaprix paid for it twice.
///
/// At the shipped values this is 28 units / (1/240 s) = 6720 units/s,
/// against a level-10 ball speed of 460 — a factor of fourteen of headroom.
pub const TUNNELLING_LIMIT: f32 = crate::state::BRICK_H / FIXED_DT;


/// Converts real elapsed time into a whole number of fixed steps.
#[derive(Debug, Default)]
pub struct Accumulator {
    carry: f32,
}

impl Accumulator {
    pub fn new() -> Self {
        Accumulator { carry: 0.0 }
    }

    /// Feed real elapsed seconds; get the number of fixed steps to run.
    ///
    /// Leftover time is carried, so the simulation neither gains nor
    /// loses time over a run — a frame that is 1.5 steps long runs one
    /// step now and half a step of credit into the next frame.
    pub fn steps_for(&mut self, dt: f32) -> u32 {
        // Guard against a negative or NaN dt from a clock going
        // backwards; treat it as no time passing.
        if !dt.is_finite() || dt < 0.0 {
            return 0;
        }
        self.carry += dt;
        let mut steps = (self.carry / FIXED_DT) as u32;
        if steps > MAX_STEPS_PER_FRAME {
            steps = MAX_STEPS_PER_FRAME;
            self.carry = 0.0; // drop the surplus rather than spiral
        } else {
            self.carry -= steps as f32 * FIXED_DT;
        }
        steps
    }
}

/// Advance the game by one frame's worth of real time.
pub fn step(state: &mut GameState, accumulator: &mut Accumulator, dt: f32) {
    let steps = accumulator.steps_for(dt);
    for _ in 0..steps {
        step_fixed(state);
    }
    if steps > 0 {
        record_trail(state);
    }
}

/// Sample the ball's position for the motion trail.
///
/// Once per frame, not once per fixed tick: the simulation runs at 240Hz
/// and a trail sampled there would be four times denser than intended,
/// and would change length with frame rate. Called only when time
/// actually advanced, so a paused or stalled frame does not stack ten
/// copies of the same point.
fn record_trail(state: &mut GameState) {
    if state.phase != Phase::Playing {
        for ball in &mut state.balls {
            ball.trail.clear();
        }
        return;
    }
    // ⚠️ Each ball samples into its OWN trail. One shared buffer with
    // several balls writing to it draws a line that whips between them.
    // ⚠️ The trail LENGTHENS with speed, so level 10 looks fast. Two
    // things compound here and it is worth being explicit about why the
    // count only moves from 10 to 16: sampling is per FRAME, so a faster
    // ball also puts its samples further apart. More points, further
    // apart — the streak grows by much more than the count suggests.
    let keep = crate::state::trail_len_for(state.ball_speed());
    for ball in &mut state.balls {
        ball.trail.insert(0, ball.pos);
        ball.trail.truncate(keep);
    }
}

/// One fixed tick.
///
/// ⚠️ **Every ball is simulated independently, start to finish, before the
/// next one begins.** Movement, walls, paddle and bricks are resolved for
/// ball 0, then for ball 1, and so on — never gathered across balls and
/// resolved together. The shallowest-collision-wins rule is a statement
/// about ONE ball's tick; applied to a pool of collisions from several
/// balls it silently picks the wrong face and the ball leaves at an angle
/// nothing on screen explains. That failure reads as bad luck, not as a
/// bug, which is exactly why it is spelled out here.
pub fn step_fixed(state: &mut GameState) {
    move_paddle(state);

    // ⚠️ **Effects advance in EVERY phase, outside the match.** They were
    // originally ticked inside the `Playing` arm, which looked right and
    // was not: losing a ball moves the game to `Ready`, so the shake
    // raised by `lose_life` was set and then never decayed — it kept
    // shaking until the player launched again and re-entered `Playing`.
    // Chips had the same bug, freezing mid-air on the Ready screen and
    // forever on `Lost` or `Won`.
    //
    // A decaying effect is presentation, not simulation. It must run
    // whenever time passes, and time passes in every phase. Reported from
    // real play; no test caught it, because every effects test was written
    // against a game already in `Playing`.
    //
    // Still on the FIXED step rather than the frame, so an arc tuned in
    // the playground is the arc the player sees at any frame rate.
    state.chips.update(FIXED_DT);
    state.shake.tick(FIXED_DT);

    match state.phase {
        // Ball rides the paddle until launch.
        Phase::Ready => state.rest_ball_on_paddle(),
        Phase::Playing => {
            let field = state.field();
            let paddle = state.paddle;
            let ball_speed = state.ball_speed();

            let magnet = state.magnet_active();

            for i in 0..state.balls.len() {
                // ⚠️ A held ball is OUT of the simulation entirely: it does
                // not move, does not bounce off walls, and cannot be
                // drained. Its position is owned by `tick_magnet`, which
                // rides it on the paddle. Letting it through this loop
                // would have it colliding with whatever it is resting on
                // every single tick.
                if state.balls[i].is_held() {
                    continue;
                }
                move_ball(&mut state.balls[i]);
                collide_walls(&mut state.balls[i], &field);
                collide_paddle(&mut state.balls[i], &paddle, ball_speed, magnet);
                // Bricks need the whole state: a kill scores, and it must
                // be visible to every later ball in this same tick.
                collide_bricks(state, i);
            }

            // Draining is per ball; losing a life is per empty field.
            // Both happen only once every ball has had its tick.
            state.retire_drained_balls();
            move_items(state);
            state.tick_paddle_effect(FIXED_DT);
            // ⚠️ AFTER `move_paddle`, so a ball held this tick sits where
            // the paddle actually ended up. Ticking it first would leave
            // the ball one frame behind the paddle it is stuck to, which
            // reads as the ball wobbling loose.
            state.tick_magnet(FIXED_DT);
            check_win(state);
        }
        // The level-clear cascade: the field resolves outward, then the
        // next one builds itself in.
        //
        // ⚠️ Nothing else runs. No balls (there are none — `begin_clearing`
        // removed them), no items, no collisions. The only thing advancing
        // is the cascade itself and the chips it throws, and those tick
        // above, outside this match, like every other decaying effect.
        Phase::Clearing => {
            cascade_chips(state);
            state.tick_clear(FIXED_DT);
        }
        Phase::Lost | Phase::Won => {}
    }
}

/// Throw light from the field as the outgoing wave sweeps down it.
///
/// ⚠️ **There are no bricks left to burst.** The cascade fires when
/// `bricks_remaining() == 0` — the player has just destroyed the last one —
/// so a wave built from live bricks sweeps an empty field and shows
/// nothing. That is a real trap: the obvious implementation compiles, runs
/// and is completely invisible.
///
/// So the wave is thrown from the field's own GEOMETRY instead: the grid
/// the bricks occupied, in the level's palette. What resolves outward is
/// the shape of the level that was just beaten, which is what "everything
/// left on screen resolves into a wave" means once the bricks themselves
/// are gone.
fn cascade_chips(state: &mut GameState) {
    let Some(clear) = state.clear else { return };
    if clear.building {
        return;
    }

    // The band the wave front crossed THIS tick. Firing on a band rather
    // than on "everything above the front" is what makes each row throw
    // exactly once, however many ticks the wave takes.
    let span = crate::state::BRICK_TOP
        + BRICK_ROWS as f32 * (crate::state::BRICK_H + crate::state::BRICK_GAP);
    let now = clear.progress() * span;
    let prev = ((clear.elapsed - FIXED_DT).max(0.0)
        / crate::state::CLEAR_WAVE_SECONDS.max(1e-6))
        .clamp(0.0, 1.0)
        * span;

    let span_w = BRICK_COLS as f32 * crate::state::BRICK_W
        + (BRICK_COLS - 1) as f32 * crate::state::BRICK_GAP;
    let left = (crate::state::FIELD_W - span_w) / 2.0;

    for row in 0..BRICK_ROWS {
        let y = crate::state::BRICK_TOP
            + row as f32 * (crate::state::BRICK_H + crate::state::BRICK_GAP);
        let mid = y + crate::state::BRICK_H / 2.0;
        if mid <= prev || mid > now {
            continue;
        }
        let color = state.palette[row % state.palette.len()];
        for col in 0..BRICK_COLS {
            let x = left + col as f32 * (crate::state::BRICK_W + crate::state::BRICK_GAP);
            let cell = Rect::new(x, y, crate::state::BRICK_W, crate::state::BRICK_H);
            // Outward from the field's centre: a field resolving outward is
            // what makes it a wave rather than one large explosion.
            let dir = Vec2::new(
                cell.center().x - crate::state::FIELD_W / 2.0,
                cell.center().y - crate::state::FIELD_H / 2.0,
            );
            crate::effects::cascade_burst(
                &mut state.chips,
                &mut state.effect_rng,
                cell,
                dir,
                color,
            );
        }
    }
}

fn move_paddle(state: &mut GameState) {
    let speed = state.paddle_speed();
    let p = &mut state.paddle;
    p.x += p.dir * speed * FIXED_DT;
    // Clamp inside the field; the paddle never leaves the play area.
    p.x = p.x.clamp(0.0, crate::state::FIELD_W - p.w);
}

fn move_ball(ball: &mut Ball) {
    ball.pos += ball.vel * FIXED_DT;
}

/// Bounce off the side and top walls; falling past the bottom drains the
/// ball.
///
/// ⚠️ **Draining only marks the ball.** The single-ball version called
/// `lose_life()` right here, because with one ball "this ball is gone" and
/// "you lost a life" were the same event. With several in play that costs
/// a life for the first ball to drain while the rest are still bouncing.
/// The flag is collected by `GameState::retire_drained_balls` after every
/// ball has moved.
fn collide_walls(b: &mut Ball, field: &Rect) {
    let r = b.radius;

    if b.pos.x - r < field.left() {
        b.pos.x = field.left() + r;
        b.vel.x = b.vel.x.abs();
    } else if b.pos.x + r > field.right() {
        b.pos.x = field.right() - r;
        b.vel.x = -b.vel.x.abs();
    }

    if b.pos.y - r < field.top() {
        b.pos.y = field.top() + r;
        b.vel.y = b.vel.y.abs();
    }

    // Bottom is not a wall — it is how you lose the ball.
    if b.pos.y - r > field.bottom() {
        b.drained = true;
    }
}

/// Bounce the ball off the paddle — or stick it there, if the magnet is
/// armed.
fn collide_paddle(ball: &mut Ball, paddle: &Paddle, speed: f32, magnet: bool) {
    // Only when moving downward. A ball on its way up that clips the
    // paddle from below should pass, not get batted back down.
    if ball.vel.y <= 0.0 {
        return;
    }

    let rect = paddle.rect();
    if !ball.rect().overlaps(&rect) {
        return;
    }

    // Sit the ball on top of the paddle so it cannot re-collide.
    ball.pos.y = rect.top() - ball.radius - 0.01;

    if magnet {
        // Caught. It rides the paddle until Space is released or the hold
        // runs out, whichever comes first.
        ball.held_for = Some(MAGNET_HOLD_SECONDS);
        ball.vel = Vec2::ZERO;
        return;
    }

    bounce_off_paddle(ball, paddle, speed);
}

/// Where the ball strikes the paddle sets the outgoing angle.
///
/// This is the mechanic that makes Breakout a game of skill rather than
/// a screensaver: hitting with the paddle's edge steers the ball.
fn bounce_off_paddle(ball: &mut Ball, paddle: &Paddle, speed: f32) {
    crate::state::aim_off_paddle(ball, paddle, speed);
}

/// Resolve the single shallowest brick collision this tick, for ONE ball.
///
/// Deliberately not "every overlapping brick": a ball touching two
/// bricks would reflect twice and reverse into the direction it came
/// from. One collision per tick, and at 240Hz the next tick handles any
/// remaining overlap.
///
/// ⚠️ **The `best` candidate is per ball and must stay that way.** Pooling
/// candidates across balls and resolving the winner would mean one ball's
/// geometry decides another ball's bounce. Each ball gets its own call,
/// its own search and its own resolution.
///
/// Takes the whole state and an index rather than `&mut Ball`, because a
/// kill has to be visible to every later ball in this same tick: ball B
/// simply sees `alive: false`. That is correct and free — and it is what
/// makes the order of the vec observable, so `state.balls` must stay
/// stable while this loop runs.
fn collide_bricks(state: &mut GameState, index: usize) {
    let ball_rect = state.balls[index].rect();
    let ball_vel = state.balls[index].vel;

    let mut best: Option<(usize, f32, Axis)> = None;
    for (i, brick) in state.bricks.iter().enumerate() {
        if !brick.alive() {
            continue;
        }
        let Some(pen) = ball_rect.penetration(&brick.rect) else {
            continue;
        };
        let Some(axis) = ball_rect.collision_axis(&brick.rect) else {
            continue;
        };
        // Depth along the axis we would resolve on.
        let depth = match axis {
            Axis::X => pen.x,
            Axis::Y => pen.y,
        };
        // SHALLOWEST wins: that is the face the ball reached first.
        // Picking the deepest instead selects a brick the ball is
        // already buried in — typically the row behind the one it
        // actually struck.
        let better = match best {
            None => true,
            Some((_, d, _)) if depth < d - EPS => true,
            // A genuine tie means the ball straddles two bricks in the
            // gap between them. Direction of travel decides which face
            // it is really about to hit; without this the first brick
            // in index order wins and the ball reflects the wrong way.
            Some((bi, d, _)) if (depth - d).abs() <= EPS => {
                let cur = state.bricks[bi].rect.center();
                let cand = brick.rect.center();
                match axis {
                    Axis::X => {
                        if ball_vel.x >= 0.0 { cand.x > cur.x } else { cand.x < cur.x }
                    }
                    Axis::Y => {
                        if ball_vel.y >= 0.0 { cand.y > cur.y } else { cand.y < cur.y }
                    }
                }
            }
            _ => false,
        };
        if better {
            best = Some((i, depth, axis));
        }
    }

    let Some((hit, depth, axis)) = best else {
        return;
    };

    // `index` is the ball; `hit` is the brick it struck.
    let brick_rect = state.bricks[hit].rect;
    // Scored once, by the ball that got there first. A second ball
    // overlapping the same brick this tick finds it already dead and
    // scores nothing — which is the rule, not an accident.
    //
    // A tiered brick takes several hits; every hit scores, and destroying
    // it scores again. Chipping an armoured brick is progress and should
    // feel like it, but the brick that actually breaks is worth more.
    let destroyed = state.bricks[hit].hit();
    state.score += if destroyed { 10 } else { 5 };

    // Chips, in the brick's OWN colour and thrown along the ball's travel.
    // ⚠️ Damage and shatter are ONE system, partial: a brick that survives
    // throws a smaller burst rather than getting an effect of its own.
    // That is what makes the reinforced feedback cheap, and it is what
    // answers S4's note that the damage shrink reads too subtly.
    let tier = state.bricks[hit].tier;
    let idx = state.bricks[hit].color_index % state.palette.len();
    let chip_color = state.palette[idx];
    let share = if destroyed { 1.0 } else { crate::effects::PARTIAL_SHARE };
    crate::effects::shatter(
        &mut state.chips,
        &mut state.effect_rng,
        brick_rect,
        ball_vel,
        chip_color,
        share,
    );

    // ⚠️ Shake only on an ARMOURED break. Shaking on every brick would
    // wobble the whole game, and the plan is explicit that shake is the
    // effect most likely to be too much.
    if destroyed && tier == crate::state::Tier::Armoured {
        state.shake.add(crate::effects::SHAKE_UNITS);
    }

    // ⚠️ Only a FINAL hit rolls for a drop. Rolling on every hit would make
    // an armoured brick roll four times, so the late levels — the ones with
    // armour — would rain power-ups exactly where the player has earned the
    // least help.
    if destroyed {
        let level = state.level;
        let levels = crate::state::LEVELS;
        if let Some(item) = state.dropper.roll(brick_rect.center(), level, levels) {
            state.items.push(item);
        }
    }

    let ball = &mut state.balls[index];

    // Push out along the collision axis, then reflect that component.
    match axis {
        Axis::X => {
            if ball.pos.x < brick_rect.center().x {
                ball.pos.x -= depth;
                ball.vel.x = -ball.vel.x.abs();
            } else {
                ball.pos.x += depth;
                ball.vel.x = ball.vel.x.abs();
            }
        }
        Axis::Y => {
            if ball.pos.y < brick_rect.center().y {
                ball.pos.y -= depth;
                ball.vel.y = -ball.vel.y.abs();
            } else {
                ball.pos.y += depth;
                ball.vel.y = ball.vel.y.abs();
            }
        }
    }

    clamp_angle(&mut ball.vel);
}

/// Keep the ball from skimming too close to horizontal.
///
/// Without this a ball can end up travelling almost sideways, drifting
/// between the walls for a very long time and making the game look
/// broken even though nothing is technically wrong.

/// Fall, catch, and forget.
///
/// ⚠️ An item that reaches the bottom is **missed harmlessly** — it is not
/// a ball and losing it costs nothing. The plan is explicit that an uncaught
/// item must never be mistaken for a ball, and punishing a miss would teach
/// exactly the wrong reflex: diving for a falling bomb.
fn move_items(state: &mut GameState) {
    if state.items.is_empty() {
        return;
    }

    let paddle = state.paddle.rect();
    let floor = state.field().bottom();
    let mut caught: Vec<crate::items::ItemKind> = Vec::new();

    // Walk backwards so removal keeps the earlier indices valid — the same
    // shape as retiring drained balls.
    for i in (0..state.items.len()).rev() {
        state.items[i].pos.y += crate::items::FALL_SPEED * FIXED_DT;

        if state.items[i].rect().overlaps(&paddle) {
            caught.push(state.items[i].kind);
            state.items.remove(i);
        } else if state.items[i].pos.y - crate::items::ITEM_H > floor {
            state.items.remove(i);
        }
    }

    // Applied after the sweep so a catch cannot disturb the vec mid-walk.
    // Reversed back into fall order: two items caught in one tick apply
    // oldest-first, so the newer one wins the paddle axis.
    for kind in caught.into_iter().rev() {
        state.apply_item(kind);
    }
}

/// A cleared field advances the level — or, on the last one, wins the game.
fn check_win(state: &mut GameState) {
    if state.bricks_remaining() == 0 {
        state.advance_level();
    }
}

#[cfg(test)]
mod tests {
    use crate::state::MIN_VERTICAL_FRACTION;
    use super::*;
    use crate::state::{BALL_RADIUS, BALL_SPEED, FIELD_H, FIELD_W};

    fn playing() -> GameState {
        let mut s = GameState::new();
        s.launch();
        s
    }

    // ---- accumulator ----

    #[test]
    fn accumulator_carries_leftover_time() {
        let mut a = Accumulator::new();
        // 1.5 steps worth: one now, half carried.
        assert_eq!(a.steps_for(FIXED_DT * 1.5), 1);
        // Another 0.5 completes the second step.
        assert_eq!(a.steps_for(FIXED_DT * 0.5), 1);
    }

    #[test]
    fn accumulator_does_not_lose_time_over_many_frames() {
        let mut a = Accumulator::new();
        let frame = 1.0 / 60.0; // 4 fixed steps per frame exactly
        let total: u32 = (0..60).map(|_| a.steps_for(frame)).sum();
        assert!((total as i32 - 240).abs() <= 1, "expected ~240 steps, got {total}");
    }

    /// The spiral of death: one enormous frame must not demand hundreds
    /// of steps.
    #[test]
    fn accumulator_clamps_a_huge_frame() {
        let mut a = Accumulator::new();
        assert_eq!(a.steps_for(10.0), MAX_STEPS_PER_FRAME);
        // And it must not still be in debt afterwards.
        assert_eq!(a.steps_for(0.0), 0);
    }

    #[test]
    fn accumulator_ignores_nonsense_dt() {
        let mut a = Accumulator::new();
        assert_eq!(a.steps_for(-1.0), 0);
        assert_eq!(a.steps_for(f32::NAN), 0);
    }

    // ---- walls ----

    #[test]
    fn ball_bounces_off_the_side_walls() {
        let mut s = playing();
        s.balls[0].pos = Vec2::new(BALL_RADIUS - 1.0, 300.0);
        s.balls[0].vel = Vec2::new(-100.0, -100.0);
        let field = s.field();
        collide_walls(&mut s.balls[0], &field);
        assert!(s.balls[0].vel.x > 0.0, "should reflect rightward");
        assert!(s.balls[0].pos.x >= BALL_RADIUS);
    }

    #[test]
    fn ball_bounces_off_the_ceiling() {
        let mut s = playing();
        s.balls[0].pos = Vec2::new(400.0, BALL_RADIUS - 1.0);
        s.balls[0].vel = Vec2::new(50.0, -100.0);
        let field = s.field();
        collide_walls(&mut s.balls[0], &field);
        assert!(s.balls[0].vel.y > 0.0, "should reflect downward");
    }

    /// Two steps now, not one: the wall pass MARKS the ball, and retiring
    /// the marked balls is what costs the life. With one ball in play the
    /// two are indistinguishable from outside — which is the point, the
    /// single-ball behaviour is unchanged.
    #[test]
    fn falling_past_the_bottom_costs_a_life() {
        let mut s = playing();
        let lives = s.lives;
        s.balls[0].pos = Vec2::new(400.0, FIELD_H + 50.0);
        let field = s.field();

        collide_walls(&mut s.balls[0], &field);
        assert!(s.balls[0].drained, "passing the bottom must mark the ball");
        assert_eq!(s.lives, lives, "but must NOT cost a life by itself");

        s.retire_drained_balls();
        assert_eq!(s.lives, lives - 1);
        assert_eq!(s.phase, Phase::Ready);
    }

    // ---- paddle ----

    #[test]
    fn paddle_is_clamped_to_the_field() {
        let mut s = playing();
        s.paddle.dir = -1.0;
        for _ in 0..10_000 {
            move_paddle(&mut s);
        }
        assert!(s.paddle.x >= 0.0);

        s.paddle.dir = 1.0;
        for _ in 0..10_000 {
            move_paddle(&mut s);
        }
        assert!(s.paddle.rect().right() <= FIELD_W);
    }

    #[test]
    fn hitting_the_paddle_left_of_centre_sends_the_ball_left() {
        let mut s = playing();
        s.paddle.x = 400.0;
        s.balls[0].pos = Vec2::new(s.paddle.center_x() - 50.0, s.paddle.y - BALL_RADIUS + 1.0);
        s.balls[0].vel = Vec2::new(0.0, 300.0);
        let paddle = s.paddle;
        let speed = s.ball_speed();
        collide_paddle(&mut s.balls[0], &paddle, speed, false);
        assert!(s.balls[0].vel.x < 0.0, "vx = {}", s.balls[0].vel.x);
        assert!(s.balls[0].vel.y < 0.0, "must go up");
    }

    #[test]
    fn hitting_the_paddle_right_of_centre_sends_the_ball_right() {
        let mut s = playing();
        s.paddle.x = 400.0;
        s.balls[0].pos = Vec2::new(s.paddle.center_x() + 50.0, s.paddle.y - BALL_RADIUS + 1.0);
        s.balls[0].vel = Vec2::new(0.0, 300.0);
        let paddle = s.paddle;
        let speed = s.ball_speed();
        collide_paddle(&mut s.balls[0], &paddle, speed, false);
        assert!(s.balls[0].vel.x > 0.0);
        assert!(s.balls[0].vel.y < 0.0);
    }

    #[test]
    fn paddle_bounce_preserves_speed() {
        let mut s = playing();
        s.balls[0].pos = Vec2::new(s.paddle.center_x() + 20.0, s.paddle.y - BALL_RADIUS + 1.0);
        s.balls[0].vel = Vec2::new(10.0, 300.0);
        let paddle = s.paddle;
        let speed = s.ball_speed();
        collide_paddle(&mut s.balls[0], &paddle, speed, false);
        assert!((s.balls[0].vel.length() - BALL_SPEED).abs() < 0.5,
            "speed drifted to {}", s.balls[0].vel.length());
    }

    /// A ball travelling upward through the paddle must not be batted
    /// back down.
    #[test]
    fn upward_ball_passes_through_the_paddle() {
        let mut s = playing();
        s.balls[0].pos = Vec2::new(s.paddle.center_x(), s.paddle.y);
        s.balls[0].vel = Vec2::new(0.0, -300.0);
        let before = s.balls[0].vel;
        let paddle = s.paddle;
        let speed = s.ball_speed();
        collide_paddle(&mut s.balls[0], &paddle, speed, false);
        assert_eq!(s.balls[0].vel, before);
    }

    // ---- bricks ----

    #[test]
    fn hitting_a_brick_kills_it_and_scores() {
        let mut s = playing();
        let brick = s.bricks[0].rect;
        s.balls[0].pos = Vec2::new(brick.center().x, brick.bottom() + BALL_RADIUS - 2.0);
        s.balls[0].vel = Vec2::new(0.0, -BALL_SPEED);
        collide_bricks(&mut s, 0);
        assert!(!s.bricks[0].alive());
        assert_eq!(s.score, 10);
        assert!(s.balls[0].vel.y > 0.0, "should bounce back downward");
    }

    /// Two bricks in one tick must produce ONE reflection, not two.
    #[test]
    fn overlapping_two_bricks_reflects_only_once() {
        let mut s = playing();
        // Sit the ball right where two adjacent bricks meet.
        let a = s.bricks[0].rect;
        let b = s.bricks[1].rect;
        let seam = (a.right() + b.left()) / 2.0;
        s.balls[0].pos = Vec2::new(seam, a.center().y);
        s.balls[0].vel = Vec2::new(BALL_SPEED, 0.0);
        let before = s.balls[0].vel;
        collide_bricks(&mut s, 0);
        // Exactly one brick dies this tick.
        let dead = s.bricks.iter().filter(|k| !k.alive()).count();
        assert_eq!(dead, 1, "one collision per tick");
        // And the velocity did not flip twice back to its original sign.
        assert_ne!(s.balls[0].vel.x.signum(), before.x.signum(),
            "a single reflection must change direction");
    }

    /// Clearing a field no longer wins — it advances. Winning is reaching
    /// the end of the last level, and that is the only route to it.
    #[test]
    fn clearing_a_field_advances_the_level() {
        let mut s = playing();
        let level = s.level;
        for b in &mut s.bricks {
            b.hits = 0;
        }
        check_win(&mut s);
        // ⚠️ Clearing a field now STARTS a cascade rather than finishing
        // the advance in one call. The level arrives when it ends.
        assert_eq!(s.phase, Phase::Clearing, "clearing should begin the cascade");
        s.skip_clear();

        assert_eq!(s.level, level + 1);
        assert_eq!(s.phase, Phase::Ready, "the next level starts on the paddle");
        assert!(s.bricks_remaining() > 0, "and it has a fresh field");
    }

    #[test]
    fn clearing_the_last_level_wins_the_game() {
        let mut s = playing();
        s.level = crate::state::LEVELS;
        for b in &mut s.bricks {
            b.hits = 0;
        }
        check_win(&mut s);
        assert_eq!(s.phase, Phase::Won);
        assert_eq!(s.level, crate::state::LEVELS, "the level does not run past the last");
    }

    // ---- angle clamping ----

    #[test]
    fn near_horizontal_velocity_is_steepened() {
        let mut v = Vec2::new(BALL_SPEED, 1.0);
        clamp_angle(&mut v);
        assert!(v.y.abs() >= BALL_SPEED * MIN_VERTICAL_FRACTION * 0.99,
            "vy {} not steep enough", v.y);
        assert!((v.length() - BALL_SPEED).abs() < 0.5, "speed must be preserved");
    }

    #[test]
    fn clamping_preserves_direction_sign() {
        let mut up = Vec2::new(100.0, -1.0);
        clamp_angle(&mut up);
        assert!(up.y < 0.0, "upward stays upward");

        let mut down = Vec2::new(100.0, 1.0);
        clamp_angle(&mut down);
        assert!(down.y > 0.0, "downward stays downward");
    }
}

#[cfg(test)]
mod multiball_tests {
    use super::*;
    use crate::state::{BALL_CAP_HIGH, BALL_CAP_LOW, BALL_SPEED, BRICK_H, FIELD_H, LEVEL_CAP_STEP};

    fn playing() -> GameState {
        let mut s = GameState::new();
        s.launch();
        s
    }

    /// Put a ball at `pos` moving `vel`, ignoring the cap. Test-only: it
    /// reaches past `spawn_ball` so a test can set up more balls than a
    /// level would ever allow.
    fn add_ball(s: &mut GameState, pos: Vec2, vel: Vec2) {
        let r = s.balls[0].radius;
        s.balls.push(crate::state::Ball::new(pos, vel, r));
    }

    // ---- the five the plan names ----

    #[test]
    fn a_ball_draining_with_others_in_play_costs_no_life() {
        let mut s = playing();
        add_ball(&mut s, Vec2::new(300.0, 300.0), Vec2::new(0.0, -200.0));
        let lives = s.lives;

        // Drop the FIRST ball past the bottom; the second is mid-field.
        s.balls[0].pos = Vec2::new(400.0, FIELD_H + 50.0);
        step_fixed(&mut s);

        assert_eq!(s.lives, lives, "a life must survive while a ball is in play");
        assert_eq!(s.balls.len(), 1, "the drained ball must be gone");
        assert_eq!(s.phase, Phase::Playing, "and play must continue");
    }

    #[test]
    fn the_last_ball_draining_costs_a_life() {
        let mut s = playing();
        add_ball(&mut s, Vec2::new(300.0, 300.0), Vec2::new(0.0, -200.0));
        let lives = s.lives;

        // Both below the floor in the same tick.
        s.balls[0].pos = Vec2::new(400.0, FIELD_H + 50.0);
        s.balls[1].pos = Vec2::new(500.0, FIELD_H + 50.0);
        step_fixed(&mut s);

        assert_eq!(s.lives, lives - 1, "the field emptied: one life, not two");
        assert_eq!(s.phase, Phase::Ready);
        assert_eq!(s.balls.len(), 1, "Ready holds exactly one ball");
    }

    /// ⚠️ Several balls draining at once must cost ONE life, not one each.
    /// With three balls and three lives, a per-ball charge is game over.
    #[test]
    fn many_balls_draining_together_cost_exactly_one_life() {
        let mut s = playing();
        add_ball(&mut s, Vec2::new(300.0, 300.0), Vec2::ZERO);
        add_ball(&mut s, Vec2::new(500.0, 300.0), Vec2::ZERO);
        let lives = s.lives;

        for b in &mut s.balls {
            b.pos = Vec2::new(b.pos.x, FIELD_H + 50.0);
        }
        step_fixed(&mut s);

        assert_eq!(s.lives, lives - 1, "one empty field, one life");
        assert_ne!(s.phase, Phase::Lost, "three balls must not end a three-life game");
    }

    #[test]
    fn two_balls_hitting_the_same_brick_in_one_tick_score_it_once() {
        let mut s = playing();
        let target = *s.bricks.iter().find(|b| b.alive()).unwrap();
        let c = target.rect.center();
        let before = s.score;
        let alive_before = s.bricks_remaining();

        // Both balls overlapping the same brick, from below.
        //
        // ⚠️ Placed against the brick's CENTRE line, not its bottom edge. At
        // bottom-1 an 8-radius ball spans 16 units and reaches into the row
        // below (row 1 starts 6 units under row 0's bottom), so each ball
        // straddles two bricks and they resolve against different ones —
        // which is correct behaviour, and would make this test assert the
        // wrong thing. Kept inside one brick, the collision is unambiguous.
        let y = c.y;
        s.balls[0].pos = Vec2::new(c.x - 4.0, y);
        s.balls[0].vel = Vec2::new(0.0, -100.0);
        add_ball(&mut s, Vec2::new(c.x + 4.0, y), Vec2::new(0.0, -100.0));

        step_fixed(&mut s);

        assert_eq!(s.bricks_remaining(), alive_before - 1, "exactly one brick died");
        assert_eq!(s.score - before, 10, "and it scored exactly once");
    }

    #[test]
    fn the_ball_cap_is_five_below_level_six_and_ten_above() {
        let mut s = playing();

        s.level = 1;
        assert_eq!(s.ball_cap(), BALL_CAP_LOW);
        s.level = LEVEL_CAP_STEP - 1;
        assert_eq!(s.ball_cap(), BALL_CAP_LOW, "level 5 is still the low cap");
        s.level = LEVEL_CAP_STEP;
        assert_eq!(s.ball_cap(), BALL_CAP_HIGH, "level 6 raises it");
        s.level = 10;
        assert_eq!(s.ball_cap(), BALL_CAP_HIGH);
    }

    #[test]
    fn a_level_starts_with_exactly_one_ball() {
        let mut s = playing();
        add_ball(&mut s, Vec2::new(300.0, 300.0), Vec2::ZERO);
        add_ball(&mut s, Vec2::new(500.0, 300.0), Vec2::ZERO);
        assert_eq!(s.balls.len(), 3);

        // Every route back to Ready collapses to one ball.
        s.rest_ball_on_paddle();
        assert_eq!(s.balls.len(), 1);

        let fresh = GameState::new();
        assert_eq!(fresh.balls.len(), 1);
    }

    // ---- the cap, enforced ----

    #[test]
    fn spawning_stops_at_the_cap_and_says_so() {
        let mut s = playing();
        s.level = 1;
        let mut spawned = 1;
        // Ask for far more than the cap allows.
        for _ in 0..20 {
            if s.spawn_ball(Vec2::new(400.0, 300.0), Vec2::new(50.0, -50.0)) {
                spawned += 1;
            }
        }
        assert_eq!(s.balls.len(), BALL_CAP_LOW);
        assert_eq!(spawned, BALL_CAP_LOW, "spawn_ball must report the refusals");

        // The high cap admits more.
        s.level = LEVEL_CAP_STEP;
        while s.spawn_ball(Vec2::new(400.0, 300.0), Vec2::new(50.0, -50.0)) {}
        assert_eq!(s.balls.len(), BALL_CAP_HIGH);
    }

    #[test]
    fn a_spawned_ball_goes_to_the_back_and_keeps_the_others_in_place() {
        let mut s = playing();
        add_ball(&mut s, Vec2::new(111.0, 300.0), Vec2::ZERO);
        let first = s.balls[0].pos;
        let second = s.balls[1].pos;

        s.spawn_ball(Vec2::new(999.0, 300.0), Vec2::new(0.0, -100.0));

        assert_eq!(s.balls[0].pos, first, "existing balls must not move in the vec");
        assert_eq!(s.balls[1].pos, second);
        assert_eq!(s.balls[2].pos.x, 999.0, "the new ball is at the back");
    }

    // ---- trails are per ball ----

    /// ⚠️ The bug this prevents renders as a line whipping between balls,
    /// and gets reported as a rendering bug. Each ball keeps its own.
    #[test]
    fn every_ball_keeps_its_own_trail() {
        let mut s = playing();
        s.balls[0].pos = Vec2::new(100.0, 300.0);
        s.balls[0].vel = Vec2::new(0.0, -150.0);
        add_ball(&mut s, Vec2::new(800.0, 300.0), Vec2::new(0.0, -150.0));

        let mut acc = Accumulator::new();
        for _ in 0..6 {
            step(&mut s, &mut acc, 1.0 / 60.0);
        }

        assert!(s.balls[0].trail.len() >= 2, "ball 0 recorded a trail");
        assert!(s.balls[1].trail.len() >= 2, "ball 1 recorded a trail");

        // Each trail must stay near its own ball. A shared buffer would put
        // points from both balls in one list, hundreds of units apart.
        for (i, ball) in s.balls.iter().enumerate() {
            for pos in &ball.trail {
                assert!(
                    (pos.x - ball.pos.x).abs() < 100.0,
                    "ball {i} trail point {pos:?} belongs to another ball (ball at {:?})",
                    ball.pos
                );
            }
        }
    }

    #[test]
    fn leaving_play_clears_every_trail() {
        let mut s = playing();
        add_ball(&mut s, Vec2::new(800.0, 300.0), Vec2::new(0.0, -150.0));
        let mut acc = Accumulator::new();
        for _ in 0..6 {
            step(&mut s, &mut acc, 1.0 / 60.0);
        }
        assert!(s.balls.iter().any(|b| !b.trail.is_empty()));

        s.phase = Phase::Lost;
        step(&mut s, &mut acc, 1.0 / 60.0);
        assert!(
            s.balls.iter().all(|b| b.trail.is_empty()),
            "no trail may outlive the run that drew it"
        );
    }

    // ---- the invariants that must survive multi-ball ----

    /// Invariant 2, per ball. Two balls each straddling their own pair of
    /// bricks must each resolve their own shallowest collision — never one
    /// ball's geometry deciding the other's bounce.
    #[test]
    fn each_ball_resolves_its_own_collision() {
        let mut s = playing();
        // Clear the field, then place two isolated bricks far apart.
        for b in &mut s.bricks {
            b.hits = 0;
        }
        s.bricks[0].hits = 1;
        let left = s.bricks[0].rect;
        let right_i = s.bricks.len() - 1;
        s.bricks[right_i].hits = 1;
        let right = s.bricks[right_i].rect;
        // ⚠️ Leave a third brick standing. Killing every brick on the field
        // now ADVANCES THE LEVEL, which rebuilds `bricks` inside the same
        // tick — the assertions below would then be reading a fresh field
        // rather than the one these balls hit.
        let spare = s.bricks.len() / 2;
        s.bricks[spare].hits = 1;

        s.balls[0].pos = Vec2::new(left.center().x, left.bottom() - 1.0);
        s.balls[0].vel = Vec2::new(0.0, -200.0);
        add_ball(&mut s, Vec2::new(right.center().x, right.bottom() - 1.0), Vec2::new(0.0, -200.0));

        step_fixed(&mut s);

        assert!(!s.bricks[0].alive(), "ball 0 broke its own brick");
        assert!(!s.bricks[right_i].alive(), "ball 1 broke its own brick");
        assert!(s.balls[0].vel.y > 0.0, "ball 0 reflected downward");
        assert!(s.balls[1].vel.y > 0.0, "ball 1 reflected downward");
    }

    /// The tunnelling guarantee is per ball and must hold for all of them.
    #[test]
    fn no_ball_tunnels_through_a_brick() {
        let mut s = playing();
        let per_tick = BALL_SPEED * FIXED_DT;
        assert!(
            per_tick < BRICK_H,
            "a tick moves {per_tick} units through a {BRICK_H}-unit brick"
        );

        // Ten balls, all launched at the field.
        s.level = LEVEL_CAP_STEP;
        while s.spawn_ball(Vec2::new(480.0, 500.0), Vec2::new(0.0, -BALL_SPEED)) {}
        assert_eq!(s.balls.len(), BALL_CAP_HIGH);

        let mut acc = Accumulator::new();
        for _ in 0..600 {
            step(&mut s, &mut acc, 1.0 / 60.0);
            for b in &s.balls {
                assert!(b.pos.x.is_finite() && b.pos.y.is_finite(), "NaN escaped");
                assert!(
                    b.pos.y > -50.0 && b.pos.y < FIELD_H + 100.0,
                    "ball left the field at {:?}",
                    b.pos
                );
            }
        }
    }

    /// Ten balls in play must not stall, NaN, or lose the phase.
    #[test]
    fn a_full_field_of_balls_stays_sane() {
        let mut s = playing();
        s.level = LEVEL_CAP_STEP;
        let mut seed = 7u32;
        while s.balls.len() < BALL_CAP_HIGH {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let ang = (seed >> 8) as f32 / 65535.0;
            s.spawn_ball(
                Vec2::new(200.0 + ang * 500.0, 400.0),
                Vec2::new(ang * 200.0 - 100.0, -BALL_SPEED),
            );
        }

        let mut acc = Accumulator::new();
        for _ in 0..1200 {
            step(&mut s, &mut acc, 1.0 / 60.0);
        }
        assert!(s.balls.len() <= BALL_CAP_HIGH, "the cap held all game");
        for b in &s.balls {
            assert!(b.pos.x.is_finite() && b.pos.y.is_finite());
        }
    }
}

#[cfg(test)]
mod level_tests {
    use super::*;
    use crate::state::{
        ball_speed_for, hits_to_clear, paddle_speed_for, tier_for, Tier, BALL_SPEED,
        BALL_SPEED_TOP, BRICK_COLS, BRICK_H, BRICK_ROWS, FIELD_W, LEVELS, PADDLE_SPEED,
        PADDLE_SPEED_TOP,
    };

    // ---- ⚠️C: the tunnelling bound, DERIVED ----

    /// ⚠️ **The guard the speed ramp exists inside.** The limit is computed
    /// from `BRICK_H` and `FIXED_DT`, never typed, so changing either moves
    /// this assertion instead of quietly invalidating it. That is L026, and
    /// Omaprix paid for it twice.
    #[test]
    fn ball_speed_at_every_level_is_far_below_the_tunnelling_limit() {
        // The limit really is "one tick crosses a whole brick".
        assert!(
            (TUNNELLING_LIMIT - BRICK_H / FIXED_DT).abs() < 1e-3,
            "the limit must be derived from the geometry, not restated"
        );

        for level in 1..=LEVELS {
            let speed = ball_speed_for(level);
            let per_tick = speed * FIXED_DT;
            assert!(
                per_tick < BRICK_H / 4.0,
                "level {level}: a tick moves {per_tick} units into a {BRICK_H}-unit brick"
            );
            assert!(
                speed < TUNNELLING_LIMIT / 4.0,
                "level {level}: {speed} is too close to the {TUNNELLING_LIMIT} limit"
            );
        }
    }

    /// The paddle must always be able to get under the ball, at every level.
    /// If the ball outruns it the game stops being winnable, and no test of
    /// either speed alone would catch it.
    #[test]
    fn the_paddle_outruns_the_ball_at_every_level() {
        for level in 1..=LEVELS {
            let ball = ball_speed_for(level);
            let paddle = paddle_speed_for(level);
            assert!(
                paddle > ball,
                "level {level}: paddle {paddle} cannot keep up with ball {ball}"
            );
            // And it can cross the field before a ball can cross it too.
            let paddle_cross = FIELD_W / paddle;
            let ball_cross = FIELD_W / ball;
            assert!(paddle_cross < ball_cross, "level {level}: paddle crosses slower");
        }
    }

    // ---- the speed ramp ----

    #[test]
    fn speed_climbs_from_level_one_to_the_top_and_stops() {
        assert!((ball_speed_for(1) - BALL_SPEED).abs() < 0.01);
        assert!((ball_speed_for(LEVELS) - BALL_SPEED_TOP).abs() < 0.01);
        assert!((paddle_speed_for(1) - PADDLE_SPEED).abs() < 0.01);
        assert!((paddle_speed_for(LEVELS) - PADDLE_SPEED_TOP).abs() < 0.01);

        // Monotone, and clamped outside the real range.
        for level in 2..=LEVELS {
            assert!(ball_speed_for(level) > ball_speed_for(level - 1));
        }
        assert_eq!(ball_speed_for(0), ball_speed_for(1), "below range clamps");
        assert_eq!(ball_speed_for(99), ball_speed_for(LEVELS), "above range clamps");
    }

    /// The whole point of a gentle ramp: work climbs 2.6x across the game,
    /// and speed must climb far less or the last level is unplayable.
    #[test]
    fn work_climbs_much_faster_than_speed() {
        let work = hits_to_clear(LEVELS) as f32 / hits_to_clear(1) as f32;
        let speed = ball_speed_for(LEVELS) / ball_speed_for(1);
        assert!(work > 2.0, "the game should get substantially longer: {work}x");
        assert!(speed < 1.5, "but not much faster: {speed}x");
        assert!(speed < work / 1.5, "speed {speed}x must lag work {work}x");
    }

    // ---- the curve ----

    #[test]
    fn level_one_is_all_plain() {
        for row in 0..BRICK_ROWS {
            assert_eq!(tier_for(1, row), Tier::Plain);
        }
    }

    /// Reinforced rows fill down from the TOP, one more per level, until
    /// the whole field is reinforced at L7.
    #[test]
    fn reinforced_rows_grow_from_the_top_to_the_peak_at_seven() {
        for level in 2..=7u32 {
            let expected = (level - 1) as usize;
            for row in 0..BRICK_ROWS {
                let want = if row < expected { Tier::Reinforced } else { Tier::Plain };
                assert_eq!(
                    tier_for(level, row),
                    want,
                    "level {level} row {row}: {expected} rows should be reinforced"
                );
            }
        }
        // L7 is the peak: every row reinforced, none plain, none armoured.
        assert!((0..BRICK_ROWS).all(|r| tier_for(7, r) == Tier::Reinforced));
    }

    /// Armour restarts the count at one row on L8 and never touches the
    /// bottom of the field.
    #[test]
    fn armour_arrives_at_eight_and_grows_to_three_rows() {
        for (level, want) in [(8u32, 1usize), (9, 2), (10, 3)] {
            let armoured = (0..BRICK_ROWS).filter(|&r| tier_for(level, r) == Tier::Armoured).count();
            assert_eq!(armoured, want, "level {level} should have {want} armoured rows");
            // Everything below the armour is reinforced — no plain rows
            // survive this far in.
            for row in want..BRICK_ROWS {
                assert_eq!(tier_for(level, row), Tier::Reinforced, "level {level} row {row}");
            }
        }
    }

    #[test]
    fn tiers_are_counted_from_the_top() {
        // The hardest row is always row 0, at every level past the first.
        for level in 2..=LEVELS {
            let top = tier_for(level, 0).hits();
            let bottom = tier_for(level, BRICK_ROWS - 1).hits();
            assert!(top >= bottom, "level {level}: the crust must be on top");
        }
    }

    // ---- the numbers the plan claims ----

    /// ⚠️ The plan stated L1 = 60, L7 = 120, L10 = **156**. The first two
    /// are right; the third is arithmetic that was never checked. The layout
    /// Brian settled — 3 armoured rows of 4 hits plus 3 reinforced of 2,
    /// across 10 columns — is 120 + 60 = **180**, and no row combination of
    /// this field produces 156 at all (the nearest are 150 and 160).
    ///
    /// The LAYOUT is the decision and it is unchanged; only the total was
    /// wrong. Asserting the real numbers here is what stops the mistake
    /// being inherited by the balance work that reads them.
    #[test]
    fn hits_to_clear_matches_the_planned_curve() {
        assert_eq!(hits_to_clear(1), 60, "L1: 6 plain rows of 10");
        assert_eq!(hits_to_clear(7), 120, "L7: 6 reinforced rows of 10");
        assert_eq!(hits_to_clear(10), 180, "L10: 3 armoured (120) + 3 reinforced (60)");

        // The shape: +10 a level while reinforced rows fill in, then +20 a
        // level once armour starts replacing them.
        for level in 2..=7 {
            assert_eq!(hits_to_clear(level) - hits_to_clear(level - 1), 10, "level {level}");
        }
        for level in 8..=10 {
            assert_eq!(hits_to_clear(level) - hits_to_clear(level - 1), 20, "level {level}");
        }

        // Never gets easier as it climbs.
        for level in 2..=LEVELS {
            assert!(
                hits_to_clear(level) >= hits_to_clear(level - 1),
                "level {level} is easier than {}",
                level - 1
            );
        }
    }

    #[test]
    fn a_built_field_matches_its_tier_table() {
        for level in 1..=LEVELS {
            let bricks = crate::state::build_bricks(level);
            assert_eq!(bricks.len(), BRICK_COLS * BRICK_ROWS);
            let total: u32 = bricks.iter().map(|b| b.hits).sum();
            assert_eq!(total, hits_to_clear(level), "level {level} field vs table");
            for (i, b) in bricks.iter().enumerate() {
                let row = i / BRICK_COLS;
                assert_eq!(b.tier, tier_for(level, row));
                assert_eq!(b.hits, b.tier.hits(), "a fresh brick starts undamaged");
                assert!(b.alive());
            }
        }
    }

    // ---- tiered bricks take the right number of hits ----

    #[test]
    fn a_tiered_brick_survives_until_its_hits_run_out() {
        for tier in [Tier::Plain, Tier::Reinforced, Tier::Armoured] {
            let mut b = crate::state::build_bricks(1)[0];
            b.tier = tier;
            b.hits = tier.hits();

            for remaining in (1..tier.hits()).rev() {
                assert!(!b.hit(), "{tier:?} died early");
                assert!(b.alive());
                assert_eq!(b.hits, remaining);
            }
            assert!(b.hit(), "{tier:?} should die on its last hit");
            assert!(!b.alive());
        }
    }

    /// Damage runs 0 to 1 and is what the renderer chips the brick by.
    #[test]
    fn damage_reports_how_much_of_a_brick_is_gone() {
        let mut b = crate::state::build_bricks(1)[0];
        b.tier = Tier::Armoured;
        b.hits = 4;
        assert_eq!(b.damage(), 0.0);
        b.hit();
        assert_eq!(b.damage(), 0.25);
        b.hit();
        assert_eq!(b.damage(), 0.5);
        b.hit();
        assert_eq!(b.damage(), 0.75);
        b.hit();
        assert_eq!(b.damage(), 1.0);
    }

    /// Chipping scores, breaking scores more. A reinforced brick is worth
    /// 5 + 10; an armoured one 5+5+5+10.
    #[test]
    fn every_hit_scores_and_the_breaking_hit_scores_more() {
        let mut s = GameState::new();
        s.launch();
        for b in &mut s.bricks {
            b.hits = 0;
        }
        // One armoured brick, alone on the field.
        s.bricks[0].tier = Tier::Armoured;
        s.bricks[0].hits = 4;
        let target = s.bricks[0].rect;

        let mut score = 0;
        for expected in [5u32, 5, 5, 10] {
            let before = s.score;
            s.balls[0].pos = Vec2::new(target.center().x, target.center().y);
            s.balls[0].vel = Vec2::new(0.0, -100.0);
            collide_bricks(&mut s, 0);
            let gained = s.score - before;
            assert_eq!(gained, expected, "hit scoring");
            score += gained;
        }
        assert_eq!(score, 25, "an armoured brick is worth 25 in total");
        assert!(!s.bricks[0].alive());
    }

    // ---- level progression ----

    #[test]
    fn advancing_keeps_score_and_lives_and_rebuilds_the_field() {
        let mut s = GameState::new();
        s.launch();
        s.score = 1234;
        s.lives = 2;
        for b in &mut s.bricks {
            b.hits = 0;
        }

        s.advance_level();
        s.skip_clear();

        assert_eq!(s.level, 2);
        assert_eq!(s.score, 1234, "score carries across levels");
        assert_eq!(s.lives, 2, "and so do lives");
        assert_eq!(s.bricks_remaining(), BRICK_COLS * BRICK_ROWS);
        assert_eq!(s.balls.len(), 1, "the next level starts with one ball");
        assert_eq!(s.phase, Phase::Ready);
    }

    #[test]
    fn a_full_run_reaches_the_last_level_and_then_wins() {
        let mut s = GameState::new();
        s.launch();
        for level in 1..LEVELS {
            assert_eq!(s.level, level);
            for b in &mut s.bricks {
                b.hits = 0;
            }
            s.advance_level();
        s.skip_clear();
        }
        assert_eq!(s.level, LEVELS);

        for b in &mut s.bricks {
            b.hits = 0;
        }
        s.advance_level();
        s.skip_clear();
        assert_eq!(s.phase, Phase::Won, "clearing the last level wins");
    }

    #[test]
    fn restarting_returns_to_level_one() {
        let mut s = GameState::new();
        s.level = 9;
        s.score = 5000;
        s.restart();
        assert_eq!(s.level, 1);
        assert_eq!(s.bricks_remaining(), BRICK_COLS * BRICK_ROWS);
        // A level-1 field is all plain.
        assert!(s.bricks.iter().all(|b| b.tier == Tier::Plain));
    }

    /// Speed is read from the level every tick, so advancing actually makes
    /// the ball faster rather than only changing a number.
    #[test]
    fn a_later_level_actually_plays_faster() {
        let mut s = GameState::new();
        s.launch();
        let slow = s.balls[0].vel.length();

        s.level = LEVELS;
        s.phase = Phase::Ready;
        s.rest_ball_on_paddle();
        s.launch();
        let fast = s.balls[0].vel.length();

        assert!(fast > slow, "level {LEVELS} launch {fast} vs level 1 {slow}");
    }
}

#[cfg(test)]
mod item_tests {
    use super::*;
    use crate::items::{Item, ItemKind, Strength, BOMB_SCALE, FALL_SPEED, ITEM_H};
    use crate::state::{BALL_RADIUS, FIELD_H, FIELD_W, PADDLE_W};

    fn playing() -> GameState {
        let mut s = GameState::new();
        s.launch();
        s
    }

    fn drop_at(s: &mut GameState, x: f32, y: f32, kind: ItemKind) {
        s.items.push(Item::new(Vec2::new(x, y), kind));
    }

    // ---- falling and catching ----

    #[test]
    fn an_item_falls_straight_down() {
        let mut s = playing();
        drop_at(&mut s, 400.0, 200.0, ItemKind::Bomb);
        let before = s.items[0].pos;
        step_fixed(&mut s);
        let after = s.items[0].pos;
        assert_eq!(after.x, before.x, "items must not drift sideways");
        assert!((after.y - before.y - FALL_SPEED * FIXED_DT).abs() < 1e-3);
    }

    #[test]
    fn the_paddle_catches_an_item_and_it_takes_effect() {
        let mut s = playing();
        let p = s.paddle.rect();
        drop_at(&mut s, p.center().x, p.center().y, ItemKind::Grow(Strength::Medium));

        step_fixed(&mut s);

        assert!(s.items.is_empty(), "a caught item leaves the field");
        assert!(s.paddle.w > PADDLE_W, "and the paddle grew");
        assert!(s.paddle_effect_left > 0.0);
    }

    /// ⚠️ A missed item costs NOTHING. Punishing a miss would teach exactly
    /// the wrong reflex — diving for a falling bomb.
    #[test]
    fn a_missed_item_is_harmless() {
        let mut s = playing();
        let lives = s.lives;
        drop_at(&mut s, 100.0, FIELD_H - 20.0, ItemKind::Grow(Strength::Large));

        for _ in 0..240 {
            step_fixed(&mut s);
        }

        assert!(s.items.is_empty(), "it fell off the bottom");
        assert_eq!(s.lives, lives, "and cost nothing");
        assert_eq!(s.paddle.w, PADDLE_W, "and had no effect");
    }

    #[test]
    fn several_items_fall_and_are_caught_independently() {
        let mut s = playing();
        let p = s.paddle.rect();
        drop_at(&mut s, p.center().x, p.center().y, ItemKind::Grow(Strength::Small));
        drop_at(&mut s, 60.0, 100.0, ItemKind::Bomb);
        drop_at(&mut s, 900.0, 100.0, ItemKind::Bomb);

        step_fixed(&mut s);

        assert_eq!(s.items.len(), 2, "only the one on the paddle was caught");
        assert!(s.paddle.w > PADDLE_W);
    }

    // ---- drops come only from final hits ----

    /// ⚠️ An armoured brick takes four hits. Rolling on each would make the
    /// late levels rain power-ups exactly where the player earned the least.
    #[test]
    fn only_a_final_hit_can_drop_an_item() {
        let mut s = playing();
        for b in &mut s.bricks {
            b.hits = 0;
        }
        s.bricks[0].tier = crate::state::Tier::Armoured;
        s.bricks[0].hits = 4;
        let target = s.bricks[0].rect;

        // The first three hits are chips: no drop is even rolled for.
        for _ in 0..3 {
            s.balls[0].pos = target.center();
            s.balls[0].vel = Vec2::new(0.0, -100.0);
            collide_bricks(&mut s, 0);
            assert!(s.items.is_empty(), "a chip must never drop an item");
        }
        assert_eq!(s.bricks[0].hits, 1, "three chips taken");
    }

    /// Over many final hits the drop rate lands near the stated chance, and
    /// the items land where the brick was.
    #[test]
    fn final_hits_drop_at_roughly_the_stated_rate() {
        let mut s = playing();
        let mut drops = 0;
        let trials = 600;

        for _ in 0..trials {
            s.bricks = crate::state::build_bricks(1);
            for b in &mut s.bricks[1..] {
                b.hits = 0;
            }
            s.items.clear();
            let target = s.bricks[0].rect;
            s.balls[0].pos = target.center();
            s.balls[0].vel = Vec2::new(0.0, -100.0);
            collide_bricks(&mut s, 0);
            if let Some(item) = s.items.first() {
                drops += 1;
                assert!(
                    (item.pos.x - target.center().x).abs() < 1.0,
                    "an item must appear where its brick was"
                );
            }
        }

        let rate = drops as f32 / trials as f32;
        assert!(
            (rate - crate::items::DROP_CHANCE_LOW).abs() < 0.06,
            "level 1 drop rate {rate:.3} vs stated {:.3}",
            crate::items::DROP_CHANCE_LOW
        );
    }

    // ---- the paddle axis ----

    #[test]
    fn a_grow_widens_and_a_bomb_narrows() {
        let mut s = playing();
        s.apply_item(ItemKind::Grow(Strength::Large));
        assert!(s.paddle.w > PADDLE_W);

        let mut s = playing();
        s.apply_item(ItemKind::Bomb);
        assert!(s.paddle.w < PADDLE_W);
        assert!((s.paddle.w - PADDLE_W * BOMB_SCALE).abs() < 0.01);
    }

    /// ⚠️ A bomb caught while grown CANCELS the grow — it does not stack
    /// into a double-negative. The last thing you caught is what you have.
    #[test]
    fn a_bomb_cancels_a_grow_rather_than_compounding_it() {
        let mut s = playing();
        s.apply_item(ItemKind::Grow(Strength::Large));
        let grown = s.paddle.w;
        assert!(grown > PADDLE_W);

        s.apply_item(ItemKind::Bomb);

        assert!((s.paddle.w - PADDLE_W * BOMB_SCALE).abs() < 0.01, "plain bomb width");
        assert!(s.paddle.w < PADDLE_W);
        assert!(s.paddle.w > PADDLE_W * BOMB_SCALE * 0.9, "not compounded smaller");
    }

    /// ⚠️ Grow stacks DURATION, not size. Two catches last longer; they do
    /// not multiply into a paddle that fills the field.
    #[test]
    fn a_second_grow_extends_the_timer_without_multiplying_the_width() {
        let mut s = playing();
        s.apply_item(ItemKind::Grow(Strength::Medium));
        let w1 = s.paddle.w;
        let t1 = s.paddle_effect_left;

        s.apply_item(ItemKind::Grow(Strength::Medium));

        assert!((s.paddle.w - w1).abs() < 0.01, "width must not stack");
        assert!(s.paddle_effect_left > t1, "duration must stack");
    }

    #[test]
    fn a_bigger_grow_caught_while_grown_takes_the_larger_width() {
        let mut s = playing();
        s.apply_item(ItemKind::Grow(Strength::Small));
        let small = s.paddle.w;
        s.apply_item(ItemKind::Grow(Strength::Large));
        assert!(s.paddle.w > small, "the larger grow wins");

        // And the reverse does not shrink it.
        let large = s.paddle.w;
        s.apply_item(ItemKind::Grow(Strength::Small));
        assert!((s.paddle.w - large).abs() < 0.01, "a smaller grow must not shrink");
    }

    #[test]
    fn the_effect_expires_and_the_paddle_returns_to_normal() {
        let mut s = playing();
        s.apply_item(ItemKind::Bomb);
        assert!(s.paddle.w < PADDLE_W);

        s.tick_paddle_effect(crate::items::BOMB_SECONDS + 0.1);

        assert_eq!(s.paddle.w, PADDLE_W);
        assert_eq!(s.paddle_scale, 1.0);
        assert_eq!(s.paddle_effect_left, 0.0);
    }

    /// ⚠️ Both ends clamped. Wider than the field cannot be moved; narrower
    /// than the ball makes a return a coin flip that skill cannot improve.
    #[test]
    fn the_paddle_never_leaves_its_sane_range() {
        let mut s = playing();
        for scale in [0.0, 0.01, 0.1, 1.0, 5.0, 50.0, 1000.0] {
            s.paddle_scale = scale;
            s.resize_paddle();
            assert!(s.paddle.w >= BALL_RADIUS * 2.0, "scale {scale}: too narrow");
            assert!(s.paddle.w <= FIELD_W, "scale {scale}: wider than the field");
            assert!(s.paddle.x >= 0.0, "scale {scale}: off the left");
            assert!(s.paddle.x + s.paddle.w <= FIELD_W + 0.01, "scale {scale}: off the right");
        }
    }

    #[test]
    fn resizing_keeps_the_paddle_where_it_was() {
        let mut s = playing();
        s.paddle.x = 500.0;
        let centre = s.paddle.center_x();
        s.apply_item(ItemKind::Grow(Strength::Large));
        assert!((s.paddle.center_x() - centre).abs() < 0.01, "the paddle must not jump");
    }

    // ---- ⚠️ the one the plan warns about: does a narrow paddle break the ball? ----

    /// ⚠️ `bounce_off_paddle` divides by `paddle.w / 2.0`, so a narrow paddle
    /// steers HARDER. That is welcome emergent difficulty — but it must not
    /// push outgoing angles so far that the ball starts behaving strangely
    /// rather than the paddle just being small.
    ///
    /// The invariants that protect the game are speed and the vertical
    /// clamp, and both must survive a bombed paddle at every strike point.
    #[test]
    fn a_bombed_paddle_never_distorts_the_ball() {
        for scale in [BOMB_SCALE, 0.3, 0.1] {
            let mut s = playing();
            s.paddle_scale = scale;
            s.resize_paddle();
            let speed = s.ball_speed();

            // Strike across the whole face, edge to edge.
            for i in 0..=20 {
                let t = i as f32 / 20.0;
                let x = s.paddle.x + s.paddle.w * t;
                s.balls[0].pos = Vec2::new(x, s.paddle.rect().top() - BALL_RADIUS + 0.5);
                s.balls[0].vel = Vec2::new(0.0, 200.0);

                let paddle = s.paddle;
                collide_paddle(&mut s.balls[0], &paddle, speed, false);

                let v = s.balls[0].vel;
                assert!(v.x.is_finite() && v.y.is_finite(), "scale {scale} t {t}: NaN");
                assert!(
                    (v.length() - speed).abs() < 0.5,
                    "scale {scale} t {t}: speed {} drifted from {speed}",
                    v.length()
                );
                assert!(v.y < 0.0, "scale {scale} t {t}: must bounce upward");
                // The clamp must still hold: no skimming.
                assert!(
                    v.y.abs() / v.length() >= 0.24,
                    "scale {scale} t {t}: too shallow at {:.3}",
                    v.y.abs() / v.length()
                );
            }
        }
    }

    /// A narrow paddle SHOULD steer harder — that is the difficulty. This
    /// pins the direction so a future change cannot quietly invert it.
    #[test]
    fn a_narrow_paddle_steers_harder_than_a_wide_one() {
        let angle_at = |scale: f32| {
            let mut s = playing();
            s.paddle_scale = scale;
            s.resize_paddle();
            let speed = s.ball_speed();
            // Strike a quarter of the way out from centre, in field units,
            // so both paddles are hit at the same PLACE, not the same ratio.
            let x = s.paddle.center_x() + 20.0;
            s.balls[0].pos = Vec2::new(x, s.paddle.rect().top() - BALL_RADIUS + 0.5);
            s.balls[0].vel = Vec2::new(0.0, 200.0);
            let paddle = s.paddle;
            collide_paddle(&mut s.balls[0], &paddle, speed, false);
            s.balls[0].vel.x.abs()
        };

        assert!(
            angle_at(BOMB_SCALE) > angle_at(1.8),
            "a bombed paddle should steer more sharply than a grown one"
        );
    }

    // ---- level boundaries ----

    /// Last level's bomb must not follow the player into the next one.
    #[test]
    fn advancing_a_level_clears_items_and_the_paddle_effect() {
        let mut s = playing();
        s.apply_item(ItemKind::Bomb);
        drop_at(&mut s, 300.0, 200.0, ItemKind::Grow(Strength::Large));
        assert!(s.paddle.w < PADDLE_W);

        for b in &mut s.bricks {
            b.hits = 0;
        }
        check_win(&mut s);
        // ⚠️ Items are cleared the INSTANT the cascade begins, not when it
        // ends — a power-up still falling through the wave would be the
        // one thing on screen not participating in it.
        assert!(s.items.is_empty(), "items should clear as the cascade starts");
        s.skip_clear();

        assert_eq!(s.level, 2);
        assert!(s.items.is_empty(), "falling items do not cross levels");
        assert_eq!(s.paddle.w, PADDLE_W, "and neither does a paddle effect");
        assert_eq!(s.paddle_effect_left, 0.0);
    }

    #[test]
    fn restarting_clears_items_and_the_paddle_effect() {
        let mut s = playing();
        s.apply_item(ItemKind::Grow(Strength::Large));
        drop_at(&mut s, 300.0, 200.0, ItemKind::Bomb);
        s.restart();
        assert!(s.items.is_empty());
        assert_eq!(s.paddle.w, PADDLE_W);
    }

    /// Items must not fall, be caught, or expire outside Playing.
    #[test]
    fn items_are_frozen_outside_play() {
        let mut s = playing();
        drop_at(&mut s, 400.0, 200.0, ItemKind::Bomb);
        s.phase = Phase::Lost;
        let before = s.items[0].pos;
        for _ in 0..60 {
            step_fixed(&mut s);
        }
        assert_eq!(s.items[0].pos, before, "a finished game does not keep dropping");
    }

    /// A long soak with drops enabled: nothing leaks, nothing NaNs, and the
    /// paddle stays in range the whole time.
    #[test]
    fn a_long_run_with_drops_stays_sane() {
        let mut s = playing();
        let mut acc = Accumulator::new();
        for _ in 0..4000 {
            let target = s.balls.iter().map(|b| b.pos.x).next().unwrap_or(FIELD_W / 2.0);
            let c = s.paddle.center_x();
            s.paddle.dir = if (target - c).abs() < 4.0 { 0.0 } else if target > c { 1.0 } else { -1.0 };
            step(&mut s, &mut acc, 1.0 / 60.0);

            assert!(s.paddle.w >= BALL_RADIUS * 2.0 && s.paddle.w <= FIELD_W);
            assert!(s.paddle.x >= 0.0 && s.paddle.x + s.paddle.w <= FIELD_W + 0.01);
            for it in &s.items {
                assert!(it.pos.x.is_finite() && it.pos.y.is_finite());
                assert!(it.pos.y - ITEM_H <= FIELD_H + 50.0, "an item outlived the field");
            }
        }
    }
}


/// The magnet, the Omarchy item, and the one key that means two things.
#[cfg(test)]
mod magnet_tests {
    use super::*;
    use crate::items::{ItemKind, Strength, MAGNET_HOLD_SECONDS, MAGNET_SECONDS};
    use crate::state::{BALL_RADIUS, FIELD_W, LEVEL_CAP_STEP, PADDLE_W};

    /// A game in play with the magnet armed and one ball falling onto the
    /// paddle from just above it.
    fn about_to_be_caught() -> GameState {
        let mut s = GameState::new();
        s.launch();
        s.apply_item(ItemKind::Magnet);
        s.paddle.x = (FIELD_W - PADDLE_W) / 2.0;
        s.balls[0].pos = Vec2::new(s.paddle.center_x(), s.paddle.y - BALL_RADIUS);
        s.balls[0].vel = Vec2::new(0.0, s.ball_speed());
        s
    }

    fn steps(s: &mut GameState, n: u32) {
        for _ in 0..n {
            step_fixed(s);
        }
    }

    // ---- catching and holding ----

    #[test]
    fn an_armed_magnet_catches_the_ball_instead_of_bouncing_it() {
        let mut s = about_to_be_caught();
        step_fixed(&mut s);
        assert!(s.balls[0].is_held(), "the magnet did not catch the ball");
        assert_eq!(s.balls[0].vel, Vec2::ZERO, "a held ball must not move");
    }

    #[test]
    fn without_the_magnet_the_ball_bounces_as_before() {
        let mut s = about_to_be_caught();
        s.magnet_left = 0.0;
        step_fixed(&mut s);
        assert!(!s.balls[0].is_held());
        assert!(s.balls[0].vel.y < 0.0, "it should have bounced upward");
    }

    /// ⚠️ The held ball rides the paddle — that IS the aiming. A ball that
    /// stayed put while the paddle moved would make the magnet useless for
    /// choosing an angle, which is the entire point of it.
    #[test]
    fn a_held_ball_tracks_the_paddle() {
        let mut s = about_to_be_caught();
        step_fixed(&mut s);
        assert!(s.balls[0].is_held());

        s.paddle.dir = 1.0;
        steps(&mut s, 30);
        let dx = (s.balls[0].pos.x - s.paddle.center_x()).abs();
        assert!(dx < 1.0, "held ball drifted {dx} from the paddle centre");
        assert!(s.balls[0].is_held(), "it should still be held after 30 ticks");
    }

    /// ⚠️ A held ball is out of the simulation: it must not drain, and it
    /// must not cost a life while it sits on the paddle.
    #[test]
    fn a_held_ball_never_drains() {
        let mut s = about_to_be_caught();
        step_fixed(&mut s);
        let lives = s.lives;
        steps(&mut s, 100);
        assert!(!s.balls[0].drained, "a held ball must never be marked drained");
        assert_eq!(s.lives, lives, "holding a ball must not cost a life");
    }

    // ---- the auto-release, which is the feature ----

    /// ⚠️ **Brian settled this and it must not be softened.** Without the
    /// auto-release, the optimal play at level 9 is catch-aim-release on
    /// every single bounce: strictly better, much slower, and it swaps the
    /// skill in the game for patience.
    #[test]
    fn the_hold_fires_itself_after_three_seconds() {
        let mut s = about_to_be_caught();
        step_fixed(&mut s);
        assert!(s.balls[0].is_held());

        let ticks = (MAGNET_HOLD_SECONDS / FIXED_DT).ceil() as u32;
        // Just before the deadline it is still held.
        steps(&mut s, ticks - 2);
        assert!(
            s.balls[0].is_held(),
            "released early — the hold must last the full {MAGNET_HOLD_SECONDS}s"
        );

        steps(&mut s, 4);
        assert!(!s.balls[0].is_held(), "the auto-release did not fire");
        assert!(s.balls[0].vel.y < 0.0, "an auto-released ball must go upward");
        let speed = s.balls[0].vel.length();
        assert!(
            (speed - s.ball_speed()).abs() < 1.0,
            "released at {speed}, expected the level's {}",
            s.ball_speed()
        );
    }

    /// The magnet running out does not drop a ball it is already holding —
    /// a ball released by a timer the player cannot see reads as a misfire.
    #[test]
    fn the_magnet_expiring_does_not_drop_a_held_ball() {
        let mut s = about_to_be_caught();
        step_fixed(&mut s);
        s.magnet_left = FIXED_DT;
        step_fixed(&mut s);
        assert!(!s.magnet_active(), "the magnet should have expired");
        assert!(s.balls[0].is_held(), "expiry must not drop a ball in hand");
    }

    #[test]
    fn the_magnet_stops_catching_once_it_expires() {
        let mut s = about_to_be_caught();
        s.magnet_left = 0.0;
        step_fixed(&mut s);
        assert!(!s.balls[0].is_held(), "an expired magnet must not catch");
    }

    /// A second magnet refreshes rather than stacking — 20 s topped up, not
    /// 38 s banked from one lucky catch.
    #[test]
    fn a_second_magnet_refreshes_rather_than_extending() {
        let mut s = GameState::new();
        s.launch();
        s.apply_item(ItemKind::Magnet);
        s.magnet_left = 5.0;
        s.apply_item(ItemKind::Magnet);
        assert!(
            (s.magnet_left - MAGNET_SECONDS).abs() < 1e-3,
            "expected a refresh to {MAGNET_SECONDS}, got {}",
            s.magnet_left
        );
    }

    // ---- ⚠️ THE KEY: one Space, two meanings ----

    /// ⚠️ **The risk the plan named as S6's real danger.** Space launches in
    /// `Ready` and fires a held ball on RELEASE during `Playing`. A press
    /// arriving while a ball is held must not re-launch anything.
    #[test]
    fn pressing_space_while_a_ball_is_held_launches_nothing() {
        let mut s = about_to_be_caught();
        step_fixed(&mut s);
        assert!(s.balls[0].is_held());

        let balls_before = s.balls.len();
        let pos_before = s.balls[0].pos;
        // What main.rs does on KeyDown(Space).
        s.launch();

        assert_eq!(s.balls.len(), balls_before, "a press spawned a ball");
        assert!(s.balls[0].is_held(), "a press knocked the ball loose");
        assert_eq!(s.balls[0].vel, Vec2::ZERO, "a press fired the held ball");
        assert_eq!(s.balls[0].pos, pos_before);
    }

    /// Releasing Space is what fires it, and it aims where the paddle is.
    #[test]
    fn releasing_space_fires_the_held_ball_at_the_aim() {
        let mut s = about_to_be_caught();
        step_fixed(&mut s);

        // Slide left so the ball sits right of centre: it must go right.
        s.paddle.x -= 40.0;
        s.balls[0].pos.x = s.paddle.center_x() + s.paddle.w * 0.4;

        assert!(s.release_held_balls(), "release reported nothing fired");
        assert!(!s.balls[0].is_held());
        assert!(s.balls[0].vel.y < 0.0, "a fired ball must travel upward");
        assert!(
            s.balls[0].vel.x > 0.0,
            "struck right of centre, so it should go right — got {:?}",
            s.balls[0].vel
        );
    }

    /// ⚠️ The magnet must aim EXACTLY as an ordinary return does. It buys
    /// the player time to choose the angle, not a different set of angles.
    /// Two copies of the steering formula would drift apart; this test
    /// fails if they ever do.
    ///
    /// ⚠️ Dead centre is EXCLUDED and has its own test below: an ordinary
    /// bounce there returns a vertical ball, which is a stall, and a held
    /// ball lands dead centre every single time.
    #[test]
    fn a_released_ball_aims_exactly_like_an_ordinary_bounce() {
        for offset in [-0.9_f32, -0.4, 0.4, 0.9] {
            let mut held = about_to_be_caught();
            step_fixed(&mut held);
            held.balls[0].pos.x = held.paddle.center_x() + held.paddle.w / 2.0 * offset;
            held.release_held_balls();

            // The same strike, bounced rather than caught.
            let mut hit = about_to_be_caught();
            hit.magnet_left = 0.0;
            hit.balls[0].pos.x = hit.paddle.center_x() + hit.paddle.w / 2.0 * offset;
            step_fixed(&mut hit);

            let d = (held.balls[0].vel - hit.balls[0].vel).length();
            assert!(
                d < 1.0,
                "offset {offset}: released {:?} but bounced {:?}",
                held.balls[0].vel,
                hit.balls[0].vel
            );
        }
    }

    /// ⚠️ **The stall the probe found.** A held ball is pinned to the
    /// paddle's centre, so a player who simply keeps the paddle under the
    /// ball releases at offset exactly 0.0 — and the steering formula
    /// returns a DEAD VERTICAL ball there. `launch` has always avoided
    /// straight up because a vertical ball in a brick corridor bounces
    /// forever; the magnet reintroduced it by a different door.
    ///
    /// Measured before the fix: `probe_balance` timed out on eight of ten
    /// levels with one ball at `vel=(0, ±340)` for 216,000 ticks.
    #[test]
    fn a_dead_centre_release_is_never_vertical() {
        for dir in [-1.0_f32, 0.0, 1.0] {
            let mut s = about_to_be_caught();
            step_fixed(&mut s);
            assert!(s.balls[0].is_held());

            // Exactly centred, which is where the magnet always leaves it.
            s.balls[0].pos.x = s.paddle.center_x();
            s.paddle.dir = dir;
            s.release_held_balls();

            let v = s.balls[0].vel;
            assert!(
                v.x.abs() > 1.0,
                "dir {dir}: released dead vertical ({v:?}) — this stalls the game"
            );
            assert!(v.y < 0.0, "dir {dir}: must still travel upward");
            assert!(
                (v.length() - s.ball_speed()).abs() < 1.0,
                "dir {dir}: speed changed to {}",
                v.length()
            );
        }
    }

    /// A paddle moving one way leans the release that way — the player's
    /// last input is the best guess at the aim they wanted.
    #[test]
    fn a_moving_paddle_leans_the_release_its_own_way() {
        for (dir, want) in [(-1.0_f32, -1.0_f32), (1.0, 1.0)] {
            let mut s = about_to_be_caught();
            step_fixed(&mut s);
            s.balls[0].pos.x = s.paddle.center_x();
            s.paddle.dir = dir;
            s.release_held_balls();
            assert!(
                s.balls[0].vel.x * want > 0.0,
                "paddle moving {dir} should lean the ball {want}, got {:?}",
                s.balls[0].vel
            );
        }
    }

    /// ⚠️ The nudge must be small enough that it does not become the aim.
    /// A player who lines up an edge shot still gets their edge shot.
    #[test]
    fn the_anti_vertical_nudge_does_not_override_a_real_aim() {
        let mut s = about_to_be_caught();
        step_fixed(&mut s);
        // A deliberate hard-right aim, with the paddle moving LEFT.
        s.balls[0].pos.x = s.paddle.center_x() + s.paddle.w * 0.45;
        s.paddle.dir = -1.0;
        s.release_held_balls();
        assert!(
            s.balls[0].vel.x > 0.0,
            "the nudge overrode a deliberate aim: {:?}",
            s.balls[0].vel
        );
    }

    /// Releasing with nothing held is harmless — which is every Space
    /// release in a game without a magnet.
    #[test]
    fn releasing_with_nothing_held_does_nothing() {
        let mut s = GameState::new();
        s.launch();
        let before = s.balls[0].vel;
        assert!(!s.release_held_balls(), "reported firing an unheld ball");
        assert_eq!(s.balls[0].vel, before);
    }

    /// Space in `Ready` still launches — the ordinary case must survive the
    /// new meaning.
    #[test]
    fn space_still_launches_a_resting_ball() {
        let mut s = GameState::new();
        assert_eq!(s.phase, Phase::Ready);
        s.launch();
        assert_eq!(s.phase, Phase::Playing);
        assert!(s.balls[0].vel.y < 0.0);
    }

    // ---- the Omarchy item: spawn_ball's first caller ----

    /// ⚠️ **The whole point of S6.** `spawn_ball` has existed since S3 with
    /// no caller, which is exactly why multi-ball was unreachable in play.
    #[test]
    fn the_omarchy_item_puts_another_ball_on_the_field() {
        let mut s = GameState::new();
        s.launch();
        assert_eq!(s.balls.len(), 1);
        s.apply_item(ItemKind::Omarchy);
        assert_eq!(s.balls.len(), 2, "the Omarchy item added no ball");
    }

    /// A spawned ball must be moving, upward, at the level's speed — never
    /// inheriting a held parent's zero velocity.
    #[test]
    fn a_spawned_ball_is_live_and_heading_upward() {
        let mut s = about_to_be_caught();
        step_fixed(&mut s);
        assert!(s.balls[0].is_held(), "parent should be held for this test");

        s.apply_item(ItemKind::Omarchy);
        assert_eq!(s.balls.len(), 2);
        let spawned = &s.balls[1];
        assert!(!spawned.is_held(), "a spawned ball must not be born held");
        assert!(spawned.vel.y < 0.0, "it must travel upward");
        assert!(
            (spawned.vel.length() - s.ball_speed()).abs() < 1.0,
            "spawned at {}, expected {}",
            spawned.vel.length(),
            s.ball_speed()
        );
    }

    /// It mirrors its parent rather than copying it: two balls on the same
    /// vector are one ball as far as the player's eye is concerned, and
    /// searching more of the field at once is the item's whole purpose.
    #[test]
    fn a_spawned_ball_does_not_share_its_parents_heading() {
        let mut s = GameState::new();
        s.launch();
        s.balls[0].vel = Vec2::new(1.0, -1.0).with_length(s.ball_speed());
        s.apply_item(ItemKind::Omarchy);
        assert_eq!(s.balls.len(), 2);
        assert!(
            s.balls[1].vel.x * s.balls[0].vel.x < 0.0,
            "expected mirrored horizontals, got {:?} and {:?}",
            s.balls[0].vel,
            s.balls[1].vel
        );
    }

    /// The cap already built at S3 finally has something pushing against
    /// it. Five on the early levels, ten from level six.
    #[test]
    fn the_omarchy_item_respects_the_ball_cap() {
        for (level, cap) in [(1_u32, 5_usize), (LEVEL_CAP_STEP, 10)] {
            let mut s = GameState::new();
            s.level = level;
            s.launch();
            for _ in 0..40 {
                s.apply_item(ItemKind::Omarchy);
            }
            assert_eq!(
                s.balls.len(),
                cap,
                "level {level} should cap at {cap} balls"
            );
        }
    }

    /// A refused spawn reports itself, so no pickup sound plays for a ball
    /// that never appeared.
    #[test]
    fn a_refused_spawn_says_so() {
        let mut s = GameState::new();
        s.launch();
        let mut refusals = 0;
        for _ in 0..40 {
            if !s.spawn_extra_ball() {
                refusals += 1;
            }
        }
        assert!(refusals > 0, "the cap never refused a spawn");
    }

    // ---- the axes stay separate ----

    /// ⚠️ A caught bomb must not cancel a magnet the player is still using.
    /// Grow and bomb share one axis because they contradict each other; the
    /// magnet contradicts neither.
    #[test]
    fn a_bomb_does_not_clear_the_magnet() {
        let mut s = GameState::new();
        s.launch();
        s.apply_item(ItemKind::Magnet);
        let magnet = s.magnet_left;
        s.apply_item(ItemKind::Bomb);
        assert!((s.magnet_left - magnet).abs() < 1e-6, "the bomb cleared the magnet");
        assert!(s.paddle.w < PADDLE_W, "the bomb should still have shrunk the paddle");
    }

    /// And the reverse: catching a magnet must not disturb a running grow.
    #[test]
    fn the_magnet_does_not_disturb_a_grow() {
        let mut s = GameState::new();
        s.launch();
        s.apply_item(ItemKind::Grow(Strength::Large));
        let (w, left) = (s.paddle.w, s.paddle_effect_left);
        s.apply_item(ItemKind::Magnet);
        assert_eq!(s.paddle.w, w, "the magnet resized the paddle");
        assert!((s.paddle_effect_left - left).abs() < 1e-6, "the magnet ate the grow timer");
    }

    /// ⚠️ S5's rule extended: a 20 s ability must not leak into the next
    /// level, the same way last level's bomb must not.
    #[test]
    fn advancing_a_level_clears_the_magnet() {
        let mut s = GameState::new();
        s.launch();
        s.apply_item(ItemKind::Magnet);
        s.advance_level();
        s.skip_clear();
        assert!(!s.magnet_active(), "the magnet followed the player to the next level");
        assert_eq!(s.balls.len(), 1, "the next level starts with one ball");
        assert!(!s.balls[0].is_held(), "a held ball survived the level change");
    }

    /// A long run with the magnet armed throughout stays sane — no NaN, no
    /// ball off-field, no negative lives.
    #[test]
    fn a_long_run_with_the_magnet_stays_sane() {
        let mut s = GameState::new();
        s.launch();
        s.apply_item(ItemKind::Magnet);
        for i in 0..40_000 {
            if i % 500 == 0 {
                s.apply_item(ItemKind::Magnet);
            }
            if i % 900 == 0 {
                s.apply_item(ItemKind::Omarchy);
            }
            if i % 137 == 0 {
                s.paddle.dir = if (i / 137) % 2 == 0 { 1.0 } else { -1.0 };
            }
            step_fixed(&mut s);
            for b in &s.balls {
                assert!(b.pos.x.is_finite() && b.pos.y.is_finite(), "NaN at tick {i}");
                assert!(b.vel.x.is_finite() && b.vel.y.is_finite(), "NaN velocity at {i}");
            }
            assert!(s.balls.len() <= s.ball_cap(), "over the cap at tick {i}");
        }
    }
}

/// Chips, damage feedback and shake, where they meet the running game.
#[cfg(test)]
mod effect_tests {
    use super::*;
    use crate::effects::{SHAKE_UNITS, SHATTER_CHIPS};
    use crate::state::{Tier, BALL_RADIUS};
    use omarcade_core::Color;

    fn playing() -> GameState {
        let mut s = GameState::new();
        s.launch();
        s
    }

    /// Put the ball on a brick and let the tick resolve the hit.
    fn strike_first_brick(s: &mut GameState) -> usize {
        let target = s.bricks.iter().position(|b| b.alive()).expect("a brick");
        let r = s.bricks[target].rect;
        s.balls[0].pos = Vec2::new(r.center().x, r.bottom() + BALL_RADIUS - 1.0);
        s.balls[0].vel = Vec2::new(0.0, -s.ball_speed());
        step_fixed(s);
        target
    }

    #[test]
    fn breaking_a_brick_throws_chips() {
        let mut s = playing();
        assert!(s.chips.is_empty(), "the pool should start empty");
        strike_first_brick(&mut s);
        assert!(!s.chips.is_empty(), "a broken brick threw nothing");
    }

    /// ⚠️ Damage and shatter are ONE system, partial. A reinforced brick's
    /// first hit throws a smaller burst rather than getting an effect of
    /// its own — that is what keeps it cheap, and it answers S4's note that
    /// the damage shrink reads too subtly on its own.
    #[test]
    fn a_surviving_brick_throws_fewer_chips_than_a_destroyed_one() {
        let mut destroyed = playing();
        strike_first_brick(&mut destroyed);
        let full = destroyed.chips.len();

        let mut survives = playing();
        let target = survives.bricks.iter().position(|b| b.alive()).unwrap();
        survives.bricks[target].tier = Tier::Armoured;
        survives.bricks[target].hits = Tier::Armoured.hits();
        strike_first_brick(&mut survives);

        assert!(survives.bricks[target].alive(), "the brick should have survived");
        assert!(
            survives.chips.len() < full,
            "a survived hit threw {} chips, a break threw {full}",
            survives.chips.len()
        );
        assert!(!survives.chips.is_empty(), "a survived hit must still show something");
        assert_eq!(full, SHATTER_CHIPS);
    }

    /// ⚠️ Chips are made OF the brick. A chip in some other colour is a
    /// generic puff, which the plan explicitly does not want.
    #[test]
    fn chips_take_the_bricks_own_colour() {
        let mut s = playing();
        s.palette = [
            Color::rgb(1, 0, 0),
            Color::rgb(2, 0, 0),
            Color::rgb(3, 0, 0),
            Color::rgb(4, 0, 0),
            Color::rgb(5, 0, 0),
            Color::rgb(6, 0, 0),
        ];
        let target = s.bricks.iter().position(|b| b.alive()).unwrap();
        let want = s.palette[s.bricks[target].color_index % s.palette.len()];
        strike_first_brick(&mut s);
        for p in s.chips.particles() {
            assert_eq!(p.color, want, "a chip did not take the brick's colour");
        }
    }

    // ---- shake ----

    /// ⚠️ Shake on an ARMOURED break only. Shaking on every brick would
    /// wobble the whole game.
    #[test]
    fn an_ordinary_break_does_not_shake() {
        let mut s = playing();
        strike_first_brick(&mut s);
        assert!(s.shake.is_still(), "a plain brick shook the screen");
    }

    #[test]
    fn an_armoured_break_shakes() {
        let mut s = playing();
        let target = s.bricks.iter().position(|b| b.alive()).unwrap();
        s.bricks[target].tier = Tier::Armoured;
        s.bricks[target].hits = 1; // one hit from breaking
        strike_first_brick(&mut s);
        assert!(!s.bricks[target].alive(), "the brick should have broken");
        assert!(!s.shake.is_still(), "an armoured break did not shake");
    }

    /// The plan asks for shake on a lost ball too — the case where it is
    /// alone, with nothing else happening to soften it.
    #[test]
    fn losing_a_ball_shakes() {
        let mut s = playing();
        s.shake.clear();
        s.lose_life();
        assert!(!s.shake.is_still(), "a lost ball did not shake");
    }

    /// ⚠️ **Shake moves the DRAWING, never the world.** A ball colliding
    /// with something it does not visually touch would be a real bug
    /// wearing an effect's clothes. Physics must be identical with a shake
    /// running and with none.
    #[test]
    fn shake_does_not_move_the_world() {
        let run = |shaking: bool| {
            let mut s = playing();
            s.balls[0].pos = Vec2::new(300.0, 400.0);
            s.balls[0].vel = Vec2::new(120.0, -260.0);
            if shaking {
                s.shake.add(SHAKE_UNITS * 4.0);
            } else {
                s.shake.clear();
            }
            for _ in 0..2000 {
                step_fixed(&mut s);
            }
            (
                s.balls.iter().map(|b| (b.pos, b.vel)).collect::<Vec<_>>(),
                s.score,
                s.lives,
            )
        };
        assert_eq!(run(true), run(false), "shake changed the simulation");
    }

    // ---- clearing ----

    /// ⚠️ S5's rule, extended again: last level's debris must not still be
    /// falling through the next one.
    #[test]
    fn advancing_a_level_clears_chips_and_shake() {
        let mut s = playing();
        strike_first_brick(&mut s);
        s.shake.add(SHAKE_UNITS);
        assert!(!s.chips.is_empty());

        s.advance_level();
        s.skip_clear();
        assert!(s.chips.is_empty(), "chips followed the player to the next level");
        assert!(s.shake.is_still(), "shake followed the player to the next level");
    }

    #[test]
    fn restarting_clears_chips_and_shake() {
        let mut s = playing();
        strike_first_brick(&mut s);
        s.shake.add(SHAKE_UNITS);
        s.restart();
        assert!(s.chips.is_empty());
        assert!(s.shake.is_still());
    }

    /// Chips must actually retire — a pool that only fills is a leak that
    /// ends with the newest break evicting the one before it.
    #[test]
    fn chips_expire_on_their_own() {
        let mut s = playing();
        strike_first_brick(&mut s);
        assert!(!s.chips.is_empty());
        let ticks = ((crate::effects::CHIP_LIFE * 3.0) / FIXED_DT).ceil() as u32;
        for _ in 0..ticks {
            step_fixed(&mut s);
        }
        assert!(s.chips.is_empty(), "{} chips outlived their life", s.chips.len());
    }

    // ---- ⚠️ effects must decay in EVERY phase ----

    /// ⚠️ **The bug real play found.** `lose_life` raises the shake and
    /// moves the game to `Ready`. With the effects ticked inside the
    /// `Playing` arm, the shake was set and then never decayed — it kept
    /// shaking until the player launched again, which could be forever.
    ///
    /// Every effects test before this one was written against a game
    /// already in `Playing`, so twenty of them passed while the one phase
    /// a player actually sits in was broken.
    #[test]
    fn shake_decays_while_waiting_to_launch() {
        let mut s = playing();
        s.lose_life();
        assert_eq!(s.phase, Phase::Ready, "losing a ball should leave us waiting");
        assert!(!s.shake.is_still(), "the lost ball should have shaken");

        for _ in 0..240 {
            step_fixed(&mut s);
        }
        assert_eq!(s.phase, Phase::Ready, "the test must stay in Ready");
        assert!(
            s.shake.is_still(),
            "shake still running after a second in Ready: {} — it must be a timed \
             event, not one that waits for the next launch",
            s.shake.amount
        );
    }

    /// The same for the two terminal phases: a game over must settle, not
    /// sit there shaking.
    #[test]
    fn shake_decays_after_the_game_ends() {
        for phase in [Phase::Lost, Phase::Won] {
            let mut s = playing();
            s.shake.add(SHAKE_UNITS);
            s.phase = phase;
            for _ in 0..240 {
                step_fixed(&mut s);
            }
            assert!(s.shake.is_still(), "{phase:?} kept shaking: {}", s.shake.amount);
        }
    }

    /// Chips had the same bug — they froze mid-air on the Ready screen
    /// rather than falling and fading.
    #[test]
    fn chips_keep_falling_while_waiting_to_launch() {
        let mut s = playing();
        strike_first_brick(&mut s);
        assert!(!s.chips.is_empty());

        // Drop into Ready the way a lost ball does.
        s.lose_life();
        assert_eq!(s.phase, Phase::Ready);
        // ⚠️ lose_life goes through rest_ball_on_paddle, not
        // clear_level_effects, so the chips SHOULD still be there and
        // should still be moving.
        let before: Vec<_> = s.chips.particles().iter().map(|p| p.pos).collect();
        assert!(!before.is_empty(), "a lost ball should not wipe the chips");

        for _ in 0..12 {
            step_fixed(&mut s);
        }
        let after: Vec<_> = s.chips.particles().iter().map(|p| p.pos).collect();
        assert_ne!(before, after, "chips froze in mid-air while waiting to launch");
    }

    #[test]
    fn chips_expire_while_waiting_to_launch() {
        let mut s = playing();
        strike_first_brick(&mut s);
        s.lose_life();
        assert_eq!(s.phase, Phase::Ready);

        let ticks = ((crate::effects::CHIP_LIFE * 3.0) / FIXED_DT).ceil() as u32;
        for _ in 0..ticks {
            step_fixed(&mut s);
        }
        assert!(s.chips.is_empty(), "{} chips outlived their life in Ready", s.chips.len());
    }

    /// A long, busy run stays sane: no NaN, no unbounded pool, no panic.
    #[test]
    fn a_long_run_with_effects_stays_sane() {
        let mut s = playing();
        s.apply_item(crate::items::ItemKind::Magnet);
        for i in 0..60_000 {
            if i % 900 == 0 {
                s.apply_item(crate::items::ItemKind::Omarchy);
            }
            if i % 137 == 0 {
                s.paddle.dir = if (i / 137) % 2 == 0 { 1.0 } else { -1.0 };
            }
            step_fixed(&mut s);
            assert!(
                s.chips.len() <= crate::effects::POOL_CAPACITY,
                "pool overran its capacity at tick {i}"
            );
            for p in s.chips.particles() {
                assert!(p.pos.x.is_finite() && p.pos.y.is_finite(), "NaN chip at tick {i}");
            }
            assert!(s.shake.amount.is_finite(), "NaN shake at tick {i}");
        }
    }
}

/// The level-clear cascade and the speed-reactive trail.
#[cfg(test)]
mod cascade_tests {
    use super::*;
    use crate::state::{
        trail_alpha_for, trail_len_for, BALL_SPEED, BALL_SPEED_TOP, CLEAR_BUILD_SECONDS,
        CLEAR_WAVE_SECONDS, LEVELS, TRAIL_ALPHA, TRAIL_ALPHA_TOP, TRAIL_LEN, TRAIL_LEN_TOP,
    };

    fn cleared() -> GameState {
        let mut s = GameState::new();
        s.launch();
        for b in &mut s.bricks {
            b.hits = 0;
        }
        check_win(&mut s);
        s
    }

    // ---- the cascade ----

    #[test]
    fn clearing_a_field_begins_the_cascade_rather_than_advancing() {
        let s = cleared();
        assert_eq!(s.phase, Phase::Clearing);
        assert_eq!(s.level, 1, "the level must not arrive until the cascade ends");
        assert!(s.clear.is_some());
    }

    /// ⚠️ No ball may survive into the cascade. One still bouncing around
    /// an empty field would be the only thing on screen not participating
    /// in the effect — and it could drain, costing a life for a level the
    /// player has already beaten.
    #[test]
    fn the_cascade_takes_the_balls_off_the_field() {
        let s = cleared();
        assert!(s.balls.is_empty(), "a ball survived into the cascade");
        assert!(s.items.is_empty(), "an item survived into the cascade");
    }

    #[test]
    fn the_cascade_cannot_cost_a_life() {
        let mut s = cleared();
        let lives = s.lives;
        s.skip_clear();
        assert_eq!(s.lives, lives, "the cascade took a life");
    }

    /// ⚠️ **The wave has to throw something.** There are no bricks left —
    /// the player just destroyed the last one — so a cascade built from
    /// live bricks would sweep an empty field and be invisible. It
    /// compiles, it runs, and nothing happens.
    #[test]
    fn the_wave_actually_throws_chips() {
        let mut s = cleared();
        s.chips.clear();
        assert!(s.bricks_remaining() == 0, "there should be no bricks to burst");

        let ticks = (CLEAR_WAVE_SECONDS / FIXED_DT) as u32;
        for _ in 0..ticks {
            step_fixed(&mut s);
        }
        assert!(!s.chips.is_empty(), "the wave threw nothing — it is invisible");
    }

    /// It is a WAVE, not one explosion: the top of the field goes before
    /// the bottom.
    #[test]
    fn the_wave_sweeps_downward() {
        let mut s = cleared();
        s.chips.clear();

        // A quarter of the way through, only the upper field has fired.
        let quarter = (CLEAR_WAVE_SECONDS * 0.25 / FIXED_DT) as u32;
        for _ in 0..quarter {
            step_fixed(&mut s);
        }
        let early_lowest = s
            .chips
            .particles()
            .iter()
            .map(|p| p.pos.y)
            .fold(f32::NEG_INFINITY, f32::max);

        for _ in 0..(CLEAR_WAVE_SECONDS / FIXED_DT) as u32 {
            step_fixed(&mut s);
        }
        let late_lowest = s
            .chips
            .particles()
            .iter()
            .map(|p| p.pos.y)
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(
            late_lowest > early_lowest,
            "the wave did not travel down the field: {early_lowest} then {late_lowest}"
        );
    }

    /// ⚠️ The wave must fit in the pool. Sixty cells at a full burst is
    /// 1080 particles into a pool of 512 — the early rows would be
    /// recycled away before the wave reached the bottom, so the cascade
    /// would eat its own head.
    #[test]
    fn the_whole_wave_fits_in_the_pool() {
        let mut s = cleared();
        let ticks = (CLEAR_WAVE_SECONDS / FIXED_DT) as u32;
        let mut peak = 0;
        for _ in 0..ticks {
            step_fixed(&mut s);
            peak = peak.max(s.chips.len());
        }
        assert!(
            peak < crate::effects::POOL_CAPACITY,
            "the wave peaked at {peak} of {} — it is recycling itself",
            crate::effects::POOL_CAPACITY
        );
    }

    #[test]
    fn the_cascade_finishes_and_hands_off_to_ready() {
        let mut s = cleared();
        let ticks = ((CLEAR_WAVE_SECONDS + CLEAR_BUILD_SECONDS) / FIXED_DT) as u32 + 10;
        for _ in 0..ticks {
            step_fixed(&mut s);
        }
        assert_eq!(s.phase, Phase::Ready, "the cascade never ended");
        assert_eq!(s.level, 2, "the level should have advanced exactly once");
        assert!(s.clear.is_none());
        assert!(s.bricks_remaining() > 0, "the next field should be built");
        assert_eq!(s.balls.len(), 1, "and a ball waiting on the paddle");
        assert!(s.just_advanced, "the LEVEL n banner should be armed");
    }

    /// ⚠️ **Exactly once.** The cascade increments the level at the seam
    /// between its two stages and again when it finishes; the second must
    /// undo the first. Off by one here and a ten-level game is five.
    #[test]
    fn the_cascade_advances_the_level_exactly_once() {
        let mut s = cleared();
        for _ in 0..5000 {
            step_fixed(&mut s);
            if s.phase == Phase::Ready {
                break;
            }
        }
        assert_eq!(s.level, 2);
    }

    /// The whole thing stays under the plan's "brief — under a second".
    #[test]
    fn the_cascade_is_brief() {
        assert!(
            CLEAR_WAVE_SECONDS + CLEAR_BUILD_SECONDS <= 1.0,
            "the plan says the cascade is under a second"
        );
    }

    /// ⚠️ Clearing the LAST level wins; it must not cascade into a level
    /// that does not exist.
    #[test]
    fn clearing_the_last_level_wins_rather_than_cascading() {
        let mut s = GameState::new();
        s.level = LEVELS;
        s.launch();
        for b in &mut s.bricks {
            b.hits = 0;
        }
        check_win(&mut s);
        assert_eq!(s.phase, Phase::Won, "the last level should win, not cascade");
        assert!(s.clear.is_none());
    }

    /// A full run still reaches the end with the cascade in the way.
    #[test]
    fn a_full_run_still_reaches_the_win() {
        let mut s = GameState::new();
        s.launch();
        for _ in 0..LEVELS {
            for b in &mut s.bricks {
                b.hits = 0;
            }
            check_win(&mut s);
            s.skip_clear();
            if s.phase == Phase::Won {
                break;
            }
            s.launch();
        }
        assert_eq!(s.phase, Phase::Won, "a full run did not reach the win");
        assert_eq!(s.level, LEVELS);
    }

    /// Input is ignored during the cascade — a mashed Space must not skip
    /// the effect or launch into a field that is still building.
    #[test]
    fn space_does_nothing_during_the_cascade() {
        let mut s = cleared();
        for _ in 0..40 {
            s.launch();
            step_fixed(&mut s);
        }
        assert_eq!(s.phase, Phase::Clearing, "Space skipped the cascade");
        assert!(s.balls.is_empty(), "Space launched a ball mid-cascade");
    }

    // ---- the speed-reactive trail ----

    #[test]
    fn the_trail_lengthens_with_speed() {
        assert_eq!(trail_len_for(BALL_SPEED), TRAIL_LEN);
        assert_eq!(trail_len_for(BALL_SPEED_TOP), TRAIL_LEN_TOP);
        assert!(trail_len_for(BALL_SPEED_TOP) > trail_len_for(BALL_SPEED));
        // Monotone across the range, so it grows rather than stepping.
        let mid = trail_len_for((BALL_SPEED + BALL_SPEED_TOP) / 2.0);
        assert!(mid > TRAIL_LEN && mid < TRAIL_LEN_TOP, "midpoint was {mid}");
    }

    #[test]
    fn the_trail_brightens_with_speed() {
        assert!((trail_alpha_for(BALL_SPEED) - TRAIL_ALPHA).abs() < 1e-3);
        assert!((trail_alpha_for(BALL_SPEED_TOP) - TRAIL_ALPHA_TOP).abs() < 1e-3);
        assert!(trail_alpha_for(BALL_SPEED_TOP) > trail_alpha_for(BALL_SPEED));
    }

    /// ⚠️ Alpha is cast to u8 for drawing. Above 255 it would wrap and the
    /// brightest trail in the game would render as the dimmest.
    #[test]
    fn the_trail_never_overflows_its_alpha() {
        for speed in [0.0, BALL_SPEED, BALL_SPEED_TOP, 10_000.0] {
            let a = trail_alpha_for(speed);
            assert!((0.0..=255.0).contains(&a), "alpha {a} at speed {speed}");
        }
    }

    /// Out of range clamps rather than extrapolating — a speed below level
    /// one or above level ten must not produce a negative or runaway trail.
    #[test]
    fn trail_values_clamp_outside_the_speed_range() {
        assert_eq!(trail_len_for(0.0), TRAIL_LEN);
        assert_eq!(trail_len_for(99_999.0), TRAIL_LEN_TOP);
    }

    /// The running game actually keeps the longer trail at speed.
    #[test]
    fn a_fast_level_records_a_longer_trail_than_a_slow_one() {
        let sample = |level: u32| {
            let mut s = GameState::new();
            s.level = level;
            s.launch();
            for _ in 0..60 {
                step(&mut s, &mut Accumulator::default(), 1.0 / 60.0);
            }
            s.balls[0].trail.len()
        };
        assert!(
            sample(LEVELS) > sample(1),
            "level {LEVELS} trail was not longer than level 1's"
        );
    }
}
