//! Measure the opponent against a player who CANNOT SEE THE FUTURE.
//!
//! `probe_ai` says every tier is distinct and ordered, and it passes.
//! The author of the game cannot beat EASY. Both of those are true at
//! once, which means the probe is answering a question nobody asked.
//!
//! The reason is one line in `probe_ai`: its benchmark player calls
//! `ai::predict_intercept` — the same forward simulation the OPPONENT
//! uses. So the yardstick is structurally a second AI. It learns where
//! the ball will arrive the instant the ball turns toward it, while the
//! ball is still most of a field away, and then drifts into place over
//! the whole flight. It is never late, so it never has to sprint, so
//! a limit on how fast a paddle can cross the field CANNOT BITE IT.
//!
//! A person does not get that. A person watches the ball, infers the
//! angle from what it is doing NOW, commits, sees the error, corrects.
//! Every correction costs field position, and position is the thing
//! there is not enough of — `PADDLE_SPEED` is 300 px/s and the field is
//! 720 px tall, so crossing it takes 2.4 s, and no rally lasts that
//! long.
//!
//! So this probe replaces the yardstick with a REACTIVE player:
//!
//! 1. **No predictor.** It reads `ball.pos` and `ball.vel` only, and
//!    extrapolates in a straight line — it does not reflect off walls.
//!    A bank shot therefore fools it exactly the way one fools a person
//!    who has not read the angle yet.
//! 2. **It re-decides continuously**, on its reaction clock, so as the
//!    ball gets closer its straight-line guess gets better. That is
//!    what tracking a ball actually feels like: wrong early, right
//!    late, and whether "right late" is soon enough depends entirely on
//!    how far you have to go.
//! 3. **It is bound by `PADDLE_SPEED`**, like the player, like the AI.
//!
//! Run it beside the predictive one:
//!
//!   cargo run --release -q -p omarcade-volley --example probe_reactive
//!
//! What to look for: the PREDICTIVE and REACTIVE columns for the same
//! tier. If reactive is far worse, the difficulty cliff in `probe_ai`
//! is an artifact of the yardstick's predictor, and the tier numbers
//! never described a human.

#[path = "../src/ai.rs"]
mod ai;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/state.rs"]
mod state;

use ai::Opponent;
use physics::{FIXED_DT, serve, step_fixed};
use state::{Difficulty, FIELD_H, GameState, MATCH_POINT, Phase, Side};

/// How close to the target the paddle must be before it stops, matching
/// the deadband the AI and the predictive bench both use.
const SETTLE: f32 = 6.0;

/// A player of a known standard who reads the ball rather than solving
/// it.
///
/// The fields deliberately mirror `probe_ai::Bench` so the two probes
/// are comparable dial for dial. The ONE difference is behavioural and
/// lives in `update`: no `predict_intercept`.
#[derive(Clone, Copy)]
struct Reactive {
    /// How much of the paddle's speed it can use.
    speed: f32,
    /// Seconds between decisions — its reaction time.
    reaction: f32,
    /// How far off it aims, in field units.
    slop: f32,
    /// How far ahead it extrapolates the ball's CURRENT velocity, in
    /// seconds. This is anticipation, not prediction: a person leads a
    /// moving ball a little, but leads it along the line it is on, and
    /// is wrong the moment it hits a wall.
    lead: f32,
}

impl Reactive {
    /// Someone who has played Pong twice. Barely leads the ball.
    const POOR: Reactive = Reactive {
        speed: 0.70,
        reaction: 0.20,
        slop: 60.0,
        lead: 0.05,
    };
    /// The player EASY is actually for: has played a few times, leads
    /// the ball a little, still slow and sloppy. The gap between POOR
    /// and FAIR was where the original cliff hid, so the ladder needs a
    /// rung in it.
    const LEARNING: Reactive = Reactive {
        speed: 0.78,
        reaction: 0.15,
        slop: 42.0,
        lead: 0.09,
    };
    /// A normal person paying attention.
    const FAIR: Reactive = Reactive {
        speed: 0.88,
        reaction: 0.10,
        slop: 28.0,
        lead: 0.12,
    };
    /// Someone who is good at this: fast hands, reads the line well,
    /// still cannot see through a wall.
    const GOOD: Reactive = Reactive {
        speed: 1.0,
        reaction: 0.045,
        slop: 10.0,
        lead: 0.25,
    };
}

