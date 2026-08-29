//! Simulation: the only file that advances time.
//!
//! Breakout's three failure modes apply here too, one of them mirrored:
//!
//! 1. **Tunnelling.** A fast ball plus a stalled frame moves further in
//!    one step than the paddle is thick, and passes clean through with
//!    no frame ever seeing an overlap. Fixed timesteps bound per-tick
//!    movement instead.
//! 2. **Angle lock, at BOTH ends.** The ball has to stay inside a band.
//!    Too vertical and the point cannot resolve — it crawls between the
//!    top and bottom walls and never reaches a paddle. Too horizontal
//!    and the point is dull — a flat line across the field with no wall
//!    angles to read.
//!
//!    The flat end is the one that actually bites, and it is Breakout's
//!    near-vertical finding rotated a quarter turn: good tracking means
//!    striking dead-centre, dead-centre steers nothing, so the ball
//!    converges on whatever "straight" means for that paddle geometry.
//!    Measured here before the floor existed, |vy| collapsed to 10-16
//!    against a speed of 375 and stayed there for hundreds of returns.
//!    The finding transfers; only its axis does not.
//! 3. **Double resolution.** A ball caught between a paddle and a wall
//!    could reflect twice in one tick and come back out the way it went
//!    in. Walls resolve first and set the sign directly rather than
//!    negating, so a second bounce in the same tick cannot undo one.

use omarcade_core::ease;
use omarcade_core::geom::Vec2;

use crate::state::{GameState, Phase, Side, FIELD_H, FIELD_W, TRAIL_LEN};

/// Simulation rate, matching Breakout. High enough that per-tick
/// movement (~2 units at the fastest ramped ball) stays far smaller
/// than the 16-unit paddle, which is what makes tunnelling impossible
/// rather than merely unlikely.
pub const FIXED_DT: f32 = 1.0 / 240.0;

/// Most fixed steps one frame may run.
///
/// Without this, a stalled frame — window dragged, machine resumed from
/// suspend — asks for hundreds of steps, which makes the next frame
/// slower, which asks for more: the spiral of death. Past this limit we
/// drop the surplus and let the game run briefly slow instead of
/// freezing.
const MAX_STEPS_PER_FRAME: u32 = 8;

/// Shallowest the ball may travel relative to vertical, as |vx| / speed.
///
/// Below this the ball crawls up and down the field and the rally
/// cannot resolve.
///
/// 0.65 rather than something steeper, and the reason is crossing time.
/// At 0.35 the steepest legal shot took over four seconds to cross,
/// which gave a paddle time to travel the field twice — so every shot
/// was reachable and points could only ever come from an opponent's
/// mistake. The floor on the angle is also a floor on how long the
/// other player has to answer it.
const MIN_HORIZONTAL_FRACTION: f32 = 0.65;

/// Flattest the ball may travel, as |vy| / speed.
///
/// Breakout's ball trends near-VERTICAL because good tracking means
/// striking dead-centre every time; Pong's paddles are rotated a
/// quarter turn, so the same mechanic drives it near-HORIZONTAL
/// instead. Measured over a long rally between two perfect trackers,
/// |vy| collapsed to 10-16 against a speed of 375 — a flat line across
/// the field, with the wall-angle play that makes Pong interesting
/// gone.
///
/// A floor rather than a nudge: the ball is never allowed to be
/// flatter than this, so the degenerate state is unreachable instead of
/// merely unlikely. Deliberately small — this is a guard against a
/// flat line, not a minimum angle the player has to fight.
const MIN_VERTICAL_FRACTION: f32 = 0.12;

/// How much the paddle steers the ball: at the very edge, this fraction
/// of the outgoing velocity is vertical.
///
/// This is the mechanic that makes Pong a game of skill rather than a
/// screensaver — the same one Breakout uses, rotated a quarter turn.
/// Striking with the paddle's end throws the ball off at an angle the
/// opponent has to travel for.
const PADDLE_STEER: f32 = 0.72;

/// How many returns it takes for the rally ramp to reach its ceiling.
///
/// Long enough that a short exchange plays at the difficulty's stated
/// speed — the ramp is a reward for a long rally, not a tax on every
/// point.
const RAMP_RALLIES: f32 = 14.0;

