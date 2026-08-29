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

use crate::geom::{Axis, Vec2};
use crate::state::{GameState, Phase, BALL_SPEED, PADDLE_SPEED};

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
}

/// One fixed tick.
pub fn step_fixed(state: &mut GameState) {
    move_paddle(state);

    match state.phase {
        // Ball rides the paddle until launch.
        Phase::Ready => state.rest_ball_on_paddle(),
        Phase::Playing => {
            move_ball(state);
            collide_walls(state);
            collide_paddle(state);
            collide_bricks(state);
            check_win(state);
        }
        Phase::Lost | Phase::Won => {}
    }
}

fn move_paddle(state: &mut GameState) {
    let p = &mut state.paddle;
    p.x += p.dir * PADDLE_SPEED * FIXED_DT;
    // Clamp inside the field; the paddle never leaves the play area.
    p.x = p.x.clamp(0.0, crate::state::FIELD_W - p.w);
}

fn move_ball(state: &mut GameState) {
    state.ball.pos += state.ball.vel * FIXED_DT;
}

/// Bounce off the side and top walls; falling past the bottom loses a
/// life.
fn collide_walls(state: &mut GameState) {
    let field = state.field();
    let r = state.ball.radius;
    let b = &mut state.ball;

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
        state.lose_life();
    }
}

fn collide_paddle(state: &mut GameState) {
    // Only when moving downward. A ball on its way up that clips the
    // paddle from below should pass, not get batted back down.
    if state.ball.vel.y <= 0.0 {
        return;
    }

    let paddle = state.paddle.rect();
    if !state.ball.rect().overlaps(&paddle) {
        return;
    }

    // Sit the ball on top of the paddle so it cannot re-collide.
    state.ball.pos.y = paddle.top() - state.ball.radius - 0.01;
    bounce_off_paddle(state);
}

/// Where the ball strikes the paddle sets the outgoing angle.
///
/// This is the mechanic that makes Breakout a game of skill rather than
/// a screensaver: hitting with the paddle's edge steers the ball.
fn bounce_off_paddle(state: &mut GameState) {
    let paddle = &state.paddle;
    // -1 at the left edge, 0 at the centre, +1 at the right edge.
    let offset = ((state.ball.pos.x - paddle.center_x()) / (paddle.w / 2.0)).clamp(-1.0, 1.0);

    let vx = offset * PADDLE_STEER;
    // Always upward, and always steep enough to keep the game moving.
    let vy = -(1.0 - vx.abs() * vx.abs()).max(MIN_VERTICAL_FRACTION).sqrt();

    state.ball.vel = Vec2::new(vx, vy).with_length(BALL_SPEED);
    clamp_angle(&mut state.ball.vel);
}

/// Resolve the single deepest brick collision this tick.
///
/// Deliberately not "every overlapping brick": a ball touching two
/// bricks would reflect twice and reverse into the direction it came
/// from. One collision per tick, and at 240Hz the next tick handles any
/// remaining overlap.
fn collide_bricks(state: &mut GameState) {
    let ball_rect = state.ball.rect();

    let mut best: Option<(usize, f32, Axis)> = None;
    for (i, brick) in state.bricks.iter().enumerate() {
        if !brick.alive {
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
                        if state.ball.vel.x >= 0.0 { cand.x > cur.x } else { cand.x < cur.x }
                    }
                    Axis::Y => {
                        if state.ball.vel.y >= 0.0 { cand.y > cur.y } else { cand.y < cur.y }
                    }
                }
            }
            _ => false,
        };
        if better {
            best = Some((i, depth, axis));
        }
    }

    let Some((index, depth, axis)) = best else {
        return;
    };

    let brick_rect = state.bricks[index].rect;
    state.bricks[index].alive = false;
    state.score += 10;

    // Push out along the collision axis, then reflect that component.
    match axis {
        Axis::X => {
            if state.ball.pos.x < brick_rect.center().x {
                state.ball.pos.x -= depth;
                state.ball.vel.x = -state.ball.vel.x.abs();
            } else {
                state.ball.pos.x += depth;
                state.ball.vel.x = state.ball.vel.x.abs();
            }
        }
        Axis::Y => {
            if state.ball.pos.y < brick_rect.center().y {
                state.ball.pos.y -= depth;
                state.ball.vel.y = -state.ball.vel.y.abs();
            } else {
                state.ball.pos.y += depth;
                state.ball.vel.y = state.ball.vel.y.abs();
            }
        }
    }

    clamp_angle(&mut state.ball.vel);
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

