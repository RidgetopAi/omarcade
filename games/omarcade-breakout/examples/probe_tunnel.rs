//! Tunnelling: can the ball ever pass through a brick without breaking it?
//! Fires the ball at a brick wall at escalating speeds and checks that a
//! ball which ends up beyond a brick always destroyed something.
#[path = "../src/geom.rs"]
mod geom;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/state.rs"]
mod state;

use geom::Vec2;
use physics::{step, Accumulator, FIXED_DT};
use state::{GameState, Phase};

fn main() {
    println!("brick row spans y90..118; firing upward from y400\n");
    let mut failures = 0;

    // Speed sweep, including absurd values far past normal play.
    for &speed in &[420.0f32, 1000.0, 3000.0, 8000.0, 20_000.0] {
        // Frame time sweep: 60fps, a hitch, and a catastrophic stall.
        for &frame_dt in &[1.0 / 60.0f32, 0.1, 1.0] {
            let mut s = GameState::new();
            s.phase = Phase::Playing;
            let target = s.bricks[25].rect;
            s.ball.pos = Vec2::new(target.center().x, 400.0);
            s.ball.vel = Vec2::new(0.0, -speed);
            let before = s.bricks_remaining();

            let mut acc = Accumulator::new();
            let mut min_y = f32::MAX;
            let mut killed_at_apex = 0;
            // Stop when the ball first clears the top of the brick field:
            // that is the instant tunnelling would have happened.
            for _ in 0..600 {
                step(&mut s, &mut acc, frame_dt);
                min_y = min_y.min(s.ball.pos.y);
                if s.ball.pos.y < 85.0 {
                    killed_at_apex = before - s.bricks_remaining();
                    break;
                }
                if s.phase != Phase::Playing { break; }
            }
            let _ = killed_at_apex;

            let after = s.bricks_remaining();
            let killed = before - after;
            let y = s.ball.pos.y;
            // The failure we care about: ball got past the brick field
            // (y above the top row) having destroyed nothing.
            // Tunnelling = reached above the brick field having broken nothing.
            let tunnelled = min_y < 85.0 && killed == 0;
            let mark = if tunnelled { failures += 1; "TUNNELLED" } else { "ok" };
            println!("speed {speed:>6} dt {frame_dt:>5.3}s: killed {killed:>2}, min_y {min_y:>7.1}, end_y {y:>7.1} [{mark}]");
        }
    }

    println!();
    let per_tick = state::BALL_SPEED * FIXED_DT;
    println!("nominal per-tick movement: {per_tick:.2} units vs 28-unit brick height");
    if failures == 0 { println!("PASS: no tunnelling at any tested speed or frame time"); }
    else { println!("FAIL: {failures} tunnelling cases"); std::process::exit(1); }
}
