//! Can you physically get there?
//!
//! "After a rally you just can't get to a ball at times, going from
//! bottom to top or vice versa." That is a claim about GEOMETRY, not
//! about skill, and geometry can be settled exactly.
//!
//! No player here, and no AI. Just: put the paddle somewhere, launch a
//! ball at somewhere else, move the paddle flat-out toward the ball
//! from the first frame — a PERFECT player, reacting instantly and
//! never guessing wrong — and see whether it arrives. Anything a
//! perfect player cannot reach is unreachable, full stop, and no amount
//! of practice will change it.
//!
//! Two numbers come out of this:
//!
//! 1. **Reachability** per tier and per starting position. A cell well
//!    under 100% means shots exist that nobody can return from there.
//! 2. **The recovery penalty**: reachability from the CENTRE (where you
//!    are at a serve) minus reachability from a CORNER (where a rally
//!    leaves you). That difference is the cost of the previous shot,
//!    and it is what "after a rally" means.
//!
//!   cargo run --release -q -p omarcade-volley --example probe_recovery
//!
//! What to look for: whether EASY is meaningfully more forgiving than
//! HARD. `PADDLE_SPEED` is a single global constant that difficulty
//! never touches, so the only thing separating the tiers here is ball
//! speed — and if that gap is small, then the part of difficulty the
//! player FEELS is small, which is exactly the "very little difference
//! between the levels" complaint.

#[path = "../src/ai.rs"]
mod ai;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/state.rs"]
mod state;

use omarcade_core::geom::Vec2;
use physics::{step_fixed, FIXED_DT};
use state::{
    Difficulty, GameState, Phase, Side, BALL_RADIUS, FIELD_H, FIELD_W,
};

/// Where a rally can leave you, as a fraction of the field height.
const STARTS: [(&str, f32); 5] = [
    ("top", 0.0),
    ("upper", 0.25),
    ("centre", 0.5),
    ("lower", 0.75),
    ("bottom", 1.0),
];

/// One launch: a ball crossing from the opponent's face toward ours.
struct Shot {
    /// Where the ball starts, vertically.
    from_y: f32,
    /// Vertical velocity component.
    vy: f32,
}

/// Did the paddle intercept, and if not, by how much did it miss?
struct Attempt {
    reached: bool,
    /// Gap between the paddle's nearest edge and the ball at the moment
    /// it crossed the face, in pixels. Zero when reached.
    miss_by: f32,
    /// Seconds the ball was in flight — the budget the player had.
    flight: f32,
    /// Pixels the paddle needed to travel.
    distance: f32,
}

/// Run one shot against a paddle that starts at `start_y` and moves
/// flat-out toward the ball every frame — no reaction delay, no aiming
/// error, no wrong guesses. The upper bound on human performance.
fn attempt(difficulty: Difficulty, start_y: f32, shot: Shot, speed_mult: f32) -> Attempt {
    let mut s = GameState::with_difficulty(difficulty);
    s.begin();
    s.phase = Phase::Playing;

    // Park the right paddle out of the way; this is about the left one.
    // (Shrinking it is what actually works — move_paddles clamps a
    // paddle back into the field every tick, so it cannot be moved out.
    // Precedent: ai.rs's the_prediction_matches_the_simulation.)
    s.right.h = 0.0;
    s.right.y = 0.0;

    let half_h = s.left.h / 2.0;
    s.left.y = (start_y - half_h).clamp(0.0, FIELD_H - s.left.h);

    // Launch from the opponent's face, heading left at this tier's
    // speed. The x component is whatever is left after vy, so a steeper
    // shot is genuinely slower across the field — same as in play.
    let speed = difficulty.ball_speed() * speed_mult;
    let vx_mag = (speed * speed - shot.vy * shot.vy).max(1.0).sqrt();
    s.ball.pos = Vec2::new(FIELD_W - state::PADDLE_INSET - state::PADDLE_W - BALL_RADIUS, shot.from_y);
    s.ball.vel = Vec2::new(-vx_mag, shot.vy);
    s.trail.clear();

    let face = s.left.face_x(Side::Left);
    let start_paddle_y = s.left.center_y();

    let mut ticks = 0u32;
    let mut reached = false;
    let mut miss_by = f32::MAX;

    // Ten seconds is far beyond any crossing at these speeds.
    for _ in 0..(240 * 10) {
        // Steer flat-out toward the ball's CURRENT y. A perfect player
        // with instant reactions; no prediction needed, because moving
        // toward the ball is never the wrong direction when the ball is
        // the thing you must touch.
        let delta = s.ball.pos.y - s.left.center_y();
        s.left.dir = if delta.abs() < 1.0 {
            0.0
        } else if delta > 0.0 {
            1.0
        } else {
            -1.0
        };

        let before_x = s.ball.pos.x;
        let before_score = s.score_right;
        step_fixed(&mut s);
        ticks += 1;

        // Track the closest approach as the ball crosses the face.
        if s.ball.pos.x <= face + BALL_RADIUS + 4.0 {
            let top = s.left.y;
            let bottom = s.left.y + s.left.h;
            let gap = if s.ball.pos.y < top {
                top - s.ball.pos.y
            } else if s.ball.pos.y > bottom {
                s.ball.pos.y - bottom
            } else {
                0.0
            };
            miss_by = miss_by.min(gap);
        }

        // The ball turned around: physics resolved a paddle hit.
        if s.ball.vel.x > 0.0 && before_x > s.ball.pos.x {
            reached = true;
            break;
        }
        // A point was scored against us: it got past.
        if s.score_right > before_score {
            reached = false;
            break;
        }
        if s.ball.pos.x < -50.0 {
            break;
        }
    }

    Attempt {
        reached,
        miss_by: if reached { 0.0 } else { miss_by.min(FIELD_H) },
        flight: ticks as f32 * FIXED_DT,
        distance: (shot.from_y - start_paddle_y).abs(),
    }
}