struct ReactivePlayer {
    cfg: Reactive,
    target: f32,
    cooldown: f32,
    ticks: u32,
    /// Ticks spent moving flat-out, and ticks spent at the target. The
    /// ratio is the interesting part: a player who is ALWAYS sprinting
    /// is a player the field is too big for.
    sprint_ticks: u32,
    total_ticks: u32,
}

impl ReactivePlayer {
    fn new(cfg: Reactive) -> Self {
        ReactivePlayer {
            cfg,
            target: FIELD_H / 2.0,
            cooldown: 0.0,
            ticks: 0,
            sprint_ticks: 0,
            total_ticks: 0,
        }
    }

    fn update(&mut self, state: &mut GameState) {
        self.ticks = self.ticks.wrapping_add(1);
        self.cooldown -= FIXED_DT;

        if self.cooldown <= 0.0 {
            self.cooldown = self.cfg.reaction;
            let wobble = match self.ticks % 4 {
                0 => 1.0,
                1 => -0.6,
                2 => 0.35,
                _ => -1.0,
            };

            let aim = if state.ball.vel.x < 0.0 {
                // Closing. Extrapolate along the line the ball is on
                // RIGHT NOW — no wall reflection, so a ball about to
                // bank is read wrong, which is the entire point.
                let y = state.ball.pos.y + state.ball.vel.y * self.cfg.lead;
                // It does know the ball stays on the field; it just
                // does not know where the bounce puts it.
                y.clamp(0.0, FIELD_H)
            } else {
                // Heading away: hold the middle.
                FIELD_H / 2.0
            };
            self.target = aim + wobble * self.cfg.slop;
        }

        let p = state.paddle_mut(Side::Left);
        let delta = self.target - p.center_y();
        p.dir = if delta.abs() < SETTLE {
            0.0
        } else if delta > 0.0 {
            self.cfg.speed
        } else {
            -self.cfg.speed
        };

        self.total_ticks = self.total_ticks.wrapping_add(1);
        if p.dir != 0.0 {
            self.sprint_ticks = self.sprint_ticks.wrapping_add(1);
        }
    }
}

/// The predictive yardstick from `probe_ai`, reproduced exactly so both
/// players can be run in ONE process against identical openings. Any
/// difference in the numbers is then the player, not the harness.
#[derive(Clone, Copy)]
struct Predictive {
    speed: f32,
    reaction: f32,
    slop: f32,
    depth: u32,
}

impl Predictive {
    const POOR: Predictive = Predictive {
        speed: 0.70,
        reaction: 0.20,
        slop: 60.0,
        depth: 1,
    };
    const FAIR: Predictive = Predictive {
        speed: 0.88,
        reaction: 0.10,
        slop: 28.0,
        depth: 2,
    };
    const GOOD: Predictive = Predictive {
        speed: 1.0,
        reaction: 0.045,
        slop: 10.0,
        depth: 4,
    };
}

struct PredictivePlayer {
    cfg: Predictive,
    target: f32,
    cooldown: f32,
    ticks: u32,
    sprint_ticks: u32,
    total_ticks: u32,
}

impl PredictivePlayer {
    fn new(cfg: Predictive) -> Self {
        PredictivePlayer {
            cfg,
            target: FIELD_H / 2.0,
            cooldown: 0.0,
            ticks: 0,
            sprint_ticks: 0,
            total_ticks: 0,
        }
    }

