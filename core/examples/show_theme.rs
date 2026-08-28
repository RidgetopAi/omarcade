//! Print the live palette. `cargo run -p omarcade-core --example show_theme`
fn main() {
    println!("path: {:?}", omarcade_core::Theme::path());
    let t = omarcade_core::Theme::load();
    println!("mode:       {:?}", t.mode);
    println!("background: {:?} -> {:#010x}", t.background, t.background.to_u32());
    println!("foreground: {:?} -> {:#010x}", t.foreground, t.foreground.to_u32());
    println!("accent:     {:?} -> {:#010x}", t.accent, t.accent.to_u32());
    println!("is fallback? {}", t == omarcade_core::Theme::fallback());
}
