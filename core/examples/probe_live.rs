//! What colours does the GAME actually produce, as opposed to the
//! still-frame example?
use omarcade_core::{Color, Theme};
fn ge(c: Color) -> f32 { c.g as f32 - (c.r as f32 + c.b as f32) / 2.0 }
fn main() {
    let t = Theme::load();
    println!("theme.blue           #{:02x}{:02x}{:02x}  green excess {:+.1}", t.blue.r, t.blue.g, t.blue.b, ge(t.blue));
    println!("theme.green          #{:02x}{:02x}{:02x}  green excess {:+.1}", t.green.r, t.green.g, t.green.b, ge(t.green));
    println!("theme.background     #{:02x}{:02x}{:02x}  green excess {:+.1}", t.background.r, t.background.g, t.background.b, ge(t.background));
    println!("theme.dark_background#{:02x}{:02x}{:02x}  green excess {:+.1}", t.dark_background.r, t.dark_background.g, t.dark_background.b, ge(t.dark_background));
    println!("theme.foreground     #{:02x}{:02x}{:02x}  green excess {:+.1}", t.foreground.r, t.foreground.g, t.foreground.b, ge(t.foreground));

    let old_sky = t.background.lerp(t.blue, 0.30);
    let new_sky = t.background.lerp(t.blue.lerp(t.background, 0.45), 0.55);
    println!("\nOLD sky #{:02x}{:02x}{:02x} ge {:+.1}", old_sky.r, old_sky.g, old_sky.b, ge(old_sky));
    println!("NEW sky #{:02x}{:02x}{:02x} ge {:+.1}", new_sky.r, new_sky.g, new_sky.b, ge(new_sky));

    let road_flat = t.dark_background.lerp(t.foreground, 0.16);
    println!("\nroad_flat            #{:02x}{:02x}{:02x}  green excess {:+.1}", road_flat.r, road_flat.g, road_flat.b, ge(road_flat));
    println!("  ^ THE ROAD'S OWN COLOUR, before any haze at all.");

    // The shipped haze tint: sky with green forced to mean(r,b).
    let sky = t.background.lerp(t.blue, 0.30);
    let haze = Color::rgb(sky.r, ((sky.r as u16 + sky.b as u16) / 2) as u8, sky.b);
    println!("\nsky  #{:02x}{:02x}{:02x} ge {:+.1}   haze #{:02x}{:02x}{:02x} ge {:+.1}",
             sky.r, sky.g, sky.b, ge(sky), haze.r, haze.g, haze.b, ge(haze));
    println!("\nroad under the SHIPPED haze:");
    for a in [0u8, 40, 80, 120, 175] {
        let hazed = road_flat.lerp(haze, a as f32 / 255.0);
        println!("  alpha {a:>3}: #{:02x}{:02x}{:02x}  ge {:+.1}", hazed.r, hazed.g, hazed.b, ge(hazed));
    }
}
