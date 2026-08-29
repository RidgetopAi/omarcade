//! Rough cost comparison of the three draw paths, at 960x720.
//! Not a microbenchmark harness — a sanity check that blending has not
//! made a frame unaffordable on a CPU renderer.
use omarcade_core::{Canvas, Color};
use std::time::Instant;

fn time<F: FnMut()>(label: &str, iters: u32, mut f: F) -> f64 {
    f(); // warm
    let t = Instant::now();
    for _ in 0..iters { f(); }
    let per = t.elapsed().as_secs_f64() / iters as f64;
    println!("{label:<34} {:>8.3} ms/frame   {:>6.2}% of a 16.67ms budget",
             per * 1000.0, per * 1000.0 / 16.67 * 100.0);
    per
}

fn main() {
    const W: u32 = 960;
    const H: u32 = 720;
    let mut buf = vec![0u32; (W * H) as usize];
    let iters = 200;

    // A realistic Breakout frame: clear + 60 bricks + paddle + ball.
    let opaque = time("60 bricks, opaque (today)", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.clear(Color::rgb(40, 44, 48));
        for i in 0..60 { c.fill_rect((i % 10) * 90, (i / 10) * 30 + 90, 84, 28, Color::rgb(200, 90, 90)); }
        c.fill_rect(400, 660, 120, 16, Color::WHITE);
        c.fill_rect(470, 400, 14, 14, Color::WHITE);
    });

    let subpixel = time("same, ball+paddle sub-pixel", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.clear(Color::rgb(40, 44, 48));
        for i in 0..60 { c.fill_rect((i % 10) * 90, (i / 10) * 30 + 90, 84, 28, Color::rgb(200, 90, 90)); }
        c.fill_rect_f(400.3, 660.7, 120.0, 16.0, Color::WHITE);
        c.fill_rect_f(470.6, 400.2, 14.0, 14.0, Color::WHITE);
    });

    let trail = time("+ 12 alpha trail quads", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.clear(Color::rgb(40, 44, 48));
        for i in 0..60 { c.fill_rect((i % 10) * 90, (i / 10) * 30 + 90, 84, 28, Color::rgb(200, 90, 90)); }
        for t in 0..12 {
            let a = 200 - t * 15;
            c.fill_rect_f(470.6 - t as f32 * 6.0, 400.2 - t as f32 * 4.0, 14.0, 14.0,
                          Color::rgba(255, 255, 255, a as u8));
        }
        c.fill_rect_f(400.3, 660.7, 120.0, 16.0, Color::WHITE);
    });

    let veiled = time("FULL-SCREEN veil (worst case)", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.clear(Color::rgb(40, 44, 48));
        c.veil(Color::rgba(0, 0, 0, 128));
    });

    println!();
    println!("sub-pixel vs opaque : {:.2}x", subpixel / opaque);
    println!("with trail vs opaque: {:.2}x", trail / opaque);
    println!("full veil vs opaque : {:.2}x", veiled / opaque);
}
