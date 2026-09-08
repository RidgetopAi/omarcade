//! Which brick does the selection loop pick, and is the comparison right?
use omarcade_core::geom;
#[path = "../src/state.rs"]
mod state;
use geom::{Axis, Rect, Vec2};
use state::{GameState, BALL_RADIUS};

fn scan(ball: Rect, s: &GameState) -> Vec<(usize, f32, Axis)> {
    let mut hits = vec![];
    for (i, b) in s.bricks.iter().enumerate() {
        if !b.alive { continue; }
        let Some(pen) = ball.penetration(&b.rect) else { continue };
        let Some(axis) = ball.collision_axis(&b.rect) else { continue };
        let depth = match axis { Axis::X => pen.x, Axis::Y => pen.y };
        hits.push((i, depth, axis));
    }
    hits
}

fn main() {
    let s = GameState::new();

    println!("=== case 1: single brick from below ===");
    let brick = s.bricks[0].rect;
    let ball = Rect::from_center(
        Vec2::new(brick.center().x, brick.bottom() + BALL_RADIUS - 2.0), BALL_RADIUS, BALL_RADIUS);
    let hits = scan(ball, &s);
    println!("candidates: {:?}", hits.iter().map(|(i,d,a)| (*i,*d,*a)).collect::<Vec<_>>());
    println!("  -> exactly one hit, so selection cannot be the issue here.");
    println!("  -> if the test saw no kill, the bug is in the code path, not selection.");

    println!();
    println!("=== case 2: seam between brick 0 and 1 ===");
    let a = s.bricks[0].rect; let b = s.bricks[1].rect;
    let seam = (a.right() + b.left()) / 2.0;
    println!("a.right={:.1} b.left={:.1} seam={:.1} gap={:.1}", a.right(), b.left(), seam, b.left()-a.right());
    let ball2 = Rect::from_center(Vec2::new(seam, a.center().y), BALL_RADIUS, BALL_RADIUS);
    println!("ball x{:.1}..{:.1}", ball2.left(), ball2.right());
    let hits2 = scan(ball2, &s);
    println!("candidates: {:?}", hits2.iter().map(|(i,d,a)| (*i,*d,*a)).collect::<Vec<_>>());
    for (i, d, ax) in &hits2 {
        println!("   brick {i}: depth {d:.2} axis {ax:?}");
    }
    println!();
    println!("SHALLOWEST is the correct pick (face reached first).");
    if let Some(min) = hits2.iter().min_by(|x,y| x.1.partial_cmp(&y.1).unwrap()) {
        println!("  shallowest = brick {} depth {:.2}", min.0, min.1);
    }
    if let Some(max) = hits2.iter().max_by(|x,y| x.1.partial_cmp(&y.1).unwrap()) {
        println!("  deepest    = brick {} depth {:.2}  <- what the code picks", max.0, max.1);
    }
}