/// Extra speed per return once the rally is past the ramp, in units
/// per second per return.
///
/// The deadlock breaker. A rally has to pass ~20 returns before this
/// adds meaningful speed and ~35 before it is decisive — well beyond an
/// ordinary exchange, so it never touches normal play, but it
/// guarantees no rally runs forever no matter how well both sides are
/// playing. Tuned by measurement: at 6 and at 14, matches between two
/// evenly-matched mediocre players still ran out the clock.
const OVERTIME_GAIN: f32 = 22.0;

/// Vertical component of a serve, as a fraction of speed. Enough that a
/// serve is not a free straight line, small enough to be returnable.
const SERVE_VY: f32 = 0.35;

/// Converts real elapsed time into a whole number of fixed steps.
///
/// Identical in shape to Breakout's. Kept per-game rather than promoted
/// to core for now: two copies is evidence, three would be a pattern
/// worth extracting, and the second consumer is where a premature
/// abstraction usually goes wrong.
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
    /// loses time over a run.
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

/// The ball's speed for the current rally.
///
/// Two stacked curves, exactly as designed: the difficulty sets the
/// starting speed and the ceiling, and rally length eases between them.
/// `out_quad` puts most of the gain early, so a rally builds pressure
/// while it is still short rather than saving it all for a length most
/// players never reach.
///
/// This is a use case Breakout never had, which makes it the honest
/// test of whether `ease` was written generic or merely written once.
pub fn rally_speed(state: &GameState) -> f32 {
    let base = state.difficulty.ball_speed();
    let ceiling = base * state.difficulty.ramp_ceiling();
    let t = ease::out_quad((state.rally as f32) / RAMP_RALLIES);
    let ramped = ease::lerp(base, ceiling, t);

    // Past the ceiling, keep climbing — slowly, and without limit.
    //
    // A flat ceiling is a stalemate machine. Measured with one: every
    // Easy match timed out, because Easy's ceiling sat below the speed
    // at which a shot can actually beat a paddle, so two competent
    // players rallied literally forever. A rally that has run 80
    // returns is no longer a rally, it is a deadlock, and the game has
    // to break it rather than wait.
    //
    // Linear and gentle, so it is invisible in a normal exchange and
    // decisive in a pathological one.
    let over = (state.rally as f32 - RAMP_RALLIES).max(0.0);
    ramped + over * OVERTIME_GAIN
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
/// Once per frame, not once per fixed tick: the simulation runs at
/// 240Hz and a trail sampled there would be four times denser than
/// intended and would change length with frame rate. Called only when
/// time actually advanced, so a stalled frame does not stack ten copies
/// of the same point.
fn record_trail(state: &mut GameState) {
    if state.phase != Phase::Playing {
        state.trail.clear();
        return;
    }
    state.trail.insert(0, state.ball.pos);
    state.trail.truncate(TRAIL_LEN);
}

/// One fixed tick.
pub fn step_fixed(state: &mut GameState) {
    match state.phase {
        // Paddles still move on the select screen and between points,
        // so the player can settle before the ball is live.
        Phase::Select => {}
        Phase::Serve => {
            move_paddles(state);
            state.park_ball();
        }
        Phase::Playing => {
            move_paddles(state);
            move_ball(state);
            collide_walls(state);
            collide_paddles(state);
            check_point(state);
        }
        Phase::Over { .. } => {}
    }
}

/// Launch the ball toward whoever is receiving.
///
/// The serve travels AWAY from the serving side, so the player who was
/// just scored on gets the ball played at them — the arcade convention,
/// and it hands the tempo back to whoever is behind.
pub fn serve(state: &mut GameState) {
    if state.phase != Phase::Serve {
        return;
    }
    let dir = state.serving.outward();
    // Alternate the vertical component by score so consecutive serves
    // are not identical, without reaching for randomness the headless
    // harnesses would have to seed.
    let total = state.score_left + state.score_right;
    let vy = if total % 2 == 0 { SERVE_VY } else { -SERVE_VY };

    state.ball.vel = Vec2::new(dir, vy).with_length(rally_speed(state));
    state.phase = Phase::Playing;
}

fn move_paddles(state: &mut GameState) {
    for side in [Side::Left, Side::Right] {
        let p = state.paddle_mut(side);
        p.y += p.dir * crate::state::PADDLE_SPEED * FIXED_DT;
        // Clamp inside the field; a paddle never leaves the play area.
        p.y = p.y.clamp(0.0, FIELD_H - p.h);
    }
}

fn move_ball(state: &mut GameState) {
    state.ball.pos += state.ball.vel * FIXED_DT;
}

/// Bounce off the top and bottom. The left and right edges are not
/// walls — they are how a point is scored.
fn collide_walls(state: &mut GameState) {
    let r = state.ball.radius;
    let b = &mut state.ball;

    if b.pos.y - r < 0.0 {
        b.pos.y = r;
        // Set the sign rather than negating: a ball that somehow ends a
        // tick still overlapping cannot flip back and forth.
        b.vel.y = b.vel.y.abs();
    } else if b.pos.y + r > FIELD_H {
        b.pos.y = FIELD_H - r;
        b.vel.y = -b.vel.y.abs();
    }
}

fn collide_paddles(state: &mut GameState) {
    for side in [Side::Left, Side::Right] {
        // Only when travelling toward that paddle's own wall. A ball
        // already heading away has been dealt with and must not be
        // batted a second time.
        let closing = match side {
            Side::Left => state.ball.vel.x < 0.0,
            Side::Right => state.ball.vel.x > 0.0,
        };
        if !closing {
            continue;
        }

        let paddle = state.paddle(side).rect();
        if !state.ball.rect().overlaps(&paddle) {
            continue;
        }

        bounce_off_paddle(state, side);
        // One paddle per tick. The ball cannot legitimately touch both.
        return;
    }
}

/// Where the ball strikes the paddle sets the outgoing angle.
fn bounce_off_paddle(state: &mut GameState, side: Side) {
    let paddle = *state.paddle(side);
    let r = state.ball.radius;

    // Sit the ball against the striking face so it cannot re-collide
    // on the next tick.
    state.ball.pos.x = match side {
        Side::Left => paddle.face_x(side) + r + 0.01,
        Side::Right => paddle.face_x(side) - r - 0.01,
    };

    // -1 at the top edge, 0 at the centre, +1 at the bottom edge.
    let offset =
        ((state.ball.pos.y - paddle.center_y()) / (paddle.h / 2.0)).clamp(-1.0, 1.0);

    let vy = offset * PADDLE_STEER;
    // Always outward, and always flat enough to actually cross.
    let vx = side.outward() * (1.0 - vy * vy).max(MIN_HORIZONTAL_FRACTION).sqrt();

    state.rally += 1;
    let speed = rally_speed(state);
    state.ball.vel = Vec2::new(vx, vy).with_length(speed);
    // A dead-centre strike steers nothing, which is how the ball flattens
    // out over a long rally. The clamp puts the floor back.
    clamp_angle(&mut state.ball.vel);
}

/// Hold the ball's angle inside the band where Pong is a game.
///
/// Two floors, guarding opposite degenerate states:
///
/// - **Too vertical** and the point cannot resolve: the ball crawls
///   between the top and bottom walls and never reaches a paddle.
/// - **Too horizontal** and the point is dull: a flat line across the
///   field, no wall angles, nothing to read. This is the one a perfect
///   tracker converges on, because dead-centre strikes steer nothing.
///
/// Both are enforced as floors on a rebuilt vector rather than as
/// nudges, so the ball cannot creep back toward either edge over a long
/// rally. Speed is preserved: a clamp that rebuilt one component
/// without the other would quietly make the ball faster on every
/// bounce.
fn clamp_angle(vel: &mut Vec2) {
    let speed = vel.length();
    if speed <= 0.0 {
        return;
    }

    // Signs are preserved throughout: this decides the ANGLE, never the
    // direction the ball is travelling.
    let sx = if vel.x >= 0.0 { 1.0 } else { -1.0 };
    let sy = if vel.y >= 0.0 { 1.0 } else { -1.0 };

    let mut fx = (vel.x / speed).abs();
    let mut fy = (vel.y / speed).abs();

    if fx < MIN_HORIZONTAL_FRACTION {
        fx = MIN_HORIZONTAL_FRACTION;
        fy = (1.0 - fx * fx).max(0.0).sqrt();
    } else if fy < MIN_VERTICAL_FRACTION {
        fy = MIN_VERTICAL_FRACTION;
        fx = (1.0 - fy * fy).max(0.0).sqrt();
    } else {
        return; // already inside the band
    }

    *vel = Vec2::new(sx * fx * speed, sy * fy * speed);
}

/// Did the ball leave the field past someone's wall?
fn check_point(state: &mut GameState) {
    let r = state.ball.radius;
    if state.ball.pos.x + r < 0.0 {
        // Past the left wall: the right side scores.
        state.award(Side::Right);
    } else if state.ball.pos.x - r > FIELD_W {
        state.award(Side::Left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{Difficulty, MATCH_POINT};

    fn served(difficulty: Difficulty) -> GameState {
        let mut s = GameState::with_difficulty(difficulty);
        s.begin();
        serve(&mut s);
        s
    }

    #[test]
    fn a_serve_travels_away_from_the_server() {
        let mut s = GameState::new();
        s.begin();
        s.serving = Side::Left;
        serve(&mut s);
        assert!(s.ball.vel.x > 0.0, "left serves toward the right");

        let mut s = GameState::new();
        s.begin();
        s.serving = Side::Right;
        serve(&mut s);
        assert!(s.ball.vel.x < 0.0, "right serves toward the left");
    }

    #[test]
    fn a_serve_starts_at_the_difficultys_speed() {
        for d in Difficulty::ALL {
            let s = served(d);
            let speed = s.ball.vel.length();
            assert!(
                (speed - d.ball_speed()).abs() < 1e-3,
                "{d:?}: served at {speed}, expected {}",
                d.ball_speed()
            );
        }
    }

    #[test]
    fn serving_only_works_from_the_serve_phase() {
        let mut s = GameState::new();
        s.begin();
        serve(&mut s);
        let v = s.ball.vel;
        serve(&mut s); // already Playing
        assert_eq!(s.ball.vel, v, "a second serve must not re-launch");
    }

    #[test]
    fn consecutive_serves_are_not_identical() {
        let mut a = GameState::new();
        a.begin();
        serve(&mut a);

        let mut b = GameState::new();
        b.begin();
        b.score_left = 1; // next point
        serve(&mut b);

        assert!(
            (a.ball.vel.y - b.ball.vel.y).abs() > 1e-3,
            "serves should alternate their angle"
        );
    }

    // ------------------------------------------------------------------
    // The ramp — two stacked curves
    // ------------------------------------------------------------------

    #[test]
    fn the_rally_ramp_climbs_from_the_base_to_the_ceiling() {
        let d = Difficulty::Normal;
        let mut s = GameState::with_difficulty(d);
        s.begin();

        assert!(
            (rally_speed(&s) - d.ball_speed()).abs() < 1e-3,
            "rally 0 must be the tier's stated speed"
        );

        // At the end of the ramp proper, before overtime contributes.
        s.rally = RAMP_RALLIES as u32;
        let top = rally_speed(&s);
        let expect = d.ball_speed() * d.ramp_ceiling();
        assert!(
            (top - expect).abs() < 1.0,
            "should reach the ceiling {expect}, got {top}"
        );
    }

    /// Past the ramp the ball keeps gaining, without limit.
    ///
    /// A flat ceiling is a stalemate machine: measured with one, every
    /// Easy match between two competent players ran out the clock,
    /// because the tier's top speed sat below the point where a shot
    /// can beat a paddle at all.
    #[test]
    fn overtime_keeps_a_deadlocked_rally_climbing() {
        let d = Difficulty::Easy;
        let mut s = GameState::with_difficulty(d);
        s.begin();

        let ceiling = d.ball_speed() * d.ramp_ceiling();
        s.rally = RAMP_RALLIES as u32 + 40;
        let overtime = rally_speed(&s);

        assert!(
            overtime > ceiling + 100.0,
            "a 54-return rally should be well past the ceiling {ceiling}, was {overtime}"
        );

        // And it must never stop climbing.
        s.rally += 20;
        assert!(rally_speed(&s) > overtime);
    }

    #[test]
    fn overtime_does_not_touch_an_ordinary_rally() {
        // The guard must be invisible in normal play.
        let d = Difficulty::Normal;
        let mut s = GameState::with_difficulty(d);
        s.begin();
        s.rally = 8; // a good, ordinary exchange
        let speed = rally_speed(&s);
        let ceiling = d.ball_speed() * d.ramp_ceiling();
        assert!(
            speed <= ceiling + 1e-3,
            "an 8-return rally should still be inside the ramp: {speed} vs {ceiling}"
        );
    }

    #[test]
    fn the_ramp_only_ever_climbs() {
        let mut s = GameState::with_difficulty(Difficulty::Hard);
        s.begin();
        let mut prev = rally_speed(&s);
        for rally in 1..40 {
            s.rally = rally;
            let now = rally_speed(&s);
            assert!(now >= prev - 1e-4, "speed dropped at rally {rally}");
            prev = now;
        }
    }

    #[test]
    fn easy_ramps_more_gently_than_hard() {
        // Level 1 is where someone finds out whether the game is fun.
        let gain = |d: Difficulty| {
            let mut s = GameState::with_difficulty(d);
            s.begin();
            let base = rally_speed(&s);
            s.rally = 5;
            rally_speed(&s) / base
        };
        assert!(gain(Difficulty::Easy) < gain(Difficulty::Hard));
    }

    // ------------------------------------------------------------------
    // Collision
    // ------------------------------------------------------------------

    #[test]
    fn the_ball_bounces_off_the_top_and_bottom() {
        let mut s = served(Difficulty::Normal);
        s.ball.pos = Vec2::new(FIELD_W / 2.0, 2.0);
        s.ball.vel = Vec2::new(100.0, -200.0);
        collide_walls(&mut s);
        assert!(s.ball.vel.y > 0.0, "must be sent back down");
        assert!(s.ball.pos.y >= s.ball.radius);

        s.ball.pos = Vec2::new(FIELD_W / 2.0, FIELD_H - 2.0);
        s.ball.vel = Vec2::new(100.0, 200.0);
        collide_walls(&mut s);
        assert!(s.ball.vel.y < 0.0);
    }

    #[test]
    fn a_paddle_hit_sends_the_ball_back_and_counts_the_rally() {
        let mut s = served(Difficulty::Normal);
        // Put the ball on the left paddle, closing on it.
        s.ball.pos = Vec2::new(s.left.face_x(Side::Left), s.left.center_y());
        s.ball.vel = Vec2::new(-380.0, 0.0);
        let rally_before = s.rally;

        collide_paddles(&mut s);

        assert!(s.ball.vel.x > 0.0, "returned toward the far side");
        assert_eq!(s.rally, rally_before + 1);
    }

    #[test]
    fn striking_the_paddles_end_steers_the_ball() {
        // The skill mechanic: where you hit sets the angle.
        let hit_at = |offset: f32| {
            let mut s = served(Difficulty::Normal);
            s.ball.pos = Vec2::new(
                s.left.face_x(Side::Left),
                s.left.center_y() + offset * (s.left.h / 2.0),
            );
            s.ball.vel = Vec2::new(-380.0, 0.0);
            collide_paddles(&mut s);
            s.ball.vel.y
        };

        assert!(hit_at(-0.9) < -1.0, "top of the paddle sends it up");
        assert!(hit_at(0.9) > 1.0, "bottom sends it down");
        assert!(hit_at(0.0).abs() < hit_at(0.9).abs(), "centre is flattest");
    }

    #[test]
    fn a_ball_moving_away_is_not_batted_twice() {
        let mut s = served(Difficulty::Normal);
        s.ball.pos = Vec2::new(s.left.face_x(Side::Left), s.left.center_y());
        // Already heading away from the left paddle.
        s.ball.vel = Vec2::new(380.0, 0.0);
        let before = s.ball.vel;
        collide_paddles(&mut s);
        assert_eq!(s.ball.vel, before, "must not re-bounce");
        assert_eq!(s.rally, 0);
    }

    #[test]
    fn a_return_never_crawls_near_vertical() {
        // The hang guard. Sweep every strike position and confirm the
        // ball always keeps enough horizontal speed to cross.
        for i in 0..=40 {
            let offset = -1.0 + (i as f32) * 0.05;
            let mut s = served(Difficulty::Hard);
            s.ball.pos = Vec2::new(
                s.left.face_x(Side::Left),
                s.left.center_y() + offset * (s.left.h / 2.0),
            );
            s.ball.vel = Vec2::new(-460.0, 0.0);
            collide_paddles(&mut s);

            let speed = s.ball.vel.length();
            let frac = s.ball.vel.x.abs() / speed;
            assert!(
                frac >= MIN_HORIZONTAL_FRACTION - 1e-3,
                "offset {offset}: |vx|/speed = {frac}, ball would not cross"
            );
        }
    }

    #[test]
    fn the_angle_clamp_preserves_speed() {
        // A clamp that rebuilt vx without fixing vy would silently make
        // the ball faster every bounce.
        let mut v = Vec2::new(1.0, 400.0);
        let before = v.length();
        clamp_angle(&mut v);
        assert!((v.length() - before).abs() < 1e-2, "speed changed: {before} -> {}", v.length());
        assert!(v.x.abs() / v.length() >= MIN_HORIZONTAL_FRACTION - 1e-3);
    }

    // ------------------------------------------------------------------
    // Points
    // ------------------------------------------------------------------

    #[test]
    fn a_ball_past_the_left_wall_scores_for_the_right() {
        let mut s = served(Difficulty::Normal);
        s.ball.pos = Vec2::new(-50.0, FIELD_H / 2.0);
        check_point(&mut s);
        assert_eq!(s.score_right, 1);
        assert_eq!(s.score_left, 0);
        assert_eq!(s.phase, Phase::Serve);
    }

    #[test]
    fn a_ball_past_the_right_wall_scores_for_the_left() {
        let mut s = served(Difficulty::Normal);
        s.ball.pos = Vec2::new(FIELD_W + 50.0, FIELD_H / 2.0);
        check_point(&mut s);
        assert_eq!(s.score_left, 1);
    }

    // ------------------------------------------------------------------
    // Whole points, played out
    // ------------------------------------------------------------------

    /// Two perfect trackers rally forever, and that is CORRECT — a
    /// flawless opponent should never be scored on. So this does not
    /// assert the point ends. It asserts the thing that actually
    /// matters: that a long rally does not degenerate.
    ///
    /// Measured before the vertical floor existed: |vy| collapsed to
    /// 10-16 against a speed of 375 and stayed there for hundreds of
    /// returns — a flat line across the field. That is Breakout's
    /// near-vertical finding rotated a quarter turn, and it is why
    /// MIN_VERTICAL_FRACTION exists.
    #[test]
    fn a_long_rally_never_flattens_into_a_straight_line() {
        for d in Difficulty::ALL {
            let mut s = served(d);
            let mut worst = f32::MAX;
            let mut returns = 0;

            for _ in 0..(240 * 120) {
                for side in [Side::Left, Side::Right] {
                    let target = s.ball.pos.y;
                    let p = s.paddle_mut(side);
                    let delta = target - p.center_y();
                    p.dir = if delta.abs() < 4.0 {
                        0.0
                    } else if delta > 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                }
                let before = s.rally;
                step_fixed(&mut s);
                if s.rally > before {
                    returns += 1;
                    let speed = s.ball.vel.length();
                    worst = worst.min(s.ball.vel.y.abs() / speed);
                }
                // Points now actually end, so keep serving to gather
                // enough returns to judge the angle over.
                if s.phase == Phase::Serve {
                    serve(&mut s);
                } else if s.is_over() {
                    s.restart();
                    serve(&mut s);
                }
            }

            assert!(returns > 50, "{d:?}: only {returns} returns, too few to judge");
            assert!(
                worst >= MIN_VERTICAL_FRACTION - 1e-3,
                "{d:?}: ball flattened to |vy|/speed = {worst} over {returns} returns"
            );
        }
    }

    /// A point must still be able to END. Drive one paddle perfectly and
    /// leave the other still: the ball has to get past it.
    #[test]
    fn a_point_against_a_still_paddle_resolves() {
        for d in Difficulty::ALL {
            let mut s = served(d);
            let mut ended = false;
            for _ in 0..(240 * 120) {
                // Only the left paddle tracks; the right never moves.
                let target = s.ball.pos.y;
                let p = s.paddle_mut(Side::Left);
                let delta = target - p.center_y();
                p.dir = if delta.abs() < 4.0 {
                    0.0
                } else if delta > 0.0 {
                    1.0
                } else {
                    -1.0
                };
                step_fixed(&mut s);
                if s.score_left > 0 {
                    ended = true;
                    break;
                }
            }
            assert!(ended, "{d:?}: a point never resolved against a still paddle");
        }
    }

    #[test]
    fn a_full_match_reaches_eleven_and_stops() {
        // The left paddle tracks perfectly, the right stays put, so the
        // left runs the match out. This is about the MATCH ending at
        // eleven, not about how the points were won.
        let mut s = served(Difficulty::Normal);
        for _ in 0..(240 * 600) {
            if s.is_over() {
                break;
            }
            if s.phase == Phase::Serve {
                serve(&mut s);
            }
            let target = s.ball.pos.y;
            let p = s.paddle_mut(Side::Left);
            let delta = target - p.center_y();
            p.dir = if delta.abs() < 4.0 {
                0.0
            } else if delta > 0.0 {
                1.0
            } else {
                -1.0
            };
            step_fixed(&mut s);
        }
        assert!(s.is_over(), "match never finished");
        assert_eq!(
            s.score_left.max(s.score_right),
            MATCH_POINT,
            "someone must finish on exactly {MATCH_POINT}"
        );
        assert!(
            s.score_left.min(s.score_right) < MATCH_POINT,
            "only one side can reach match point"
        );
    }

    #[test]
    fn nothing_moves_before_the_serve() {
        let mut s = GameState::new();
        s.begin();
        for _ in 0..240 {
            step_fixed(&mut s);
        }
        assert_eq!(s.ball.vel, Vec2::ZERO);
        assert_eq!(s.ball.pos, Vec2::new(FIELD_W / 2.0, FIELD_H / 2.0));
    }

    #[test]
    fn the_ball_never_escapes_the_field_vertically() {
        let mut s = served(Difficulty::Hard);
        for _ in 0..(240 * 60) {
            step_fixed(&mut s);
            if s.phase == Phase::Serve {
                serve(&mut s);
            }
            assert!(
                s.ball.pos.y >= -1.0 && s.ball.pos.y <= FIELD_H + 1.0,
                "ball left the field at y = {}",
                s.ball.pos.y
            );
        }
    }

    #[test]
    fn the_simulation_stays_finite_over_a_long_run() {
        let mut s = served(Difficulty::Hard);
        for _ in 0..(240 * 120) {
            step_fixed(&mut s);
            if s.phase == Phase::Serve {
                serve(&mut s);
            }
            assert!(
                s.ball.pos.x.is_finite() && s.ball.pos.y.is_finite(),
                "NaN reached the ball position"
            );
        }
    }

    // ------------------------------------------------------------------
    // Accumulator
    // ------------------------------------------------------------------

    #[test]
    fn the_accumulator_carries_leftover_time() {
        let mut a = Accumulator::new();
        // 1.5 steps' worth: one now, half carried.
        assert_eq!(a.steps_for(FIXED_DT * 1.5), 1);
        assert_eq!(a.steps_for(FIXED_DT * 0.5), 1, "the carry completes a step");
    }

    #[test]
    fn a_stalled_frame_does_not_spiral() {
        let mut a = Accumulator::new();
        assert_eq!(a.steps_for(5.0), MAX_STEPS_PER_FRAME);
        // Surplus dropped, not banked: the next frame starts clean.
        assert_eq!(a.steps_for(0.0), 0);
    }

    #[test]
    fn a_backwards_clock_is_treated_as_no_time() {
        let mut a = Accumulator::new();
        assert_eq!(a.steps_for(-1.0), 0);
        assert_eq!(a.steps_for(f32::NAN), 0);
    }
}
