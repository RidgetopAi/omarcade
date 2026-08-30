//! Can this CPU renderer afford a pseudo-3D road?
//!
//! Breakout and Pong paint a few dozen small rects per frame. A Pole
//! Position-style road paints EVERY PIXEL, EVERY FRAME — 691,200 of
//! them at 960x720 — because sky, grass and road together cover the
//! whole screen with nothing left blank.
//!
//! That is a different order of work, and whether it fits in a 16.67ms
//! frame decides the renderer's whole shape. So measure it before
//! designing anything:
//!
//!   cargo run --release -p omarcade-core --example bench_road
//!
//! The candidate approaches, cheapest first:
//!
//! 1. BANDED — one rect per band of scanlines. Cheapest, but the road
//!    edges stair-step.
//! 2. SCANLINE — one rect per scanline, integer. The classic.
//! 3. SCANLINE_F — one sub-pixel rect per scanline. Smooth edges, which
//!    is the whole "modern feel" argument.
//! 4. + SPRITES — scaled opponent cars on top.
//! 5. + LIGHTING — an alpha wash for depth haze and headlight glow.
//!
//! Anything under ~8ms leaves real headroom at 60fps. Over 16.67ms is
//! a dropped frame no matter what else we do.

use omarcade_core::{Canvas, Color};
use std::time::Instant;

const W: u32 = 960;
const H: u32 = 720;
/// Where the horizon sits. Everything below it is road and grass.
const HORIZON: u32 = 260;

fn time<F: FnMut()>(label: &str, iters: u32, mut f: F) -> f64 {
    f(); // warm
    let t = Instant::now();
    for _ in 0..iters {
        f();
    }
    let per = t.elapsed().as_secs_f64() / iters as f64;
    let pct = per * 1000.0 / 16.67 * 100.0;
    let verdict = if pct < 50.0 {
        "comfortable"
    } else if pct < 100.0 {
        "tight"
    } else {
        "DROPS FRAMES"
    };
    println!(
        "{label:<38} {:>7.3} ms  {:>6.1}% of frame   {verdict}",
        per * 1000.0,
        pct
    );
    per
}

/// Road geometry for one scanline, as a real projection would give it.
///
/// Perspective: distance grows non-linearly down the screen, so the
/// road widens non-linearly too. `curve` shifts the centre, which is
/// what a bend actually is in this technique — no rotation anywhere.
fn road_at(y: u32, camera_x: f32, curve: f32) -> (f32, f32) {
    // 0 at the horizon, 1 at the bottom of the screen.
    let t = (y - HORIZON) as f32 / (H - HORIZON) as f32;
    // Perspective divide. The +0.06 keeps the horizon from going to
    // zero width and vanishing into a NaN.
    let scale = 1.0 / (1.0 - t * 0.94 + 0.06);
    let half_width = 26.0 * scale;
    // A curve accumulates with the square of distance, which is what
    // makes a bend read as a bend rather than a diagonal.
    let centre = W as f32 / 2.0 + curve * (1.0 - t) * (1.0 - t) * 900.0 - camera_x * scale;
    (centre, half_width)
}

