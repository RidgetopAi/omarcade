//! The opponent.
//!
//! The naive version is one line — `paddle.y = ball.y` — and it is
//! unbeatable, because it teleports to the right place every tick. The
//! usual fix is to cap its speed, and that is where most Pong clones
//! stop. It does not work: a speed-capped tracker is unbeatable until
//! the ball outruns the paddle and then trivially beatable forever, so
//! difficulty is a cliff rather than a slope and the player wins by
//! finding the one shot it cannot reach and repeating it. Capping speed
//! gives the opponent a HANDICAP; what makes it interesting is
//! JUDGEMENT.
//!
//! So this one predicts, and then gets the prediction wrong on purpose:
//!
//! 1. **Prediction.** When the ball is closing, simulate it forward —
//!    reflecting off the top and bottom walls — to where it will cross
//!    the paddle's face. When it is heading away, drift back toward the
//!    centre, which is what a real player does. This is what makes it
//!    look like it is thinking: it moves EARLY, toward where the ball
//!    will be, not toward where the ball is.
//! 2. **Corruption.** Aim error offsets the target, so it commits to
//!    slightly the wrong spot. Reaction delay means it only re-decides
//!    every N seconds, so between decisions it is committed and a
//!    mid-flight deflection can wrong-foot it. Prediction depth limits
//!    how many wall bounces it can see through — a weak opponent is
//!    confused by the second one, which means it fails specifically on
//!    COMPLEX shots, exactly the skill the player is developing.
//! 3. **Motion.** It has a max speed, so a big correction costs real
//!    time and a late realisation is unrecoverable.
//!
//! The property all three serve: it misses because it COMMITTED TO THE
//! WRONG PLACE, not because it was too slow. That is a mistake with a
//! story — the player watches it happen and feels they caused it,
//! because on a well-disguised shot they did.
//!
//! Nothing here moves a paddle. It sets `dir`, and `physics.rs` moves
//! everything, so the opponent is structurally incapable of teleporting.

use omarcade_core::geom::Vec2;

use crate::state::{Difficulty, GameState, Phase, Side, FIELD_H};

/// How close to its target the paddle must be before it stops.
///
/// Without a deadband it overshoots, corrects, overshoots the other way
/// and visibly buzzes around the target.
const SETTLE: f32 = 6.0;

/// Ceiling on the forward simulation, in wall bounces. A guard against
/// a pathological angle spending unbounded time in the predictor, not a
/// gameplay number — the per-difficulty limit is what the game uses.
const MAX_BOUNCES: u32 = 16;

/// How the opponent plays at one difficulty.
#[derive(Debug, Clone, Copy)]
pub struct Skill {
    /// Fraction of the paddle's half-height it may aim off by. The
    /// primary difficulty dial: at 1.0 it commits to a spot a full
    /// half-paddle from the truth, which is a clean miss on a fast ball.
    pub aim_error: f32,
    /// Seconds between decisions. Longer means more committed, so a
    /// deflection after it has decided goes unanswered.
    pub reaction: f32,
    /// How many wall bounces it can see through. 1 means it is fooled by
    /// any shot that banks twice.
    pub depth: u32,
    /// Top speed, as a fraction of the player's. Below 1.0 it simply
    /// cannot cover the field as fast as the player can.
    pub speed: f32,
}

impl Skill {
    pub fn for_difficulty(d: Difficulty) -> Skill {
        match d {
            // Beatable by someone who has never played Pong: it aims
            // well off, thinks slowly, cannot read a double bank, and
            // cannot sprint.
            Difficulty::Easy => Skill {
                aim_error: 0.85,
                reaction: 0.22,
                depth: 1,
                speed: 0.78,
            },
            // Reads one bank reliably, misses when pulled corner to
            // corner.
            Difficulty::Normal => Skill {
                aim_error: 0.45,
                reaction: 0.12,
                depth: 2,
                speed: 0.92,
            },
            // Sees the whole shot. Beating it means out-angling it, not
            // out-running it.
            Difficulty::Hard => Skill {
                aim_error: 0.18,
                reaction: 0.06,
                depth: 4,
                speed: 1.0,
            },
        }
    }
}