    fn update(&mut self, state: &mut GameState) {
        self.ticks = self.ticks.wrapping_add(1);
        self.cooldown -= FIXED_DT;

        if self.cooldown <= 0.0 {
            self.cooldown = self.cfg.reaction;
            let wobble = match self.ticks % 4 {
                0 => 1.0,
                1 => -0.6,
                2 => 0.35,
                _ => -1.0,
            };
            let face = state.paddle(Side::Left).face_x(Side::Left);
            let aim = if state.ball.vel.x < 0.0 {
                ai::predict_intercept(
                    state.ball.pos,
                    state.ball.vel,
                    face,
                    state.ball.radius,
                    self.cfg.depth,
                )
                .unwrap_or(state.ball.pos.y)
            } else {
                FIELD_H / 2.0
            };
            self.target = aim + wobble * self.cfg.slop;
        }

        let p = state.paddle_mut(Side::Left);
        let delta = self.target - p.center_y();
        p.dir = if delta.abs() < SETTLE {
            0.0
        } else if delta > 0.0 {
            self.cfg.speed
        } else {
            -self.cfg.speed
        };

        self.total_ticks = self.total_ticks.wrapping_add(1);
        if p.dir != 0.0 {
            self.sprint_ticks = self.sprint_ticks.wrapping_add(1);
        }
    }
}

/// Either kind of player, so one match loop serves both.
enum Player {
    Reactive(ReactivePlayer),
    Predictive(PredictivePlayer),
}

impl Player {
    fn update(&mut self, state: &mut GameState) {
        match self {
            Player::Reactive(p) => p.update(state),
            Player::Predictive(p) => p.update(state),
        }
    }

    fn set_ticks(&mut self, v: u32) {
        match self {
            Player::Reactive(p) => p.ticks = v,
            Player::Predictive(p) => p.ticks = v,
        }
    }

    /// Fraction of ticks spent moving rather than parked on target.
    fn sprint_fraction(&self) -> f32 {
        let (s, t) = match self {
            Player::Reactive(p) => (p.sprint_ticks, p.total_ticks),
            Player::Predictive(p) => (p.sprint_ticks, p.total_ticks),
        };
        if t == 0 { 0.0 } else { s as f32 / t as f32 }
    }
}

struct Outcome {
    ai_won: bool,
    ai_points: u32,
    player_points: u32,
    longest_rally: u32,
    sprint_fraction: f32,
}

fn play_match(difficulty: Difficulty, mut player: Player, variant: u32) -> Outcome {
    let mut s = GameState::with_difficulty(difficulty);
    s.begin();

    let mut opponent = Opponent::new(Side::Right, difficulty);
    // Identical opening to probe_ai, so the two are comparable.
    s.serving = if variant % 2 == 0 {
        Side::Left
    } else {
        Side::Right
    };
    player.set_ticks(variant);
    let start = (variant % 7) as f32 * 40.0;
    s.left.y = start.min(FIELD_H - s.left.h);

    for _ in 0..(240 * 600) {
        if s.is_over() {
            break;
        }
        if s.phase == Phase::Serve {
            serve(&mut s);
        }
        player.update(&mut s);
        opponent.update(&mut s, FIXED_DT);
        step_fixed(&mut s);
    }

    Outcome {
        ai_won: s.score_right > s.score_left,
        ai_points: s.score_right,
        player_points: s.score_left,
        longest_rally: s.longest_rally,
        sprint_fraction: player.sprint_fraction(),
    }
}

struct Cell {
    win_pct: f32,
    ai_avg: f32,
    player_avg: f32,
    longest: u32,
    sprint: f32,
}

fn measure(difficulty: Difficulty, make: impl Fn() -> Player, matches: u32) -> Cell {
    let mut wins = 0u32;
    let mut ai_pts = 0u32;
    let mut player_pts = 0u32;
    let mut longest = 0u32;
    let mut sprint = 0.0f32;

    for v in 0..matches {
        let o = play_match(difficulty, make(), v);
        if o.ai_won {
            wins += 1;
        }
        ai_pts += o.ai_points;
        player_pts += o.player_points;
        longest = longest.max(o.longest_rally);
        sprint += o.sprint_fraction;
    }

    Cell {
        win_pct: 100.0 * wins as f32 / matches as f32,
        ai_avg: ai_pts as f32 / matches as f32,
        player_avg: player_pts as f32 / matches as f32,
        longest,
        sprint: sprint / matches as f32,
    }
}