fn main() {
    let mut buf = vec![0u32; (W * H) as usize];
    let iters = 120;

    let sky = Color::rgb(42, 52, 68);
    let grass_a = Color::rgb(38, 68, 48);
    let grass_b = Color::rgb(34, 60, 43);
    let road_a = Color::rgb(58, 58, 62);
    let road_b = Color::rgb(52, 52, 56);
    let stripe = Color::rgb(214, 210, 190);

    println!("Pseudo-3D road cost at {W}x{H} — {} pixels per frame\n", W * H);
    println!("{:<38} {:>7}  {:>6}", "approach", "ms", "budget");
    println!("{}", "-".repeat(78));

    // --- 1. Banded: one rect per group of scanlines -------------------
    let banded = time("1. banded road (8px bands)", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.fill_rect(0, 0, W, HORIZON, sky);
        let mut y = HORIZON;
        while y < H {
            let (centre, half) = road_at(y, 0.0, 0.3);
            let band = 8u32.min(H - y);
            let g = if (y / 32) % 2 == 0 { grass_a } else { grass_b };
            c.fill_rect(0, y as i32, W, band, g);
            c.fill_rect((centre - half) as i32, y as i32, (half * 2.0) as u32, band, road_a);
            y += band;
        }
    });

    // --- 2. Scanline, integer ----------------------------------------
    let scanline = time("2. scanline road, integer", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.fill_rect(0, 0, W, HORIZON, sky);
        for y in HORIZON..H {
            let (centre, half) = road_at(y, 0.0, 0.3);
            // Alternating bands give the sense of speed; the phase would
            // scroll with the camera in the real thing.
            let phase = (y / 12) % 2 == 0;
            let g = if phase { grass_a } else { grass_b };
            let r = if phase { road_a } else { road_b };
            c.fill_rect(0, y as i32, W, 1, g);
            c.fill_rect((centre - half) as i32, y as i32, (half * 2.0) as u32, 1, r);
        }
    });

    // --- 3. Scanline, sub-pixel --------------------------------------
    //
    // The edges of the road are where stair-stepping shows worst, and
    // they move every frame. This is the version that looks modern.
    let scanline_f = time("3. scanline road, SUB-PIXEL", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.fill_rect(0, 0, W, HORIZON, sky);
        for y in HORIZON..H {
            let (centre, half) = road_at(y, 0.0, 0.3);
            let phase = (y / 12) % 2 == 0;
            let g = if phase { grass_a } else { grass_b };
            let r = if phase { road_a } else { road_b };
            c.fill_rect(0, y as i32, W, 1, g);
            c.fill_rect_f(centre - half, y as f32, half * 2.0, 1.0, r);
        }
    });

    // --- 3b. Sub-pixel road + edge stripes ---------------------------
    let with_stripes = time("3b. + edge stripes + centre line", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.fill_rect(0, 0, W, HORIZON, sky);
        for y in HORIZON..H {
            let (centre, half) = road_at(y, 0.0, 0.3);
            let phase = (y / 12) % 2 == 0;
            let g = if phase { grass_a } else { grass_b };
            let r = if phase { road_a } else { road_b };
            c.fill_rect(0, y as i32, W, 1, g);
            c.fill_rect_f(centre - half, y as f32, half * 2.0, 1.0, r);
            // Rumble strips at both edges, scaled with distance.
            let edge = (half * 0.10).max(1.0);
            c.fill_rect_f(centre - half, y as f32, edge, 1.0, stripe);
            c.fill_rect_f(centre + half - edge, y as f32, edge, 1.0, stripe);
            // Dashed centre line.
            if phase {
                c.fill_rect_f(centre - edge * 0.4, y as f32, edge * 0.8, 1.0, stripe);
            }
        }
    });

    // --- 4. + opponent car sprites -----------------------------------
    //
    // A car built from rectangles, scaled by distance. Six per car is
    // enough for a readable silhouette: body, roof, two wheels, two
    // lights. Five cars on screen is a busy field.
    let with_sprites = time("4. + 5 opponent cars (6 rects each)", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.fill_rect(0, 0, W, HORIZON, sky);
        for y in HORIZON..H {
            let (centre, half) = road_at(y, 0.0, 0.3);
            let phase = (y / 12) % 2 == 0;
            let g = if phase { grass_a } else { grass_b };
            let r = if phase { road_a } else { road_b };
            c.fill_rect(0, y as i32, W, 1, g);
            c.fill_rect_f(centre - half, y as f32, half * 2.0, 1.0, r);
            let edge = (half * 0.10).max(1.0);
            c.fill_rect_f(centre - half, y as f32, edge, 1.0, stripe);
            c.fill_rect_f(centre + half - edge, y as f32, edge, 1.0, stripe);
        }
        // Back to front, so nearer cars overdraw further ones.
        for i in 0..5 {
            let t = 0.25 + i as f32 * 0.16;
            let y = HORIZON as f32 + t * (H - HORIZON) as f32;
            let (centre, half) = road_at(y as u32, 0.0, 0.3);
            let s = half / 26.0; // sprite scale from road width
            let cx = centre + (i as f32 - 2.0) * half * 0.4;
            let (bw, bh) = (34.0 * s, 18.0 * s);
            c.fill_rect_f(cx - bw / 2.0, y - bh, bw, bh, Color::rgb(180, 70, 60));
            c.fill_rect_f(cx - bw * 0.32, y - bh * 1.6, bw * 0.64, bh * 0.7, Color::rgb(150, 55, 48));
            c.fill_rect_f(cx - bw * 0.5, y - bh * 0.35, bw * 0.2, bh * 0.4, Color::rgb(24, 24, 26));
            c.fill_rect_f(cx + bw * 0.3, y - bh * 0.35, bw * 0.2, bh * 0.4, Color::rgb(24, 24, 26));
            c.fill_rect_f(cx - bw * 0.42, y - bh * 0.9, bw * 0.16, bh * 0.2, Color::rgb(240, 200, 120));
            c.fill_rect_f(cx + bw * 0.26, y - bh * 0.9, bw * 0.16, bh * 0.2, Color::rgb(240, 200, 120));
        }
    });

    // --- 5. + lighting: distance haze -------------------------------
    //
    // The cheap version of atmosphere: an alpha wash that thickens
    // toward the horizon, so distance reads as distance. One extra
    // blended rect per scanline.
    let with_haze = time("5. + distance haze (alpha/scanline)", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.fill_rect(0, 0, W, HORIZON, sky);
        for y in HORIZON..H {
            let (centre, half) = road_at(y, 0.0, 0.3);
            let phase = (y / 12) % 2 == 0;
            let g = if phase { grass_a } else { grass_b };
            let r = if phase { road_a } else { road_b };
            c.fill_rect(0, y as i32, W, 1, g);
            c.fill_rect_f(centre - half, y as f32, half * 2.0, 1.0, r);
            let edge = (half * 0.10).max(1.0);
            c.fill_rect_f(centre - half, y as f32, edge, 1.0, stripe);
            c.fill_rect_f(centre + half - edge, y as f32, edge, 1.0, stripe);
            // Haze: strongest at the horizon, gone by the bottom.
            let t = (y - HORIZON) as f32 / (H - HORIZON) as f32;
            let a = ((1.0 - t) * (1.0 - t) * 150.0) as u8;
            if a > 2 {
                c.fill_rect(0, y as i32, W, 1, sky.with_alpha(a));
            }
        }
    });

    // --- 6. THE PESSIMISTIC FRAME ------------------------------------
    //
    // Everything at once, and more of it than a real frame should have:
    // road, stripes, haze, 8 cars, 40 roadside objects, and a full-width
    // alpha wash for a headlight/sunset gradient. If THIS fits, the real
    // game has room to spare.
    let worst = time("6. PESSIMISTIC: everything + 40 props", iters, || {
        let mut c = Canvas::new(&mut buf, W, H);
        c.fill_rect(0, 0, W, HORIZON, sky);
        for y in HORIZON..H {
            let (centre, half) = road_at(y, 0.0, 0.3);
            let phase = (y / 12) % 2 == 0;
            let g = if phase { grass_a } else { grass_b };
            let r = if phase { road_a } else { road_b };
            c.fill_rect(0, y as i32, W, 1, g);
            c.fill_rect_f(centre - half, y as f32, half * 2.0, 1.0, r);
            let edge = (half * 0.10).max(1.0);
            c.fill_rect_f(centre - half, y as f32, edge, 1.0, stripe);
            c.fill_rect_f(centre + half - edge, y as f32, edge, 1.0, stripe);
            if phase {
                c.fill_rect_f(centre - edge * 0.4, y as f32, edge * 0.8, 1.0, stripe);
            }
            let t = (y - HORIZON) as f32 / (H - HORIZON) as f32;
            let a = ((1.0 - t) * (1.0 - t) * 150.0) as u8;
            if a > 2 { c.fill_rect(0, y as i32, W, 1, sky.with_alpha(a)); }
        }
        // 40 roadside props (posts, trees, billboards) — 3 rects each.
        for i in 0..40 {
            let t = 0.06 + (i % 20) as f32 * 0.047;
            let y = HORIZON as f32 + t * (H - HORIZON) as f32;
            let (centre, half) = road_at(y as u32, 0.0, 0.3);
            let s = half / 26.0;
            let side = if i % 2 == 0 { -1.0 } else { 1.0 };
            let px = centre + side * half * 1.5;
            c.fill_rect_f(px - 2.0 * s, y - 40.0 * s, 4.0 * s, 40.0 * s, Color::rgb(70, 60, 50));
            c.fill_rect_f(px - 14.0 * s, y - 62.0 * s, 28.0 * s, 24.0 * s, Color::rgb(60, 96, 66));
            c.fill_rect_f(px - 9.0 * s, y - 78.0 * s, 18.0 * s, 20.0 * s, Color::rgb(70, 110, 76));
        }
        // 8 cars.
        for i in 0..8 {
            let t = 0.18 + i as f32 * 0.10;
            let y = HORIZON as f32 + t * (H - HORIZON) as f32;
            let (centre, half) = road_at(y as u32, 0.0, 0.3);
            let s = half / 26.0;
            let cx = centre + ((i % 3) as f32 - 1.0) * half * 0.5;
            let (bw, bh) = (34.0 * s, 18.0 * s);
            c.fill_rect_f(cx - bw / 2.0, y - bh, bw, bh, Color::rgb(180, 70, 60));
            c.fill_rect_f(cx - bw * 0.32, y - bh * 1.6, bw * 0.64, bh * 0.7, Color::rgb(150, 55, 48));
            c.fill_rect_f(cx - bw * 0.5, y - bh * 0.35, bw * 0.2, bh * 0.4, Color::rgb(24, 24, 26));
            c.fill_rect_f(cx + bw * 0.3, y - bh * 0.35, bw * 0.2, bh * 0.4, Color::rgb(24, 24, 26));
            c.fill_rect_f(cx - bw * 0.42, y - bh * 0.9, bw * 0.16, bh * 0.2, Color::rgb(240, 200, 120));
            c.fill_rect_f(cx + bw * 0.26, y - bh * 0.9, bw * 0.16, bh * 0.2, Color::rgb(240, 200, 120));
        }
        // The player's car, big, at the bottom.
        c.fill_rect_f(420.0, 600.0, 120.0, 60.0, Color::rgb(200, 90, 70));
        c.fill_rect_f(445.0, 570.0, 70.0, 34.0, Color::rgb(170, 74, 58));
        // Full-screen sunset wash.
        c.veil(Color::rgba(255, 170, 90, 26));
    });

    println!();
    println!("{}", "-".repeat(78));
    println!("pessimistic vs sprites    : {:.2}x", worst / with_sprites);
    println!("scanline vs banded        : {:.2}x", scanline / banded);
    println!("sub-pixel vs integer      : {:.2}x", scanline_f / scanline);
    println!("stripes cost              : {:.2}x", with_stripes / scanline_f);
    println!("sprites cost              : {:.2}x", with_sprites / with_stripes);
    println!("haze cost                 : {:.2}x", with_haze / with_stripes);
    println!();

    let full = with_sprites.max(with_haze).max(worst);
    let pct = full * 1000.0 / 16.67 * 100.0;
    println!("A FULL FRAME (road + stripes + sprites) costs {:.2} ms = {:.0}% of 60fps.",
             with_sprites * 1000.0, with_sprites * 1000.0 / 16.67 * 100.0);
    println!("Worst measured path: {:.2} ms ({:.0}% of budget).", full * 1000.0, pct);
    println!();
    if pct < 50.0 {
        println!("VERDICT: comfortable. A pseudo-3D road fits with room for game logic.");
    } else if pct < 100.0 {
        println!("VERDICT: it fits, but the renderer needs care. Prefer banded or integer paths.");
    } else {
        println!("VERDICT: does NOT fit at 60fps on this path. Needs a cheaper approach.");
    }
}
