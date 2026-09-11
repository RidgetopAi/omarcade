//! Search for tier constants that survive BOTH instruments.
//!
//! `probe_reactive` showed every tier sweeps an attentive human 100-0.
//! `probe_recovery` showed that after a rally, 25-57% of shots are
//! unreachable by a PERFECT player. Two different problems, and a fix
//! for one can easily make the other worse: slowing the ball helps
//! reachability but also gives the opponent more time to get set, and
//! growing the paddle helps both but flattens the tiers into mush.
//!
//! So do not guess. Sweep the candidate space and measure every cell
//! against both properties at once, then read off the settings that
//! satisfy both.
//!
//!   cargo run --release -q -p omarcade-volley --example probe_sweep
//!
//! The two properties a tier must satisfy, stated as numbers:
//!
//! 1. **Reachable.** From a corner, at the ramped ball speed, a perfect
//!    player must be able to reach essentially everything. Under ~95%
//!    means shots exist that no human can answer, and those feel like
//!    the game cheating.
//! 2. **Winnable, and ordered.** The AI's win rate against a REACTIVE
//!    player of the matching standard must land in a band — not 100%
//!    (unplayable) and not 0% (pointless) — and must rise across tiers.
//!
//! The reference standard per tier is deliberate: EASY is measured
//! against a POOR player (someone who has played twice — they should
//! win more than they lose), NORMAL against FAIR, HARD against GOOD.
//! That is what "difficulty" should mean: each tier is pitched at a
//! different person, not at the same person three times.

#[path = "../src/ai.rs"]
mod ai;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/state.rs"]
mod state;

use ai::{Opponent, Skill};
use omarcade_core::geom::Vec2;
use physics::{FIXED_DT, serve, step_fixed};
use state::{
    BALL_RADIUS, Difficulty, FIELD_H, FIELD_W, GameState, PADDLE_INSET, PADDLE_W, Phase, Side,
};

const SETTLE: f32 = 6.0;

/// A candidate tier: every number we are allowed to move.
#[derive(Clone, Copy, Debug)]
struct Candidate {
    paddle_speed: f32,
    paddle_half_h: f32,
    ball_speed: f32,
    ramp_ceiling: f32,
    aim_error: f32,
    reaction: f32,
    depth: u32,
    ai_speed: f32,
}

/// The reactive (non-predicting) yardstick from probe_reactive.
#[derive(Clone, Copy)]
struct Reactive {
    speed: f32,
    reaction: f32,
    slop: f32,
    lead: f32,
}

impl Reactive {
    const POOR: Reactive = Reactive {
        speed: 0.70,
        reaction: 0.20,
        slop: 60.0,
        lead: 0.05,
    };
    const FAIR: Reactive = Reactive {
        speed: 0.88,
        reaction: 0.10,
        slop: 28.0,
        lead: 0.12,
    };
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
}

impl ReactivePlayer {
    fn new(cfg: Reactive, ticks: u32) -> Self {
        ReactivePlayer {
            cfg,
            target: FIELD_H / 2.0,
            cooldown: 0.0,
            ticks,
        }
    }

    fn update(&mut self, s: &mut GameState) {
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
            let aim = if s.ball.vel.x < 0.0 {
                (s.ball.pos.y + s.ball.vel.y * self.cfg.lead).clamp(0.0, FIELD_H)
            } else {
                FIELD_H / 2.0
            };
            self.target = aim + wobble * self.cfg.slop;
        }
        let p = s.paddle_mut(Side::Left);
        let delta = self.target - p.center_y();
        p.dir = if delta.abs() < SETTLE {
            0.0
        } else if delta > 0.0 {
            self.cfg.speed
        } else {
            -self.cfg.speed
        };
    }
}

/// Apply a candidate to a fresh state. The real game reads these from
/// `Difficulty`; here we write them in directly so one binary can try
/// many combinations without recompiling.
fn state_for(c: Candidate) -> GameState {
    let mut s = GameState::new();
    let h = c.paddle_half_h * 2.0;
    s.left.h = h;
    s.right.h = h;
    s.left.y = (FIELD_H - h) / 2.0;
    s.right.y = (FIELD_H - h) / 2.0;
    s
}

/// Speed the ball to this candidate's base, preserving direction.
fn apply_ball_speed(s: &mut GameState, c: Candidate, mult: f32) {
    let v = s.ball.vel;
    let mag = (v.x * v.x + v.y * v.y).sqrt();
    if mag > 1.0 {
        let want = c.ball_speed * mult;
        s.ball.vel = Vec2::new(v.x / mag * want, v.y / mag * want);
    }
}

