//! The world, with no behaviour.
//!
//! Nothing here moves anything. Every rule that advances time lives in
//! `physics.rs`, and every rule that draws lives in `render.rs`. The
//! payoff is testability: a test can construct "ball one pixel from the
//! last brick with one life left" directly, instead of playing the game
//! until that situation happens.

use crate::geom::{Rect, Vec2};

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
/// Level-1 pace. Later levels are expected to raise this.
pub const BALL_SPEED: f32 = 340.0;

pub const BRICK_COLS: usize = 10;
pub const BRICK_ROWS: usize = 6;
pub const BRICK_W: f32 = 84.0;
pub const BRICK_H: f32 = 28.0;
pub const BRICK_GAP: f32 = 6.0;
/// Empty space above the brick field, leaving room for the HUD.
pub const BRICK_TOP: f32 = 90.0;

pub const STARTING_LIVES: u32 = 3;

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

#[derive(Debug, Clone, Copy)]
pub struct Ball {
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: f32,
}

impl Ball {
    /// The ball as a rect, which is how collision sees it.
    ///
    /// A square standing in for a circle is the standard Breakout
    /// simplification: at this size the difference is invisible, and it
    /// keeps every collision a single AABB test.
    pub fn rect(&self) -> Rect {
        Rect::from_center(self.pos, self.radius, self.radius)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Brick {
    pub rect: Rect,
    pub alive: bool,
    /// Index into the renderer's palette, NOT a resolved colour.
    ///
    /// Storing a `Color` here would freeze the theme into the level, so
    /// a live theme change could not repaint it.
    pub color_index: usize,
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub paddle: Paddle,
    pub ball: Ball,
    pub bricks: Vec<Brick>,
    pub lives: u32,
    pub score: u32,
    pub phase: Phase,
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
            ball: Ball {
                pos: Vec2::ZERO,
                vel: Vec2::ZERO,
                radius: BALL_RADIUS,
            },
            paddle,
            bricks: build_bricks(),
            lives: STARTING_LIVES,
            score: 0,
            phase: Phase::Ready,
        };
        state.rest_ball_on_paddle();
        state
    }

    /// Park the ball on the paddle, motionless.
    ///
    /// Placed one pixel clear of the paddle rather than touching it: at
    /// launch the ball must not already be overlapping, or the first
    /// collision check would immediately bounce it back down.
    pub fn rest_ball_on_paddle(&mut self) {
        self.ball.pos = Vec2::new(
            self.paddle.center_x(),
            self.paddle.y - self.ball.radius - 1.0,
        );
        self.ball.vel = Vec2::ZERO;
    }

    /// Send the ball on its way, upward and slightly angled.
    ///
    /// Never straight up: a perfectly vertical ball in a brick corridor
    /// bounces forever on the same column and the game stalls.
    pub fn launch(&mut self) {
        if self.phase != Phase::Ready {
            return;
        }
        self.ball.vel = Vec2::new(0.35, -1.0).with_length(BALL_SPEED);
        self.phase = Phase::Playing;
    }

    pub fn bricks_remaining(&self) -> usize {
        self.bricks.iter().filter(|b| b.alive).count()
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

    /// Start over, keeping nothing.
    pub fn restart(&mut self) {
        *self = GameState::new();
    }
}

impl Default for GameState {
    fn default() -> Self {
        GameState::new()
    }
}

/// Lay out the brick grid, centred horizontally.
fn build_bricks() -> Vec<Brick> {
    let total_w = BRICK_COLS as f32 * BRICK_W + (BRICK_COLS - 1) as f32 * BRICK_GAP;
    let x0 = (FIELD_W - total_w) / 2.0;

    let mut bricks = Vec::with_capacity(BRICK_COLS * BRICK_ROWS);
    for row in 0..BRICK_ROWS {
        for col in 0..BRICK_COLS {
            bricks.push(Brick {
                rect: Rect::new(
                    x0 + col as f32 * (BRICK_W + BRICK_GAP),
                    BRICK_TOP + row as f32 * (BRICK_H + BRICK_GAP),
                    BRICK_W,
                    BRICK_H,
                ),
                alive: true,
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
        assert!(!s.ball.rect().overlaps(&s.paddle.rect()));
        assert_eq!(s.ball.vel, Vec2::ZERO);
    }

    #[test]
    fn launch_sends_the_ball_upward_at_full_speed() {
        let mut s = GameState::new();
        s.launch();
        assert_eq!(s.phase, Phase::Playing);
        assert!(s.ball.vel.y < 0.0, "must travel up (y grows downward)");
        assert!((s.ball.vel.length() - BALL_SPEED).abs() < 0.01);
    }

    /// A perfectly vertical launch stalls the game in a brick corridor.
    #[test]
    fn launch_is_never_perfectly_vertical() {
        let mut s = GameState::new();
        s.launch();
        assert!(s.ball.vel.x.abs() > 1.0, "vx = {} is too vertical", s.ball.vel.x);
    }

    #[test]
    fn launch_does_nothing_unless_ready() {
        let mut s = GameState::new();
        s.phase = Phase::Playing;
        let before = s.ball.vel;
        s.launch();
        assert_eq!(s.ball.vel, before);
    }

    #[test]
    fn losing_a_life_resets_to_ready() {
        let mut s = GameState::new();
        s.phase = Phase::Playing;
        s.lose_life();
        assert_eq!(s.lives, STARTING_LIVES - 1);
        assert_eq!(s.phase, Phase::Ready);
        assert!(!s.ball.rect().overlaps(&s.paddle.rect()));
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
        s.bricks[0].alive = false;
        s.restart();
        assert_eq!(s.score, 0);
        assert_eq!(s.lives, STARTING_LIVES);
        assert_eq!(s.bricks_remaining(), BRICK_COLS * BRICK_ROWS);
    }
}
