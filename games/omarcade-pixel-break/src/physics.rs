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
use crate::state::{Ball, GameState, Paddle, Phase, TRAIL_LEN};

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

/// Steepest the ball may travel relative to horizontal, as |vy| / speed.
/// Below this the ball is skimming and the game stalls.
const MIN_VERTICAL_FRACTION: f32 = 0.25;

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

/// How much the paddle steers the ball: at the very edge, this fraction
/// of the outgoing velocity is horizontal.
const PADDLE_STEER: f32 = 0.75;

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
    for ball in &mut state.balls {
        ball.trail.insert(0, ball.pos);
        ball.trail.truncate(TRAIL_LEN);
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

    match state.phase {
        // Ball rides the paddle until launch.
        Phase::Ready => state.rest_ball_on_paddle(),
        Phase::Playing => {
            let field = state.field();
            let paddle = state.paddle;
            let ball_speed = state.ball_speed();

            for i in 0..state.balls.len() {
                move_ball(&mut state.balls[i]);
                collide_walls(&mut state.balls[i], &field);
                collide_paddle(&mut state.balls[i], &paddle, ball_speed);
                // Bricks need the whole state: a kill scores, and it must
                // be visible to every later ball in this same tick.
                collide_bricks(state, i);
            }

            // Draining is per ball; losing a life is per empty field.
            // Both happen only once every ball has had its tick.
            state.retire_drained_balls();
            move_items(state);
            state.tick_paddle_effect(FIXED_DT);
            check_win(state);
        }
        Phase::Lost | Phase::Won => {}
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

fn collide_paddle(ball: &mut Ball, paddle: &Paddle, speed: f32) {
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
    bounce_off_paddle(ball, paddle, speed);
}

/// Where the ball strikes the paddle sets the outgoing angle.
///
/// This is the mechanic that makes Breakout a game of skill rather than
/// a screensaver: hitting with the paddle's edge steers the ball.
fn bounce_off_paddle(ball: &mut Ball, paddle: &Paddle, speed: f32) {
    // -1 at the left edge, 0 at the centre, +1 at the right edge.
    let offset = ((ball.pos.x - paddle.center_x()) / (paddle.w / 2.0)).clamp(-1.0, 1.0);

    let vx = offset * PADDLE_STEER;
    // Always upward, and always steep enough to keep the game moving.
    let vy = -(1.0 - vx.abs() * vx.abs()).max(MIN_VERTICAL_FRACTION).sqrt();

    ball.vel = Vec2::new(vx, vy).with_length(speed);
    clamp_angle(&mut ball.vel);
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
fn clamp_angle(vel: &mut Vec2) {
    let speed = vel.length();
    if speed == 0.0 {
        return;
    }
    let min_vy = speed * MIN_VERTICAL_FRACTION;
    if vel.y.abs() < min_vy {
        // Solve for BOTH components rather than setting vy and
        // renormalizing: raising vy alone makes the vector longer, so
        // the renormalize scales vy straight back down below target.
        // vy is fixed at the minimum; vx takes whatever speed is left.
        let sign_y = if vel.y < 0.0 { -1.0 } else { 1.0 };
        let sign_x = if vel.x < 0.0 { -1.0 } else { 1.0 };
        let vy = min_vy;
        let vx = (speed * speed - vy * vy).max(0.0).sqrt();
        *vel = Vec2::new(sign_x * vx, sign_y * vy);
    }
}

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
        collide_paddle(&mut s.balls[0], &paddle, speed);
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
        collide_paddle(&mut s.balls[0], &paddle, speed);
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
        collide_paddle(&mut s.balls[0], &paddle, speed);
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
        collide_paddle(&mut s.balls[0], &paddle, speed);
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
        }
        assert_eq!(s.level, LEVELS);

        for b in &mut s.bricks {
            b.hits = 0;
        }
        s.advance_level();
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
                collide_paddle(&mut s.balls[0], &paddle, speed);

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
            collide_paddle(&mut s.balls[0], &paddle, speed);
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

