//! Headless soak test: play the game for thousands of ticks with a
//! perfect-tracking paddle and assert nothing pathological happens.
//! No window, no compositor, deterministic.
use omarcade_core::geom;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/state.rs"]
mod state;

use physics::{step_fixed, FIXED_DT};
use state::{GameState, Phase, BALL_SPEED, FIELD_H, FIELD_W};

fn main() {
    let ticks: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(200_000);
    let mut s = GameState::new();
    s.launch();

    let (mut min_x, mut max_x) = (f32::MAX, f32::MIN);
    let (mut min_y, mut max_y) = (f32::MAX, f32::MIN);
    let mut min_speed = f32::MAX;
    let mut max_speed = f32::MIN;
    let mut min_vy_frac = f32::MAX;
    let mut escapes = 0;
    let mut nans = 0;
    let mut bricks_killed = 0;
    let mut prev_alive = s.bricks_remaining();
    let mut horizontal_run = 0u32;
    let mut worst_horizontal_run = 0u32;

    for t in 0..ticks {
        // Perfect paddle: always track the ball. Keeps the rally alive
        // so we exercise brick collisions rather than losing instantly.
        let target = s.ball.pos.x;
        let c = s.paddle.center_x();
        s.paddle.dir = if (target - c).abs() < 4.0 { 0.0 } else if target > c { 1.0 } else { -1.0 };

        step_fixed(&mut s);

        let p = s.ball.pos;
        let v = s.ball.vel;

        if !p.x.is_finite() || !p.y.is_finite() || !v.x.is_finite() || !v.y.is_finite() {
            nans += 1;
            if nans == 1 { println!("!! NaN at tick {t}: pos {p:?} vel {v:?}"); }
        }

        if s.phase == Phase::Playing {
            min_x = min_x.min(p.x); max_x = max_x.max(p.x);
            min_y = min_y.min(p.y); max_y = max_y.max(p.y);
            let sp = v.length();
            min_speed = min_speed.min(sp); max_speed = max_speed.max(sp);
            if sp > 0.0 {
                let frac = v.y.abs() / sp;
                min_vy_frac = min_vy_frac.min(frac);
                if frac < 0.2 { horizontal_run += 1; worst_horizontal_run = worst_horizontal_run.max(horizontal_run); }
                else { horizontal_run = 0; }
            }
            // Escape check: ball outside the field (beyond a small margin)
            if p.x < -20.0 || p.x > FIELD_W + 20.0 || p.y < -20.0 {
                escapes += 1;
                if escapes == 1 { println!("!! ESCAPED at tick {t}: pos {p:?} vel {v:?}"); }
            }
        }

        let alive = s.bricks_remaining();
        if alive < prev_alive { bricks_killed += prev_alive - alive; prev_alive = alive; }

        if s.phase == Phase::Won {
            println!("WON at tick {t} ({:.1}s sim time), score {}", t as f32 * FIXED_DT, s.score);
            break;
        }
        if s.phase == Phase::Lost { println!("LOST at tick {t}"); break; }
        if s.phase == Phase::Ready { s.launch(); }
    }

    println!();
    println!("=== soak result over {ticks} ticks ({:.1}s sim) ===", ticks as f32 * FIXED_DT);
    println!("phase:            {:?}", s.phase);
    println!("bricks killed:    {bricks_killed} / 60   (remaining {})", s.bricks_remaining());
    println!("score:            {}", s.score);
    println!("lives:            {}", s.lives);
    println!("ball x range:     {min_x:.1} .. {max_x:.1}   (field 0..{FIELD_W})");
    println!("ball y range:     {min_y:.1} .. {max_y:.1}   (field 0..{FIELD_H})");
    println!("speed range:      {min_speed:.2} .. {max_speed:.2}  (nominal {BALL_SPEED})");
    println!("min |vy|/speed:   {min_vy_frac:.4}  (clamp target 0.25)");
    println!("longest near-horizontal run: {worst_horizontal_run} ticks");
    println!();
    println!("escapes: {escapes}   NaNs: {nans}");
    if escapes == 0 && nans == 0 { println!("PASS: ball stayed in the field, no NaN"); }
    else { println!("FAIL"); std::process::exit(1); }
}
