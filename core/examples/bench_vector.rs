//! What the vector primitives cost, so the "use them for things that move"
//! doctrine rests on a number rather than a feeling.
//!
//!   cargo run --release -p omarcade-core --example bench_vector
//!
//! The reference is a 60fps frame budget: 16.67ms. Anything reported as a
//! percentage of that is a percentage of one frame.

use omarcade_core::backend::vector::{Shape, Transform};
use omarcade_core::{Canvas, Color};
use std::time::Instant;

const W: u32 = 960;
const H: u32 = 720;
const BUDGET_MS: f64 = 1000.0 / 60.0;

fn bench(name: &str, iters: u32, mut f: impl FnMut(&mut Canvas<'_>)) {
    let mut px = vec![0u32; (W * H) as usize];
    // One untimed pass so the first-touch page faults are not in the number.
    {
        let mut c = Canvas::new(&mut px, W, H);
        f(&mut c);
    }
    let t = Instant::now();
    for _ in 0..iters {
        let mut c = Canvas::new(&mut px, W, H);
        f(&mut c);
    }
    let per = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("{name:<44} {per:7.3} ms   {:5.2}% of a frame", per / BUDGET_MS * 100.0);
}

fn main() {
    println!("canvas {W}x{H}, 60fps budget {BUDGET_MS:.2} ms\n");

    let ink = Color::rgb(220, 230, 245);
    const SHIP: Shape = Shape::new(&[(14.0, 0.0), (-6.0, -7.0), (-2.0, 0.0), (-6.0, 7.0)]);

    bench("fill_rect_f 64x64 (the reference)", 2000, |c| {
        c.fill_rect_f(100.0, 100.0, 64.0, 64.0, ink);
    });
    bench("polygon_f, same 64x64 area as a quad", 2000, |c| {
        c.polygon_f(&[(100.0, 100.0), (164.0, 100.0), (164.0, 164.0), (100.0, 164.0)], ink);
    });
    bench("circle_f r=32", 2000, |c| c.circle_f(300.0, 300.0, 32.0, ink));
    bench("one ship, scale 2", 2000, |c| {
        SHIP.fill(c, &Transform::at(400.0, 300.0).facing(0.7).scaled(2.0), ink);
    });
    bench("64 ships (a dense arcade frame)", 200, |c| {
        for i in 0..64 {
            let a = i as f32 * 0.1;
            SHIP.fill(
                c,
                &Transform::at(60.0 + (i % 16) as f32 * 55.0, 80.0 + (i / 16) as f32 * 60.0)
                    .facing(a)
                    .scaled(2.0),
                ink,
            );
        }
    });
    bench("200 small circles (particle-scale)", 200, |c| {
        for i in 0..200 {
            let x = 40.0 + (i % 40) as f32 * 22.0;
            let y = 40.0 + (i / 40) as f32 * 60.0;
            c.circle_add_f(x, y, 3.0, Color::rgb(60, 40, 20));
        }
    });
    bench("full-screen polygon (the abuse case)", 200, |c| {
        c.polygon_f(&[(0.0, 0.0), (W as f32, 0.0), (W as f32, H as f32), (0.0, H as f32)], ink);
    });
}