/// Can a PERFECT player reach this fan of shots from this start?
fn reachability(c: Candidate, start_frac: f32, speed_mult: f32) -> f32 {
    let start_y = start_frac * FIELD_H;
    let mut reached = 0u32;
    let mut total = 0u32;

    for i in 0..9 {
        let from_y = 60.0 + (i as f32) * (FIELD_H - 120.0) / 8.0;
        for j in 0..11 {
            let vy = -400.0 + (j as f32) * 80.0;
            total += 1;

            let mut s = state_for(c);
            s.begin();
            s.phase = Phase::Playing;
            s.right.h = 0.0;
            s.right.y = 0.0;
            let half_h = s.left.h / 2.0;
            s.left.y = (start_y - half_h).clamp(0.0, FIELD_H - s.left.h);

            let speed = c.ball_speed * speed_mult;
            let vx_mag = (speed * speed - vy * vy).max(1.0).sqrt();
            s.ball.pos = Vec2::new(FIELD_W - PADDLE_INSET - PADDLE_W - BALL_RADIUS, from_y);
            s.ball.vel = Vec2::new(-vx_mag, vy);
            s.trail.clear();

            let mut hit = false;
            for _ in 0..(240 * 10) {
                let delta = s.ball.pos.y - s.left.center_y();
                s.left.dir = if delta.abs() < 1.0 {
                    0.0
                } else if delta > 0.0 {
                    c.paddle_speed / state::PADDLE_SPEED
                } else {
                    -(c.paddle_speed / state::PADDLE_SPEED)
                };
                let before_x = s.ball.pos.x;
                let before_score = s.score_right;
                step_fixed(&mut s);
                if s.ball.vel.x > 0.0 && before_x > s.ball.pos.x {
                    hit = true;
                    break;
                }
                if s.score_right > before_score || s.ball.pos.x < -50.0 {
                    break;
                }
            }
            if hit {
                reached += 1;
            }
        }
    }

    100.0 * reached as f32 / total as f32
}

/// AI win rate against a reactive player of the given standard.
fn win_rate(c: Candidate, bench: Reactive, matches: u32) -> f32 {
    let mut wins = 0u32;

    for v in 0..matches {
        let mut s = state_for(c);
        s.begin();

        let mut opponent = Opponent::new(Side::Right, Difficulty::Normal);
        opponent.skill = Skill {
            aim_error: c.aim_error,
            reaction: c.reaction,
            depth: c.depth,
            speed: c.ai_speed,
        };
        let mut player = ReactivePlayer::new(bench, v);

        s.serving = if v % 2 == 0 { Side::Left } else { Side::Right };
        let start = (v % 7) as f32 * 40.0;
        s.left.y = start.min(FIELD_H - s.left.h);

        let player_scale = c.paddle_speed / state::PADDLE_SPEED;

        for _ in 0..(240 * 600) {
            if s.is_over() {
                break;
            }
            if s.phase == Phase::Serve {
                serve(&mut s);
                apply_ball_speed(&mut s, c, 1.0);
            }
            player.update(&mut s);
            // Scale the player's paddle speed to the candidate's.
            s.left.dir *= player_scale;
            opponent.update(&mut s, FIXED_DT);
            // The AI's `speed` is a fraction of the PLAYER's, so it
            // scales with the same factor and the relationship the
            // design intends is preserved.
            s.right.dir *= player_scale;
            step_fixed(&mut s);
        }

        if s.score_right > s.score_left {
            wins += 1;
        }
    }

    100.0 * wins as f32 / matches as f32
}

/// The two properties, measured together.
struct Verdict {
    corner_reach: f32,
    centre_reach: f32,
    win_vs_own: f32,
}

fn evaluate(c: Candidate, own_standard: Reactive, matches: u32) -> Verdict {
    Verdict {
        corner_reach: reachability(c, 0.0, c.ramp_ceiling),
        centre_reach: reachability(c, 0.5, c.ramp_ceiling),
        win_vs_own: win_rate(c, own_standard, matches),
    }
}

fn current(d: Difficulty) -> Candidate {
    let sk = Skill::for_difficulty(d);
    Candidate {
        paddle_speed: state::PADDLE_SPEED,
        paddle_half_h: d.paddle_half_h(),
        ball_speed: d.ball_speed(),
        ramp_ceiling: d.ramp_ceiling(),
        aim_error: sk.aim_error,
        reaction: sk.reaction,
        depth: sk.depth,
        ai_speed: sk.speed,
    }
}

