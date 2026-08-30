//! The racer — third Omarcade title, in prototype.
//!
//! Not playable yet. The art pipeline and the road projection are being
//! built first, and `examples/dump_art.rs` is how they are looked at.
//!
//! This binary also exists so the crate has a compiled target, which is
//! what makes `art.rs`'s tests actually run — before it, they were only
//! reachable through the example's `#[path]` include and never executed.
//! The first thing it caught was an assertion still expecting the old
//! 32x20 grid after the art moved to 48x30.

mod art;
mod road;

use art::Art;
use omarcade_core::Theme;
use road::{Camera, Road};

fn main() {
    // Loading the art is a real check: Sprite::new panics on a ragged
    // row or an unpalletted character, so this fails loudly on a typo.
    let theme = Theme::load();
    let art = Art::load(&theme);

    println!("omarcade-racer — prototype, not playable yet.\n");
    println!("sprites loaded:");
    for (name, s) in [
        ("player", &art.player),
        ("rival", art.rival(0)),
        ("post", &art.post),
    ] {
        println!(
            "  {name:<7} {:>3}x{:<3}  {:>4} drawn pixels",
            s.width(),
            s.height(),
            s.ink()
        );
    }
    // Report the road too, so the model has a live consumer rather than
    // existing only for its tests — and so a broken projection shows up
    // when someone runs the binary, not only when they run the suite.
    let track = Road::straight(400);
    let camera = Camera::for_road(&track, 0.85);
    let bands = track.visible(&camera, 0.0, 0.0, 960.0, 720.0);
    println!(
        "\nroad: {} segments, {:.0} units around, {} bands visible at 960x720",
        track.segment_count(),
        track.length(),
        bands.len(),
    );
    println!(
        "  camera {:.0} units up, {:.0}° lens; nearest band {:.0}px wide at y={:.0}",
        camera.height,
        camera.fov.to_degrees(),
        bands[0].half_width * 2.0,
        bands[0].y,
    );

    println!("\nlook at it:");
    println!("  cargo run -p omarcade-racer --example dump_art -- out.png road");
    println!("  cargo run -p omarcade-racer --example dump_art -- out.png curve");
    println!("  cargo run -p omarcade-racer --example dump_art -- out.png sheet");
    println!("\nedit it:");
    println!("  xdg-open tools/sprite-playground.html");
}