fn check_win(state: &mut GameState) {
    if state.bricks_remaining() == 0 {
        state.phase = Phase::Won;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{BALL_RADIUS, FIELD_H, FIELD_W};

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
        s.ball.pos = Vec2::new(BALL_RADIUS - 1.0, 300.0);
        s.ball.vel = Vec2::new(-100.0, -100.0);
        collide_walls(&mut s);
        assert!(s.ball.vel.x > 0.0, "should reflect rightward");
        assert!(s.ball.pos.x >= BALL_RADIUS);
    }

    #[test]
    fn ball_bounces_off_the_ceiling() {
        let mut s = playing();
        s.ball.pos = Vec2::new(400.0, BALL_RADIUS - 1.0);
        s.ball.vel = Vec2::new(50.0, -100.0);
        collide_walls(&mut s);
        assert!(s.ball.vel.y > 0.0, "should reflect downward");
    }

    #[test]
    fn falling_past_the_bottom_costs_a_life() {
        let mut s = playing();
        let lives = s.lives;
        s.ball.pos = Vec2::new(400.0, FIELD_H + 50.0);
        collide_walls(&mut s);
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
        s.ball.pos = Vec2::new(s.paddle.center_x() - 50.0, s.paddle.y - BALL_RADIUS + 1.0);
        s.ball.vel = Vec2::new(0.0, 300.0);
        collide_paddle(&mut s);
        assert!(s.ball.vel.x < 0.0, "vx = {}", s.ball.vel.x);
        assert!(s.ball.vel.y < 0.0, "must go up");
    }

    #[test]
    fn hitting_the_paddle_right_of_centre_sends_the_ball_right() {
        let mut s = playing();
        s.paddle.x = 400.0;
        s.ball.pos = Vec2::new(s.paddle.center_x() + 50.0, s.paddle.y - BALL_RADIUS + 1.0);
        s.ball.vel = Vec2::new(0.0, 300.0);
        collide_paddle(&mut s);
        assert!(s.ball.vel.x > 0.0);
        assert!(s.ball.vel.y < 0.0);
    }

    #[test]
    fn paddle_bounce_preserves_speed() {
        let mut s = playing();
        s.ball.pos = Vec2::new(s.paddle.center_x() + 20.0, s.paddle.y - BALL_RADIUS + 1.0);
        s.ball.vel = Vec2::new(10.0, 300.0);
        collide_paddle(&mut s);
        assert!((s.ball.vel.length() - BALL_SPEED).abs() < 0.5,
            "speed drifted to {}", s.ball.vel.length());
    }

    /// A ball travelling upward through the paddle must not be batted
    /// back down.
    #[test]
    fn upward_ball_passes_through_the_paddle() {
        let mut s = playing();
        s.ball.pos = Vec2::new(s.paddle.center_x(), s.paddle.y);
        s.ball.vel = Vec2::new(0.0, -300.0);
        let before = s.ball.vel;
        collide_paddle(&mut s);
        assert_eq!(s.ball.vel, before);
    }

    // ---- bricks ----

    #[test]
    fn hitting_a_brick_kills_it_and_scores() {
        let mut s = playing();
        let brick = s.bricks[0].rect;
        s.ball.pos = Vec2::new(brick.center().x, brick.bottom() + BALL_RADIUS - 2.0);
        s.ball.vel = Vec2::new(0.0, -BALL_SPEED);
        collide_bricks(&mut s);
        assert!(!s.bricks[0].alive);
        assert_eq!(s.score, 10);
        assert!(s.ball.vel.y > 0.0, "should bounce back downward");
    }

    /// Two bricks in one tick must produce ONE reflection, not two.
    #[test]
    fn overlapping_two_bricks_reflects_only_once() {
        let mut s = playing();
        // Sit the ball right where two adjacent bricks meet.
        let a = s.bricks[0].rect;
        let b = s.bricks[1].rect;
        let seam = (a.right() + b.left()) / 2.0;
        s.ball.pos = Vec2::new(seam, a.center().y);
        s.ball.vel = Vec2::new(BALL_SPEED, 0.0);
        let before = s.ball.vel;
        collide_bricks(&mut s);
        // Exactly one brick dies this tick.
        let dead = s.bricks.iter().filter(|k| !k.alive).count();
        assert_eq!(dead, 1, "one collision per tick");
        // And the velocity did not flip twice back to its original sign.
        assert_ne!(s.ball.vel.x.signum(), before.x.signum(),
            "a single reflection must change direction");
    }

    #[test]
    fn clearing_every_brick_wins() {
        let mut s = playing();
        for b in &mut s.bricks {
            b.alive = false;
        }
        check_win(&mut s);
        assert_eq!(s.phase, Phase::Won);
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
