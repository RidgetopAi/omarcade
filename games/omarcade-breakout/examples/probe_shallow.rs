//! Force shallow-angle play: hit the ball with the paddle EDGE every
//! time, which is what actually produces near-horizontal trajectories.
#[path = "../src/geom.rs"]
mod geom;
#[path = "../src/physics.rs"]
mod physics;
#[path = "../src/state.rs"]
mod state;

use physics::step_fixed;
use state::{GameState, Phase, BALL_SPEED, FIELD_W};

fn main() {
    let mut s = GameState::new();
    // Clear bricks so this is a pure paddle-wall rally — but leave ONE
    // alive and park it off in a corner the ball cannot reach. With
    // zero bricks, check_win fires instantly and the sim never plays.
    for b in &mut s.bricks { b.alive = false; }
    s.bricks[0].alive = true;
    s.bricks[0].rect = geom::Rect::new(-500.0, -500.0, 10.0, 10.0);
    s.phase = state::Phase::Ready;
    s.launch();
    s.phase = Phase::Playing;

    let mut min_frac = f32::MAX;
    let mut worst = (0u32, 0.0f32);
    let mut horizontal_run = 0u32;
    let mut worst_run = 0u32;
    let mut escapes = 0;
    let mut samples = 0u64;
    let mut samples_skipped = 0u64;

    for t in 0..300_000u32 {
        // Adversarial paddle: aim so the ball lands on the very edge,
        // maximising horizontal deflection.
        let edge_target = s.ball.pos.x - (s.paddle.w / 2.0) * 0.97;
        let c = s.paddle.center_x();
        s.paddle.dir = if (edge_target - c).abs() < 2.0 { 0.0 }
                       else if edge_target > c { 1.0 } else { -1.0 };

        step_fixed(&mut s);
        // Sample BEFORE any relaunch, and only when actually playing
        // with a moving ball — otherwise we silently measure nothing.
        if s.phase == Phase::Ready { s.launch(); samples_skipped += 1; continue; }
        if s.phase != Phase::Playing { continue; }
        if s.ball.vel.length() == 0.0 { continue; }
        samples += 1;

        let v = s.ball.vel;
        let sp = v.length();
        if sp > 0.0 {
            let frac = v.y.abs() / sp;
            if frac < min_frac { min_frac = frac; worst = (t, frac); }
            if frac < 0.2 { horizontal_run += 1; worst_run = worst_run.max(horizontal_run); }
            else { horizontal_run = 0; }
        }
        let p = s.ball.pos;
        if p.x < -20.0 || p.x > FIELD_W + 20.0 || p.y < -20.0 { escapes += 1; }
        if !p.x.is_finite() || !p.y.is_finite() { println!("NaN at {t}"); break; }
    }

    println!("=== adversarial edge-hit rally, 300k ticks ===");
    println!("samples collected: {samples}  (relaunches {samples_skipped})");
    println!("min |vy|/speed observed: {min_frac:.4}  (clamp floor 0.25)");
    println!("  worst at tick {} = {:.4}", worst.0, worst.1);
    println!("longest run below 0.2: {worst_run} ticks");
    println!("escapes: {escapes}");
    println!("final speed: {:.3} (nominal {BALL_SPEED})", s.ball.vel.length());
    println!();
    if samples == 0 {
        println!("FAIL: harness collected ZERO samples — measured nothing");
        std::process::exit(1);
    }
    if min_frac >= 0.245 && escapes == 0 {
        println!("PASS: clamp held; ball never went near-horizontal");
    } else if escapes > 0 {
        println!("FAIL: ball escaped");
    } else {
        println!("FAIL: clamp breached, {min_frac:.4} < 0.25");
    }
}