fn main() {
    const MATCHES: u32 = 60;

    println!("Volley opponent — PREDICTIVE vs REACTIVE yardsticks, {MATCHES} matches per cell");
    println!(
        "Paddle speed PER TIER: easy {:.0} / normal {:.0} / hard {:.0}; match to {MATCH_POINT}",
        Difficulty::Easy.paddle_speed(),
        Difficulty::Normal.paddle_speed(),
        Difficulty::Hard.paddle_speed()
    );
    println!(
        "Full-field traversal: easy {:.2}s / normal {:.2}s / hard {:.2}s.\n",
        FIELD_H / Difficulty::Easy.paddle_speed(),
        FIELD_H / Difficulty::Normal.paddle_speed(),
        FIELD_H / Difficulty::Hard.paddle_speed()
    );
    println!("The ONLY difference between the two players is whether they may call");
    println!("predict_intercept. Same speed, same reaction, same slop, same openings.\n");

    println!(
        "{:<7} {:<7} {:>11} {:>10} {:>8} {:>9} {:>8}",
        "AI", "PLAYER", "AI WIN%", "AVG SCORE", "LONGEST", "SPRINT%", "DELTA"
    );
    println!("{}", "-".repeat(70));

    let mut reactive_vs_fair = Vec::new();

    for d in Difficulty::ALL {
        for (label, pred, react) in [
            ("poor", Predictive::POOR, Reactive::POOR),
            ("learn", Predictive::FAIR, Reactive::LEARNING),
            ("fair", Predictive::FAIR, Reactive::FAIR),
            ("good", Predictive::GOOD, Reactive::GOOD),
        ] {
            let p = measure(
                d,
                || Player::Predictive(PredictivePlayer::new(pred)),
                MATCHES,
            );
            let r = measure(d, || Player::Reactive(ReactivePlayer::new(react)), MATCHES);

            println!(
                "{:<7} {:<7} {:>10.0}% {:>6.1}-{:<3.1} {:>8} {:>8.0}% {:>8}",
                d.label(),
                format!("{label} P"),
                p.win_pct,
                p.ai_avg,
                p.player_avg,
                p.longest,
                100.0 * p.sprint,
                ""
            );
            println!(
                "{:<7} {:<7} {:>10.0}% {:>6.1}-{:<3.1} {:>8} {:>8.0}% {:>+8.0}",
                "",
                format!("{label} R"),
                r.win_pct,
                r.ai_avg,
                r.player_avg,
                r.longest,
                100.0 * r.sprint,
                r.win_pct - p.win_pct
            );

            if label == "fair" {
                reactive_vs_fair.push((d, r.win_pct, p.win_pct));
            }
        }
        println!();
    }

    println!("P = predictive yardstick (what probe_ai measures)");
    println!("R = reactive yardstick (what a person can actually do)");
    println!("SPRINT% = fraction of ticks the paddle spent moving rather than parked.\n");

    println!("Against a FAIR player, AI win rate:");
    for (d, r, p) in &reactive_vs_fair {
        println!(
            "  {:<7} predictive {:>3.0}%   reactive {:>3.0}%   ({:+.0} points harder for a human)",
            d.label(),
            p,
            r,
            r - p
        );
    }

    // The question probe_ai never asks: is EASY winnable by a
    // reactive player of ordinary skill? That is the claim Brian is
    // disputing, so state it as a number rather than a vibe.
    let easy_fair_reactive = reactive_vs_fair
        .iter()
        .find(|(d, _, _)| matches!(d, Difficulty::Easy))
        .map(|(_, r, _)| *r)
        .unwrap_or(0.0);

    println!();
    if easy_fair_reactive >= 50.0 {
        println!(
            "FINDING — EASY beats an attentive reactive player {easy_fair_reactive:.0}% of the time."
        );
        println!("          That is not an easy setting, and probe_ai cannot see it.");
    } else {
        println!(
            "FINDING — EASY loses to an attentive reactive player ({easy_fair_reactive:.0}% AI wins)."
        );
        println!("          The difficulty complaint is NOT explained by the yardstick alone.");
    }
}
