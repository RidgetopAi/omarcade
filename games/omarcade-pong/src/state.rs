//! The world, with no behaviour.
//!
//! Nothing here moves anything. Every rule that advances time lives in
//! `physics.rs`, every decision the opponent makes lives in `ai.rs`,
//! and every rule that draws lives in `render.rs`. The payoff is the
//! same one Breakout gets: a test can construct "match point, ball
//! about to reach the far paddle" directly, instead of playing until
//! that situation happens.

use omarcade_core::geom::{Rect, Vec2};

/// Play-field size in logical units.
///
/// Same field as Breakout, for the same reason: gameplay happens
/// entirely in these coordinates and the renderer scales to whatever
/// size Hyprland hands us, so the game plays identically at any window
/// size.
pub const FIELD_W: f32 = 960.0;
pub const FIELD_H: f32 = 720.0;

/// First to this many points takes the match. Pong has been first-to-11
/// since 1972; the number is not arbitrary and not ours to improve.
pub const MATCH_POINT: u32 = 11;

/// Paddle thickness, and how far its face sits from the wall behind it.
pub const PADDLE_W: f32 = 16.0;
pub const PADDLE_INSET: f32 = 40.0;

/// How fast a paddle travels.
///
/// Measured, not inherited. Breakout's 600 came over first and made the
/// game unplayable in a way that only showed up in a probe: at that
/// speed a paddle covers 1,100+ units while the ball crosses a field
/// only 720 tall, so a perfect tracker reaches EVERY legal shot and no
/// rally can ever end. Both sides returned everything and matches timed
/// out at 0-0.
///
/// Pong's geometry is not Breakout's. The paddle travels the same axis
/// the ball has to be beaten on, so paddle speed IS the difficulty of
/// covering the field, and it has to be tuned against the crossing time
/// rather than against how twitchy it feels. At 300, against the ball
/// speeds below, a steep shot leaves the defender covering roughly
/// 70-100% of the field: marginal, which is what makes reaching it a
/// play rather than a formality.
pub const PADDLE_SPEED: f32 = 300.0;

pub const BALL_RADIUS: f32 = 8.0;

/// How many past ball positions the trail keeps. Long enough to read as
/// motion, short enough that a slow ball does not smear.
pub const TRAIL_LEN: usize = 10;

/// Which end of the field.
///
/// The player is always [`Side::Left`]. Making this an enum rather than
/// a bool means `score[side]` and "whose paddle is this" ask the same
/// question of the same value, so they cannot disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    pub fn other(self) -> Side {
        match self {
            Side::Left => Side::Right,
            Side::Right => Side::Left,
        }
    }

    /// Which way this side's paddle sends the ball: away from its own
    /// wall.
    pub fn outward(self) -> f32 {
        match self {
            Side::Left => 1.0,
            Side::Right => -1.0,
        }
    }
}

/// How hard the game is.
///
/// Expressed as paddle size and ball speed rather than as a hidden
/// competence number, because both are things the player can SEE. A
/// difficulty the player can watch is one they can trust; a difficulty
/// that only lives in the opponent's aim error feels like the game is
/// lying about something.
///
/// The AI's own competence rides along on top (see `ai.rs`), so Easy is
/// a genuinely different game rather than the same game against a
/// worse opponent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

impl Difficulty {
    /// The order the select screen presents, and the order the cabinet
    /// lists them.
    pub const ALL: [Difficulty; 3] = [Difficulty::Easy, Difficulty::Normal, Difficulty::Hard];

