//! Trace clamp_angle step by step to see where vy is lost.
#[path = "../src/geom.rs"]
mod geom;
use geom::Vec2;

const SPEED: f32 = 420.0;
const MIN_FRAC: f32 = 0.25;

fn main() {
    let mut v = Vec2::new(SPEED, 1.0);
    println!("in:          ({:.3}, {:.3})  len={:.3}", v.x, v.y, v.length());
    let speed = v.length();
    let min_vy = speed * MIN_FRAC;
    println!("target min_vy = {:.3} (speed {:.3} * {MIN_FRAC})", min_vy, speed);

    v.y = min_vy;
    println!("after set y: ({:.3}, {:.3})  len={:.3}  <-- len grew!", v.x, v.y, v.length());

    let v2 = v.with_length(speed);
    println!("after renorm:({:.3}, {:.3})  len={:.3}", v2.x, v2.y, v2.length());
    println!("  vy fell from {:.3} to {:.3}  <-- THE BUG", min_vy, v2.y);
    println!();
    println!("fix: solve for the components directly.");
    // vy = speed*frac; vx makes up the rest, preserving x's sign.
    let vy = speed * MIN_FRAC;
    let vx = (speed * speed - vy * vy).max(0.0).sqrt() * v.x.signum();
    let fixed = Vec2::new(vx, vy);
    println!("solved:      ({:.3}, {:.3})  len={:.3}  vy/len={:.3}",
        fixed.x, fixed.y, fixed.length(), fixed.y / fixed.length());
}
