//! Sweep a small rect around a brick from every direction and print the
//! axis chosen. Reveals systematic errors that hand-picked tests hide.
use omarcade_core::geom;
use geom::{Axis, Rect, Vec2};

fn main() {
    let brick = Rect::new(100.0, 100.0, 40.0, 20.0);
    let ball_r = 4.0;
    println!("brick: x100..140 y100..120, ball radius {ball_r}");
    println!("approach from each direction, 2px into the brick:\n");

    // Sample the perimeter: for each direction, place the ball so it has
    // just penetrated 2px, and ask which axis we resolve on.
    let cases: Vec<(&str, Vec2)> = vec![
        ("left face,  mid",   Vec2::new(100.0 - ball_r + 2.0, 110.0)),
        ("right face, mid",   Vec2::new(140.0 + ball_r - 2.0, 110.0)),
        ("top face,   mid",   Vec2::new(120.0, 100.0 - ball_r + 2.0)),
        ("bot face,   mid",   Vec2::new(120.0, 120.0 + ball_r - 2.0)),
        ("top-left corner",   Vec2::new(100.0 - ball_r + 2.0, 100.0 - ball_r + 2.0)),
        ("top-right corner",  Vec2::new(140.0 + ball_r - 2.0, 100.0 - ball_r + 2.0)),
        ("bot-left corner",   Vec2::new(100.0 - ball_r + 2.0, 120.0 + ball_r - 2.0)),
        ("bot-right corner",  Vec2::new(140.0 + ball_r - 2.0, 120.0 + ball_r - 2.0)),
        ("near-left, high y", Vec2::new(100.0 - ball_r + 2.0, 101.0)),
        ("near-top, left x",  Vec2::new(101.0, 100.0 - ball_r + 2.0)),
    ];

    for (name, c) in cases {
        let b = Rect::from_center(c, ball_r, ball_r);
        let axis = b.collision_axis(&brick);
        let pen = b.penetration(&brick);
        let expect = match name {
            n if n.starts_with("left") || n.starts_with("right") || n.starts_with("near-left") => Some(Axis::X),
            n if n.starts_with("top f") || n.starts_with("bot f") || n.starts_with("near-top") => Some(Axis::Y),
            _ => None, // corners: either is defensible
        };
        let mark = match (expect, axis) {
            (Some(e), Some(a)) if e == a => "ok",
            (Some(_), _) => "MISMATCH",
            (None, _) => "corner",
        };
        println!("  {name:<20} axis={axis:?}  pen={:?}  [{mark}]",
            pen.map(|p| (p.x, p.y)));
    }
}
