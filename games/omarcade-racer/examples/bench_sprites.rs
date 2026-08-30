//! What do the REAL sprites cost, at the real resolution?
//!
//! bench_road measured hand-written rectangles standing in for cars.
//! Now that the art is 48x30 with ~800 drawn pixels each, that stand-in
//! is no longer representative — one car is 800 sub-pixel rects, not 6.
//! This measures the sprites themselves.
//!
//!   cargo run --release -p omarcade-racer --example bench_sprites

#[path = "../src/art.rs"]
mod art;

use art::Art;
use omarcade_core::{Canvas, Theme};
use std::time::Instant;

const W: u32 = 960;
const H: u32 = 720;

fn time<F: FnMut()>(label: &str, iters: u32, mut f: F) -> f64 {
    f();
    let t = Instant::now();
    for _ in 0..iters {
        f();
    }
    let per = t.elapsed().as_secs_f64() / iters as f64;
    let pct = per * 1000.0 / 16.67 * 100.0;
    println!("{label:<44} {:>7.3} ms  {:>5.1}% of frame", per * 1000.0, pct);
    per
}

fn main() {
    let theme = Theme::load();
    let art = Art::load(&theme);
    let mut buf = vec![0u32; (W * H) as usize];
    let iters = 200;

    println!(
        "Real sprites: player {}x{} ({} px), rival {} px\n",
        art.player.width(),
        art.player.height(),
        art.player.ink(),
        art.rival.ink()
    );

    time("1 player car, near (scale 6)", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        art.player.draw_ground(&mut c, 480.0, 700.0, 6.0);
    });

    time("8 rivals, mixed distance", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        for i in 0..8 {
            let s = 0.6 + i as f32 * 0.45;
            art.rival.draw_ground(&mut c, 120.0 + i as f32 * 95.0, 300.0 + i as f32 * 45.0, s);
        }
    });

    let worst = time("PESSIMISTIC: player + 8 rivals + 40 posts", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        for i in 0..40 {
            let s = 0.5 + (i % 8) as f32 * 0.4;
            art.post.draw_ground(&mut c, 30.0 + i as f32 * 23.0, 280.0 + (i % 9) as f32 * 42.0, s);
        }
        for i in 0..8 {
            let s = 0.6 + i as f32 * 0.45;
            art.rival.draw_ground(&mut c, 120.0 + i as f32 * 95.0, 300.0 + i as f32 * 45.0, s);
        }
        art.player.draw_ground(&mut c, 480.0, 700.0, 6.0);
    });

    println!();
    let road = 0.53; // measured by bench_road: road + stripes + haze
    let total = worst * 1000.0 + road;
    println!("sprites worst case : {:.2} ms", worst * 1000.0);
    println!("+ road (bench_road): {road:.2} ms");
    println!("= full frame       : {total:.2} ms = {:.0}% of a 60fps budget", total / 16.67 * 100.0);
    println!();
    if total / 16.67 < 0.5 {
        println!("VERDICT: comfortable at 48x30. The higher-detail art fits.");
    } else if total / 16.67 < 1.0 {
        println!("VERDICT: it fits, but sprite count now matters. Cap traffic.");
    } else {
        println!("VERDICT: does NOT fit. 48x30 is too expensive on this path.");
    }
}
