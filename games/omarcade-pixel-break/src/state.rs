//! The world, with no behaviour.
//!
//! Nothing here moves anything. Every rule that advances time lives in
//! `physics.rs`, and every rule that draws lives in `render.rs`. The
//! payoff is testability: a test can construct "ball one pixel from the
//! last brick with one life left" directly, instead of playing the game
//! until that situation happens.

use crate::geom::{Rect, Vec2};
use crate::items::{Dropper, Item, ItemKind, BOMB_SCALE};

/// Play-field size in logical units.
///
/// Gameplay happens entirely in these coordinates. The window is tiled
/// to whatever size Hyprland decides — 1261x701 last session — and the
/// renderer scales to fit, so the game plays identically at any size.
pub const FIELD_W: f32 = 960.0;
pub const FIELD_H: f32 = 720.0;

pub const PADDLE_W: f32 = 120.0;
pub const PADDLE_H: f32 = 16.0;
/// How far above the bottom edge the paddle sits.
pub const PADDLE_Y: f32 = FIELD_H - 60.0;
/// Level-1 pace. Comfortably faster than the ball so the paddle can
/// always get under it, without feeling twitchy.
pub const PADDLE_SPEED: f32 = 600.0;

pub const BALL_RADIUS: f32 = 8.0;
/// Level-1 pace. Every later level scales up from here.
pub const BALL_SPEED: f32 = 340.0;
/// Level-10 pace. The climb is deliberately gentle: clearing L10 takes
/// 2.6x the hits of L1, and matching that in speed would make it
/// unplayable rather than hard.
pub const BALL_SPEED_TOP: f32 = 460.0;
/// Level-10 paddle pace, up from `PADDLE_SPEED`. It rises with the ball so
/// the paddle can always still get under it.
pub const PADDLE_SPEED_TOP: f32 = 700.0;

/// Ball speed on a given level: `BALL_SPEED` at 1, `BALL_SPEED_TOP` at
/// `LEVELS`, linear between.
///
/// ⚠️ Bounded by the tunnelling invariant — see
/// `physics::TUNNELLING_LIMIT`, which DERIVES the ceiling from `BRICK_H`
/// and the fixed timestep rather than restating a number here.
pub fn ball_speed_for(level: u32) -> f32 {
    lerp_by_level(level, BALL_SPEED, BALL_SPEED_TOP)
}

pub fn paddle_speed_for(level: u32) -> f32 {
    lerp_by_level(level, PADDLE_SPEED, PADDLE_SPEED_TOP)
}

fn lerp_by_level(level: u32, at_one: f32, at_top: f32) -> f32 {
    let level = level.clamp(1, LEVELS);
    let t = (level - 1) as f32 / (LEVELS - 1) as f32;
    at_one + (at_top - at_one) * t
}

pub const BRICK_COLS: usize = 10;
pub const BRICK_ROWS: usize = 6;
pub const BRICK_W: f32 = 84.0;
pub const BRICK_H: f32 = 28.0;
pub const BRICK_GAP: f32 = 6.0;
/// Empty space above the brick field, leaving room for the HUD.
pub const BRICK_TOP: f32 = 90.0;

pub const STARTING_LIVES: u32 = 3;

/// Most balls in play at once, on levels 1-5 and from level 6 on.
///
/// Lives persist across the whole game; balls are what is in play right
/// now. A life is lost only when the last ball drains, so the cap is a
/// ceiling on how much rescue a good run can bank, not on how long it
/// lasts.
pub const BALL_CAP_LOW: usize = 5;
pub const BALL_CAP_HIGH: usize = 10;
/// The level at which the cap goes up.
pub const LEVEL_CAP_STEP: u32 = 6;

/// How many levels the game runs to. Clearing the last one beats it.
pub const LEVELS: u32 = 10;
/// First level that puts armoured bricks on the field.
pub const ARMOUR_FROM_LEVEL: u32 = 8;

/// How many past ball positions the trail keeps. Long enough to read as
/// motion, short enough that a slow ball does not smear.
pub const TRAIL_LEN: usize = 10;