/// Sweep a fan of shots at one tier from one starting position.
fn sweep(difficulty: Difficulty, start_frac: f32, speed_mult: f32) -> (u32, u32, f32, f32) {
    let start_y = start_frac * FIELD_H;
    let mut reached = 0u32;
    let mut total = 0u32;
    let mut worst_miss = 0.0f32;
    let mut max_reach_distance = 0.0f32;

    // Launch from a spread of heights, on a spread of angles. Together
    // these cover the shots a rally actually produces.
    for i in 0..9 {
        let from_y = 60.0 + (i as f32) * (FIELD_H - 120.0) / 8.0;
        for j in 0..11 {
            // vy from steeply up to steeply down.
            let vy = -400.0 + (j as f32) * 80.0;
            let a = attempt(
                difficulty,
                start_y,
                Shot { from_y, vy },
                speed_mult,
            );
            total += 1;
            if a.reached {
                reached += 1;
                max_reach_distance = max_reach_distance.max(a.distance);
            } else {
                worst_miss = worst_miss.max(a.miss_by);
            }
            let _ = a.flight;
        }
    }

    (
        reached,
        total,
        worst_miss,
        max_reach_distance,
    )
}

fn main() {
    println!("Volley — CAN A PERFECT PLAYER EVEN GET THERE?");
    println!(
        "Paddle speed is now PER TIER (easy {:.0} / normal {:.0} / hard {:.0}); field {FIELD_H} px.",
        Difficulty::Easy.paddle_speed(),
        Difficulty::Normal.paddle_speed(),
        Difficulty::Hard.paddle_speed()
    );
    println!("The paddle below reacts INSTANTLY and never guesses wrong. Anything");
    println!("it cannot reach, nobody can reach.\n");

    for (mult, when) in [(1.0f32, "FRESH rally (ball at tier base speed)")] {
        println!("=== {when} ===\n");
        println!(
            "{:<8} {:<8} {:>10} {:>12} {:>14}",
            "TIER", "START", "REACHED", "WORST MISS", "PADDLE HALF-H"
        );
        println!("{}", "-".repeat(58));

        for d in Difficulty::ALL {
            for (label, frac) in STARTS {
                let (reached, total, worst, _) = sweep(d, frac, mult);
                println!(
                    "{:<8} {:<8} {:>8}/{:<3} {:>10.0}px {:>14.0}",
                    d.label(),
                    label,
                    reached,
                    total,
                    worst,
                    d.paddle_half_h()
                );
            }
            println!();
        }
    }

    // Now the ramped ball — the end of a long rally, which is exactly
    // the moment Brian describes.
    println!("=== RAMPED rally (ball at the tier's ceiling — a long point) ===\n");
    println!(
        "{:<8} {:<8} {:>10} {:>12} {:>14}",
        "TIER", "START", "REACHED", "WORST MISS", "BALL SPEED"
    );
    println!("{}", "-".repeat(58));

    let mut ramped: Vec<(Difficulty, f32, f32)> = Vec::new();
    for d in Difficulty::ALL {
        let mult = d.ramp_ceiling();
        let mut centre_pct = 0.0;
        let mut corner_pct = 0.0;
        for (label, frac) in STARTS {
            let (reached, total, worst, _) = sweep(d, frac, mult);
            let pct = 100.0 * reached as f32 / total as f32;
            if label == "centre" {
                centre_pct = pct;
            }
            if label == "top" {
                corner_pct = pct;
            }
            println!(
                "{:<8} {:<8} {:>8}/{:<3} {:>10.0}px {:>14.0}",
                d.label(),
                label,
                reached,
                total,
                worst,
                d.ball_speed() * mult
            );
        }
        ramped.push((d, centre_pct, corner_pct));
        println!();
    }

    println!("=== THE RECOVERY PENALTY (ramped ball) ===\n");
    println!("What the previous shot costs you: reachability from the CENTRE versus");
    println!("from the TOP corner, where a rally leaves you.\n");
    for (d, centre, corner) in &ramped {
        println!(
            "  {:<7} centre {:>5.1}%   top corner {:>5.1}%   penalty {:>5.1} points",
            d.label(),
            centre,
            corner,
            centre - corner
        );
    }

    println!();
    println!("=== WHAT DIFFICULTY ACTUALLY CHANGES FOR THE PLAYER ===\n");
    println!("Paddle speed is ONE GLOBAL CONSTANT — difficulty never touches it.");
    println!("So everything the player physically feels comes from these two:\n");
    println!(
        "{:<8} {:>12} {:>12} {:>14} {:>12}",
        "TIER", "PADDLE H", "BALL BASE", "BALL RAMPED", "REACH*"
    );
    println!("{}", "-".repeat(62));
    for d in Difficulty::ALL {
        // Reach: how far the paddle travels during one face-to-face
        // crossing at the ramped speed, assuming a typical angle.
        let face_to_face = FIELD_W - 2.0 * (state::PADDLE_INSET + state::PADDLE_W);
        let vx = d.ball_speed() * d.ramp_ceiling() * 0.85;
        let reach = d.paddle_speed() * (face_to_face / vx);
        println!(
            "{:<8} {:>12.0} {:>12.0} {:>14.0} {:>11.0}px",
            d.label(),
            d.paddle_half_h() * 2.0,
            d.ball_speed(),
            d.ball_speed() * d.ramp_ceiling(),
            reach
        );
    }
    println!("\n*REACH = how far the paddle can travel while the ball crosses, at the");
    println!(" ramped speed. Compare against {FIELD_H:.0}px of field.");
}