/// The opponent's mutable state: what it has decided, and when it will
/// next allow itself to think.
#[derive(Debug, Clone)]
pub struct Opponent {
    pub side: Side,
    pub skill: Skill,
    /// Where it is currently trying to put the centre of its paddle.
    target_y: f32,
    /// Seconds until it re-decides.
    cooldown: f32,
    /// Counts decisions, and is the only source of variation in the aim
    /// error. Deliberately a counter rather than a random number
    /// generator: the headless harnesses must be reproducible, and a
    /// seeded RNG is one more thing every probe would have to thread
    /// through.
    decisions: u32,
}

impl Opponent {
    pub fn new(side: Side, difficulty: Difficulty) -> Self {
        Opponent {
            side,
            skill: Skill::for_difficulty(difficulty),
            target_y: FIELD_H / 2.0,
            cooldown: 0.0,
            decisions: 0,
        }
    }

    /// Re-arm for a new match at a possibly different difficulty.
    pub fn reset(&mut self, difficulty: Difficulty) {
        self.skill = Skill::for_difficulty(difficulty);
        self.target_y = FIELD_H / 2.0;
        self.cooldown = 0.0;
        self.decisions = 0;
    }

    /// Where it currently believes it should be.
    ///
    /// For the probes and tests, which measure whether it is committing
    /// to the wrong PLACE rather than merely arriving late — the
    /// distinction the whole design rests on. The game itself never
    /// asks; it only sees the paddle move.
    #[allow(dead_code)]
    pub fn target(&self) -> f32 {
        self.target_y
    }

    /// Think if it is time to, then steer toward whatever it decided.
    ///
    /// Call once per fixed tick, before `physics::step_fixed`.
    pub fn update(&mut self, state: &mut GameState, dt: f32) {
        if state.phase != Phase::Playing {
            // Between points it recentres, which is both what a player
            // does and what stops it starting the next rally already
            // committed to a stale target.
            self.target_y = FIELD_H / 2.0;
            self.cooldown = 0.0;
            self.steer(state);
            return;
        }

        self.cooldown -= dt;
        if self.cooldown <= 0.0 {
            self.decide(state);
            self.cooldown = self.skill.reaction;
        }
        self.steer(state);
    }

    /// Commit to a target.
    fn decide(&mut self, state: &GameState) {
        let closing = match self.side {
            Side::Left => state.ball.vel.x < 0.0,
            Side::Right => state.ball.vel.x > 0.0,
        };

        if !closing {
            // Nothing to answer yet. Drift back toward the middle —
            // the position that covers the most of the next shot.
            self.target_y = FIELD_H / 2.0;
            return;
        }

        let face = state.paddle(self.side).face_x(self.side);
        let truth = predict_intercept(
            state.ball.pos,
            state.ball.vel,
            face,
            state.ball.radius,
            self.skill.depth,
        );

        let Some(truth) = truth else {
            // The predictor could not resolve this shot within its
            // depth — which for a weak opponent is the common case on a
            // multi-bank ball. It holds the middle and hopes.
            self.target_y = FIELD_H / 2.0;
            return;
        };

        // Aim error, in paddle half-heights. The sign alternates and the
        // magnitude cycles, so consecutive decisions are not identical
        // and the error does not bias toward one edge of the field —
        // a constant offset would make it miss the same way every time,
        // which a player would read and exploit within a rally.
        let half_h = state.paddle(self.side).h / 2.0;
        let phase = self.decisions % 4;
        let magnitude = match phase {
            0 => 1.0,
            1 => 0.45,
            2 => 0.8,
            _ => 0.15,
        };
        let sign = if phase % 2 == 0 { 1.0 } else { -1.0 };
        let error = sign * magnitude * self.skill.aim_error * half_h;

        self.decisions = self.decisions.wrapping_add(1);
        self.target_y = (truth + error).clamp(0.0, FIELD_H);
    }

    /// Turn the committed target into a direction. Never a position.
    fn steer(&self, state: &mut GameState) {
        let side = self.side;
        let speed = self.skill.speed;
        let p = state.paddle_mut(side);
        let delta = self.target_y - p.center_y();

        p.dir = if delta.abs() < SETTLE {
            0.0
        } else if delta > 0.0 {
            speed
        } else {
            -speed
        };
    }
}

