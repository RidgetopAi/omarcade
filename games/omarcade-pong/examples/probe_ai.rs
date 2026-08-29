//! Measure how good the opponent actually is.
//!
//! Difficulty is an empirical claim, not a vibe. "Easy is beatable but
//! not trivial" is either a number or it is a hope, and the only way to
//! get the number is to play the matches — thousands of them, headless,
//! in milliseconds.
//!
//! The opponent plays a scripted benchmark player of a known standard,
//! so the win rate means something specific: not "is the AI good" but
//! "how does it do against a player who tracks with THIS much skill".
//!
//!   cargo run -p omarcade-pong --example probe_ai
//!
//! What to look for: Easy should lose most matches to a mediocre
//! player, Hard should win most against a good one, and every tier
//! should sit somewhere strictly between "never wins" and "never
//! loses". A tier that is 0% or 100% against every benchmark is a
//! difficulty setting that does not exist.

#[path = "../src/ai.rs"]
mod ai;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/state.rs"]
mod state;

use ai::Opponent;
use physics::{serve, step_fixed, FIXED_DT};
use state::{Difficulty, GameState, Phase, Side, FIELD_H, MATCH_POINT, PADDLE_SPEED};

/// A scripted player of a known, fixed standard.
///
/// Not the AI: this is a yardstick, and it must stay simple enough that
/// "Easy loses 70% to Fair" is a statement about the AI rather than
/// about a second opponent nobody tuned.
#[derive(Clone, Copy)]
struct Bench {
    /// How much of the paddle's speed it can use.
    speed: f32,
    /// Seconds between decisions — its reaction time.
    reaction: f32,
    /// How far off it aims, in field units.
    slop: f32,
    /// How many wall bounces it reads through — the same dial the AI
    /// has, so "poor" is poor in a way the AI could also be.
    depth: u32,
}

impl Bench {
    /// Someone who has played Pong twice.
    const POOR: Bench = Bench { speed: 0.70, reaction: 0.20, slop: 60.0, depth: 1 };
    /// A normal person paying attention.
    const FAIR: Bench = Bench { speed: 0.88, reaction: 0.10, slop: 28.0, depth: 2 };
    /// Someone who is good at this.
    const GOOD: Bench = Bench { speed: 1.0, reaction: 0.045, slop: 10.0, depth: 4 };
}

/// The benchmark player's running state.
struct BenchPlayer {
    cfg: Bench,
    target: f32,
    cooldown: f32,
    ticks: u32,
}

impl BenchPlayer {
    fn new(cfg: Bench) -> Self {
        BenchPlayer { cfg, target: FIELD_H / 2.0, cooldown: 0.0, ticks: 0 }
    }

    /// Predict where the ball will arrive, then aim off by `slop`.
    ///
    /// An earlier version tracked the ball's CURRENT y instead, which
    /// measured nothing: against a ball fast enough for a shot to win a
    /// point, a pure tracker is always arriving where the ball just
    /// was, so it lost every match regardless of how the AI played.
    /// A human reads the angle and moves to meet it; the yardstick has
    /// to do the same or it is not a yardstick.
    fn update(&mut self, state: &mut GameState) {
        self.ticks = self.ticks.wrapping_add(1);
        self.cooldown -= FIXED_DT;

        if self.cooldown <= 0.0 {
            self.cooldown = self.cfg.reaction;
            // Deterministic wobble, so a run is reproducible and no
            // probe has to thread a seed around.
            let wobble = match self.ticks % 4 {
                0 => 1.0,
                1 => -0.6,
                2 => 0.35,
                _ => -1.0,
            };
            let face = state.paddle(Side::Left).face_x(Side::Left);
            let aim = if state.ball.vel.x < 0.0 {
                // Closing: read where it will actually arrive.
                ai::predict_intercept(
                    state.ball.pos,
                    state.ball.vel,
                    face,
                    state.ball.radius,
                    self.cfg.depth,
                )
                .unwrap_or(state.ball.pos.y)
            } else {
                // Heading away: hold the middle, which covers the most
                // of whatever comes back.
                FIELD_H / 2.0
            };
            self.target = aim + wobble * self.cfg.slop;
        }

        let p = state.paddle_mut(Side::Left);
        let delta = self.target - p.center_y();
        p.dir = if delta.abs() < 6.0 {
            0.0
        } else if delta > 0.0 {
            self.cfg.speed
        } else {
            -self.cfg.speed
        };
    }
}