fn main() {
    const MATCHES: u32 = 40;

    println!("Volley — SWEEPING CANDIDATE TIER CONSTANTS\n");
    println!("Each tier is judged against the player it is FOR:");
    println!("  EASY vs a poor player · NORMAL vs fair · HARD vs good\n");
    println!("Targets: corner reachability >= 95% (nothing unanswerable),");
    println!("         AI win rate 35-65% against its own standard (a real contest).\n");

    // ---- Where we are now ----
    println!("=== CURRENT (shipped) ===\n");
    println!(
        "{:<8} {:>8} {:>8} {:>10} {:>12} {:>12}",
        "TIER", "PADDLE", "BALL", "CORNER%", "CENTRE%", "AI WIN%"
    );
    println!("{}", "-".repeat(64));
    for (d, std) in [
        (Difficulty::Easy, Reactive::POOR),
        (Difficulty::Normal, Reactive::FAIR),
        (Difficulty::Hard, Reactive::GOOD),
    ] {
        let c = current(d);
        let v = evaluate(c, std, MATCHES);
        println!(
            "{:<8} {:>8.0} {:>8.0} {:>9.0}% {:>11.0}% {:>11.0}%",
            d.label(),
            c.paddle_speed,
            c.ball_speed,
            v.corner_reach,
            v.centre_reach,
            v.win_vs_own
        );
    }

    // ---- Paddle speed alone ----
    println!("\n=== FIX 1 ALONE: raise PADDLE_SPEED per tier (nothing else changed) ===\n");
    println!(
        "{:<8} {:>8} {:>10} {:>12} {:>12}",
        "TIER", "PADDLE", "CORNER%", "CENTRE%", "AI WIN%"
    );
    println!("{}", "-".repeat(54));
    for (d, std) in [
        (Difficulty::Easy, Reactive::POOR),
        (Difficulty::Normal, Reactive::FAIR),
        (Difficulty::Hard, Reactive::GOOD),
    ] {
        for ps in [300.0f32, 360.0, 420.0, 480.0, 540.0] {
            let mut c = current(d);
            c.paddle_speed = ps;
            let v = evaluate(c, std, MATCHES);
            let flag = if v.corner_reach >= 95.0 {
                " <- reachable"
            } else {
                ""
            };
            println!(
                "{:<8} {:>8.0} {:>9.0}% {:>11.0}% {:>11.0}%{}",
                d.label(),
                ps,
                v.corner_reach,
                v.centre_reach,
                v.win_vs_own,
                flag
            );
        }
        println!();
    }

    // ---- The AI ladder, at a paddle speed that makes shots reachable ----
    println!("=== FIX 2: soften the AI, at the reachable paddle speed ===\n");
    println!("Paddle speed held at the FIX-1 pick for each tier.\n");
    println!(
        "{:<8} {:>8} {:>7} {:>8} {:>6} {:>7} {:>11}",
        "TIER", "PADDLE", "AIM", "REACT", "DEPTH", "AISPD", "AI WIN%"
    );
    println!("{}", "-".repeat(62));

    // Candidate AI ladders per tier, from the current values outward.
    let ladders: [(Difficulty, f32, Reactive, &[(f32, f32, u32, f32)]); 3] = [
        (
            Difficulty::Easy,
            480.0,
            Reactive::POOR,
            &[
                (0.85, 0.22, 1, 0.78),
                (1.10, 0.26, 1, 0.70),
                (1.35, 0.30, 1, 0.62),
                (1.60, 0.34, 1, 0.55),
            ],
        ),
        (
            Difficulty::Normal,
            420.0,
            Reactive::FAIR,
            &[
                (0.45, 0.12, 2, 0.92),
                (0.65, 0.16, 2, 0.84),
                (0.85, 0.20, 2, 0.76),
                (1.05, 0.24, 1, 0.70),
            ],
        ),
        (
            Difficulty::Hard,
            360.0,
            Reactive::GOOD,
            &[
                (0.18, 0.06, 4, 1.0),
                (0.30, 0.08, 3, 0.95),
                (0.45, 0.10, 3, 0.90),
                (0.60, 0.13, 2, 0.85),
            ],
        ),
    ];

    for (d, ps, std, rungs) in ladders {
        for &(aim, react, depth, aispd) in rungs {
            let mut c = current(d);
            c.paddle_speed = ps;
            c.aim_error = aim;
            c.reaction = react;
            c.depth = depth;
            c.ai_speed = aispd;
            let v = evaluate(c, std, MATCHES);
            let flag = if (35.0..=65.0).contains(&v.win_vs_own) {
                " <- in band"
            } else {
                ""
            };
            println!(
                "{:<8} {:>8.0} {:>7.2} {:>8.2} {:>6} {:>7.2} {:>10.0}%{}",
                d.label(),
                ps,
                aim,
                react,
                depth,
                aispd,
                v.win_vs_own,
                flag
            );
        }
        println!();
    }
}
