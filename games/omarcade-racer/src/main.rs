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

use art::Art;
use omarcade_core::Theme;

fn main() {
    // Loading the art is a real check: Sprite::new panics on a ragged
    // row or an unpalletted character, so this fails loudly on a typo.
    let theme = Theme::load();
    let art = Art::load(&theme);

    println!("omarcade-racer — prototype, not playable yet.\n");
    println!("sprites loaded:");
    for (name, s) in [
        ("player", &art.player),
        ("rival", &art.rival),
        ("post", &art.post),
    ] {
        println!(
            "  {name:<7} {:>3}x{:<3}  {:>4} drawn pixels",
            s.width(),
            s.height(),
            s.ink()
        );
    }
    println!("\nlook at it:");
    println!("  cargo run -p omarcade-racer --example dump_art -- out.png road");
    println!("  cargo run -p omarcade-racer --example dump_art -- out.png sheet");
    println!("\nedit it:");
    println!("  xdg-open tools/sprite-playground.html");
}
