//! Why did the brick collision not register?
#[path = "../src/geom.rs"]
mod geom;
#[path = "../src/state.rs"]
mod state;
use geom::{Rect, Vec2};
use state::{GameState, BALL_RADIUS};

fn main() {
    let s = GameState::new();
    let brick = s.bricks[0].rect;
    println!("brick[0]: x{:.1}..{:.1}  y{:.1}..{:.1}",
        brick.left(), brick.right(), brick.top(), brick.bottom());

    // The test's placement: "just below the brick, 2px overlapping"
    let pos = Vec2::new(brick.center().x, brick.bottom() + BALL_RADIUS - 2.0);
    let ball = Rect::from_center(pos, BALL_RADIUS, BALL_RADIUS);
    println!("ball pos: ({:.1}, {:.1}) radius {BALL_RADIUS}", pos.x, pos.y);
    println!("ball rect: x{:.1}..{:.1}  y{:.1}..{:.1}",
        ball.left(), ball.right(), ball.top(), ball.bottom());
    println!();
    println!("ball.top {:.1} vs brick.bottom {:.1}", ball.top(), brick.bottom());
    println!("overlaps? {}", ball.overlaps(&brick));
    println!("penetration: {:?}", ball.penetration(&brick).map(|p| (p.x, p.y)));
    println!();
    // What placement actually overlaps by 2?
    let good = Vec2::new(brick.center().x, brick.bottom() + BALL_RADIUS - 2.0);
    println!("intended: ball.top() should be 2 above brick.bottom()");
    println!("  ball.top() = pos.y - r = {:.1}", good.y - BALL_RADIUS);
    println!("  brick.bottom() = {:.1}", brick.bottom());
    println!("  => top is {:.1} BELOW bottom, so no overlap",
        (good.y - BALL_RADIUS) - brick.bottom());
}