/// Where the game is in its lifecycle.
///
/// Explicit states rather than a scatter of booleans: "is the ball
/// stuck to the paddle" and "is the game over" are the same question
/// asked of one value, so they cannot contradict each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Ball rests on the paddle; Space launches it.
    Ready,
    Playing,
    /// Out of lives.
    Lost,
    /// Field cleared.
    Won,
}

#[derive(Debug, Clone, Copy)]
pub struct Paddle {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// -1 left, 0 still, +1 right. Set from input, consumed by physics.
    pub dir: f32,
}

impl Paddle {
    pub fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.w / 2.0
    }
}

#[derive(Debug, Clone)]
pub struct Ball {
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: f32,
    /// Recent positions, newest first, for this ball's motion trail.
    ///
    /// ⚠️ **Per ball, deliberately.** One shared trail with several balls
    /// writing into it draws a line that whips between them — which looks
    /// exactly like a rendering bug and gets reported as one. The trail
    /// belongs to the thing that made it.
    ///
    /// Presentation state living in the world model on purpose: physics is
    /// the only thing that knows where a ball has actually been, and
    /// sampling it in `render` would tie the trail to frame rate instead of
    /// to the fixed timestep.
    pub trail: Vec<Vec2>,
    /// Set by physics when this ball passes the bottom edge; the ball is
    /// removed after the per-ball loop finishes.
    ///
    /// ⚠️ A flag rather than an immediate removal. Losing a life the moment
    /// one ball drains is correct with a single ball and wrong with several
    /// — see `GameState::retire_drained_balls`.
    pub drained: bool,
}

impl Ball {
    /// A ball at rest. Trail empty, not drained.
    pub fn new(pos: Vec2, vel: Vec2, radius: f32) -> Self {
        Ball {
            pos,
            vel,
            radius,
            trail: Vec::with_capacity(TRAIL_LEN),
            drained: false,
        }
    }

    /// The ball as a rect, which is how collision sees it.
    ///
    /// A square standing in for a circle is the standard Breakout
    /// simplification: at this size the difference is invisible, and it
    /// keeps every collision a single AABB test.
    pub fn rect(&self) -> Rect {
        Rect::from_center(self.pos, self.radius, self.radius)
    }
}

/// How much punishment a brick takes, and how it reads on screen.
///
/// ⚠️ **A brick's remaining hits must be readable from its SHAPE**, never
/// from a colour the player has to learn. The tier picks the border; the
/// hits left pick how much has already chipped away. `render` owns both,
/// and neither is a palette lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// One hit. A solid brick in its row colour.
    Plain,
    /// Two hits. Carries a visible seam; the first hit breaks a corner off.
    Reinforced,
    /// Four hits. Heavy border, chipping away in four visible stages.
    Armoured,
}