struct Outcome {
    ai_won: bool,
    ai_points: u32,
    bench_points: u32,
    longest_rally: u32,
}

/// Play one match to eleven. `variant` shifts the opening so a batch of
/// matches is not the same match played repeatedly.
fn play_match(difficulty: Difficulty, cfg: Bench, variant: u32) -> Outcome {
    let mut s = GameState::with_difficulty(difficulty);
    s.begin();

    let mut opponent = Opponent::new(Side::Right, difficulty);
    let mut bench = BenchPlayer::new(cfg);
    // Vary the opening: who serves, and where the benchmark starts.
    s.serving = if variant % 2 == 0 { Side::Left } else { Side::Right };
    bench.ticks = variant;
    let start = (variant % 7) as f32 * 40.0;
    s.left.y = start.min(FIELD_H - s.left.h);

    // Ten minutes of simulated play is far past any honest match.
    for _ in 0..(240 * 600) {
        if s.is_over() {
            break;
        }
        if s.phase == Phase::Serve {
            serve(&mut s);
        }
        bench.update(&mut s);
        opponent.update(&mut s, FIXED_DT);
        step_fixed(&mut s);
    }

    Outcome {
        ai_won: s.score_right > s.score_left,
        ai_points: s.score_right,
        bench_points: s.score_left,
        longest_rally: s.longest_rally,
    }
}

fn main() {
    const MATCHES: u32 = 60;

    println!("Pong opponent — measured over {MATCHES} matches per cell\n");
    println!("PADDLE_SPEED {PADDLE_SPEED}, match to {MATCH_POINT}\n");

    println!(
        "{:<8} {:<8} {:>7} {:>12} {:>10}",
        "AI", "PLAYER", "AI WIN%", "AVG SCORE", "LONGEST"
    );
    println!("{}", "-".repeat(50));

    let mut failures: Vec<String> = Vec::new();

    for d in Difficulty::ALL {
        for (label, cfg) in [("poor", Bench::POOR), ("fair", Bench::FAIR), ("good", Bench::GOOD)] {
            let mut wins = 0u32;
            let mut ai_pts = 0u32;
            let mut bench_pts = 0u32;
            let mut longest = 0u32;

            for v in 0..MATCHES {
                let o = play_match(d, cfg, v);
                if o.ai_won {
                    wins += 1;
                }
                ai_pts += o.ai_points;
                bench_pts += o.bench_points;
                longest = longest.max(o.longest_rally);
            }

            let pct = 100.0 * wins as f32 / MATCHES as f32;
            println!(
                "{:<8} {:<8} {:>6.0}% {:>7.1}-{:<4.1} {:>10}",
                d.label(),
                label,
                pct,
                ai_pts as f32 / MATCHES as f32,
                bench_pts as f32 / MATCHES as f32,
                longest
            );
        }

        // Every tier must be a real difficulty, not an absolute.
        let vs_fair = (0..MATCHES).filter(|&v| play_match(d, Bench::FAIR, v).ai_won).count();
        let rate = 100.0 * vs_fair as f32 / MATCHES as f32;
        if rate == 0.0 || rate == 100.0 {
            failures.push(format!(
                "{} is {rate:.0}% against a fair player — that is not a difficulty, it is an absolute",
                d.label()
            ));
        }
        println!();
    }

    // The shape the design asks for: harder must actually be harder.
    let win_vs_fair = |d: Difficulty| {
        (0..MATCHES).filter(|&v| play_match(d, Bench::FAIR, v).ai_won).count()
    };
    let (e, n, h) = (
        win_vs_fair(Difficulty::Easy),
        win_vs_fair(Difficulty::Normal),
        win_vs_fair(Difficulty::Hard),
    );
    println!("Against a fair player: easy {e}/{MATCHES}, normal {n}/{MATCHES}, hard {h}/{MATCHES}");

    if !(e < n && n < h) {
        failures.push(format!(
            "difficulty does not increase monotonically: {e} < {n} < {h} is false"
        ));
    }

    if failures.is_empty() {
        println!("\nPASS — every tier is beatable, every tier is distinct, and harder is harder.");
    } else {
        println!();
        for f in &failures {
            println!("FAIL — {f}");
        }
        std::process::exit(1);
    }
}