/// Where the ball will cross `face_x`, reflecting off the top and
/// bottom walls up to `max_bounces` times.
///
/// `None` when it cannot be resolved within the bounce budget, which is
/// how prediction depth becomes a difficulty dial: a shallow predictor
/// genuinely cannot see the end of a shot that banks twice, so the
/// opponent it drives is confused by exactly the shots that are hard to
/// play.
///
/// Pure geometry over a copy of the ball's position and velocity — it
/// never touches `GameState`, so a probe can ask it questions directly.
pub fn predict_intercept(
    pos: Vec2,
    vel: Vec2,
    face_x: f32,
    radius: f32,
    max_bounces: u32,
) -> Option<f32> {
    if vel.x == 0.0 || !vel.x.is_finite() || !vel.y.is_finite() {
        return None;
    }

    // The ball's centre stops `radius` short of the face.
    let target_x = if vel.x > 0.0 { face_x - radius } else { face_x + radius };

    // Already past it.
    if (vel.x > 0.0 && pos.x >= target_x) || (vel.x < 0.0 && pos.x <= target_x) {
        return Some(pos.y);
    }

    let mut y = pos.y;
    let mut vy = vel.y;
    let mut t_remaining = (target_x - pos.x) / vel.x;
    if !t_remaining.is_finite() || t_remaining < 0.0 {
        return None;
    }

    // Walls the CENTRE can reach, radius accounted for.
    let top = radius;
    let bottom = FIELD_H - radius;
    if bottom <= top {
        return None;
    }

    let budget = max_bounces.min(MAX_BOUNCES);
    for _ in 0..=budget {
        if vy == 0.0 {
            return Some(y.clamp(top, bottom));
        }

        // Time until the next wall, if any.
        let wall = if vy > 0.0 { bottom } else { top };
        let t_wall = (wall - y) / vy;

        if !t_wall.is_finite() || t_wall < 0.0 {
            return Some(y.clamp(top, bottom));
        }

        if t_wall >= t_remaining {
            // Reaches the face before the wall.
            return Some((y + vy * t_remaining).clamp(top, bottom));
        }

        // Bounce and keep going.
        y = wall;
        vy = -vy;
        t_remaining -= t_wall;
    }

    // Out of bounces: the shot is deeper than this predictor can see.
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::physics::{self, serve};
    use crate::state::BALL_RADIUS;

    const FACE: f32 = 900.0;

    #[test]
    fn a_straight_ball_is_predicted_exactly() {
        let hit = predict_intercept(
            Vec2::new(100.0, 300.0),
            Vec2::new(400.0, 0.0),
            FACE,
            BALL_RADIUS,
            4,
        );
        assert_eq!(hit, Some(300.0));
    }

    #[test]
    fn a_ball_moving_away_from_the_face_is_reported_as_already_there() {
        // Travelling left, face is to the right: it is behind us.
        let hit = predict_intercept(
            Vec2::new(500.0, 200.0),
            Vec2::new(-400.0, 0.0),
            100.0,
            BALL_RADIUS,
            4,
        );
        assert_eq!(hit, Some(200.0));
    }

    #[test]
    fn a_single_bounce_is_predicted() {
        // Aimed at the floor, shallow enough to bounce once on the way.
        let pos = Vec2::new(100.0, FIELD_H - 100.0);
        let vel = Vec2::new(400.0, 400.0);
        let hit = predict_intercept(pos, vel, FACE, BALL_RADIUS, 4).expect("resolvable");
        assert!(
            hit > BALL_RADIUS && hit < FIELD_H - BALL_RADIUS,
            "prediction {hit} left the field"
        );
    }

    /// The prediction is only worth anything if it matches what physics
    /// actually does. Simulate the ball and compare.
    #[test]
    fn the_prediction_matches_the_simulation() {
        for (vy, label) in [
            (0.0, "flat"),
            (180.0, "down"),
            (-180.0, "up"),
            (320.0, "steep down"),
            (-320.0, "steep up"),
        ] {
            let mut s = GameState::new();
            s.begin();
            serve(&mut s);
            s.ball.pos = Vec2::new(200.0, 360.0);
            s.ball.vel = Vec2::new(380.0, vy);

            let face = s.right.face_x(Side::Right);
            let predicted =
                predict_intercept(s.ball.pos, s.ball.vel, face, s.ball.radius, MAX_BOUNCES)
                    .expect("should resolve with a full budget");

            // Run the real simulation and watch for the crossing.
            //
            // The paddles cannot simply be moved out of the field —
            // move_paddles clamps them back every tick, so the right
            // paddle would still be there returning the ball. Shrinking
            // it to nothing is what actually gets it out of the way,
            // and it leaves the walls and the timestep untouched, which
            // is what this is comparing against.
            s.right.h = 0.0;
            s.right.y = 0.0;
            let mut actual = None;
            for _ in 0..(240 * 20) {
                let before = s.ball.pos.x;
                physics::step_fixed(&mut s);
                if before < face - s.ball.radius && s.ball.pos.x >= face - s.ball.radius {
                    actual = Some(s.ball.pos.y);
                    break;
                }
            }

            let actual = actual.expect("ball never reached the face");
            assert!(
                (predicted - actual).abs() < 6.0,
                "{label}: predicted {predicted}, simulation gave {actual}"
            );
        }
    }

    #[test]
    fn a_shallow_predictor_cannot_see_a_deep_shot() {
        // A steep ball from one corner banks several times before it
        // crosses. Depth 1 must fail where a full budget succeeds.
        let pos = Vec2::new(60.0, 40.0);
        let vel = Vec2::new(200.0, 700.0);

        let deep = predict_intercept(pos, vel, FACE, BALL_RADIUS, MAX_BOUNCES);
        let shallow = predict_intercept(pos, vel, FACE, BALL_RADIUS, 1);

        assert!(deep.is_some(), "a full budget should resolve this shot");
        assert!(shallow.is_none(), "depth 1 should not see through it");
    }

    #[test]
    fn a_prediction_never_leaves_the_field() {
        // Sweep a fan of angles; every resolved answer must be somewhere
        // the paddle could actually be.
        for i in 0..80 {
            let vy = -800.0 + (i as f32) * 20.0;
            if let Some(hit) =
                predict_intercept(Vec2::new(80.0, 360.0), Vec2::new(300.0, vy), FACE, BALL_RADIUS, 8)
            {
                assert!(
                    (BALL_RADIUS - 1e-3..=FIELD_H - BALL_RADIUS + 1e-3).contains(&hit),
                    "vy {vy}: prediction {hit} is off the field"
                );
            }
        }
    }

    #[test]
    fn a_stationary_ball_predicts_nothing() {
        assert_eq!(
            predict_intercept(Vec2::new(100.0, 100.0), Vec2::ZERO, FACE, BALL_RADIUS, 4),
            None
        );
    }

    #[test]
    fn nan_input_predicts_nothing_rather_than_nan() {
        assert_eq!(
            predict_intercept(
                Vec2::new(100.0, 100.0),
                Vec2::new(f32::NAN, 1.0),
                FACE,
                BALL_RADIUS,
                4
            ),
            None
        );
    }

    // ------------------------------------------------------------------
    // The opponent itself
    // ------------------------------------------------------------------

    #[test]
    fn harder_opponents_are_better_on_every_dial() {
        let (e, n, h) = (
            Skill::for_difficulty(Difficulty::Easy),
            Skill::for_difficulty(Difficulty::Normal),
            Skill::for_difficulty(Difficulty::Hard),
        );
        assert!(e.aim_error > n.aim_error && n.aim_error > h.aim_error);
        assert!(e.reaction > n.reaction && n.reaction > h.reaction);
        assert!(e.depth < n.depth && n.depth < h.depth);
        assert!(e.speed < n.speed && n.speed <= h.speed);
    }

    #[test]
    fn the_opponent_only_ever_sets_a_direction() {
        // The structural guarantee: it cannot teleport, because it never
        // writes a position.
        let mut s = GameState::new();
        s.begin();
        serve(&mut s);
        let before = s.right.y;

        let mut ai = Opponent::new(Side::Right, Difficulty::Hard);
        ai.update(&mut s, physics::FIXED_DT);

        assert_eq!(s.right.y, before, "update must not move the paddle itself");
        assert!(s.right.dir != 0.0 || (ai.target() - s.right.center_y()).abs() < SETTLE);
    }

    #[test]
    fn it_moves_early_rather_than_chasing() {
        // The prediction property. Put the ball far away on a steep
        // angle and confirm the opponent commits toward where the ball
        // WILL be, not toward where it currently is.
        let mut s = GameState::new();
        s.begin();
        serve(&mut s);
        s.ball.pos = Vec2::new(200.0, 100.0);
        s.ball.vel = Vec2::new(380.0, 300.0);

        let mut ai = Opponent::new(Side::Right, Difficulty::Hard);
        ai.update(&mut s, physics::FIXED_DT);

        assert!(
            ai.target() > s.ball.pos.y,
            "target {} should lead a ball at y={} moving down",
            ai.target(),
            s.ball.pos.y
        );
    }

    #[test]
    fn it_recentres_when_the_ball_is_going_the_other_way() {
        let mut s = GameState::new();
        s.begin();
        serve(&mut s);
        // Heading toward the LEFT, so the right opponent has nothing to do.
        s.ball.pos = Vec2::new(500.0, 80.0);
        s.ball.vel = Vec2::new(-380.0, 0.0);

        let mut ai = Opponent::new(Side::Right, Difficulty::Hard);
        ai.update(&mut s, physics::FIXED_DT);

        assert!(
            (ai.target() - FIELD_H / 2.0).abs() < 1e-3,
            "should drift to the centre, went to {}",
            ai.target()
        );
    }

    #[test]
    fn reaction_delay_keeps_it_committed() {
        // The point of the delay: a deflection AFTER it decides goes
        // unanswered until the cooldown expires.
        let mut s = GameState::new();
        s.begin();
        serve(&mut s);
        s.ball.pos = Vec2::new(300.0, 200.0);
        s.ball.vel = Vec2::new(380.0, 0.0);

        let mut ai = Opponent::new(Side::Right, Difficulty::Easy);
        ai.update(&mut s, physics::FIXED_DT);
        let committed = ai.target();

        // The ball is deflected hard, but not enough time passes.
        s.ball.pos.y = 600.0;
        ai.update(&mut s, physics::FIXED_DT);

        assert_eq!(ai.target(), committed, "must stay committed within the delay");
    }

    #[test]
    fn aim_error_is_not_a_constant_bias() {
        // A fixed offset would make it miss the same way every time,
        // which a player reads and exploits inside one rally.
        let mut s = GameState::new();
        s.begin();
        serve(&mut s);
        s.ball.pos = Vec2::new(300.0, 360.0);
        s.ball.vel = Vec2::new(380.0, 0.0);

        let mut ai = Opponent::new(Side::Right, Difficulty::Easy);
        let mut targets = Vec::new();
        for _ in 0..4 {
            ai.decide(&s);
            targets.push(ai.target());
        }

        let above = targets.iter().filter(|&&t| t > 360.0).count();
        let below = targets.iter().filter(|&&t| t < 360.0).count();
        assert!(above > 0 && below > 0, "error should fall both ways: {targets:?}");
    }

    #[test]
    fn a_perfect_prediction_still_gets_the_easy_opponent_wrong() {
        // Easy must genuinely commit to the wrong place — that is the
        // difference between a weak opponent and a slow one.
        let mut s = GameState::new();
        s.begin();
        serve(&mut s);
        s.ball.pos = Vec2::new(300.0, 360.0);
        s.ball.vel = Vec2::new(380.0, 0.0);

        let mut ai = Opponent::new(Side::Right, Difficulty::Easy);
        ai.decide(&s);
        let err = (ai.target() - 360.0).abs();

        assert!(
            err > 20.0,
            "easy should misjudge a straight ball by a real margin, was {err}"
        );
    }

    #[test]
    fn reset_re_arms_for_a_new_difficulty() {
        let mut ai = Opponent::new(Side::Right, Difficulty::Easy);
        ai.reset(Difficulty::Hard);
        assert_eq!(ai.skill.depth, Skill::for_difficulty(Difficulty::Hard).depth);
        assert!((ai.target() - FIELD_H / 2.0).abs() < 1e-3);
    }
}