    /// Stable identifier written into the score file. Lowercase and
    /// ASCII on purpose: it is a key the marquee groups by, so it is
    /// public surface like `GAME_ID` — renaming one orphans those
    /// scores into a difficulty nobody plays.
    pub fn id(self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Normal => "normal",
            Difficulty::Hard => "hard",
        }
    }

    /// Label for the select screen and the HUD. The 5x7 font covers
    /// A-Z, 0-9, space and dash only, so these stay uppercase ASCII.
    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Easy => "EASY",
            Difficulty::Normal => "NORMAL",
            Difficulty::Hard => "HARD",
        }
    }

    /// Half the paddle's height. The visible half of the difficulty.
    pub fn paddle_half_h(self) -> f32 {
        match self {
            Difficulty::Easy => 70.0,
            Difficulty::Normal => 50.0,
            Difficulty::Hard => 34.0,
        }
    }

    /// Ball speed at the start of a rally, before the ramp. The other
    /// visible half.
    ///
    /// These are large numbers next to Breakout's 340, and they have to
    /// be. In Breakout the ball only has to beat a paddle across the
    /// bottom of the field; here it has to beat one that starts already
    /// tracking it, so what decides a point is whether the ball crosses
    /// faster than a paddle can cover the y-distance. Measured against
    /// this field and PADDLE_SPEED, a shot only becomes unanswerable
    /// somewhere north of 600 — below that a competent tracker reaches
    /// everything and the rally never ends.
    pub fn ball_speed(self) -> f32 {
        match self {
            Difficulty::Easy => 520.0,
            Difficulty::Normal => 570.0,
            Difficulty::Hard => 660.0,
        }
    }

    /// Ceiling the rally ramp climbs towards, as a multiple of
    /// [`ball_speed`](Self::ball_speed).
    ///
    /// Easy ramps least: level 1 is where someone finds out whether the
    /// game is fun, and a rally that accelerates hard punishes exactly
    /// the player still learning to return it.
    pub fn ramp_ceiling(self) -> f32 {
        match self {
            Difficulty::Easy => 1.30,
            Difficulty::Normal => 1.45,
            Difficulty::Hard => 1.55,
        }
    }

    pub fn next(self) -> Difficulty {
        match self {
            Difficulty::Easy => Difficulty::Normal,
            Difficulty::Normal => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Easy,
        }
    }

    pub fn prev(self) -> Difficulty {
        match self {
            Difficulty::Easy => Difficulty::Hard,
            Difficulty::Normal => Difficulty::Easy,
            Difficulty::Hard => Difficulty::Normal,
        }
    }
}

/// Where the game is in its lifecycle.
///
/// Explicit states rather than a scatter of booleans, so "are we
/// choosing a difficulty" and "is the match over" are the same question
/// asked of one value and cannot contradict each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Choosing a difficulty. Where a fresh game starts.
    Select,
    /// Ball is parked, waiting to be served.
    Serve,
    Playing,
    /// Someone reached [`MATCH_POINT`]. `winner` says who.
    Over { winner: Side },
}

/// A vertical paddle, positioned by its top edge.
#[derive(Debug, Clone, Copy)]
pub struct Paddle {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// -1 up, 0 still, +1 down. Set from input or by the AI, consumed
    /// by physics. Neither writes position directly — that keeps one
    /// file in charge of movement, so the AI cannot teleport.
    pub dir: f32,
}

impl Paddle {
    pub fn rect(&self) -> Rect {
        Rect::new(self.x, self.y, self.w, self.h)
    }

    pub fn center_y(&self) -> f32 {
        self.y + self.h / 2.0
    }