impl Tier {
    /// Hits this tier starts with.
    pub const fn hits(self) -> u32 {
        match self {
            Tier::Plain => 1,
            Tier::Reinforced => 2,
            Tier::Armoured => 4,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Brick {
    pub rect: Rect,
    /// Hits left. Zero is dead; there is no separate `alive` flag, so the
    /// two can never disagree.
    pub hits: u32,
    /// What this brick was built as. Kept after damage so the renderer can
    /// still draw an armoured brick's heavy border at one hit left.
    pub tier: Tier,
    /// Index into the renderer's palette, NOT a resolved colour.
    ///
    /// Storing a `Color` here would freeze the theme into the level, so
    /// a live theme change could not repaint it.
    pub color_index: usize,
}

impl Brick {
    pub fn alive(&self) -> bool {
        self.hits > 0
    }

    /// Take one hit. Returns true if this destroyed the brick.
    pub fn hit(&mut self) -> bool {
        self.hits = self.hits.saturating_sub(1);
        self.hits == 0
    }

    /// How much of this brick is gone, in `0.0..=1.0`.
    ///
    /// Drives the chipping in `render`: 0.0 is untouched, and a brick at
    /// 1.0 is destroyed and no longer drawn at all.
    pub fn damage(&self) -> f32 {
        let start = self.tier.hits() as f32;
        (start - self.hits as f32) / start
    }
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub paddle: Paddle,
    /// Every ball in play. Ordinary play is a one-element vec.
    ///
    /// ⚠️ **Order is observable.** Balls are resolved one at a time, so a
    /// brick killed by the first is already dead for the second in the same
    /// tick. That is correct and free, but it means the vec must stay
    /// stable: new balls are pushed to the BACK, and drained ones are
    /// removed AFTER the per-ball loop, never during it.
    pub balls: Vec<Ball>,
    pub bricks: Vec<Brick>,
    pub lives: u32,
    pub score: u32,
    /// Which level is being played, from 1. Only the ball cap reads it
    /// today; the ten-level progression itself is still to come.
    pub level: u32,
    pub phase: Phase,
    /// Best score on record, shown on the end screen. Owned by the caller:
    /// the simulation never sets it, so the headless harnesses see 0 and
    /// stay deterministic.
    pub best: u32,
    /// Power-ups currently falling.
    pub items: Vec<Item>,
    /// Decides what drops. Seeded, so a probe run is reproducible.
    pub dropper: Dropper,
    /// ⚠️ **Grow and bomb are ONE axis, not two effects.** A single scale
    /// with a single timer, driven by whichever was caught last. Two
    /// independent effects would contradict each other the moment a bomb
    /// landed on a grown paddle, and there is no reading of "half grown and
    /// also shrunk" that a player would guess right.
    ///
    /// 1.0 is the base width. Above is a grow, below is a bomb.
    pub paddle_scale: f32,
    /// Seconds left on `paddle_scale` before it returns to 1.0.
    pub paddle_effect_left: f32,
}

impl GameState {
    /// A fresh game: full lives, zero score, ball on the paddle.
    pub fn new() -> Self {
        let paddle = Paddle {
            x: (FIELD_W - PADDLE_W) / 2.0,
            y: PADDLE_Y,
            w: PADDLE_W,
            h: PADDLE_H,
            dir: 0.0,
        };

        let mut state = GameState {
            balls: vec![Ball::new(Vec2::ZERO, Vec2::ZERO, BALL_RADIUS)],
            paddle,
            bricks: build_bricks(1),
            lives: STARTING_LIVES,
            score: 0,
            level: 1,
            phase: Phase::Ready,
            best: 0,
            items: Vec::new(),
            dropper: Dropper::default(),
            paddle_scale: 1.0,
            paddle_effect_left: 0.0,
        };
        state.rest_ball_on_paddle();
        state
    }

    /// Collapse to exactly one ball, parked on the paddle, motionless.
    ///
    /// Placed one pixel clear of the paddle rather than touching it: at
    /// launch the ball must not already be overlapping, or the first
    /// collision check would immediately bounce it back down.
    ///
    /// ⚠️ **`Phase::Ready` holds exactly one ball.** Multi-ball exists only
    /// during `Playing`, so every path back to `Ready` — a lost life, a new
    /// level, a restart — comes through here and discards the rest.
    pub fn rest_ball_on_paddle(&mut self) {
        let radius = self.balls.first().map_or(BALL_RADIUS, |b| b.radius);
        self.balls.clear();
        self.balls.push(Ball::new(
            Vec2::new(self.paddle.center_x(), self.paddle.y - radius - 1.0),
            Vec2::ZERO,
            radius,
        ));
    }

    /// Send the ball on its way, upward and slightly angled.
    ///
    /// Never straight up: a perfectly vertical ball in a brick corridor
    /// bounces forever on the same column and the game stalls.
    ///
    /// Written as "every ball" even though `Ready` holds exactly one: it
    /// cannot then become wrong if a later phase ever launches several.
    pub fn launch(&mut self) {
        if self.phase != Phase::Ready {
            return;
        }
        // ⚠️ The LEVEL's speed, not the constant. Reading BALL_SPEED here
        // would leave the ramp working everywhere except the one moment the
        // player actually feels it.
        let vel = Vec2::new(0.35, -1.0).with_length(self.ball_speed());
        for ball in &mut self.balls {
            ball.vel = vel;
        }
        self.phase = Phase::Playing;
    }

    /// How many balls may be in play at once on the current level.
    ///
    /// Five through level five, ten from level six. The jump is what makes
    /// the back half of the game feel different rather than merely faster.
    pub fn ball_cap(&self) -> usize {
        if self.level >= LEVEL_CAP_STEP {
            BALL_CAP_HIGH
        } else {
            BALL_CAP_LOW
        }
    }

    /// Add a ball travelling `vel` from `pos`, if the cap allows it.
    ///
    /// Returns whether one was actually added, so a caller can decline to
    /// play the pickup sound for a ball that never appeared.
    ///
    /// ⚠️ Pushed to the BACK. Balls are resolved in order and that order is
    /// observable through same-tick brick kills; appending keeps every
    /// existing ball's position in the vec stable.
    pub fn spawn_ball(&mut self, pos: Vec2, vel: Vec2) -> bool {
        if self.balls.len() >= self.ball_cap() {
            return false;
        }
        let radius = self.balls.first().map_or(BALL_RADIUS, |b| b.radius);
        self.balls.push(Ball::new(pos, vel, radius));
        true
    }

    /// Remove every ball that drained this tick, and lose a life only if
    /// that leaves none in play.
    ///
    /// ⚠️ **This is the rule multi-ball is most likely to get wrong.** With
    /// one ball, "ball past the bottom" and "life lost" are the same event,
    /// and the single-ball code fired `lose_life` inline the moment it
    /// happened. With several, that costs a life for the first ball to
    /// drain while the others are still in play. Draining is per ball;
    /// losing a life is per empty field.
    ///
    /// Removal walks backwards so each index stays valid as earlier ones
    /// are removed.
    pub fn retire_drained_balls(&mut self) {
        if !self.balls.iter().any(|b| b.drained) {
            return;
        }
        for i in (0..self.balls.len()).rev() {
            if self.balls[i].drained {
                self.balls.remove(i);
            }
        }
        if self.balls.is_empty() {
            self.lose_life();
        }
    }

    pub fn bricks_remaining(&self) -> usize {
        self.bricks.iter().filter(|b| b.alive()).count()
    }

    /// Ball speed for the current level.
    pub fn ball_speed(&self) -> f32 {
        ball_speed_for(self.level)
    }

    /// Paddle speed for the current level.
    pub fn paddle_speed(&self) -> f32 {
        paddle_speed_for(self.level)
    }

    /// Apply a caught item.
    ///
    /// ⚠️ **A bomb caught while grown CANCELS the grow** rather than
    /// stacking into a double-negative. It is the simplest rule to explain
    /// and the only one a player guesses right: the last thing you caught is
    /// the thing you have.
    ///
    /// ⚠️ **Grow stacks DURATION, not size.** Catching a second grow while
    /// grown extends the timer and takes the larger of the two widths — it
    /// never multiplies, or two lucky catches would make the paddle absurd
    /// and a third would fill the field.
    pub fn apply_item(&mut self, kind: ItemKind) {
        match kind {
            ItemKind::Grow(strength) => {
                let already_grown = self.paddle_scale > 1.0;
                self.paddle_scale = if already_grown {
                    self.paddle_scale.max(strength.scale())
                } else {
                    // Growing out of a bomb replaces it outright.
                    strength.scale()
                };
                self.paddle_effect_left = if already_grown {
                    self.paddle_effect_left + strength.seconds()
                } else {
                    strength.seconds()
                };
            }
            ItemKind::Bomb => {
                self.paddle_scale = BOMB_SCALE;
                self.paddle_effect_left = kind.seconds();
            }
        }
        self.resize_paddle();
    }

    /// Set the paddle's width from `paddle_scale`, clamped at both ends.
    ///
    /// ⚠️ Never wider than the field and never narrower than the ball, and
    /// both ends are tested. A paddle wider than the field cannot be moved
    /// meaningfully; one narrower than the ball makes a return a coin flip
    /// that no amount of skill improves.
    ///
    /// Resizing keeps the paddle CENTRED on where it already was, so an
    /// item caught at the edge of the field does not teleport the paddle.
    pub fn resize_paddle(&mut self) {
        let centre = self.paddle.center_x();
        let want = PADDLE_W * self.paddle_scale;
        self.paddle.w = want.clamp(BALL_RADIUS * 2.0, FIELD_W);
        self.paddle.x = (centre - self.paddle.w / 2.0).clamp(0.0, FIELD_W - self.paddle.w);
    }

    /// Count down the paddle effect and return the paddle to normal when it
    /// runs out.
    pub fn tick_paddle_effect(&mut self, dt: f32) {
        if self.paddle_effect_left <= 0.0 {
            return;
        }
        self.paddle_effect_left -= dt;
        if self.paddle_effect_left <= 0.0 {
            self.paddle_effect_left = 0.0;
            self.paddle_scale = 1.0;
            self.resize_paddle();
        }
    }

    /// Drop everything the level was carrying: falling items and any paddle
    /// effect. Called wherever a level ends, so last level's bomb does not
    /// follow the player into the next one.
    pub fn clear_level_effects(&mut self) {
        self.items.clear();
        self.paddle_scale = 1.0;
        self.paddle_effect_left = 0.0;
        self.resize_paddle();
    }

    /// Clear the field: advance to the next level, or win the game.
    ///
    /// Lives, score and the best carry forward; the ball collapses back to
    /// one on the paddle and the next field is built. Clearing the last
    /// level is the only way to reach `Phase::Won`.
    pub fn advance_level(&mut self) {
        if self.level >= LEVELS {
            self.phase = Phase::Won;
            return;
        }
        self.level += 1;
        self.bricks = build_bricks(self.level);
        self.clear_level_effects();
        self.phase = Phase::Ready;
        self.rest_ball_on_paddle();
    }

    /// The play field itself, for wall collisions.
    pub fn field(&self) -> Rect {
        Rect::new(0.0, 0.0, FIELD_W, FIELD_H)
    }

    /// Lose a life and reset for the next ball, or end the game.
    pub fn lose_life(&mut self) {
        self.lives = self.lives.saturating_sub(1);
        if self.lives == 0 {
            self.phase = Phase::Lost;
        } else {
            self.phase = Phase::Ready;
            self.rest_ball_on_paddle();
        }
    }

    /// Start over. Keeps only the best score, which outlives any one run.
    pub fn restart(&mut self) {
        // Carry the best across: it belongs to the player, not the run.
        let best = self.best;
        *self = GameState::new();
        self.best = best;
    }
}

impl Default for GameState {
    fn default() -> Self {
        GameState::new()
    }
}

/// Which tier the given row gets on the given level.
///
/// Rows are counted **from the top** — row 0 is the topmost and hardest to
/// reach, so the field grows a harder crust as the levels climb and the
/// ball has to work its way behind it.
///
/// ```text
/// L1   . . . . . .      all plain
/// L2   # . . . . .      1 row reinforced
/// ...
/// L7   # # # # # #      6   <- reinforced peak
/// L8   = # # # # #      1 row armoured, 5 reinforced
/// L9   = = # # # #      2 armoured
/// L10  = = = # # #      3 armoured
/// ```
pub fn tier_for(level: u32, row: usize) -> Tier {
    let level = level.clamp(1, LEVELS);

    // L8-10 put a band of armoured rows at the very top, restarting the
    // count at one row: the third tier arrives as its own escalation
    // rather than continuing the reinforced ramp.
    let armoured_rows = level.saturating_sub(ARMOUR_FROM_LEVEL - 1) as usize;
    if row < armoured_rows {
        return Tier::Armoured;
    }

    // Below the armour, reinforced rows fill down from the top. L2 has one,
    // L7 has six — the whole field.
    let reinforced_rows = (level.saturating_sub(1) as usize).min(BRICK_ROWS);
    if row < reinforced_rows {
        Tier::Reinforced
    } else {
        Tier::Plain
    }
}

/// Total hits needed to clear a level, without building it.
///
/// Used by tests and `probe_balance` to check the shape of the difficulty
/// curve against what the plan claims.
pub fn hits_to_clear(level: u32) -> u32 {
    (0..BRICK_ROWS)
        .map(|row| tier_for(level, row).hits() * BRICK_COLS as u32)
        .sum()
}

/// Lay out the brick grid for a level, centred horizontally.
pub fn build_bricks(level: u32) -> Vec<Brick> {
    let total_w = BRICK_COLS as f32 * BRICK_W + (BRICK_COLS - 1) as f32 * BRICK_GAP;
    let x0 = (FIELD_W - total_w) / 2.0;

    let mut bricks = Vec::with_capacity(BRICK_COLS * BRICK_ROWS);
    for row in 0..BRICK_ROWS {
        let tier = tier_for(level, row);
        for col in 0..BRICK_COLS {
            bricks.push(Brick {
                rect: Rect::new(
                    x0 + col as f32 * (BRICK_W + BRICK_GAP),
                    BRICK_TOP + row as f32 * (BRICK_H + BRICK_GAP),
                    BRICK_W,
                    BRICK_H,
                ),
                hits: tier.hits(),
                tier,
                // One colour per row.
                color_index: row,
            });
        }
    }
    bricks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_game_is_ready_with_full_lives() {
        let s = GameState::new();
        assert_eq!(s.phase, Phase::Ready);
        assert_eq!(s.lives, STARTING_LIVES);
        assert_eq!(s.score, 0);
        assert_eq!(s.bricks_remaining(), BRICK_COLS * BRICK_ROWS);
    }

    #[test]
    fn bricks_fit_inside_the_field_and_do_not_overlap() {
        let s = GameState::new();
        for b in &s.bricks {
            assert!(b.rect.left() >= 0.0, "brick off left edge: {:?}", b.rect);
            assert!(b.rect.right() <= FIELD_W, "brick off right edge: {:?}", b.rect);
            assert!(b.rect.top() >= 0.0);
            assert!(b.rect.bottom() < PADDLE_Y, "bricks must not reach the paddle");
        }
        // No two bricks overlap.
        for (i, a) in s.bricks.iter().enumerate() {
            for b in &s.bricks[i + 1..] {
                assert!(!a.rect.overlaps(&b.rect), "bricks overlap: {:?} {:?}", a.rect, b.rect);
            }
        }
    }

    #[test]
    fn brick_grid_is_centred() {
        let s = GameState::new();
        let left = s.bricks.iter().map(|b| b.rect.left()).fold(f32::MAX, f32::min);
        let right = s.bricks.iter().map(|b| b.rect.right()).fold(f32::MIN, f32::max);
        assert!(
            ((left) - (FIELD_W - right)).abs() < 0.01,
            "left margin {left} != right margin {}",
            FIELD_W - right
        );
    }

    /// The "ball spawns inside the paddle" bug: at rest the ball must be
    /// clear of the paddle, or its first collision test bounces it.
    #[test]
    fn resting_ball_does_not_overlap_the_paddle() {
        let s = GameState::new();
        assert!(!s.balls[0].rect().overlaps(&s.paddle.rect()));
        assert_eq!(s.balls[0].vel, Vec2::ZERO);
    }

    #[test]
    fn launch_sends_the_ball_upward_at_full_speed() {
        let mut s = GameState::new();
        s.launch();
        assert_eq!(s.phase, Phase::Playing);
        assert!(s.balls[0].vel.y < 0.0, "must travel up (y grows downward)");
        assert!((s.balls[0].vel.length() - BALL_SPEED).abs() < 0.01);
    }

    /// A perfectly vertical launch stalls the game in a brick corridor.
    #[test]
    fn launch_is_never_perfectly_vertical() {
        let mut s = GameState::new();
        s.launch();
        assert!(s.balls[0].vel.x.abs() > 1.0, "vx = {} is too vertical", s.balls[0].vel.x);
    }

    #[test]
    fn launch_does_nothing_unless_ready() {
        let mut s = GameState::new();
        s.phase = Phase::Playing;
        let before = s.balls[0].vel;
        s.launch();
        assert_eq!(s.balls[0].vel, before);
    }

    #[test]
    fn losing_a_life_resets_to_ready() {
        let mut s = GameState::new();
        s.phase = Phase::Playing;
        s.lose_life();
        assert_eq!(s.lives, STARTING_LIVES - 1);
        assert_eq!(s.phase, Phase::Ready);
        assert!(!s.balls[0].rect().overlaps(&s.paddle.rect()));
    }

    #[test]
    fn losing_the_last_life_ends_the_game() {
        let mut s = GameState::new();
        s.lives = 1;
        s.lose_life();
        assert_eq!(s.lives, 0);
        assert_eq!(s.phase, Phase::Lost);
    }

    #[test]
    fn restart_restores_everything() {
        let mut s = GameState::new();
        s.score = 500;
        s.lives = 1;
        s.bricks[0].hits = 0;
        s.restart();
        assert_eq!(s.score, 0);
        assert_eq!(s.lives, STARTING_LIVES);
        assert_eq!(s.bricks_remaining(), BRICK_COLS * BRICK_ROWS);
    }
}

#[cfg(test)]
mod multiball_state_tests {
    use super::*;

    #[test]
    fn a_new_game_has_exactly_one_ball_resting_on_the_paddle() {
        let s = GameState::new();
        assert_eq!(s.balls.len(), 1);
        assert_eq!(s.balls[0].vel, Vec2::ZERO);
        assert!(s.balls[0].trail.is_empty());
        assert!(!s.balls[0].drained);
        assert_eq!(s.level, 1);
    }

    #[test]
    fn launch_sends_every_ball_out() {
        let mut s = GameState::new();
        s.launch();
        assert_eq!(s.phase, Phase::Playing);
        for b in &s.balls {
            assert!(b.vel.y < 0.0);
            assert!((b.vel.length() - BALL_SPEED).abs() < 0.01);
        }
    }

    /// Every route back to Ready discards the extra balls: a lost life, a
    /// new level, a restart. Multi-ball exists only during Playing.
    #[test]
    fn losing_a_life_collapses_back_to_one_ball() {
        let mut s = GameState::new();
        s.launch();
        s.spawn_ball(Vec2::new(300.0, 300.0), Vec2::new(0.0, -100.0));
        s.spawn_ball(Vec2::new(500.0, 300.0), Vec2::new(0.0, -100.0));
        assert_eq!(s.balls.len(), 3);

        s.lose_life();
        assert_eq!(s.phase, Phase::Ready);
        assert_eq!(s.balls.len(), 1, "Ready holds exactly one ball");
        assert_eq!(s.balls[0].vel, Vec2::ZERO, "and it is at rest on the paddle");
    }

    #[test]
    fn a_spawned_ball_inherits_the_radius_and_starts_clean() {
        let mut s = GameState::new();
        s.launch();
        assert!(s.spawn_ball(Vec2::new(400.0, 300.0), Vec2::new(10.0, -10.0)));
        let b = &s.balls[1];
        assert_eq!(b.radius, BALL_RADIUS);
        assert!(b.trail.is_empty());
        assert!(!b.drained);
    }

    /// Retiring is a no-op when nothing drained — it must not cost a life
    /// or disturb the vec just for being called every tick.
    #[test]
    fn retiring_with_nothing_drained_changes_nothing() {
        let mut s = GameState::new();
        s.launch();
        s.spawn_ball(Vec2::new(300.0, 300.0), Vec2::ZERO);
        let lives = s.lives;
        let n = s.balls.len();

        s.retire_drained_balls();

        assert_eq!(s.lives, lives);
        assert_eq!(s.balls.len(), n);
    }

    /// Removal walks backwards so earlier indices stay valid. With the
    /// middle ball drained, the two survivors must be the outer two — a
    /// forward-walking removal would shift and drop the wrong one.
    #[test]
    fn retiring_removes_exactly_the_drained_balls() {
        let mut s = GameState::new();
        s.launch();
        s.balls[0].pos = Vec2::new(100.0, 300.0);
        s.spawn_ball(Vec2::new(200.0, 300.0), Vec2::ZERO);
        s.spawn_ball(Vec2::new(300.0, 300.0), Vec2::ZERO);

        s.balls[1].drained = true;
        s.retire_drained_balls();

        assert_eq!(s.balls.len(), 2);
        assert_eq!(s.balls[0].pos.x, 100.0);
        assert_eq!(s.balls[1].pos.x, 300.0, "the survivor after the gap must be the last one");
    }

    #[test]
    fn restart_returns_to_one_ball_and_level_one() {
        let mut s = GameState::new();
        s.launch();
        s.level = 7;
        s.spawn_ball(Vec2::new(300.0, 300.0), Vec2::ZERO);
        s.restart();
        assert_eq!(s.balls.len(), 1);
        assert_eq!(s.level, 1);
        assert_eq!(s.lives, STARTING_LIVES);
    }
}