    /// The face the ball strikes.
    pub fn face_x(&self, side: Side) -> f32 {
        match side {
            Side::Left => self.x + self.w,
            Side::Right => self.x,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Ball {
    pub pos: Vec2,
    pub vel: Vec2,
    pub radius: f32,
}

impl Ball {
    /// The ball as a rect, which is how collision sees it. A square
    /// standing in for a circle is the standard simplification: at this
    /// size the difference is invisible and every collision stays a
    /// single AABB test.
    pub fn rect(&self) -> Rect {
        Rect::from_center(self.pos, self.radius, self.radius)
    }
}

#[derive(Debug, Clone)]
pub struct GameState {
    pub difficulty: Difficulty,
    pub phase: Phase,

    pub left: Paddle,
    pub right: Paddle,
    pub ball: Ball,

    pub score_left: u32,
    pub score_right: u32,

    /// How many times the ball has been returned in the current rally.
    /// Drives the speed ramp, and is reset by every point.
    pub rally: u32,
    /// Longest rally of the match so far — the number worth showing at
    /// the end, since it describes the play rather than the result.
    pub longest_rally: u32,

    /// Which side serves next. The player who was scored ON serves,
    /// which is the arcade convention and gives the loser of a point
    /// the tempo back.
    pub serving: Side,

    /// Best result on record for the current difficulty, shown on the
    /// end screen. Owned by the caller: the simulation never sets it,
    /// so the headless harnesses see 0 and stay deterministic.
    pub best: u32,

    /// Recent ball positions, newest first, for the motion trail.
    ///
    /// Presentation state in the world model on purpose: physics is the
    /// only thing that knows where the ball has actually been, and
    /// sampling it in `render` would tie the trail to frame rate
    /// instead of to the fixed timestep.
    pub trail: Vec<Vec2>,
}

impl GameState {
    /// A fresh game, sitting on the difficulty select.
    pub fn new() -> Self {
        Self::with_difficulty(Difficulty::Normal)
    }

    /// A fresh game at a chosen difficulty, still on the select screen.
    pub fn with_difficulty(difficulty: Difficulty) -> Self {
        let half_h = difficulty.paddle_half_h();
        let mid = FIELD_H / 2.0;

        GameState {
            difficulty,
            phase: Phase::Select,
            left: Paddle {
                x: PADDLE_INSET,
                y: mid - half_h,
                w: PADDLE_W,
                h: half_h * 2.0,
                dir: 0.0,
            },
            right: Paddle {
                x: FIELD_W - PADDLE_INSET - PADDLE_W,
                y: mid - half_h,
                w: PADDLE_W,
                h: half_h * 2.0,
                dir: 0.0,
            },
            ball: Ball {
                pos: Vec2::new(FIELD_W / 2.0, mid),
                vel: Vec2::ZERO,
                radius: BALL_RADIUS,
            },
            score_left: 0,
            score_right: 0,
            rally: 0,
            longest_rally: 0,
            serving: Side::Left,
            best: 0,
            trail: Vec::new(),
        }
    }

    /// The play field as a rect.
    pub fn field(&self) -> Rect {
        Rect::new(0.0, 0.0, FIELD_W, FIELD_H)
    }

    pub fn paddle(&self, side: Side) -> &Paddle {
        match side {
            Side::Left => &self.left,
            Side::Right => &self.right,
        }
    }

    pub fn paddle_mut(&mut self, side: Side) -> &mut Paddle {
        match side {
            Side::Left => &mut self.left,
            Side::Right => &mut self.right,
        }
    }

    pub fn score(&self, side: Side) -> u32 {
        match side {
            Side::Left => self.score_left,
            Side::Right => self.score_right,
        }
    }

    /// Apply the current difficulty's paddle size, keeping each paddle
    /// centred on where it already was.
    ///
    /// Resizing from the top edge would make a difficulty change nudge
    /// both paddles upward, which reads as the game twitching.
    pub fn apply_difficulty(&mut self) {
        let h = self.difficulty.paddle_half_h() * 2.0;
        for side in [Side::Left, Side::Right] {
            let p = self.paddle_mut(side);
            let c = p.center_y();
            p.h = h;
            p.y = (c - h / 2.0).clamp(0.0, FIELD_H - h);
        }
    }

    /// Leave the select screen and start the match.
    pub fn begin(&mut self) {
        self.apply_difficulty();
        self.phase = Phase::Serve;
        self.park_ball();
    }

    /// Sit the ball at the centre, still, ready to be served.
    pub fn park_ball(&mut self) {
        self.ball.pos = Vec2::new(FIELD_W / 2.0, FIELD_H / 2.0);
        self.ball.vel = Vec2::ZERO;
        self.trail.clear();
    }

    /// Award a point to `side` and set up the next serve, or end the
    /// match if this was the eleventh.
    pub fn award(&mut self, side: Side) {
        match side {
            Side::Left => self.score_left += 1,
            Side::Right => self.score_right += 1,
        }
        self.longest_rally = self.longest_rally.max(self.rally);
        self.rally = 0;

        if self.score(side) >= MATCH_POINT {
            self.phase = Phase::Over { winner: side };
            self.park_ball();
        } else {
            // The side that conceded serves next.
            self.serving = side.other();
            self.phase = Phase::Serve;
            self.park_ball();
        }
    }

    /// Start a new match at the same difficulty.
    pub fn restart(&mut self) {
        let difficulty = self.difficulty;
        let best = self.best;
        *self = GameState::with_difficulty(difficulty);
        self.best = best;
        self.begin();
    }

    pub fn is_over(&self) -> bool {
        matches!(self.phase, Phase::Over { .. })
    }
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_game_starts_on_the_select_screen() {
        let s = GameState::new();
        assert_eq!(s.phase, Phase::Select);
        assert_eq!(s.score_left, 0);
        assert_eq!(s.score_right, 0);
        assert_eq!(s.ball.vel, Vec2::ZERO);
    }

    #[test]
    fn difficulty_ids_are_stable_and_distinct() {
        // These are keys the marquee groups by — public surface.
        let ids: Vec<&str> = Difficulty::ALL.iter().map(|d| d.id()).collect();
        assert_eq!(ids, vec!["easy", "normal", "hard"]);
    }

    #[test]
    fn difficulty_labels_use_only_glyphs_the_font_has() {
        // The 5x7 font covers A-Z 0-9 space dash and SKIPS anything else
        // silently — "BEST" once shipped as "EST".
        for d in Difficulty::ALL {
            for ch in d.label().chars() {
                assert!(
                    ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == ' ' || ch == '-',
                    "{:?} label has an unrenderable char {ch:?}",
                    d
                );
            }
        }
    }

    #[test]
    fn harder_means_a_smaller_paddle_and_a_faster_ball() {
        // The whole design claim: difficulty is two things the player
        // can SEE. If this ever inverts, the selector is lying.
        let (e, n, h) = (Difficulty::Easy, Difficulty::Normal, Difficulty::Hard);
        assert!(e.paddle_half_h() > n.paddle_half_h());
        assert!(n.paddle_half_h() > h.paddle_half_h());
        assert!(e.ball_speed() < n.ball_speed());
        assert!(n.ball_speed() < h.ball_speed());
    }

    #[test]
    fn easy_ramps_the_least() {
        assert!(Difficulty::Easy.ramp_ceiling() < Difficulty::Normal.ramp_ceiling());
        assert!(Difficulty::Normal.ramp_ceiling() < Difficulty::Hard.ramp_ceiling());
        // Every tier must actually ramp, or the rally has no arc.
        for d in Difficulty::ALL {
            assert!(d.ramp_ceiling() > 1.0, "{d:?} does not ramp at all");
        }
    }

    #[test]
    fn difficulty_cycles_both_ways() {
        for d in Difficulty::ALL {
            assert_eq!(d.next().prev(), d);
            assert_eq!(d.prev().next(), d);
        }
    }

    #[test]
    fn sides_are_opposites_that_point_apart() {
        assert_eq!(Side::Left.other(), Side::Right);
        assert_eq!(Side::Right.other(), Side::Left);
        assert!(Side::Left.outward() > 0.0, "left sends the ball rightward");
        assert!(Side::Right.outward() < 0.0);
    }

    #[test]
    fn paddles_face_each_other_across_the_field() {
        let s = GameState::new();
        // Left's striking face is its right edge, and vice versa.
        assert!(s.left.face_x(Side::Left) < s.right.face_x(Side::Right));
        assert!(s.left.face_x(Side::Left) < FIELD_W / 2.0);
        assert!(s.right.face_x(Side::Right) > FIELD_W / 2.0);
    }

    #[test]
    fn resizing_for_difficulty_keeps_a_paddle_centred() {
        let mut s = GameState::with_difficulty(Difficulty::Easy);
        s.begin();
        s.left.y = 300.0;
        let before = s.left.center_y();

        s.difficulty = Difficulty::Hard;
        s.apply_difficulty();

        assert!(
            (s.left.center_y() - before).abs() < 1e-3,
            "a difficulty change must not nudge the paddle: {before} -> {}",
            s.left.center_y()
        );
        assert!(s.left.h < s.right.h + 1e-3);
    }

    #[test]
    fn resizing_near_an_edge_stays_inside_the_field() {
        let mut s = GameState::with_difficulty(Difficulty::Hard);
        s.begin();
        s.left.y = 0.0;
        s.difficulty = Difficulty::Easy;
        s.apply_difficulty();

        assert!(s.left.y >= 0.0);
        assert!(s.left.y + s.left.h <= FIELD_H + 1e-3);
    }

    #[test]
    fn a_point_resets_the_rally_and_hands_over_the_serve() {
        let mut s = GameState::new();
        s.begin();
        s.rally = 7;
        s.award(Side::Left);

        assert_eq!(s.score_left, 1);
        assert_eq!(s.rally, 0);
        assert_eq!(s.longest_rally, 7, "the rally is remembered after it ends");
        assert_eq!(s.serving, Side::Right, "the side scored ON serves next");
        assert_eq!(s.phase, Phase::Serve);
        assert_eq!(s.ball.vel, Vec2::ZERO);
    }

    #[test]
    fn the_eleventh_point_ends_the_match() {
        let mut s = GameState::new();
        s.begin();
        for _ in 0..(MATCH_POINT - 1) {
            s.award(Side::Left);
            assert!(!s.is_over(), "not over at {}", s.score_left);
        }
        s.award(Side::Left);
        assert_eq!(s.phase, Phase::Over { winner: Side::Left });
        assert_eq!(s.score_left, MATCH_POINT);
    }

    #[test]
    fn either_side_can_win() {
        let mut s = GameState::new();
        s.begin();
        for _ in 0..MATCH_POINT {
            s.award(Side::Right);
        }
        assert_eq!(s.phase, Phase::Over { winner: Side::Right });
    }

    #[test]
    fn a_longer_rally_replaces_the_record_but_a_shorter_one_does_not() {
        let mut s = GameState::new();
        s.begin();
        s.rally = 12;
        s.award(Side::Left);
        s.rally = 4;
        s.award(Side::Right);
        assert_eq!(s.longest_rally, 12);
    }

    #[test]
    fn restart_keeps_the_difficulty_and_the_best_but_clears_the_score() {
        let mut s = GameState::with_difficulty(Difficulty::Hard);
        s.begin();
        s.best = 9;
        s.score_left = 5;
        s.score_right = 7;
        s.longest_rally = 30;
        s.restart();

        assert_eq!(s.difficulty, Difficulty::Hard);
        assert_eq!(s.best, 9, "the record survives a restart");
        assert_eq!(s.score_left, 0);
        assert_eq!(s.score_right, 0);
        assert_eq!(s.longest_rally, 0);
        assert_eq!(s.phase, Phase::Serve, "restart goes straight to play");
    }
}
